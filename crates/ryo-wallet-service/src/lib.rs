//! Reusable, Tauri-independent foundation for Ryo Wallet Next.
//!
//! Domain types, verified sidecar ownership, typed RPC clients and wallet
//! operations, including immutable transaction preparation and one-shot relay.
//! The desktop host exposes scoped commands; this crate is independent of Tauri.

pub mod application;
pub mod domain;
pub mod process;
pub mod rpc;
pub mod storage;
