//! Reusable, Tauri-independent foundation for Ryo Wallet Next.
//!
//! This crate implements domain types, a verified sidecar lifecycle, and typed
//! wallet RPC clients. It does not expose wallet operations to a renderer or
//! authorize transfers.

pub mod application;
pub mod domain;
pub mod process;
pub mod rpc;
pub mod storage;
