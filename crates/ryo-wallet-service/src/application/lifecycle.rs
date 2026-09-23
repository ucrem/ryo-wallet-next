use serde::Serialize;
use thiserror::Error;
use ts_rs::TS;

/// The key-bearing wallet process lifecycle. Synchronization is tracked
/// separately so that a synchronized wallet is never confused with an open one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "lowercase")]
pub enum LifecycleState {
    Stopped,
    Starting,
    Locked,
    Opening,
    Open,
    Closing,
    Faulted,
}

/// Non-sensitive snapshot suitable for the desktop UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct LifecycleStatus {
    pub state: LifecycleState,
    /// Changes whenever a new wallet session can start. UI clients use it to
    /// discard delayed snapshots from an earlier session. It is decimal digits
    /// because JavaScript numbers cannot represent every `u64` exactly.
    pub session_generation: String,
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
#[error("lifecycle transition from {from:?} is not permitted")]
pub struct LifecycleTransitionError {
    from: LifecycleState,
}

/// Enforces the lifecycle contract independently of Tauri or a child process.
#[derive(Debug, Clone, Copy)]
pub struct LifecycleMachine {
    state: LifecycleState,
    session_generation: u64,
}

impl Default for LifecycleMachine {
    fn default() -> Self {
        Self::new()
    }
}

impl LifecycleMachine {
    pub const fn new() -> Self {
        Self {
            state: LifecycleState::Stopped,
            session_generation: 0,
        }
    }

    pub fn status(&self) -> LifecycleStatus {
        LifecycleStatus {
            state: self.state,
            session_generation: self.session_generation.to_string(),
        }
    }

    /// Begins app-owned sidecar startup; no wallet is open at this point.
    pub fn start(&mut self) -> Result<LifecycleStatus, LifecycleTransitionError> {
        self.transition(LifecycleState::Stopped, LifecycleState::Starting)
    }

    /// Called only after the managed sidecar is ready and authenticated.
    pub fn sidecar_ready(&mut self) -> Result<LifecycleStatus, LifecycleTransitionError> {
        self.transition(LifecycleState::Starting, LifecycleState::Locked)
    }

    /// Starts a new wallet session after password validation begins.
    pub fn begin_open(&mut self) -> Result<LifecycleStatus, LifecycleTransitionError> {
        if self.state != LifecycleState::Locked {
            return Err(LifecycleTransitionError { from: self.state });
        }
        self.session_generation = self.session_generation.saturating_add(1);
        self.state = LifecycleState::Opening;
        Ok(self.status())
    }

    /// Called after the upstream wallet RPC confirms the active wallet is open.
    pub fn opened(&mut self) -> Result<LifecycleStatus, LifecycleTransitionError> {
        self.transition(LifecycleState::Opening, LifecycleState::Open)
    }

    /// Verifies that a read-only wallet query belongs to the currently open
    /// session. It does not change lifecycle state.
    pub fn require_open(&self) -> Result<(), LifecycleTransitionError> {
        if self.state == LifecycleState::Open {
            Ok(())
        } else {
            Err(LifecycleTransitionError { from: self.state })
        }
    }

    /// Returns to the locked state when password validation or wallet opening
    /// fails without starting a usable wallet session.
    pub fn opening_failed(&mut self) -> Result<LifecycleStatus, LifecycleTransitionError> {
        self.transition(LifecycleState::Opening, LifecycleState::Locked)
    }

    /// Invalidates UI work before stopping the key-bearing process.
    pub fn begin_lock(&mut self) -> Result<LifecycleStatus, LifecycleTransitionError> {
        self.transition(LifecycleState::Open, LifecycleState::Closing)
    }

    /// Called only after the wallet process exits or has been force-stopped.
    pub fn locked(&mut self) -> Result<LifecycleStatus, LifecycleTransitionError> {
        self.transition(LifecycleState::Closing, LifecycleState::Locked)
    }

    /// Records a fatal adapter/process error without exposing raw diagnostics.
    pub fn fault(&mut self) -> LifecycleStatus {
        self.state = LifecycleState::Faulted;
        self.status()
    }

    /// Allows an explicit recovery attempt after cleanup has completed.
    pub fn reset_after_fault(&mut self) -> Result<LifecycleStatus, LifecycleTransitionError> {
        self.transition(LifecycleState::Faulted, LifecycleState::Stopped)
    }

    fn transition(
        &mut self,
        expected: LifecycleState,
        next: LifecycleState,
    ) -> Result<LifecycleStatus, LifecycleTransitionError> {
        if self.state != expected {
            return Err(LifecycleTransitionError { from: self.state });
        }
        self.state = next;
        Ok(self.status())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lifecycle_requires_sidecar_readiness_and_process_exit() {
        let mut lifecycle = LifecycleMachine::new();
        assert_eq!(lifecycle.status().state, LifecycleState::Stopped);
        assert_eq!(lifecycle.start().unwrap().state, LifecycleState::Starting);
        assert!(lifecycle.begin_open().is_err());
        assert_eq!(
            lifecycle.sidecar_ready().unwrap().state,
            LifecycleState::Locked
        );
        assert_eq!(lifecycle.begin_open().unwrap().session_generation, "1");
        assert_eq!(lifecycle.opened().unwrap().state, LifecycleState::Open);
        assert_eq!(
            lifecycle.begin_lock().unwrap().state,
            LifecycleState::Closing
        );
        assert!(lifecycle.begin_open().is_err());
        assert_eq!(lifecycle.locked().unwrap().state, LifecycleState::Locked);
    }

    #[test]
    fn generation_advances_only_for_new_wallet_sessions() {
        let mut lifecycle = LifecycleMachine::new();
        lifecycle.start().unwrap();
        lifecycle.sidecar_ready().unwrap();
        lifecycle.begin_open().unwrap();
        lifecycle.opened().unwrap();
        lifecycle.begin_lock().unwrap();
        lifecycle.locked().unwrap();
        assert_eq!(lifecycle.begin_open().unwrap().session_generation, "2");
    }

    #[test]
    fn failed_open_returns_to_locked_without_reusing_the_session() {
        let mut lifecycle = LifecycleMachine::new();
        lifecycle.start().unwrap();
        lifecycle.sidecar_ready().unwrap();
        assert_eq!(lifecycle.begin_open().unwrap().session_generation, "1");
        assert_eq!(
            lifecycle.opening_failed().unwrap().state,
            LifecycleState::Locked
        );
        assert_eq!(lifecycle.begin_open().unwrap().session_generation, "2");
    }

    #[test]
    fn a_fault_requires_explicit_recovery_before_restart() {
        let mut lifecycle = LifecycleMachine::new();
        lifecycle.start().unwrap();
        assert_eq!(lifecycle.fault().state, LifecycleState::Faulted);
        assert!(lifecycle.start().is_err());
        assert_eq!(
            lifecycle.reset_after_fault().unwrap().state,
            LifecycleState::Stopped
        );
    }

    #[test]
    fn read_snapshots_require_an_open_wallet() {
        let mut lifecycle = LifecycleMachine::new();
        assert!(lifecycle.require_open().is_err());
        lifecycle.start().unwrap();
        lifecycle.sidecar_ready().unwrap();
        lifecycle.begin_open().unwrap();
        lifecycle.opened().unwrap();
        assert!(lifecycle.require_open().is_ok());
    }
}
