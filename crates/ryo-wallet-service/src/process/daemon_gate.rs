//! A byte-preserving, same-host transport owned by one wallet RPC session.
//! Closing it ends a blocking daemon refresh before `stop_wallet` saves the
//! actual encrypted wallet cache. It never invents blocks or scan heights.
use std::io;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::time::Duration;

use tokio::io::copy_bidirectional;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::oneshot;
use tokio::task::{JoinHandle, JoinSet};

use crate::domain::NodeConfig;

pub(super) struct DaemonGate {
    pub address: SocketAddr,
    close: Option<oneshot::Sender<()>>,
    task: JoinHandle<()>,
}

impl DaemonGate {
    pub async fn start(node: &NodeConfig) -> io::Result<Self> {
        node.validate().map_err(io::Error::other)?;
        // Upstream 0.6.1.0 automatically trusts loopback daemon addresses and
        // has no force-untrusted switch. Preserve that trust boundary: real
        // remote daemons use this machine's routed IPv4 address, with foreign
        // source addresses rejected before any request is read or forwarded.
        let original_loopback = node.uses_local()
            || node.host.eq_ignore_ascii_case("localhost")
            || node.host.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback());
        let bind_ip = if original_loopback {
            Ipv4Addr::LOCALHOST
        } else {
            routed_local_ip()?
        };
        let listener = TcpListener::bind((bind_ip, 0)).await?;
        let address = listener.local_addr()?;
        let (close, closed) = oneshot::channel();
        let upstream = (node.host.clone(), node.port);
        let task = tokio::spawn(serve(listener, bind_ip.into(), upstream, closed));
        Ok(Self {
            address,
            close: Some(close),
            task,
        })
    }

    /// Await socket cancellation before queueing the graceful wallet RPC stop.
    pub async fn quiesce(&mut self) {
        if let Some(close) = self.close.take() {
            let _ = close.send(());
            let _ = (&mut self.task).await;
        }
    }
}

impl Drop for DaemonGate {
    fn drop(&mut self) {
        self.task.abort(); // serve's JoinSet also owns and aborts every connection.
    }
}

fn routed_local_ip() -> io::Result<Ipv4Addr> {
    // UDP connect selects a route without sending a datagram. The documentation
    // address is never contacted. A remote daemon must not be silently promoted
    // to trusted if the machine has no usable non-loopback IPv4 interface.
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))?;
    socket.connect((Ipv4Addr::new(192, 0, 2, 1), 9))?;
    match socket.local_addr()?.ip() {
        IpAddr::V4(ip) if !ip.is_loopback() && !ip.is_unspecified() => Ok(ip),
        _ => Err(io::Error::other(
            "no same-host route for an untrusted daemon",
        )),
    }
}

async fn serve(
    listener: TcpListener,
    allowed_ip: IpAddr,
    upstream: (String, u16),
    mut closed: oneshot::Receiver<()>,
) {
    let mut connections = JoinSet::new();
    loop {
        tokio::select! {
            biased;
            _ = &mut closed => break,
            Some(_) = connections.join_next(), if !connections.is_empty() => {},
            accepted = listener.accept() => {
                let Ok((mut local, peer)) = accepted else { break };
                if peer.ip() != allowed_ip || connections.len() >= 16 { continue; }
                let upstream = upstream.clone();
                connections.spawn(async move {
                    let Ok(Ok(mut remote)) = tokio::time::timeout(
                        Duration::from_secs(10), TcpStream::connect(upstream)
                    ).await else { return };
                    let _ = copy_bidirectional(&mut local, &mut remote).await;
                });
            }
        }
    }
    drop(listener);
    connections.abort_all();
    while connections.join_next().await.is_some() {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::Network;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[tokio::test]
    async fn a_public_remote_is_not_promoted_to_loopback_trust() {
        let node = NodeConfig::remote(Network::Mainnet, "192.0.2.10".into(), 12211).unwrap();
        // No datagrams or daemon requests are sent by starting a gate.
        let mut gate = DaemonGate::start(&node).await.unwrap();
        assert!(!gate.address.ip().is_loopback());
        assert!(!gate.address.ip().is_unspecified());
        gate.quiesce().await;
    }

    #[tokio::test]
    async fn quiesce_closes_inflight_reads_and_prevents_reconnection() {
        let upstream = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let mut node = NodeConfig::managed_local(Network::Testnet);
        node.port = upstream.local_addr().unwrap().port();
        let mut gate = DaemonGate::start(&node).await.unwrap();
        let mut wallet = TcpStream::connect(gate.address).await.unwrap();
        let (mut daemon, _) = upstream.accept().await.unwrap();
        wallet.write_all(b"unaltered wallet request").await.unwrap();
        let mut data = [0; 24];
        daemon.read_exact(&mut data).await.unwrap();
        assert_eq!(&data, b"unaltered wallet request");
        daemon
            .write_all(b"unaltered daemon response")
            .await
            .unwrap();
        let mut data = [0; 25];
        wallet.read_exact(&mut data).await.unwrap();
        assert_eq!(&data, b"unaltered daemon response");
        gate.quiesce().await;
        assert_eq!(wallet.read(&mut [0]).await.unwrap(), 0);
        assert_eq!(daemon.read(&mut [0]).await.unwrap(), 0);
        assert!(TcpStream::connect(gate.address).await.is_err());
    }

    #[tokio::test]
    async fn foreign_source_is_rejected_without_forwarding() {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let address = listener.local_addr().unwrap();
        let upstream = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let (close, closed) = oneshot::channel();
        let task = tokio::spawn(serve(
            listener,
            "192.0.2.2".parse().unwrap(),
            ("127.0.0.1".into(), upstream.local_addr().unwrap().port()),
            closed,
        ));
        let mut foreign = TcpStream::connect(address).await.unwrap();
        assert_eq!(foreign.read(&mut [0]).await.unwrap(), 0);
        assert!(
            tokio::time::timeout(Duration::from_millis(50), upstream.accept())
                .await
                .is_err()
        );
        close.send(()).unwrap();
        task.await.unwrap();
    }
}
