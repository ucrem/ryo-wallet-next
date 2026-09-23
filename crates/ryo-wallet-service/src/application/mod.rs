//! Application state coordinating the wallet process and RPC adapters.
//!
//! The actor makes privileged lifecycle transitions explicit and serializes
//! them. Tauri commands for wallet operations are added only alongside their
//! dedicated, security-reviewed UI flows.

mod lifecycle;
mod service;

pub use lifecycle::{LifecycleMachine, LifecycleState, LifecycleStatus, LifecycleTransitionError};
pub use service::{CreatedWallet, WalletOverview, WalletService, WalletServiceError};
