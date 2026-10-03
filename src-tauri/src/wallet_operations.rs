use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use ryo_wallet_service::application::{
    NodeService, WalletOperation, WalletOperationOutput, WalletService, WalletServiceError,
};
use ryo_wallet_service::process::upstream_path;
use tauri::Manager;
use tauri_plugin_dialog::DialogExt;
use zeroize::Zeroizing;

use super::{
    ActiveWalletState, DataRootState, SyncMonitorState, require_completed_backup, selected_paths,
};

fn operation_error(error: WalletServiceError) -> String {
    match error {
        WalletServiceError::StaleSession => "wallet session changed; reopen the wallet page".into(),
        WalletServiceError::Operation(error) => error.to_string(),
        _ => "wallet operation unavailable; check that the wallet is open".into(),
    }
}

#[tauri::command]
pub async fn wallet_operation(
    session_generation: String,
    operation: WalletOperation,
    state: tauri::State<'_, DataRootState>,
    active: tauri::State<'_, ActiveWalletState>,
    service: tauri::State<'_, WalletService>,
) -> Result<WalletOperationOutput, String> {
    require_completed_backup(&state, &active).map_err(str::to_owned)?;
    // Files are selected and validated by the host, never supplied by the renderer.
    if matches!(
        operation,
        WalletOperation::ExportKeyImages { .. }
            | WalletOperation::ImportKeyImages { .. }
            | WalletOperation::Authenticate { .. }
    ) {
        return Err("use the native file action".into());
    }
    service
        .operation(session_generation, operation)
        .await
        .map_err(operation_error)
}

async fn pick_file(
    app: tauri::AppHandle,
    save: bool,
    name: &'static str,
    extension: &'static str,
) -> Result<Option<PathBuf>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let picker = app.dialog().file().add_filter(name, &[extension]);
        let picked = if save {
            picker.set_file_name(name).blocking_save_file()
        } else {
            picker.blocking_pick_file()
        };
        picked
            .map(|p| {
                p.into_path()
                    .map_err(|_| "file path is unavailable".to_owned())
            })
            .transpose()
    })
    .await
    .map_err(|_| "file picker is unavailable".to_owned())?
}
fn external_destination(path: &Path, state: &DataRootState) -> Result<(), String> {
    let paths = selected_paths(state).map_err(str::to_owned)?;
    let parent = path
        .parent()
        .and_then(|p| fs::canonicalize(p).ok())
        .ok_or("destination folder is unavailable")?;
    let network =
        fs::canonicalize(paths.network_root()).map_err(|_| "wallet data folder is unavailable")?;
    if !path.is_absolute()
        || parent.starts_with(network)
        || fs::symlink_metadata(path).is_ok_and(|m| !m.is_file() || m.file_type().is_symlink())
    {
        return Err("choose an ordinary file outside the active wallet data folder".into());
    }
    Ok(())
}

#[tauri::command]
pub async fn wallet_key_images(
    session_generation: String,
    import: bool,
    password: String,
    app: tauri::AppHandle,
    state: tauri::State<'_, DataRootState>,
    active: tauri::State<'_, ActiveWalletState>,
    service: tauri::State<'_, WalletService>,
) -> Result<Option<WalletOperationOutput>, String> {
    let password = Zeroizing::new(password);
    require_completed_backup(&state, &active).map_err(str::to_owned)?;
    let Some(path) = pick_file(app, !import, "key-images.ryoki", "ryoki").await? else {
        return Ok(None);
    };
    let paths = selected_paths(&state).map_err(str::to_owned)?;
    // Use a fresh private working file; upstream never receives an arbitrary write destination.
    let temporary = paths.runtime_root().join(format!(
        "key-images-{}.ryoki",
        uuid::Uuid::new_v4().simple()
    ));
    if import {
        let metadata = fs::symlink_metadata(&path).map_err(|_| "key image file is unavailable")?;
        if !metadata.is_file()
            || metadata.file_type().is_symlink()
            || metadata.len() > 16 * 1024 * 1024
        {
            return Err("invalid or oversized key image file".into());
        }
        let mut source = fs::File::open(&path).map_err(|_| "key image file is unavailable")?;
        let mut target = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|_| "private temporary file is unavailable")?;
        std::io::copy(&mut source, &mut target)
            .map_err(|_| "key image import could not be staged")?;
    } else {
        external_destination(&path, &state)?;
    }
    let filename = upstream_path(&temporary)
        .map_err(|_| "unsupported runtime file path")?
        .to_string_lossy()
        .into_owned();
    let operation = if import {
        WalletOperation::ImportKeyImages { filename }
    } else {
        WalletOperation::ExportKeyImages { filename, password }
    };
    let result = service
        .operation(session_generation, operation)
        .await
        .map_err(operation_error);
    let result = match result {
        Ok(output) if !import => fs::read(&temporary)
            .and_then(|bytes| fs::write(&path, bytes))
            .map(|_| Some(output))
            .map_err(|_| "key image export could not be saved".into()),
        Ok(output) => Ok(Some(output)),
        Err(error) => Err(error),
    };
    let _ = fs::remove_file(&temporary);
    result
}

#[tauri::command]
pub async fn wallet_export_artwork(
    size: u16,
    cells: Vec<[u16; 2]>,
    color: String,
    app: tauri::AppHandle,
    state: tauri::State<'_, DataRootState>,
) -> Result<bool, String> {
    if !(5..=200).contains(&size)
        || cells.len() > 40000
        || cells.iter().any(|[x, y]| *x >= size || *y >= size)
        || color.len() != 7
        || !color.starts_with('#')
        || !color[1..].bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err("invalid image".into());
    }
    let Some(path) = pick_file(app, true, "ryo-address.svg", "svg").await? else {
        return Ok(false);
    };
    external_destination(&path, &state)?;
    let mut svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {size} {size}\" width=\"512\" height=\"512\" shape-rendering=\"crispEdges\"><rect width=\"100%\" height=\"100%\" fill=\"white\"/><g fill=\"{color}\">"
    );
    for [x, y] in cells {
        svg.push_str(&format!(
            "<rect x=\"{x}\" y=\"{y}\" width=\"1\" height=\"1\"/>"
        ));
    }
    svg.push_str("</g></svg>");
    fs::write(path, svg).map_err(|_| "image could not be saved")?;
    Ok(true)
}

#[tauri::command]
pub async fn wallet_remove(
    session_generation: String,
    password: String,
    confirmation: String,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let state = app.state::<DataRootState>();
    let active = app.state::<ActiveWalletState>();
    let service = app.state::<WalletService>();
    let monitor = app.state::<SyncMonitorState>();
    let nodes = app.state::<NodeService>();
    let setup = app.state::<super::SetupState>();
    let _guard = setup.0.lock().await;
    let password = Zeroizing::new(password);
    if confirmation != "DELETE" {
        return Err("type DELETE to confirm removal".into());
    }
    require_completed_backup(&state, &active).map_err(str::to_owned)?;
    let paths = selected_paths(&state).map_err(str::to_owned)?;
    let id = active
        .0
        .lock()
        .map_err(|_| "wallet state unavailable")?
        .clone()
        .ok_or("open a wallet first")?;
    service
        .operation(
            session_generation.clone(),
            WalletOperation::Authenticate { password },
        )
        .await
        .map_err(operation_error)?;
    // Removal is recoverable: preserve encrypted files outside the active wallet list.
    let source =
        fs::canonicalize(paths.wallet_dir(&id)).map_err(|_| "wallet folder unavailable")?;
    let root = fs::canonicalize(paths.wallets_root()).map_err(|_| "wallet folder unavailable")?;
    if source.parent() != Some(root.as_path()) {
        return Err("invalid wallet location".into());
    }
    let archive = paths.network_root().join("removed-wallets");
    fs::create_dir_all(&archive).map_err(|_| "wallet archive unavailable")?;
    let archive = fs::canonicalize(archive).map_err(|_| "wallet archive unavailable")?;
    if archive.parent() != root.parent() {
        return Err("invalid wallet archive location".into());
    }
    let destination = archive.join(format!("{}-{}", id, uuid::Uuid::new_v4().simple()));
    service
        .lock_current(Duration::from_secs(5), session_generation)
        .await
        .map_err(operation_error)?;
    super::stop_wallet_sync_monitor(&monitor);
    *active.0.lock().map_err(|_| "wallet state unavailable")? = None;
    super::start_wallet_sync_monitor_for_current_node(
        app.clone(),
        &service,
        &nodes,
        &state,
        &monitor,
    );
    fs::rename(source, destination).map_err(|_| {
        "wallet was locked but could not be moved; reopen it from the wallet list".into()
    })
}
