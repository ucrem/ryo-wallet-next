//! A real daemon proxy that drops one reply only AFTER the daemon accepted a tx.
//! It forwards all bytes unchanged and never logs transaction/request bodies.
use reqwest::Client;
use serde_json::Value;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinHandle;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

pub struct LossyDaemon {
    pub port: u16,
    replies_before_loss: Arc<AtomicUsize>,
    pub accepted_dropped: Arc<AtomicUsize>,
    observations: Arc<Mutex<Vec<SendObservation>>>,
    task: JoinHandle<()>,
}

/// Non-sensitive diagnostics: no transaction bytes, addresses or wallet material.
#[derive(Debug, Clone)]
pub struct SendObservation {
    pub http: u16,
    pub accepted: bool,
    pub rejection_flags: Vec<&'static str>,
}

impl LossyDaemon {
    pub async fn start(daemon_port: u16) -> Result<Self> {
        let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).await?;
        let port = listener.local_addr()?.port();
        let replies_before_loss = Arc::new(AtomicUsize::new(usize::MAX));
        let accepted_dropped = Arc::new(AtomicUsize::new(0));
        let observations = Arc::new(Mutex::new(Vec::new()));
        let observed = observations.clone();
        let drop_flag = replies_before_loss.clone();
        let drop_count = accepted_dropped.clone();
        let client = Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(10))
            .build()?;
        let task = tokio::spawn(async move {
            while let Ok((socket, _)) = listener.accept().await {
                let client = client.clone();
                let flag = drop_flag.clone();
                let count = drop_count.clone();
                let observed = observed.clone();
                tokio::spawn(async move {
                    let _ = tokio::time::timeout(
                        Duration::from_secs(15),
                        forward(socket, daemon_port, client, flag, count, observed),
                    )
                    .await;
                });
            }
        });
        Ok(Self {
            port,
            replies_before_loss,
            accepted_dropped,
            observations,
            task,
        })
    }
    pub fn send_observations(&self) -> Vec<SendObservation> {
        self.observations.lock().unwrap().clone()
    }
    pub fn lose_next_send_reply(&self) {
        self.lose_send_reply_after(0);
    }
    pub fn lose_send_reply_after(&self, successful_replies: usize) {
        self.replies_before_loss
            .store(successful_replies, Ordering::Release);
    }
}
impl Drop for LossyDaemon {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn forward(
    mut socket: TcpStream,
    daemon_port: u16,
    client: Client,
    flag: Arc<AtomicUsize>,
    count: Arc<AtomicUsize>,
    observations: Arc<Mutex<Vec<SendObservation>>>,
) -> Result<()> {
    let mut request = Vec::new();
    let mut chunk = [0_u8; 4096];
    let header_end = loop {
        let read = socket.read(&mut chunk).await?;
        if read == 0 {
            return Err("proxy request closed early".into());
        }
        request.extend_from_slice(&chunk[..read]);
        if let Some(offset) = request.windows(4).position(|bytes| bytes == b"\r\n\r\n") {
            break offset + 4;
        }
        if request.len() > 65536 {
            return Err("proxy headers exceeded limit".into());
        }
    };
    let headers = std::str::from_utf8(&request[..header_end])?;
    let mut line = headers
        .lines()
        .next()
        .ok_or("missing proxy request line")?
        .split_whitespace();
    let method = line.next().ok_or("missing method")?;
    let path = line.next().ok_or("missing path")?.to_owned();
    if !["GET", "POST"].contains(&method)
        || !path.starts_with('/')
        || path.starts_with("//")
        || path.contains(['?', '#'])
    {
        return Err("unexpected proxy route".into());
    }
    let length: usize = headers
        .lines()
        .filter_map(|line| line.split_once(':'))
        .find(|(key, _)| key.eq_ignore_ascii_case("content-length"))
        .map(|(_, value)| value.trim().parse())
        .transpose()?
        .unwrap_or(0);
    let method = reqwest::Method::from_bytes(method.as_bytes())?;
    if length > 2 * 1024 * 1024 {
        return Err("proxy request exceeded limit".into());
    }
    while request.len() < header_end + length {
        let read = socket.read(&mut chunk).await?;
        if read == 0 {
            return Err("proxy body closed early".into());
        }
        request.extend_from_slice(&chunk[..read]);
    }
    let response = client
        .request(method, format!("http://127.0.0.1:{daemon_port}{path}"))
        .header("content-type", "application/json")
        .body(request[header_end..header_end + length].to_vec())
        .send()
        .await?;
    let status = response.status();
    let body = response.bytes().await?;
    if ["/sendrawtransaction", "/send_raw_transaction"].contains(&path.as_str()) {
        let value: Value = serde_json::from_slice(&body)?;
        let accepted = status.is_success() && value["status"] == "OK";
        let rejection_flags = [
            "double_spend",
            "fee_too_low",
            "invalid_input",
            "invalid_output",
            "low_mixin",
            "not_rct",
            "not_relayed",
            "overspend",
            "too_big",
        ]
        .into_iter()
        .filter(|key| value[*key] == true)
        .collect();
        observations.lock().unwrap().push(SendObservation {
            http: status.as_u16(),
            accepted,
            rejection_flags,
        });
        if accepted && flag.load(Ordering::Acquire) != usize::MAX {
            // Only accepted transactions consume the countdown. One drop disarms it.
            let previous =
                flag.fetch_update(
                    Ordering::AcqRel,
                    Ordering::Acquire,
                    |remaining| match remaining {
                        usize::MAX => None,
                        0 => Some(usize::MAX),
                        n => Some(n - 1),
                    },
                );
            if previous == Ok(0) {
                count.fetch_add(1, Ordering::AcqRel);
                socket.shutdown().await?;
                return Ok(());
            }
        }
    }
    socket.write_all(format!("HTTP/1.1 {} OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", status.as_u16(), body.len()).as_bytes()).await?;
    socket.write_all(&body).await?;
    socket.shutdown().await?;
    Ok(())
}
