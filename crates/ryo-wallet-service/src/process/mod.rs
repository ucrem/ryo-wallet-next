//! Sidecar verification primitives.
//!
//! Process launching is intentionally separate from verification: an unknown
//! executable must never be launched merely to inspect its version.

mod binary;
mod credentials;
mod scan_progress;
mod supervisor;
pub(crate) use scan_progress::ScanProgress;
mod upstream_path;
pub use upstream_path::upstream_path;
mod wallet_rpc_launch;
mod wallet_rpc_session;

pub use binary::{BinaryDigest, BinaryKind, BinaryVerificationError, VerifiedBinary};
pub use credentials::{CredentialFileError, login_path, read_generated_login};
pub use supervisor::{ManagedProcess, ProcessError};
pub use wallet_rpc_launch::{WalletRpcLaunch, WalletRpcLaunchError};
pub use wallet_rpc_session::{WalletRpcSession, WalletRpcStartupError};
mod daemon_session;
pub use daemon_session::{DaemonError, DaemonSession};
