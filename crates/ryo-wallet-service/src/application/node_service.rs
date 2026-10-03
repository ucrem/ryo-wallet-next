use serde::Serialize;
use std::path::PathBuf;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};
use tokio::time::timeout;
use ts_rs::TS;

use crate::domain::{NodeConfig, NodeMode};
use crate::process::{DaemonError, DaemonSession, VerifiedBinary};
use crate::rpc::DaemonRpcClient;
use crate::storage::AppPaths;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "snake_case")]
pub enum NodeState {
    Stopped,
    Running,
    Faulted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct NodeStatus {
    pub mode: NodeMode,
    pub state: NodeState,
    pub generation: String,
    pub height: Option<String>,
    #[ts(optional)]
    pub rpc_height: Option<String>,
    pub target_height: Option<String>,
    pub reachable: bool,
    pub ready: bool,
    pub offline: bool,
    pub untrusted: bool,
}

#[derive(Clone)]
pub struct NodeService {
    sender: mpsc::Sender<Command>,
}

#[derive(Clone)]
struct Snapshot {
    state: NodeState,
    generation: u64,
    endpoint: Option<NodeConfig>,
}

enum Command {
    Start {
        binary: VerifiedBinary,
        paths: AppPaths,
        node: NodeConfig,
        response: oneshot::Sender<Result<NodeConfig, DaemonError>>,
    },
    Stop {
        response: oneshot::Sender<Result<(), DaemonError>>,
    },
    Inspect {
        response: oneshot::Sender<Snapshot>,
    },
}

impl Default for NodeService {
    fn default() -> Self {
        Self::new()
    }
}

impl NodeService {
    /// A separate owner and queue: wallet lock cannot terminate the daemon.
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::channel(8);
        std::thread::Builder::new()
            .name("ryo-node-service".to_owned())
            .spawn(move || {
                tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("node runtime must be available")
                    .block_on(run_actor(receiver));
            })
            .expect("node service thread must be available");
        Self { sender }
    }

    pub async fn start(
        &self,
        binary: VerifiedBinary,
        paths: AppPaths,
        node: NodeConfig,
    ) -> Result<NodeConfig, DaemonError> {
        let (response, reply) = oneshot::channel();
        self.sender
            .send(Command::Start {
                binary,
                paths,
                node,
                response,
            })
            .await
            .map_err(|_| DaemonError::Configuration)?;
        reply.await.map_err(|_| DaemonError::Configuration)?
    }

    pub async fn stop(&self) -> Result<(), DaemonError> {
        let (response, reply) = oneshot::channel();
        self.sender
            .send(Command::Stop { response })
            .await
            .map_err(|_| DaemonError::Configuration)?;
        reply.await.map_err(|_| DaemonError::Configuration)?
    }

    async fn inspect(&self) -> Result<Snapshot, DaemonError> {
        let (response, reply) = oneshot::channel();
        self.sender
            .send(Command::Inspect { response })
            .await
            .map_err(|_| DaemonError::Configuration)?;
        reply.await.map_err(|_| DaemonError::Configuration)
    }

    pub async fn is_idle(&self) -> Result<bool, DaemonError> {
        Ok(self.inspect().await?.endpoint.is_none())
    }

    pub async fn status(&self, node: &NodeConfig) -> Result<NodeStatus, DaemonError> {
        node.validate().map_err(|_| DaemonError::Configuration)?;
        let snapshot = self.inspect().await?;
        let mut status = empty_status(node.mode, &snapshot);
        let endpoint = if node.mode == NodeMode::Remote {
            Some(node.clone())
        } else {
            snapshot.endpoint.clone()
        };
        let Some(endpoint) = endpoint else {
            return Ok(status);
        };
        if node.mode == NodeMode::Remote {
            status.state = NodeState::Running;
        }
        let client =
            DaemonRpcClient::configured(&endpoint).map_err(|_| DaemonError::Configuration)?;
        if let Ok(Ok(health)) = timeout(Duration::from_secs(3), client.health()).await
            && health.network == node.network
        {
            status.height = Some(health.local_height.to_string());
            status.rpc_height = Some(health.height.to_string());
            let target = health.target_height.max(health.height);
            // No known target yet must not appear as 100% synchronized.
            status.target_height =
                (health.target_height > 0 || health.ready).then(|| target.to_string());
            status.reachable = true;
            status.ready = health.ready;
            status.offline = health.offline;
            status.untrusted = health.untrusted;
        }
        if node.uses_local() {
            let current = self.inspect().await?;
            if current.generation != snapshot.generation {
                return Ok(empty_status(node.mode, &current));
            }
        }
        Ok(status)
    }
}

fn empty_status(mode: NodeMode, snapshot: &Snapshot) -> NodeStatus {
    NodeStatus {
        mode,
        state: snapshot.state,
        generation: snapshot.generation.to_string(),
        height: None,
        rpc_height: None,
        target_height: None,
        reachable: false,
        ready: false,
        offline: true,
        untrusted: false,
    }
}

async fn run_actor(mut receiver: mpsc::Receiver<Command>) {
    let mut session: Option<DaemonSession> = None;
    let mut root: Option<PathBuf> = None;
    let mut state = NodeState::Stopped;
    let mut generation = 0_u64;
    while let Some(command) = receiver.recv().await {
        if session
            .as_mut()
            .is_some_and(|session| session.has_exited().unwrap_or(true))
        {
            session = None;
            root = None;
            state = NodeState::Faulted;
            generation = generation.wrapping_add(1);
        }
        match command {
            Command::Start {
                binary,
                paths,
                node,
                response,
            } => {
                let result = if let Some(current) = &session {
                    if root.as_deref() == Some(paths.root())
                        && current.node().network == node.network
                        && node.uses_local()
                        && current.node().mode == node.mode
                        && current.node().advanced == node.advanced
                        && current.node().bootstrap == node.bootstrap
                    {
                        Ok(current.node().clone())
                    } else {
                        Err(DaemonError::Configuration)
                    }
                } else {
                    generation = generation.wrapping_add(1);
                    match DaemonSession::start(&binary, &paths, &node).await {
                        Ok(started) => {
                            let endpoint = started.node().clone();
                            session = Some(started);
                            root = Some(paths.root().to_owned());
                            state = NodeState::Running;
                            Ok(endpoint)
                        }
                        Err(error) => {
                            state = NodeState::Faulted;
                            Err(error)
                        }
                    }
                };
                let _ = response.send(result);
            }
            Command::Stop { response } => {
                generation = generation.wrapping_add(1);
                let result = match session.as_mut() {
                    Some(current) => current.stop().await.map_err(DaemonError::Process),
                    None => Ok(()),
                };
                if result.is_ok() {
                    session = None;
                    root = None;
                    state = NodeState::Stopped;
                } else {
                    state = NodeState::Faulted;
                }
                let _ = response.send(result);
            }
            Command::Inspect { response } => {
                let _ = response.send(Snapshot {
                    state,
                    generation,
                    endpoint: session.as_ref().map(|session| session.node().clone()),
                });
            }
        }
    }
    if let Some(mut session) = session {
        let _ = session.stop().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::Network;

    #[tokio::test]
    async fn stopped_local_node_is_not_an_external_daemon_and_stop_is_idempotent() {
        let service = NodeService::new();
        let node = NodeConfig::managed_local(Network::Mainnet);
        assert!(service.is_idle().await.unwrap());
        let status = service.status(&node).await.unwrap();
        assert_eq!(status.state, NodeState::Stopped);
        assert!(!status.reachable);
        assert_eq!(status.height, None);
        service.stop().await.unwrap();
        service.stop().await.unwrap();
        assert!(service.is_idle().await.unwrap());
    }
}
