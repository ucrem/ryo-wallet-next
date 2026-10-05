//! Explicit allowlist: never serialize settings, wallet data, raw errors or logs.
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use ryo_wallet_service::application::{
    LifecycleState, NodeService, NodeState, NodeStatus, WalletService,
};
use ryo_wallet_service::domain::{Network, NodeConfig, NodeMode};
use ryo_wallet_service::storage::{load_settings_if_present, secure_file};
use serde::Serialize;
use tauri::Manager;
use tauri_plugin_dialog::DialogExt;

use super::{DataRootState, selected_paths};

#[derive(Serialize)]
struct Report {
    schema_version: u8,
    app_version: &'static str,
    os: &'static str,
    architecture: &'static str,
    collected_at_unix_seconds: u64,
    network: Option<Network>,
    node_mode: Option<NodeMode>,
    wallet_state: Option<LifecycleState>,
    wallet_height: Option<String>,
    wallet_height_source: &'static str,
    wallet_rpc_busy: bool,
    node: Option<NodeReport>,
    observations: Vec<&'static str>,
}

#[derive(Serialize)]
struct NodeReport {
    state: NodeState,
    chain_height: Option<String>,
    network_height: Option<String>,
    reachable: bool,
    ready: bool,
    offline: bool,
    untrusted: bool,
}

// Decimal-only conversion also prevents malformed upstream heights becoming text exports.
fn height(value: Option<&str>) -> Option<String> {
    value
        .and_then(|value| value.parse::<u64>().ok())
        .map(|value| value.to_string())
}

impl Report {
    fn new(node: Option<&NodeConfig>) -> Self {
        Self {
            schema_version: 1,
            app_version: env!("CARGO_PKG_VERSION"),
            os: std::env::consts::OS,
            architecture: std::env::consts::ARCH,
            collected_at_unix_seconds: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            network: node.map(|node| node.network),
            node_mode: node.map(|node| node.mode),
            wallet_state: None,
            wallet_height: None,
            wallet_height_source: "unavailable",
            wallet_rpc_busy: false,
            node: None,
            observations: Vec::new(),
        }
    }

    fn set_node(&mut self, status: &NodeStatus) {
        self.node = Some(NodeReport {
            state: status.state,
            chain_height: height(status.rpc_height.as_deref().or(status.height.as_deref())),
            network_height: height(status.target_height.as_deref()),
            reachable: status.reachable,
            ready: status.ready,
            offline: status.offline,
            untrusted: status.untrusted,
        });
        if status.state == NodeState::Running && !status.reachable {
            self.observations.push("node_unreachable");
        }
    }
}

async fn collect(
    service: &WalletService,
    nodes: &NodeService,
    node: Option<&NodeConfig>,
) -> Report {
    let mut report = Report::new(node);
    let (lifecycle, health) = tokio::join!(
        tokio::time::timeout(Duration::from_secs(2), service.status()),
        async {
            match node {
                Some(node) => {
                    Some(tokio::time::timeout(Duration::from_secs(6), nodes.status(node)).await)
                }
                None => None,
            }
        }
    );
    match lifecycle {
        Ok(Ok(status)) => report.wallet_state = Some(status.state),
        _ => report.observations.push("wallet_status_unavailable"),
    }
    match health {
        Some(Ok(Ok(status))) => report.set_node(&status),
        Some(Err(_)) => report.observations.push("node_status_timeout"),
        Some(Ok(Err(_))) => report.observations.push("node_status_unavailable"),
        None => report.observations.push("node_not_configured"),
    }
    // Cached scan progress is read without queueing behind upstream refresh.
    report.wallet_height = service.scan_height().map(|value| value.to_string());
    if report.wallet_height.is_some() {
        report.wallet_height_source = "scan";
    }
    report.wallet_rpc_busy = service.read_is_busy();
    if report.wallet_rpc_busy {
        report.observations.push("wallet_rpc_busy");
    }
    report
}

fn save(
    report: &Report,
    destination: Option<&Path>,
    protected: &[PathBuf],
) -> Result<bool, &'static str> {
    let Some(destination) = destination else {
        return Ok(false);
    };
    if !destination.is_absolute()
        || destination.extension().and_then(|value| value.to_str()) != Some("json")
    {
        return Err("choose a new JSON file outside application storage");
    }
    let parent = destination
        .parent()
        .and_then(|path| fs::canonicalize(path).ok())
        .ok_or("destination folder is unavailable")?;
    for directory in protected {
        if fs::canonicalize(directory).is_ok_and(|directory| parent.starts_with(directory)) {
            return Err("choose a new JSON file outside application storage");
        }
    }
    let mut temporary = tempfile::NamedTempFile::new_in(&parent)
        .map_err(|_| "diagnostic file could not be saved")?;
    secure_file(temporary.path()).map_err(|_| "diagnostic file could not be secured")?;
    serde_json::to_writer_pretty(&mut temporary, report)
        .map_err(|_| "diagnostic file could not be saved")?;
    temporary
        .write_all(b"\n")
        .and_then(|_| temporary.flush())
        .and_then(|_| temporary.as_file().sync_all())
        .map_err(|_| "diagnostic file could not be saved")?;
    temporary
        .persist_noclobber(destination)
        .map_err(|_| "choose a new filename; existing files are not overwritten")?;
    Ok(true)
}

#[tauri::command]
pub async fn app_export_diagnostics(app: tauri::AppHandle) -> Result<bool, &'static str> {
    let picker_app = app.clone();
    let destination = tauri::async_runtime::spawn_blocking(move || {
        picker_app
            .dialog()
            .file()
            .add_filter("Diagnostic report", &["json"])
            .set_file_name("ryo-wallet-next-diagnostics.json")
            .blocking_save_file()
            .map(|file| {
                file.into_path()
                    .map_err(|_| "destination path is unavailable")
            })
            .transpose()
    })
    .await
    .map_err(|_| "file picker is unavailable")??;
    if destination.is_none() {
        return Ok(false);
    }
    let state = app.state::<DataRootState>();
    let paths = selected_paths(&state).ok();
    let settings = paths.as_ref().map(load_settings_if_present).transpose();
    let configuration_failed = settings.is_err();
    let settings = settings.ok().flatten().flatten();
    let service = app.state::<WalletService>();
    let nodes = app.state::<NodeService>();
    let mut report = collect(
        &service,
        &nodes,
        settings.as_ref().map(|settings| &settings.node),
    )
    .await;
    if configuration_failed {
        report.observations.push("configuration_unavailable");
    }
    let mut protected = Vec::new();
    if let Ok(config) = app.path().app_config_dir() {
        protected.push(config);
    }
    if let Some(paths) = paths {
        for network in [Network::Mainnet, Network::Testnet, Network::Stagenet] {
            if let Ok(paths) =
                ryo_wallet_service::storage::AppPaths::new(paths.root().to_owned(), network)
            {
                protected.push(paths.network_root());
            }
        }
    }
    tauri::async_runtime::spawn_blocking(move || save(&report, destination.as_deref(), &protected))
        .await
        .map_err(|_| "diagnostic file could not be saved")?
}

#[cfg(test)]
mod tests {
    use super::*;
    use ryo_wallet_service::domain::NodeTrust;

    #[test]
    fn export_allowlist_excludes_endpoints_generations_and_malformed_heights() {
        let node = NodeConfig {
            host: "private-endpoint-canary".into(),
            port: 12211,
            mode: NodeMode::Remote,
            network: Network::Mainnet,
            trust: NodeTrust::ExplicitRemote,
            bootstrap: None,
            advanced: None,
        };
        let mut report = Report::new(Some(&node));
        report.set_node(&NodeStatus {
            mode: NodeMode::Remote,
            state: NodeState::Running,
            generation: "session-canary".into(),
            height: Some("unsafe-text-canary".into()),
            rpc_height: None,
            target_height: Some("1197000".into()),
            reachable: false,
            ready: false,
            offline: true,
            untrusted: true,
        });
        let json = serde_json::to_string(&report).unwrap();
        for forbidden in [
            "private-endpoint-canary",
            "session-canary",
            "unsafe-text-canary",
            "password",
            "seed",
            "address",
            "host",
            "path",
            "credentials",
            "wallet_id",
        ] {
            assert!(!json.contains(forbidden), "export contained {forbidden}");
        }
        assert_eq!(
            report.node.unwrap().network_height.as_deref(),
            Some("1197000")
        );
    }

    #[test]
    fn cancelled_export_writes_nothing_and_exports_cannot_replace_wallets_or_existing_files() {
        let temp = tempfile::tempdir().unwrap();
        let report = Report::new(None);
        assert!(!save(&report, None, &[]).unwrap());
        assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 0);
        let destination = temp.path().join("report.json");
        assert!(save(&report, Some(&destination), &[temp.path().into()]).is_err());
        assert!(!destination.exists());
        assert!(save(&report, Some(&destination), &[]).unwrap());
        let bytes = fs::read(&destination).unwrap();
        assert!(save(&report, Some(&destination), &[]).is_err());
        assert_eq!(fs::read(destination).unwrap(), bytes);
        assert!(save(&report, Some(&temp.path().join("wallet.keys")), &[]).is_err());
        assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 1);
    }

    #[tokio::test]
    async fn locked_unconfigured_report_preserves_wallet_and_node_lifecycles() {
        let service = WalletService::new();
        let nodes = NodeService::new();
        let before = service.status().await.unwrap();
        let report = collect(&service, &nodes, None).await;
        assert_eq!(report.wallet_state, Some(before.state));
        assert_eq!(service.status().await.unwrap(), before);
        assert!(nodes.is_idle().await.unwrap());
        assert!(report.observations.contains(&"node_not_configured"));
    }
}
