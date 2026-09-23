use std::time::Duration;

use thiserror::Error;
use tokio::sync::{mpsc, oneshot};
use ts_rs::TS;
use zeroize::Zeroizing;

use crate::domain::{AtomicAmount, AtomicAmountDto, NodeConfig};
use crate::process::{ProcessError, VerifiedBinary, WalletRpcSession, WalletRpcStartupError};
use crate::rpc::{RecoveryPhrase, RpcError};
use crate::storage::{AppPaths, PathError, WalletId};

use super::{LifecycleMachine, LifecycleStatus, LifecycleTransitionError};

const COMMAND_QUEUE_CAPACITY: usize = 8;

#[derive(Debug, Error)]
pub enum WalletServiceError {
    #[error("wallet service lifecycle does not allow this operation")]
    Lifecycle(#[source] LifecycleTransitionError),
    #[error("wallet RPC could not be started")]
    Startup(#[source] WalletRpcStartupError),
    #[error("wallet RPC rejected the operation")]
    Rpc(#[source] RpcError),
    #[error("private wallet storage could not be allocated")]
    Storage(#[source] PathError),
    #[error("wallet process could not be stopped")]
    Shutdown(#[source] ProcessError),
    #[error("wallet type is outside the MVP account-zero scope")]
    UnsupportedWalletScope,
    #[error("wallet service is unavailable")]
    Unavailable,
}

/// Async handle for the single-owner wallet actor. The actor serializes every
/// lifecycle mutation and RPC operation, avoiding locks held across awaits.
#[derive(Clone)]
pub struct WalletService {
    sender: mpsc::Sender<Command>,
}

/// Result of a newly created wallet. The recovery phrase is moved exactly
/// once into the dedicated backup flow; it cannot be cloned, logged,
/// serialized, or kept in ordinary application state.
pub struct CreatedWallet {
    status: LifecycleStatus,
    recovery_phrase: RecoveryPhrase,
}

/// Read-only account-zero snapshot for the currently open wallet. Amounts
/// stay canonical atomic-digit strings across the Tauri boundary.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, TS)]
#[ts(export)]
pub struct WalletOverview {
    pub session_generation: String,
    pub primary_address: String,
    pub total: AtomicAmountDto,
    pub unlocked: AtomicAmountDto,
    pub locked: AtomicAmountDto,
    pub multisig_import_needed: bool,
}

impl CreatedWallet {
    pub fn status(&self) -> &LifecycleStatus {
        &self.status
    }

    pub fn into_recovery_phrase(self) -> RecoveryPhrase {
        self.recovery_phrase
    }
}

impl WalletService {
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::channel(COMMAND_QUEUE_CAPACITY);
        std::thread::Builder::new()
            .name("ryo-wallet-service".to_owned())
            .spawn(move || {
                tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("wallet service runtime must be available")
                    .block_on(run_actor(receiver));
            })
            .expect("wallet service thread must be available");
        Self { sender }
    }

    pub async fn status(&self) -> Result<LifecycleStatus, WalletServiceError> {
        let (response, reply) = oneshot::channel();
        self.send(Command::Status { response }).await?;
        reply.await.map_err(|_| WalletServiceError::Unavailable)
    }

    pub async fn start_wallet_rpc(
        &self,
        binary: VerifiedBinary,
        paths: AppPaths,
        node: NodeConfig,
        rpc_port: u16,
    ) -> Result<LifecycleStatus, WalletServiceError> {
        let (response, reply) = oneshot::channel();
        self.send(Command::Start {
            binary,
            paths,
            node,
            rpc_port,
            response,
        })
        .await?;
        reply.await.map_err(|_| WalletServiceError::Unavailable)?
    }

    pub async fn open_imported_wallet(
        &self,
        wallet_id: WalletId,
        password: Zeroizing<String>,
    ) -> Result<LifecycleStatus, WalletServiceError> {
        let (response, reply) = oneshot::channel();
        self.send(Command::Open {
            wallet_id,
            password,
            response,
        })
        .await?;
        reply.await.map_err(|_| WalletServiceError::Unavailable)?
    }

    pub async fn create_wallet(
        &self,
        wallet_id: WalletId,
        password: Zeroizing<String>,
        language: String,
    ) -> Result<CreatedWallet, WalletServiceError> {
        let (response, reply) = oneshot::channel();
        self.send(Command::Create {
            wallet_id,
            password,
            language,
            response,
        })
        .await?;
        reply.await.map_err(|_| WalletServiceError::Unavailable)?
    }

    pub async fn restore_wallet(
        &self,
        wallet_id: WalletId,
        password: Zeroizing<String>,
        seed: Zeroizing<String>,
        refresh_start_height: u64,
    ) -> Result<LifecycleStatus, WalletServiceError> {
        let (response, reply) = oneshot::channel();
        self.send(Command::Restore {
            wallet_id,
            password,
            seed,
            refresh_start_height,
            response,
        })
        .await?;
        reply.await.map_err(|_| WalletServiceError::Unavailable)?
    }

    pub async fn lock(&self, deadline: Duration) -> Result<LifecycleStatus, WalletServiceError> {
        let (response, reply) = oneshot::channel();
        self.send(Command::Lock { deadline, response }).await?;
        reply.await.map_err(|_| WalletServiceError::Unavailable)?
    }

    pub async fn overview(&self) -> Result<WalletOverview, WalletServiceError> {
        let (response, reply) = oneshot::channel();
        self.send(Command::Overview { response }).await?;
        reply.await.map_err(|_| WalletServiceError::Unavailable)?
    }

    async fn send(&self, command: Command) -> Result<(), WalletServiceError> {
        self.sender
            .send(command)
            .await
            .map_err(|_| WalletServiceError::Unavailable)
    }
}

enum Command {
    Status {
        response: oneshot::Sender<LifecycleStatus>,
    },
    Start {
        binary: VerifiedBinary,
        paths: AppPaths,
        node: NodeConfig,
        rpc_port: u16,
        response: oneshot::Sender<Result<LifecycleStatus, WalletServiceError>>,
    },
    Open {
        wallet_id: WalletId,
        password: Zeroizing<String>,
        response: oneshot::Sender<Result<LifecycleStatus, WalletServiceError>>,
    },
    Create {
        wallet_id: WalletId,
        password: Zeroizing<String>,
        language: String,
        response: oneshot::Sender<Result<CreatedWallet, WalletServiceError>>,
    },
    Restore {
        wallet_id: WalletId,
        password: Zeroizing<String>,
        seed: Zeroizing<String>,
        refresh_start_height: u64,
        response: oneshot::Sender<Result<LifecycleStatus, WalletServiceError>>,
    },
    Lock {
        deadline: Duration,
        response: oneshot::Sender<Result<LifecycleStatus, WalletServiceError>>,
    },
    Overview {
        response: oneshot::Sender<Result<WalletOverview, WalletServiceError>>,
    },
}

async fn run_actor(mut receiver: mpsc::Receiver<Command>) {
    let mut lifecycle = LifecycleMachine::new();
    let mut session: Option<WalletRpcSession> = None;
    let mut app_paths: Option<AppPaths> = None;
    while let Some(command) = receiver.recv().await {
        match command {
            Command::Status { response } => {
                let _ = response.send(lifecycle.status());
            }
            Command::Start {
                binary,
                paths,
                node,
                rpc_port,
                response,
            } => {
                if let Err(error) = lifecycle.start() {
                    let _ = response.send(Err(WalletServiceError::Lifecycle(error)));
                    continue;
                }
                match WalletRpcSession::start(&binary, &paths, &node, rpc_port)
                    .await
                    .map_err(WalletServiceError::Startup)
                {
                    Ok(started) => {
                        session = Some(started);
                        app_paths = Some(paths);
                        let _ = response.send(
                            lifecycle
                                .sidecar_ready()
                                .map_err(WalletServiceError::Lifecycle),
                        );
                    }
                    Err(error) => {
                        lifecycle.fault();
                        let _ = response.send(Err(error));
                    }
                }
            }
            Command::Open {
                wallet_id,
                password,
                response,
            } => {
                let result = match lifecycle.begin_open() {
                    Ok(_) => match session.as_ref() {
                        Some(session) => match session
                            .client()
                            .open_imported_wallet(&wallet_id, password)
                            .await
                            .map_err(WalletServiceError::Rpc)
                        {
                            Ok(()) => {
                                complete_supported_open(&mut lifecycle, session.client()).await
                            }
                            Err(error) => Err(error),
                        },
                        None => Err(WalletServiceError::Unavailable),
                    },
                    Err(error) => Err(WalletServiceError::Lifecycle(error)),
                };
                if result.is_err() && lifecycle.status().state == super::LifecycleState::Opening {
                    let _ = lifecycle.opening_failed();
                }
                let _ = response.send(result);
            }
            Command::Create {
                wallet_id,
                password,
                language,
                response,
            } => {
                let result = match begin_create_or_restore(
                    &mut lifecycle,
                    session.as_ref(),
                    app_paths.as_ref(),
                    &wallet_id,
                ) {
                    Ok(client) => match client
                        .create_wallet(&wallet_id, password, &language)
                        .await
                        .map_err(WalletServiceError::Rpc)
                    {
                        Ok(()) => match client
                            .query_mnemonic()
                            .await
                            .map_err(WalletServiceError::Rpc)
                        {
                            Ok(recovery_phrase) => complete_supported_open(&mut lifecycle, client)
                                .await
                                .map(|status| CreatedWallet {
                                    status,
                                    recovery_phrase,
                                }),
                            Err(error) => Err(error),
                        },
                        Err(error) => Err(error),
                    },
                    Err(error) => Err(error),
                };
                if result.is_err() && lifecycle.status().state == super::LifecycleState::Opening {
                    lifecycle.fault();
                }
                let _ = response.send(result);
            }
            Command::Restore {
                wallet_id,
                password,
                seed,
                refresh_start_height,
                response,
            } => {
                let result = match begin_create_or_restore(
                    &mut lifecycle,
                    session.as_ref(),
                    app_paths.as_ref(),
                    &wallet_id,
                ) {
                    Ok(client) => match client
                        .restore_wallet(&wallet_id, password, seed, refresh_start_height)
                        .await
                        .map_err(WalletServiceError::Rpc)
                    {
                        Ok(()) => complete_supported_open(&mut lifecycle, client).await,
                        Err(error) => Err(error),
                    },
                    Err(error) => Err(error),
                };
                if result.is_err() && lifecycle.status().state == super::LifecycleState::Opening {
                    lifecycle.fault();
                }
                let _ = response.send(result);
            }
            Command::Lock { deadline, response } => {
                let result = match lifecycle.begin_lock() {
                    Ok(_) => match session.take() {
                        Some(session) => session
                            .stop(deadline)
                            .await
                            .map_err(WalletServiceError::Shutdown)
                            .and_then(|_| {
                                lifecycle.locked().map_err(WalletServiceError::Lifecycle)
                            }),
                        None => Err(WalletServiceError::Unavailable),
                    },
                    Err(error) => Err(WalletServiceError::Lifecycle(error)),
                };
                if result.is_err() && lifecycle.status().state == super::LifecycleState::Closing {
                    lifecycle.fault();
                }
                let _ = response.send(result);
            }
            Command::Overview { response } => {
                let result = match lifecycle
                    .require_open()
                    .map_err(WalletServiceError::Lifecycle)
                {
                    Ok(()) => match session.as_ref() {
                        Some(session) => {
                            wallet_overview(session.client(), lifecycle.status()).await
                        }
                        None => Err(WalletServiceError::Unavailable),
                    },
                    Err(error) => Err(error),
                };
                let _ = response.send(result);
            }
        }
    }
}

async fn complete_supported_open(
    lifecycle: &mut LifecycleMachine,
    client: &crate::rpc::WalletRpcClient,
) -> Result<LifecycleStatus, WalletServiceError> {
    let scope = client.scope().await.map_err(WalletServiceError::Rpc)?;
    if scope.account_count != 1 || scope.multisig {
        let _ = client.close_wallet().await;
        return Err(WalletServiceError::UnsupportedWalletScope);
    }
    lifecycle.opened().map_err(WalletServiceError::Lifecycle)
}

async fn wallet_overview(
    client: &crate::rpc::WalletRpcClient,
    lifecycle: LifecycleStatus,
) -> Result<WalletOverview, WalletServiceError> {
    let balance = client.balance().await.map_err(WalletServiceError::Rpc)?;
    let primary_address = client
        .primary_address()
        .await
        .map_err(WalletServiceError::Rpc)?;
    let locked = AtomicAmount::from_atomic(balance.total.atomic() - balance.unlocked.atomic());
    Ok(WalletOverview {
        session_generation: lifecycle.session_generation,
        primary_address,
        total: balance.total.into(),
        unlocked: balance.unlocked.into(),
        locked: locked.into(),
        multisig_import_needed: balance.multisig_import_needed,
    })
}

fn begin_create_or_restore<'a>(
    lifecycle: &mut LifecycleMachine,
    session: Option<&'a WalletRpcSession>,
    app_paths: Option<&AppPaths>,
    wallet_id: &WalletId,
) -> Result<&'a crate::rpc::WalletRpcClient, WalletServiceError> {
    lifecycle
        .begin_open()
        .map_err(WalletServiceError::Lifecycle)?;
    let paths = app_paths.ok_or(WalletServiceError::Unavailable)?;
    paths
        .create_wallet_dir(wallet_id)
        .map_err(WalletServiceError::Storage)?;
    session
        .map(WalletRpcSession::client)
        .ok_or(WalletServiceError::Unavailable)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn actor_starts_stopped_and_rejects_open_before_sidecar_start() {
        let service = WalletService::new();
        assert_eq!(
            service.status().await.unwrap().state,
            super::super::LifecycleState::Stopped
        );
        let id = WalletId::parse("0123456789abcdef0123456789abcdef").unwrap();
        assert!(matches!(
            service
                .open_imported_wallet(id, Zeroizing::new("password".to_owned()))
                .await,
            Err(WalletServiceError::Lifecycle(_))
        ));
        assert_eq!(
            service.status().await.unwrap().state,
            super::super::LifecycleState::Stopped
        );
    }
}
