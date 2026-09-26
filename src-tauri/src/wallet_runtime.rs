use std::collections::BTreeMap;
#[cfg(any(test, not(debug_assertions)))]
use std::path::Path;
use std::path::PathBuf;

use ryo_wallet_service::process::{BinaryDigest, BinaryKind, VerifiedBinary};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReviewedRuntime {
    binary_sha256: String,
}

#[derive(Deserialize)]
struct RuntimeManifest {
    platforms: BTreeMap<String, ReviewedRuntime>,
}

#[cfg(debug_assertions)]
pub const MISSING_RUNTIME_MESSAGE: &str = "verified wallet runtime is unavailable; restart the development command and check its terminal output";
#[cfg(not(debug_assertions))]
pub const MISSING_RUNTIME_MESSAGE: &str = "the packaged wallet runtime is missing or failed verification; reinstall Ryo Wallet Next from a verified release package";

fn target() -> Option<&'static str> {
    if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        Some("x86_64-unknown-linux-gnu")
    } else if cfg!(all(target_os = "macos", target_arch = "x86_64")) {
        Some("x86_64-apple-darwin")
    } else if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
        Some("x86_64-pc-windows-msvc")
    } else {
        None
    }
}

#[cfg(any(test, not(debug_assertions)))]
fn packaged_sidecar(main_executable: &Path) -> Option<PathBuf> {
    let directory = main_executable.parent()?;
    Some(directory.join(if cfg!(target_os = "windows") {
        "ryo-wallet-rpc.exe"
    } else {
        "ryo-wallet-rpc"
    }))
}

#[cfg(any(test, not(debug_assertions)))]
fn verify_packaged_sidecar(main_executable: &Path, digest: BinaryDigest) -> Option<VerifiedBinary> {
    let path = packaged_sidecar(main_executable)?;
    VerifiedBinary::verify(BinaryKind::WalletRpc, &path, digest).ok()
}

pub fn reviewed_wallet_rpc() -> Option<VerifiedBinary> {
    let target = target()?;
    let manifest: RuntimeManifest =
        serde_json::from_str(include_str!("../runtime-manifest.json")).ok()?;
    let expected = manifest.platforms.get(target)?;
    let digest = BinaryDigest::parse_hex(&expected.binary_sha256).ok()?;
    #[cfg(debug_assertions)]
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".dev-runtime")
        .join(if cfg!(target_os = "windows") {
            "ryo-wallet-rpc.exe"
        } else {
            "ryo-wallet-rpc"
        });
    #[cfg(not(debug_assertions))]
    return verify_packaged_sidecar(&std::env::current_exe().ok()?, digest);
    #[cfg(debug_assertions)]
    return VerifiedBinary::verify(BinaryKind::WalletRpc, &path, digest).ok();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_covers_the_host_and_has_valid_digest() {
        let manifest: RuntimeManifest =
            serde_json::from_str(include_str!("../runtime-manifest.json")).unwrap();
        if let Some(target) = target() {
            let entry = manifest.platforms.get(target).unwrap();
            BinaryDigest::parse_hex(&entry.binary_sha256).unwrap();
        }
        assert!(!manifest.platforms.contains_key("aarch64-apple-darwin"));
    }

    #[test]
    fn packaged_path_is_anchored_to_the_application_executable() {
        let base = Path::new("/opt/ryo-wallet-next/ryo-wallet-next");
        let path = packaged_sidecar(base).unwrap();
        assert_eq!(path.parent(), Some(Path::new("/opt/ryo-wallet-next")));
        assert!(
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("ryo-wallet-rpc")
        );
    }

    #[test]
    fn packaged_runtime_requires_the_expected_digest() {
        let temporary = tempfile::tempdir().unwrap();
        let app = temporary.path().join("ryo-wallet-next");
        let path = packaged_sidecar(&app).unwrap();
        std::fs::write(&path, b"hello world").unwrap();
        let expected = BinaryDigest::parse_hex(
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9",
        )
        .unwrap();
        assert!(verify_packaged_sidecar(&app, expected).is_some());
        std::fs::write(&path, b"different executable").unwrap();
        assert!(verify_packaged_sidecar(&app, expected).is_none());
    }
}
