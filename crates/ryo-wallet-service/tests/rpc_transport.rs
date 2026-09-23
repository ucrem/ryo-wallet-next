use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};

use ryo_wallet_service::rpc::{DaemonRpcClient, RpcCredentials, RpcError, WalletRpcClient};
use ryo_wallet_service::storage::WalletId;
use serde_json::Value;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};
use zeroize::Zeroizing;

async fn read_request(stream: &mut TcpStream) -> (String, Value) {
    let mut bytes = Vec::new();
    let header_end = loop {
        let mut chunk = [0_u8; 4096];
        let count = stream.read(&mut chunk).await.unwrap();
        assert!(count > 0, "unexpected EOF before headers");
        bytes.extend_from_slice(&chunk[..count]);
        if let Some(index) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
            break index + 4;
        }
        assert!(bytes.len() < 16 * 1024);
    };
    let headers = String::from_utf8(bytes[..header_end].to_vec()).unwrap();
    let content_length: usize = headers
        .lines()
        .find_map(|line| {
            line.to_ascii_lowercase()
                .strip_prefix("content-length: ")
                .and_then(|value| value.parse().ok())
        })
        .unwrap();
    while bytes.len() < header_end + content_length {
        let mut chunk = [0_u8; 4096];
        let count = stream.read(&mut chunk).await.unwrap();
        assert!(count > 0, "unexpected EOF in body");
        bytes.extend_from_slice(&chunk[..count]);
    }
    let body = serde_json::from_slice(&bytes[header_end..header_end + content_length]).unwrap();
    (headers, body)
}

async fn reply(stream: &mut TcpStream, status: &str, extra_headers: &str, body: &str) {
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n{extra_headers}\r\n{body}",
        body.len()
    );
    stream.write_all(response.as_bytes()).await.unwrap();
}

async fn listener() -> (TcpListener, SocketAddrV4) {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
    let SocketAddr::V4(address) = listener.local_addr().unwrap() else {
        unreachable!()
    };
    (listener, address)
}

#[tokio::test]
async fn wallet_uses_digest_and_the_source_defined_method() {
    let (listener, address) = listener().await;
    let server = tokio::spawn(async move {
        let (mut first, _) = listener.accept().await.unwrap();
        let (headers, body) = read_request(&mut first).await;
        assert!(headers.starts_with("POST /json_rpc HTTP/1.1"));
        assert_eq!(body["method"], "get_languages");
        assert_eq!(body["params"], serde_json::json!({}));
        assert!(!headers.to_ascii_lowercase().contains("authorization:"));
        reply(
            &mut first,
            "401 Unauthorized",
            "WWW-Authenticate: Digest realm=\"ryo\", nonce=\"unit-test-nonce\", qop=\"auth\", algorithm=MD5\r\n",
            "",
        )
        .await;
        drop(first);

        let (mut second, _) = listener.accept().await.unwrap();
        let (headers, authenticated_body) = read_request(&mut second).await;
        assert!(
            headers
                .to_ascii_lowercase()
                .contains("authorization: digest ")
        );
        assert!(headers.contains("username=\"test-user\""));
        assert!(headers.contains("uri=\"/json_rpc\""));
        assert_eq!(authenticated_body, body);
        let response = serde_json::json!({
            "jsonrpc": "2.0", "id": body["id"], "result": {"languages": ["English"]}
        });
        reply(
            &mut second,
            "200 OK",
            "Content-Type: application/json\r\n",
            &response.to_string(),
        )
        .await;
    });

    let creds = RpcCredentials::new("test-user".into(), "test-password".into()).unwrap();
    assert_eq!(format!("{creds:?}"), "RpcCredentials([REDACTED])");
    let wallet = WalletRpcClient::new(address, creds).unwrap();
    assert_eq!(wallet.languages().await.unwrap(), vec!["English"]);
    server.await.unwrap();
}

#[tokio::test]
async fn daemon_rejects_malformed_network_and_mismatched_request_id() {
    let (listener, address) = listener().await;
    let server = tokio::spawn(async move {
        for bad_id in [false, true] {
            let (mut stream, _) = listener.accept().await.unwrap();
            let (_, request) = read_request(&mut stream).await;
            assert_eq!(request["method"], "get_info");
            let response = serde_json::json!({
                "jsonrpc": "2.0",
                "id": if bad_id { request["id"].as_u64().unwrap() + 1 } else { request["id"].as_u64().unwrap() },
                "result": {
                    "height": 90, "target_height": 100,
                    "mainnet": true, "testnet": true, "stagenet": false,
                    "is_ready": false, "offline": false, "untrusted": false
                }
            });
            reply(&mut stream, "200 OK", "", &response.to_string()).await;
        }
    });
    let daemon = DaemonRpcClient::local(address, None).unwrap();
    assert_eq!(
        daemon.health().await.unwrap_err(),
        RpcError::InvalidResponse
    );
    assert_eq!(
        daemon.health().await.unwrap_err(),
        RpcError::InvalidResponse
    );
    server.await.unwrap();
}

#[tokio::test]
async fn response_size_limit_prevents_unbounded_allocation() {
    let (listener, address) = listener().await;
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        read_request(&mut stream).await;
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2097153\r\nConnection: close\r\n\r\n")
            .await
            .unwrap();
    });
    let daemon = DaemonRpcClient::local(address, None).unwrap();
    assert_eq!(
        daemon.health().await.unwrap_err(),
        RpcError::ResponseTooLarge
    );
    server.await.unwrap();
}

#[tokio::test]
async fn only_managed_ipv4_loopback_is_allowed() {
    let creds = RpcCredentials::new("u".into(), "p".into()).unwrap();
    assert!(matches!(
        WalletRpcClient::new(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 2), 3000), creds),
        Err(RpcError::InvalidEndpoint)
    ));
}

#[tokio::test]
async fn wallet_methods_preserve_large_amounts_and_use_registered_address_parser() {
    let (listener, address) = listener().await;
    let server = tokio::spawn(async move {
        let (mut balance_stream, _) = listener.accept().await.unwrap();
        let (_, request) = read_request(&mut balance_stream).await;
        assert_eq!(request["method"], "get_balance");
        assert_eq!(request["params"], serde_json::json!({ "account_index": 0 }));
        let response = serde_json::json!({
            "jsonrpc": "2.0", "id": request["id"],
            "result": {
                "balance": 9_007_199_254_740_993_u64,
                "unlocked_balance": 9_007_199_254_740_992_u64,
                "multisig_import_needed": false
            }
        });
        reply(&mut balance_stream, "200 OK", "", &response.to_string()).await;
        drop(balance_stream);

        let (mut address_stream, _) = listener.accept().await.unwrap();
        let (_, request) = read_request(&mut address_stream).await;
        assert_eq!(request["method"], "parse_uri");
        assert_eq!(request["params"]["uri"], "ryo:RYoExample123");
        let response = serde_json::json!({
            "jsonrpc": "2.0", "id": request["id"],
            "result": { "uri": { "address": "RYoExample123" }, "unknown_parameters": [] }
        });
        reply(&mut address_stream, "200 OK", "", &response.to_string()).await;
    });
    let wallet = WalletRpcClient::new(
        address,
        RpcCredentials::new("u".into(), "p".into()).unwrap(),
    )
    .unwrap();
    let balance = wallet.balance().await.unwrap();
    assert_eq!(balance.total.atomic(), 9_007_199_254_740_993);
    assert_eq!(balance.unlocked.atomic(), 9_007_199_254_740_992);
    wallet.validate_address("RYoExample123").await.unwrap();
    assert_eq!(
        wallet.validate_address("ryo:bad?amount=1").await,
        Err(RpcError::InvalidAddress)
    );
    server.await.unwrap();
}

#[tokio::test]
async fn wallet_lifecycle_methods_only_accept_an_internal_wallet_id() {
    let (listener, address) = listener().await;
    let server = tokio::spawn(async move {
        for expected_method in ["open_wallet", "close_wallet", "stop_wallet"] {
            let (mut stream, _) = listener.accept().await.unwrap();
            let (_, request) = read_request(&mut stream).await;
            assert_eq!(request["method"], expected_method);
            if expected_method == "open_wallet" {
                assert_eq!(
                    request["params"]["filename"],
                    "0123456789abcdef0123456789abcdef/wallet"
                );
                assert_eq!(request["params"]["password"], "test-password");
            } else {
                assert_eq!(request["params"], serde_json::json!({}));
            }
            let response =
                serde_json::json!({ "jsonrpc": "2.0", "id": request["id"], "result": {} });
            reply(&mut stream, "200 OK", "", &response.to_string()).await;
        }
    });
    let wallet = WalletRpcClient::new(
        address,
        RpcCredentials::new("u".into(), "p".into()).unwrap(),
    )
    .unwrap();
    let id = WalletId::parse("0123456789abcdef0123456789abcdef").unwrap();
    wallet
        .open_imported_wallet(&id, Zeroizing::new("test-password".to_owned()))
        .await
        .unwrap();
    wallet.close_wallet().await.unwrap();
    wallet.stop_wallet().await.unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn wallet_scope_uses_registered_account_and_multisig_probes() {
    let (listener, address) = listener().await;
    let server = tokio::spawn(async move {
        let (mut accounts_stream, _) = listener.accept().await.unwrap();
        let (_, request) = read_request(&mut accounts_stream).await;
        assert_eq!(request["method"], "get_accounts");
        assert_eq!(request["params"], serde_json::json!({ "tag": "" }));
        let response = serde_json::json!({
            "jsonrpc": "2.0", "id": request["id"],
            "result": { "subaddress_accounts": [{ "account_index": 0 }] }
        });
        reply(&mut accounts_stream, "200 OK", "", &response.to_string()).await;

        let (mut multisig_stream, _) = listener.accept().await.unwrap();
        let (_, request) = read_request(&mut multisig_stream).await;
        assert_eq!(request["method"], "is_multisig");
        assert_eq!(request["params"], serde_json::json!({}));
        let response = serde_json::json!({
            "jsonrpc": "2.0", "id": request["id"], "result": { "multisig": false }
        });
        reply(&mut multisig_stream, "200 OK", "", &response.to_string()).await;
    });
    let wallet = WalletRpcClient::new(
        address,
        RpcCredentials::new("u".into(), "p".into()).unwrap(),
    )
    .unwrap();
    assert_eq!(
        wallet.scope().await.unwrap(),
        ryo_wallet_service::rpc::WalletScope {
            account_count: 1,
            multisig: false,
        }
    );
    server.await.unwrap();
}

#[tokio::test]
async fn incorrect_password_does_not_expose_the_upstream_error_body() {
    let (listener, address) = listener().await;
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let (_, request) = read_request(&mut stream).await;
        let response = serde_json::json!({
            "jsonrpc": "2.0", "id": request["id"],
            "error": { "code": -22, "message": "wrong password leaked here" }
        });
        reply(&mut stream, "200 OK", "", &response.to_string()).await;
    });
    let wallet = WalletRpcClient::new(
        address,
        RpcCredentials::new("u".into(), "p".into()).unwrap(),
    )
    .unwrap();
    let id = WalletId::parse("0123456789abcdef0123456789abcdef").unwrap();
    assert_eq!(
        wallet
            .open_imported_wallet(&id, Zeroizing::new("wrong".to_owned()))
            .await,
        Err(RpcError::IncorrectPassword)
    );
    server.await.unwrap();
}

#[tokio::test]
async fn create_and_restore_keep_seed_and_password_in_typed_internal_requests() {
    let (listener, address) = listener().await;
    let server = tokio::spawn(async move {
        for expected_method in ["create_wallet", "restore_wallet"] {
            let (mut stream, _) = listener.accept().await.unwrap();
            let (_, request) = read_request(&mut stream).await;
            assert_eq!(request["method"], expected_method);
            assert_eq!(
                request["params"]["filename"],
                "0123456789abcdef0123456789abcdef/wallet"
            );
            if expected_method == "create_wallet" {
                assert_eq!(request["params"]["language"], "English");
                assert_eq!(request["params"]["short_address"], false);
            } else {
                assert_eq!(request["params"]["seed"], "one two three");
                assert_eq!(request["params"]["refresh_start_height"], 123_u64);
            }
            let response =
                serde_json::json!({ "jsonrpc": "2.0", "id": request["id"], "result": {} });
            reply(&mut stream, "200 OK", "", &response.to_string()).await;
        }
    });
    let wallet = WalletRpcClient::new(
        address,
        RpcCredentials::new("u".into(), "p".into()).unwrap(),
    )
    .unwrap();
    let id = WalletId::parse("0123456789abcdef0123456789abcdef").unwrap();
    wallet
        .create_wallet(&id, Zeroizing::new("test-password".to_owned()), "English")
        .await
        .unwrap();
    wallet
        .restore_wallet(
            &id,
            Zeroizing::new("test-password".to_owned()),
            Zeroizing::new("one two three".to_owned()),
            123,
        )
        .await
        .unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn backup_api_requests_only_the_mnemonic_key_type() {
    let (listener, address) = listener().await;
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let (_, request) = read_request(&mut stream).await;
        assert_eq!(request["method"], "query_key");
        assert_eq!(
            request["params"],
            serde_json::json!({ "key_type": "mnemonic" })
        );
        let response = serde_json::json!({
            "jsonrpc": "2.0", "id": request["id"], "result": { "key": "one two three" }
        });
        reply(&mut stream, "200 OK", "", &response.to_string()).await;
    });
    let wallet = WalletRpcClient::new(
        address,
        RpcCredentials::new("u".into(), "p".into()).unwrap(),
    )
    .unwrap();
    let phrase = wallet.query_mnemonic().await.unwrap();
    assert_eq!(phrase.into_secret().as_str(), "one two three");
    server.await.unwrap();
}
