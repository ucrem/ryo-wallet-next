use std::sync::{Arc, Mutex};

use tokio::io::{AsyncRead, AsyncReadExt};
use zeroize::{Zeroize, Zeroizing};

/// Only processed block heights survive the private stdout boundary.
#[derive(Clone, Default)]
pub(crate) struct ScanProgress(Arc<Mutex<Option<u64>>>);

impl ScanProgress {
    pub(crate) fn clear(&self) {
        if let Ok(mut value) = self.0.lock() {
            *value = None;
        }
    }
    pub(crate) fn height(&self) -> Option<u64> {
        self.0.lock().ok().and_then(|value| *value)
    }

    fn record(&self, height: u64) {
        if let Ok(mut value) = self.0.lock() {
            *value = Some(height);
        }
    }
}

fn processed_height(line: &[u8]) -> Option<u64> {
    let line = std::str::from_utf8(line).ok()?;
    let (_, progress) = line.split_once("On new block ")?;
    let (height, hash) = progress.split_once(" - ")?;
    if height.is_empty() || !height.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let hash = hash.trim().trim_end_matches("\u{1b}[0m");
    if hash.len() != 64 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    // wallet2 logs the block index before pushing it; RPC reports chain length.
    height.parse::<u64>().ok()?.checked_add(1)
}

pub(crate) async fn consume(mut stdout: impl AsyncRead + Unpin, progress: ScanProgress) {
    const MAX_LINE: usize = 4096;
    let mut chunk = Zeroizing::new([0_u8; 4096]);
    let mut line = Zeroizing::new(Vec::with_capacity(MAX_LINE));
    let mut oversized = false;
    while let Ok(count) = stdout.read(chunk.as_mut()).await {
        if count == 0 {
            break;
        }
        for &byte in &chunk[..count] {
            if byte == b'\n' {
                if !oversized && let Some(height) = processed_height(&line) {
                    progress.record(height);
                }
                line.zeroize();
                line.clear();
                oversized = false;
            } else if !oversized {
                if line.len() == MAX_LINE {
                    line.zeroize();
                    line.clear();
                    oversized = true;
                } else {
                    line.push(byte);
                }
            }
        }
        chunk.zeroize();
    }
    progress.clear();
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncWriteExt;

    #[tokio::test]
    async fn fragmented_output_extracts_processed_heights_without_downloads_or_secrets() {
        let (mut writer, reader) = tokio::io::duplex(64);
        let progress = ScanProgress::default();
        let consumer = tokio::spawn(consume(reader, progress.clone()));
        let hash = "a".repeat(64);
        writer
            .write_all(b"Pulled blocks 1-900 / 1000\nprivate material ignored\nOn new blo")
            .await
            .unwrap();
        writer
            .write_all(format!("ck 41 - {hash}\r\n").as_bytes())
            .await
            .unwrap();
        writer.write_all(&vec![b'x'; 5000]).await.unwrap();
        writer
            .write_all(format!("On new block 900 - {hash}\nOn new block 42 - {hash}\n").as_bytes())
            .await
            .unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(1), async {
            while progress.height() != Some(43) {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        assert_eq!(progress.height(), Some(43));
        drop(writer);
        consumer.await.unwrap();
        assert_eq!(progress.height(), None);
        assert!(processed_height(b"On new block 2 - invalid").is_none());
        assert!(
            processed_height(format!("On new block {} - {hash}", u64::MAX).as_bytes()).is_none()
        );
        assert_eq!(
            processed_height(format!("\u{1b}[32mOn new block 3 - {hash}\u{1b}[0m").as_bytes()),
            Some(4)
        );
    }
}
