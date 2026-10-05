use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

use thiserror::Error;
use tokio::sync::{mpsc, oneshot};
use ts_rs::TS;
use zeroize::Zeroizing;

use crate::domain::{AtomicAmount, AtomicAmountDto, NodeConfig};
use crate::process::ScanProgress;
use crate::process::{ProcessError, VerifiedBinary, WalletRpcSession, WalletRpcStartupError};
use crate::rpc::{ReceiveAddress, RecoveryPhrase, RpcError};
use crate::storage::{AppPaths, PathError, WalletId, load_wallet_name};

use super::operations::{
    OperationError, OperationsState, WalletOperation, WalletOperationOutput, read_wallet_name,
};
use super::{LifecycleMachine, LifecycleStatus, LifecycleTransitionError};

const COMMAND_QUEUE_CAPACITY: usize = 8;
const READ_EXECUTION_TIMEOUT: Duration = Duration::from_secs(4);
const READ_REQUEST_TIMEOUT: Duration = Duration::from_secs(6);

async fn bounded_read<T>(
    deadline: Duration,
    future: impl std::future::Future<Output = Result<T, WalletServiceError>>,
) -> Result<T, WalletServiceError> {
    tokio::time::timeout(deadline, future)
        .await
        .unwrap_or(Err(WalletServiceError::Busy))
}

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
    #[error("wallet RPC busy")]
    Busy,
    #[error("wallet session has changed")]
    StaleSession,
    #[error(transparent)]
    Operation(#[from] OperationError),
}

/// Async handle for the single-owner wallet actor. The actor serializes every
/// lifecycle mutation and RPC operation, avoiding locks held across awaits.
#[derive(Clone)]
pub struct WalletService {
    sender: mpsc::Sender<Command>,
    scan: Arc<Mutex<Option<ScanProgress>>>,
    read_busy: Arc<AtomicBool>,
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
        let scan = Arc::new(Mutex::new(None));
        let actor_scan = scan.clone();
        let read_busy = Arc::new(AtomicBool::new(false));
        let actor_busy = read_busy.clone();
        std::thread::Builder::new()
            .name("ryo-wallet-service".to_owned())
            .spawn(move || {
                tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("wallet service runtime must be available")
                    .block_on(run_actor(receiver, actor_scan, actor_busy));
            })
            .expect("wallet service thread must be available");
        Self {
            sender,
            scan,
            read_busy,
        }
    }

    /// Numeric progress from this session's private stdout remains available during refresh.
    pub fn scan_height(&self) -> Option<u64> {
        self.scan
            .lock()
            .ok()
            .and_then(|scan| scan.as_ref().and_then(ScanProgress::height))
    }

    pub fn read_is_busy(&self) -> bool {
        self.read_busy.load(Ordering::Acquire)
    }

    async fn read_request<T>(
        &self,
        future: impl std::future::Future<Output = Result<T, WalletServiceError>>,
    ) -> Result<T, WalletServiceError> {
        let result = bounded_read(READ_REQUEST_TIMEOUT, future).await;
        if matches!(result, Err(WalletServiceError::Busy)) {
            self.read_busy.store(true, Ordering::Release);
        }
        result
    }

    pub async fn status(&self) -> Result<LifecycleStatus, WalletServiceError> {
        let (response, reply) = oneshot::channel();
        self.send(Command::Status { response }).await?;
        reply.await.map_err(|_| WalletServiceError::Unavailable)
    }

    pub async fn is_idle(&self) -> Result<bool, WalletServiceError> {
        let (response, reply) = oneshot::channel();
        self.send(Command::IsIdle { response }).await?;
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
        self.send(Command::Lock {
            deadline,
            response,
            generation: None,
        })
        .await?;
        reply.await.map_err(|_| WalletServiceError::Unavailable)?
    }

    pub async fn lock_current(
        &self,
        deadline: Duration,
        generation: String,
    ) -> Result<LifecycleStatus, WalletServiceError> {
        let (response, reply) = oneshot::channel();
        self.send(Command::Lock {
            deadline,
            response,
            generation: Some(generation),
        })
        .await?;
        reply.await.map_err(|_| WalletServiceError::Unavailable)?
    }

    pub async fn overview(&self) -> Result<WalletOverview, WalletServiceError> {
        self.read_request(async {
            let (response, reply) = oneshot::channel();
            self.send(Command::Overview { response }).await?;
            reply.await.map_err(|_| WalletServiceError::Unavailable)?
        })
        .await
    }

    pub async fn receive_addresses(
        &self,
        session_generation: String,
    ) -> Result<Vec<ReceiveAddress>, WalletServiceError> {
        self.read_request(async {
            let (response, reply) = oneshot::channel();
            self.send(Command::ReceiveAddresses {
                session_generation,
                response,
            })
            .await?;
            reply.await.map_err(|_| WalletServiceError::Unavailable)?
        })
        .await
    }

    pub async fn create_receive_address(
        &self,
        session_generation: String,
    ) -> Result<ReceiveAddress, WalletServiceError> {
        let (response, reply) = oneshot::channel();
        self.send(Command::CreateReceiveAddress {
            session_generation,
            response,
        })
        .await?;
        reply.await.map_err(|_| WalletServiceError::Unavailable)?
    }

    pub async fn height(&self) -> Result<u64, WalletServiceError> {
        self.read_request(async {
            let (response, reply) = oneshot::channel();
            self.send(Command::Height { response }).await?;
            reply.await.map_err(|_| WalletServiceError::Unavailable)?
        })
        .await
    }

    /// Recovery is available only for an already-open wallet. The caller must
    /// enforce its backup-pending policy before handing the phrase to the UI.
    pub async fn recovery_phrase(&self) -> Result<RecoveryPhrase, WalletServiceError> {
        let (response, reply) = oneshot::channel();
        self.send(Command::RecoveryPhrase { response }).await?;
        reply.await.map_err(|_| WalletServiceError::Unavailable)?
    }

    async fn send(&self, command: Command) -> Result<(), WalletServiceError> {
        self.sender
            .send(command)
            .await
            .map_err(|_| WalletServiceError::Unavailable)
    }

    pub async fn operation(
        &self,
        session_generation: String,
        operation: WalletOperation,
    ) -> Result<WalletOperationOutput, WalletServiceError> {
        let background_read = operation.is_background_read();
        let request = async {
            let (response, reply) = oneshot::channel();
            self.send(Command::Operation {
                session_generation,
                operation,
                response,
            })
            .await?;
            reply.await.map_err(|_| WalletServiceError::Unavailable)?
        };
        if background_read {
            self.read_request(request).await
        } else {
            request.await
        }
    }
}

impl Default for WalletService {
    fn default() -> Self {
        Self::new()
    }
}

enum Command {
    Operation {
        session_generation: String,
        operation: WalletOperation,
        response: oneshot::Sender<Result<WalletOperationOutput, WalletServiceError>>,
    },
    Status {
        response: oneshot::Sender<LifecycleStatus>,
    },
    IsIdle {
        response: oneshot::Sender<bool>,
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
        generation: Option<String>,
        response: oneshot::Sender<Result<LifecycleStatus, WalletServiceError>>,
    },
    Overview {
        response: oneshot::Sender<Result<WalletOverview, WalletServiceError>>,
    },
    ReceiveAddresses {
        session_generation: String,
        response: oneshot::Sender<Result<Vec<ReceiveAddress>, WalletServiceError>>,
    },
    CreateReceiveAddress {
        session_generation: String,
        response: oneshot::Sender<Result<ReceiveAddress, WalletServiceError>>,
    },
    Height {
        response: oneshot::Sender<Result<u64, WalletServiceError>>,
    },
    RecoveryPhrase {
        response: oneshot::Sender<Result<RecoveryPhrase, WalletServiceError>>,
    },
}

impl Command {
    fn abandoned_read(&self) -> bool {
        match self {
            Self::Overview { response } => response.is_closed(),
            Self::Height { response } => response.is_closed(),
            Self::ReceiveAddresses { response, .. } => response.is_closed(),
            Self::Operation {
                operation,
                response,
                ..
            } => operation.is_background_read() && response.is_closed(),
            _ => false,
        }
    }
}

async fn run_actor(
    mut receiver: mpsc::Receiver<Command>,
    scan: Arc<Mutex<Option<ScanProgress>>>,
    read_busy: Arc<AtomicBool>,
) {
    let mut lifecycle = LifecycleMachine::new();
    let mut session: Option<WalletRpcSession> = None;
    let mut app_paths: Option<AppPaths> = None;
    let mut selected_node: Option<NodeConfig> = None;
    let mut active_id: Option<WalletId> = None;
    let mut operations = OperationsState::default();
    loop {
        let command = tokio::select! {
            command = receiver.recv() => match command { Some(command) => command, None => break },
            _ = tokio::time::sleep(Duration::from_secs(1)) => { operations.expire(); continue; }
        };
        operations.expire();
        if command.abandoned_read() {
            continue;
        }
        match command {
            Command::Operation {
                session_generation,
                operation,
                response,
            } => {
                let background_read = operation.is_background_read();
                let result = require_current_session(&lifecycle, &session_generation)
                    .and_then(|_| session.as_ref().ok_or(WalletServiceError::Unavailable));
                let result = match (
                    result,
                    app_paths.as_ref(),
                    active_id.as_ref(),
                    selected_node.as_ref(),
                ) {
                    (Ok(session), Some(paths), Some(id), Some(node)) => {
                        if matches!(operation, WalletOperation::Rescan { spent_only: false })
                            && let Ok(scan) = scan.lock()
                            && let Some(progress) = scan.as_ref()
                        {
                            progress.clear();
                        }
                        let request = async {
                            operations
                                .execute(
                                    session.client(),
                                    node,
                                    &paths.wallet_dir(id),
                                    &session_generation,
                                    operation,
                                )
                                .await
                                .map_err(WalletServiceError::Operation)
                        };
                        if background_read {
                            bounded_read(READ_EXECUTION_TIMEOUT, request).await
                        } else {
                            request.await
                        }
                    }
                    (Err(error), _, _, _) => Err(error),
                    _ => Err(WalletServiceError::Unavailable),
                };
                if background_read {
                    read_busy.store(
                        matches!(result, Err(WalletServiceError::Busy)),
                        Ordering::Release,
                    );
                }
                let _ = response.send(result);
            }
            Command::Status { response } => {
                let _ = response.send(lifecycle.status());
            }
            Command::IsIdle { response } => {
                let _ = response.send(
                    session.is_none()
                        && matches!(
                            lifecycle.status().state,
                            super::LifecycleState::Stopped | super::LifecycleState::Locked
                        ),
                );
            }
            Command::Start {
                binary,
                paths,
                node,
                rpc_port,
                response,
            } => {
                if lifecycle.status().state == super::LifecycleState::Locked && session.is_some() {
                    let _ = response.send(Ok(lifecycle.status()));
                    continue;
                }
                let transition = if lifecycle.status().state == super::LifecycleState::Locked {
                    lifecycle.restart_after_lock()
                } else {
                    lifecycle.start()
                };
                if let Err(error) = transition {
                    let _ = response.send(Err(WalletServiceError::Lifecycle(error)));
                    continue;
                }
                match WalletRpcSession::start(&binary, &paths, &node, rpc_port)
                    .await
                    .map_err(WalletServiceError::Startup)
                {
                    Ok(started) => {
                        selected_node = Some(node);
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
                let result = match lifecycle
                    .begin_open()
                    .map_err(WalletServiceError::Lifecycle)
                    .and_then(|_| {
                        app_paths
                            .as_ref()
                            .ok_or(WalletServiceError::Unavailable)
                            .and_then(|paths| {
                                paths
                                    .secure_existing_wallet(&wallet_id)
                                    .map_err(WalletServiceError::Storage)
                            })
                    }) {
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
                    Err(error) => Err(error),
                };
                if result.is_err() && lifecycle.status().state == super::LifecycleState::Opening {
                    let _ = lifecycle.opening_failed();
                }
                if result.is_ok() {
                    if let (Some(session), Some(paths)) = (session.as_ref(), app_paths.as_ref()) {
                        let directory = paths.wallet_dir(&wallet_id);
                        if !matches!(load_wallet_name(&directory), Ok(Some(_))) {
                            // Recover older names during unlock, including wallets that
                            // have not completed backup or mounted the dashboard yet.
                            // A busy scan or broken display cache must not reject unlock.
                            let _ = bounded_read(READ_EXECUTION_TIMEOUT, async {
                                read_wallet_name(session.client(), &directory)
                                    .await
                                    .map_err(WalletServiceError::Operation)
                            })
                            .await;
                        }
                    }
                    active_id = Some(wallet_id);
                    operations.invalidate();
                    *scan.lock().expect("scan state") =
                        session.as_ref().map(WalletRpcSession::scan_progress);
                    read_busy.store(false, Ordering::Release);
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
                if result.is_ok() {
                    active_id = Some(wallet_id);
                    operations.invalidate();
                    *scan.lock().expect("scan state") =
                        session.as_ref().map(WalletRpcSession::scan_progress);
                    read_busy.store(false, Ordering::Release);
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
                if result.is_ok() {
                    active_id = Some(wallet_id);
                    operations.invalidate();
                    *scan.lock().expect("scan state") =
                        session.as_ref().map(WalletRpcSession::scan_progress);
                    read_busy.store(false, Ordering::Release);
                }
                let _ = response.send(result);
            }
            Command::Lock {
                deadline,
                response,
                generation,
            } => {
                if let Some(generation) = generation
                    && let Err(error) = require_current_session(&lifecycle, &generation)
                {
                    let _ = response.send(Err(error));
                    continue;
                }
                operations.invalidate();
                active_id = None;
                *scan.lock().expect("scan state") = None;
                read_busy.store(false, Ordering::Release);
                if session.is_none()
                    && matches!(
                        lifecycle.status().state,
                        super::LifecycleState::Locked | super::LifecycleState::Stopped
                    )
                {
                    let _ = response.send(Ok(lifecycle.status()));
                    continue;
                }
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
                            bounded_read(
                                READ_EXECUTION_TIMEOUT,
                                wallet_overview(session.client(), lifecycle.status()),
                            )
                            .await
                        }
                        None => Err(WalletServiceError::Unavailable),
                    },
                    Err(error) => Err(error),
                };
                read_busy.store(
                    matches!(result, Err(WalletServiceError::Busy)),
                    Ordering::Release,
                );
                let _ = response.send(result);
            }
            Command::ReceiveAddresses {
                session_generation,
                response,
            } => {
                let result = require_current_session(&lifecycle, &session_generation)
                    .and_then(|_| session.as_ref().ok_or(WalletServiceError::Unavailable));
                let result = match result {
                    Ok(session) => {
                        bounded_read(READ_EXECUTION_TIMEOUT, async {
                            session
                                .client()
                                .receive_addresses()
                                .await
                                .map_err(WalletServiceError::Rpc)
                        })
                        .await
                    }
                    Err(error) => Err(error),
                };
                read_busy.store(
                    matches!(result, Err(WalletServiceError::Busy)),
                    Ordering::Release,
                );
                let _ = response.send(result);
            }
            Command::CreateReceiveAddress {
                session_generation,
                response,
            } => {
                let result = require_current_session(&lifecycle, &session_generation)
                    .and_then(|_| session.as_ref().ok_or(WalletServiceError::Unavailable));
                let result = match result {
                    Ok(session) => session
                        .client()
                        .create_receive_address()
                        .await
                        .map_err(WalletServiceError::Rpc),
                    Err(error) => Err(error),
                };
                let _ = response.send(result);
            }
            Command::Height { response } => {
                let result = lifecycle
                    .require_open()
                    .map_err(WalletServiceError::Lifecycle)
                    .and_then(|_| session.as_ref().ok_or(WalletServiceError::Unavailable));

                let result = match result {
                    Ok(session) => {
                        bounded_read(READ_EXECUTION_TIMEOUT, async {
                            session
                                .client()
                                .height()
                                .await
                                .map_err(WalletServiceError::Rpc)
                        })
                        .await
                    }
                    Err(error) => Err(error),
                };

                read_busy.store(
                    matches!(result, Err(WalletServiceError::Busy)),
                    Ordering::Release,
                );
                let _ = response.send(result);
            }
            Command::RecoveryPhrase { response } => {
                let result = lifecycle
                    .require_open()
                    .map_err(WalletServiceError::Lifecycle)
                    .and_then(|_| session.as_ref().ok_or(WalletServiceError::Unavailable));
                let result = match result {
                    Ok(session) => session
                        .client()
                        .query_mnemonic()
                        .await
                        .map_err(WalletServiceError::Rpc),
                    Err(error) => Err(error),
                };
                let _ = response.send(result);
            }
        }
    }
}

fn require_current_session(
    lifecycle: &LifecycleMachine,
    session_generation: &str,
) -> Result<(), WalletServiceError> {
    lifecycle
        .require_open()
        .map_err(WalletServiceError::Lifecycle)?;
    if lifecycle.status().session_generation != session_generation {
        return Err(WalletServiceError::StaleSession);
    }
    Ok(())
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
    async fn read_deadline_and_abandoned_reads_do_not_discard_mutations() {
        assert!(matches!(
            bounded_read::<()>(Duration::from_millis(10), std::future::pending()).await,
            Err(WalletServiceError::Busy)
        ));
        let (response, reply) = oneshot::channel();
        drop(reply);
        assert!(Command::Height { response }.abandoned_read());
        let (response, reply) = oneshot::channel();
        drop(reply);
        assert!(
            Command::Operation {
                session_generation: "1".into(),
                operation: WalletOperation::History,
                response
            }
            .abandoned_read()
        );
        let (response, reply) = oneshot::channel();
        drop(reply);
        assert!(
            !Command::Operation {
                session_generation: "1".into(),
                operation: WalletOperation::SetName {
                    name: "saved".into()
                },
                response
            }
            .abandoned_read()
        );
        let (response, reply) = oneshot::channel();
        drop(reply);
        assert!(
            !Command::Lock {
                deadline: Duration::from_secs(5),
                generation: None,
                response
            }
            .abandoned_read()
        );
    }

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
        assert!(matches!(
            service.create_receive_address("0".to_owned()).await,
            Err(WalletServiceError::Lifecycle(_))
        ));
    }
}
