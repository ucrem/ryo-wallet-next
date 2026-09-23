use std::fs::{self, File};
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryKind {
    WalletRpc,
    Daemon,
}

/// An expected SHA-256 digest from a reviewed, target-specific manifest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BinaryDigest([u8; 32]);

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum BinaryDigestError {
    #[error("binary digest must be 64 lowercase hexadecimal characters")]
    InvalidFormat,
}

#[derive(Debug, Error)]
pub enum BinaryVerificationError {
    #[error("binary path must be absolute")]
    RelativePath,
    #[error("binary path must name a regular non-symlink file")]
    UnsafeFile,
    #[error("binary file could not be read")]
    Io(#[source] std::io::Error),
    #[error("binary digest did not match the reviewed manifest")]
    DigestMismatch,
}

/// A successfully checked absolute binary path. Constructing this value is the
/// only intended route to a later process-launch adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedBinary {
    pub kind: BinaryKind,
    path: PathBuf,
}

impl BinaryDigest {
    pub fn parse_hex(value: &str) -> Result<Self, BinaryDigestError> {
        if value.len() != 64
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(BinaryDigestError::InvalidFormat);
        }
        let mut bytes = [0_u8; 32];
        for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
            bytes[index] = (hex_nibble(pair[0]) << 4) | hex_nibble(pair[1]);
        }
        Ok(Self(bytes))
    }
}

impl VerifiedBinary {
    pub fn verify(
        kind: BinaryKind,
        path: &Path,
        expected_digest: BinaryDigest,
    ) -> Result<Self, BinaryVerificationError> {
        if !path.is_absolute() {
            return Err(BinaryVerificationError::RelativePath);
        }
        let metadata = fs::symlink_metadata(path).map_err(BinaryVerificationError::Io)?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(BinaryVerificationError::UnsafeFile);
        }
        let mut reader = BufReader::new(File::open(path).map_err(BinaryVerificationError::Io)?);
        let mut hasher = Sha256::new();
        let mut buffer = [0_u8; 64 * 1024];
        loop {
            let read = reader
                .read(&mut buffer)
                .map_err(BinaryVerificationError::Io)?;
            if read == 0 {
                break;
            }
            hasher.update(&buffer[..read]);
        }
        let actual: [u8; 32] = hasher.finalize().into();
        if actual != expected_digest.0 {
            return Err(BinaryVerificationError::DigestMismatch);
        }
        Ok(Self {
            kind,
            path: path.to_path_buf(),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

fn hex_nibble(value: u8) -> u8 {
    match value {
        b'0'..=b'9' => value - b'0',
        b'a'..=b'f' => value - b'a' + 10,
        _ => unreachable!("validated hexadecimal input"),
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    const HELLO_WORLD_SHA256: &str =
        "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9";

    #[test]
    fn verification_requires_an_exact_reviewed_digest() {
        let temp = tempfile::tempdir().unwrap();
        let binary = temp.path().join("ryo-wallet-rpc");
        File::create(&binary)
            .unwrap()
            .write_all(b"hello world")
            .unwrap();
        let expected = BinaryDigest::parse_hex(HELLO_WORLD_SHA256).unwrap();
        let verified = VerifiedBinary::verify(BinaryKind::WalletRpc, &binary, expected).unwrap();
        assert_eq!(verified.path(), binary);
        assert_eq!(verified.kind, BinaryKind::WalletRpc);

        let wrong = BinaryDigest::parse_hex(
            "0000000000000000000000000000000000000000000000000000000000000000",
        )
        .unwrap();
        assert!(matches!(
            VerifiedBinary::verify(BinaryKind::WalletRpc, &binary, wrong),
            Err(BinaryVerificationError::DigestMismatch)
        ));
    }

    #[test]
    fn unreviewed_path_forms_are_rejected_before_launch() {
        let digest = BinaryDigest::parse_hex(HELLO_WORLD_SHA256).unwrap();
        assert!(matches!(
            VerifiedBinary::verify(BinaryKind::Daemon, Path::new("ryod"), digest),
            Err(BinaryVerificationError::RelativePath)
        ));
        for digest in [
            "",
            "B94D27B9934D3E08A52E52D7DA7DABFAC484EFE37A5380EE9088F7ACE2EF CDE9",
        ] {
            assert_eq!(
                BinaryDigest::parse_hex(digest),
                Err(BinaryDigestError::InvalidFormat)
            );
        }
    }
}
