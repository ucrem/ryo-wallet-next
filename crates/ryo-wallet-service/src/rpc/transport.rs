use std::fmt;
use std::net::{Ipv4Addr, SocketAddrV4};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use diqwest::WithDigestAuth;
use reqwest::{Client, StatusCode, redirect::Policy};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use thiserror::Error;
use zeroize::Zeroizing;

const MAX_RESPONSE_BYTES: usize = 2 * 1024 * 1024;

#[derive(Error, Debug, PartialEq, Eq)]
pub enum RpcError {
    #[error("RPC endpoint must be the managed IPv4 loopback address")]
    InvalidEndpoint,
    #[error("RPC credentials are invalid")]
    InvalidCredentials,
    #[error("RPC connection failed or timed out")]
    Transport,
    #[error("RPC authentication failed")]
    Authentication,
    #[error("RPC response exceeded the size limit")]
    ResponseTooLarge,
    #[error("RPC HTTP status {0}")]
    HttpStatus(u16),
    #[error("RPC response was malformed or unexpected")]
    InvalidResponse,
    #[error("RPC method is unavailable")]
    MethodUnavailable,
    #[error("address is invalid for the open wallet network")]
    InvalidAddress,
    #[error("wallet password was rejected")]
    IncorrectPassword,
    #[error("RPC server returned code {0}")]
    Remote(i32),
}

/// Credentials are held only in Rust; Debug output is deliberately redacted.
pub struct RpcCredentials {
    username: Zeroizing<String>,
    password: Zeroizing<String>,
}

impl fmt::Debug for RpcCredentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RpcCredentials([REDACTED])")
    }
}

impl RpcCredentials {
    pub fn new(username: String, password: String) -> Result<Self, RpcError> {
        let username = Zeroizing::new(username);
        let password = Zeroizing::new(password);
        if username.is_empty()
            || password.is_empty()
            || username.len() > 256
            || password.len() > 256
            || username.bytes().any(|b| b == b':' || b.is_ascii_control())
            || password.bytes().any(|b| b.is_ascii_control())
        {
            return Err(RpcError::InvalidCredentials);
        }
        Ok(Self { username, password })
    }
}

pub(crate) struct JsonRpcTransport {
    endpoint: String,
    credentials: Option<RpcCredentials>,
    next_id: AtomicU64,
}

impl JsonRpcTransport {
    pub(crate) fn local(
        address: SocketAddrV4,
        credentials: Option<RpcCredentials>,
    ) -> Result<Self, RpcError> {
        if *address.ip() != Ipv4Addr::LOCALHOST || address.port() == 0 {
            return Err(RpcError::InvalidEndpoint);
        }
        new_loopback_client()?;
        Ok(Self {
            endpoint: format!("http://{address}/json_rpc"),
            credentials,
            next_id: AtomicU64::new(1),
        })
    }

    pub(crate) async fn call<P, R>(&self, method: &'static str, params: &P) -> Result<R, RpcError>
    where
        P: Serialize + ?Sized,
        R: DeserializeOwned,
    {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let payload = Zeroizing::new(
            serde_json::to_vec(&RpcRequest {
                jsonrpc: "2.0",
                id,
                method,
                params,
            })
            .map_err(|_| RpcError::InvalidResponse)?,
        );
        // reqwest owns its body after this point. Its internal copy cannot be zeroized here.
        // The 0.6.1.0 loopback server accepts a fresh Digest handshake per
        // request but fails after connection reuse. The endpoint is private
        // and local, so a short-lived client is the compatible safe boundary.
        let client = new_loopback_client()?;
        let request = client
            .post(&self.endpoint)
            .header("Content-Type", "application/json")
            .body(payload.to_vec());
        let mut response = match &self.credentials {
            Some(c) => request
                .send_digest_auth((c.username.as_str(), c.password.as_str()))
                .await
                .map_err(|_| RpcError::Transport)?,
            None => request.send().await.map_err(|_| RpcError::Transport)?,
        };
        if response.status() == StatusCode::UNAUTHORIZED {
            return Err(RpcError::Authentication);
        }
        if !response.status().is_success() {
            return Err(RpcError::HttpStatus(response.status().as_u16()));
        }
        if response
            .content_length()
            .is_some_and(|len| len > MAX_RESPONSE_BYTES as u64)
        {
            return Err(RpcError::ResponseTooLarge);
        }
        let mut body = Zeroizing::new(Vec::new());
        while let Some(chunk) = response.chunk().await.map_err(|_| RpcError::Transport)? {
            if chunk.len() > MAX_RESPONSE_BYTES.saturating_sub(body.len()) {
                return Err(RpcError::ResponseTooLarge);
            }
            body.extend_from_slice(&chunk);
        }
        let decoded: RpcResponse<R> =
            serde_json::from_slice(&body).map_err(|_| RpcError::InvalidResponse)?;
        if decoded.id != id || decoded.jsonrpc != "2.0" {
            return Err(RpcError::InvalidResponse);
        }
        match (decoded.result, decoded.error) {
            (Some(result), None) => Ok(result),
            (None, Some(error)) if error.code == -32601 => Err(RpcError::MethodUnavailable),
            (None, Some(error)) => Err(RpcError::Remote(error.code)),
            _ => Err(RpcError::InvalidResponse),
        }
    }
}

fn new_loopback_client() -> Result<Client, RpcError> {
    Client::builder()
        .no_proxy()
        .redirect(Policy::none())
        .connect_timeout(Duration::from_secs(2))
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|_| RpcError::Transport)
}

#[derive(Serialize)]
struct RpcRequest<'a, P: ?Sized> {
    jsonrpc: &'static str,
    id: u64,
    method: &'static str,
    params: &'a P,
}

#[derive(Deserialize)]
struct RpcResponse<R> {
    jsonrpc: String,
    id: u64,
    result: Option<R>,
    error: Option<RpcRemoteError>,
}

#[derive(Deserialize)]
struct RpcRemoteError {
    code: i32,
}
