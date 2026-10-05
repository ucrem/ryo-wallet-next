//! Read-only support facts. Never collect user/device identifiers or raw API errors.
use std::path::Path;

use serde::Serialize;

#[cfg(any(target_os = "linux", test))]
mod linux;
#[cfg(any(target_os = "macos", test))]
mod macos;
#[cfg(any(target_os = "linux", target_os = "macos"))]
mod unix;

#[derive(Serialize)]
pub(super) struct EnvironmentReport {
    os_version: Option<OsVersion>,
    webview_version: Option<String>,
    available_parallelism: Option<usize>,
    physical_memory: Option<MemoryReport>,
    data_volume: Option<VolumeReport>,
    ryo_runtime: RuntimeReport,
}

#[derive(Serialize)]
#[serde(tag = "platform", rename_all = "snake_case")]
enum OsVersion {
    #[cfg(any(windows, test))]
    Windows {
        major: u32,
        minor: u32,
        build: u32,
        update_build_revision: Option<u32>,
    },
    #[cfg(any(target_os = "macos", test))]
    Macos {
        product_version: Option<String>,
        kernel_version: Option<String>,
        build: Option<String>,
    },
    #[cfg(any(target_os = "linux", test))]
    Linux {
        distribution: Option<&'static str>,
        distribution_version: Option<String>,
        kernel_version: Option<String>,
    },
}

#[derive(Serialize)]
struct MemoryReport {
    total_bytes: String,
    available_bytes: String,
    availability_source: &'static str,
}

#[derive(Serialize)]
struct VolumeReport {
    filesystem: Option<&'static str>,
    supports_persistent_acls: Option<bool>,
    read_only: Option<bool>,
    total_bytes: Option<String>,
    free_bytes_available_to_user: Option<String>,
    space_source: &'static str,
}

#[derive(Serialize)]
struct RuntimeReport {
    manifest_version: Option<String>,
    wallet_rpc_verified: bool,
    daemon_verified: bool,
}

// Versions come from an API/manifest, not raw upstream text or a registry label.
fn numeric_version(value: &str) -> Option<String> {
    (!value.is_empty()
        && value.len() <= 64
        && value.split('.').all(|component| {
            !component.is_empty() && component.bytes().all(|byte| byte.is_ascii_digit())
        }))
    .then(|| value.to_owned())
}

#[cfg(any(target_os = "linux", target_os = "macos", test))]
fn byte_count(blocks: u128, block_size: u128) -> Option<String> {
    if block_size == 0 {
        return None;
    }
    u64::try_from(blocks.checked_mul(block_size)?)
        .ok()
        .map(|value| value.to_string())
}

#[cfg(any(target_os = "linux", target_os = "macos", test))]
fn kernel_release(value: &str) -> Option<String> {
    let mut components = value.split(['-', '+']);
    let base = numeric_version(components.next()?)?;
    let revision = components.next().and_then(numeric_version);
    // Keep a numeric distribution ABI/package revision (e.g. 6.8.0-60), but
    // omit kernel flavor names and custom build suffixes.
    Some(match revision {
        Some(revision) => format!("{base}-{revision}"),
        None => base,
    })
}

#[cfg(any(windows, target_os = "macos", test))]
fn filesystem_name(value: &str) -> &'static str {
    match value {
        "NTFS" => "NTFS",
        "ReFS" => "ReFS",
        "FAT" => "FAT",
        "FAT32" => "FAT32",
        "exFAT" => "exFAT",
        "UDF" => "UDF",
        "CDFS" => "CDFS",
        "apfs" => "APFS",
        "hfs" => "HFS",
        "msdos" => "FAT",
        "exfat" => "exFAT",
        "ntfs" => "NTFS",
        "udf" => "UDF",
        "cd9660" => "CDFS",
        _ => "other",
    }
}

pub(super) fn collect(data_root: Option<&Path>) -> EnvironmentReport {
    let manifest: Option<serde_json::Value> =
        serde_json::from_str(include_str!("../../runtime-manifest.json")).ok();
    EnvironmentReport {
        os_version: os_version(),
        webview_version: tauri::webview_version()
            .ok()
            .and_then(|value| numeric_version(&value)),
        available_parallelism: std::thread::available_parallelism()
            .ok()
            .map(|value| value.get()),
        physical_memory: physical_memory(),
        data_volume: data_root.and_then(data_volume),
        ryo_runtime: RuntimeReport {
            manifest_version: manifest
                .as_ref()
                .and_then(|value| value.get("version"))
                .and_then(|value| value.as_str())
                .and_then(numeric_version),
            wallet_rpc_verified: crate::wallet_runtime::reviewed_wallet_rpc().is_some(),
            daemon_verified: crate::wallet_runtime::reviewed_daemon().is_some(),
        },
    }
}

#[cfg(target_os = "linux")]
use linux::{data_volume, os_version, physical_memory};
#[cfg(target_os = "macos")]
use macos::{data_volume, os_version, physical_memory};

#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
fn os_version() -> Option<OsVersion> {
    None
}
#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
fn physical_memory() -> Option<MemoryReport> {
    None
}
#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
fn data_volume(_: &Path) -> Option<VolumeReport> {
    None
}

#[cfg(windows)]
fn os_version() -> Option<OsVersion> {
    use windows_sys::Wdk::System::SystemServices::RtlGetVersion;
    use windows_sys::Win32::System::Registry::{
        HKEY_LOCAL_MACHINE, RRF_RT_REG_DWORD, RegGetValueW,
    };
    use windows_sys::Win32::System::SystemInformation::OSVERSIONINFOW;
    let mut version = OSVERSIONINFOW {
        dwOSVersionInfoSize: size_of::<OSVERSIONINFOW>() as u32,
        ..Default::default()
    };
    // A correctly sized, writable version structure; no CSD/product text is serialized.
    if unsafe { RtlGetVersion(&mut version) } != 0 {
        return None;
    }
    let key: Vec<u16> = "SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\0"
        .encode_utf16()
        .collect();
    let value: Vec<u16> = "UBR\0".encode_utf16().collect();
    let mut revision = 0u32;
    let mut size = size_of::<u32>() as u32;
    // Only a fixed numeric OS patch value, never registry/product/device identifiers.
    let result = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            key.as_ptr(),
            value.as_ptr(),
            RRF_RT_REG_DWORD,
            std::ptr::null_mut(),
            (&mut revision as *mut u32).cast(),
            &mut size,
        )
    };
    Some(OsVersion::Windows {
        major: version.dwMajorVersion,
        minor: version.dwMinorVersion,
        build: version.dwBuildNumber,
        update_build_revision: (result == 0 && size == size_of::<u32>() as u32).then_some(revision),
    })
}

#[cfg(windows)]
fn physical_memory() -> Option<MemoryReport> {
    use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
    let mut memory = MEMORYSTATUSEX {
        dwLength: size_of::<MEMORYSTATUSEX>() as u32,
        ..Default::default()
    };
    // Caller-owned, correctly sized output buffer; values are volatile support facts.
    (unsafe { GlobalMemoryStatusEx(&mut memory) } != 0).then(|| MemoryReport {
        total_bytes: memory.ullTotalPhys.to_string(),
        available_bytes: memory.ullAvailPhys.to_string(),
        availability_source: "windows_physical_available",
    })
}

#[cfg(windows)]
fn data_volume(root: &Path) -> Option<VolumeReport> {
    use std::os::windows::ffi::OsStrExt;
    use std::path::{Component, Prefix};
    use windows_sys::Win32::Storage::FileSystem::{
        GetDiskFreeSpaceExW, GetDriveTypeW, GetVolumeInformationW, GetVolumePathNameW,
    };
    use windows_sys::Win32::System::SystemServices::{FILE_PERSISTENT_ACLS, FILE_READ_ONLY_VOLUME};
    if !root.is_absolute() {
        return None;
    }
    let mut wide: Vec<u16> = root.as_os_str().encode_wide().collect();
    if wide.is_empty() || wide.len() >= 32768 || wide.contains(&0) {
        return None;
    }
    // Only local drive paths; reject UNC/device namespaces before any OS probe.
    let Component::Prefix(prefix) = root.components().next()? else {
        return None;
    };
    let letter = match prefix.kind() {
        Prefix::Disk(letter) | Prefix::VerbatimDisk(letter) => letter,
        _ => return None,
    };
    let drive = [u16::from(letter), u16::from(b':'), u16::from(b'\\'), 0];
    // Removable/fixed/optical/RAM disk. In particular, skip mapped remote drives (4)
    // before resolving volume metadata or free space.
    if !matches!(unsafe { GetDriveTypeW(drive.as_ptr()) }, 2 | 3 | 5 | 6) {
        return None;
    }
    wide.push(0);
    let mut volume = vec![0u16; 32768];
    // Valid input string and bounded caller-owned output buffer.
    if unsafe { GetVolumePathNameW(wide.as_ptr(), volume.as_mut_ptr(), volume.len() as u32) } == 0 {
        return None;
    }
    // DRIVE_REMOTE (4) also detects a mapped network drive. Skip before querying it.
    if unsafe { GetDriveTypeW(volume.as_ptr()) } == 4 {
        return None;
    }
    let mut filesystem = [0u16; 32];
    let mut flags = 0u32;
    // Volume labels and serial numbers are deliberately not requested.
    let volume_ok = unsafe {
        GetVolumeInformationW(
            volume.as_ptr(),
            std::ptr::null_mut(),
            0,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut flags,
            filesystem.as_mut_ptr(),
            filesystem.len() as u32,
        )
    } != 0;
    let filesystem_end = filesystem
        .iter()
        .position(|value| *value == 0)
        .unwrap_or(filesystem.len());
    let mut free = 0u64;
    let mut total = 0u64;
    // Space available to this user, accounting for quotas. No files are opened or written.
    let space_ok =
        unsafe { GetDiskFreeSpaceExW(wide.as_ptr(), &mut free, &mut total, std::ptr::null_mut()) }
            != 0;
    Some(VolumeReport {
        filesystem: volume_ok
            .then(|| filesystem_name(&String::from_utf16_lossy(&filesystem[..filesystem_end]))),
        supports_persistent_acls: volume_ok.then_some(flags & FILE_PERSISTENT_ACLS != 0),
        read_only: volume_ok.then_some(flags & FILE_READ_ONLY_VOLUME != 0),
        total_bytes: space_ok.then(|| total.to_string()),
        free_bytes_available_to_user: space_ok.then(|| free.to_string()),
        space_source: "windows_user_available_capacity",
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_text_is_bounded_and_cannot_export_identifiers() {
        assert_eq!(
            numeric_version("148.0.1234.5").as_deref(),
            Some("148.0.1234.5")
        );
        for value in [
            "",
            "1..2",
            "1.2-private-device-canary",
            "C:\\Users\\private-user-canary",
            &"1".repeat(65),
        ] {
            assert!(numeric_version(value).is_none());
        }
        assert_eq!(filesystem_name("private-volume-canary"), "other");
        assert_eq!(filesystem_name("apfs"), "APFS");
        assert_eq!(byte_count(2, 4096).as_deref(), Some("8192"));
        assert!(byte_count(u128::MAX, 4096).is_none());
        assert!(byte_count(u64::MAX.into(), 2).is_none());
        assert!(byte_count(1, 0).is_none());
        assert_eq!(
            kernel_release("6.8.0-60-generic").as_deref(),
            Some("6.8.0-60")
        );
        assert_eq!(
            kernel_release("6.17.0-private-machine-canary").as_deref(),
            Some("6.17.0")
        );
        assert_eq!(kernel_release("24.6.0").as_deref(), Some("24.6.0"));
        assert!(kernel_release("private-host-canary").is_none());
        let version = OsVersion::Windows {
            major: 10,
            minor: 0,
            build: 26100,
            update_build_revision: Some(1),
        };
        assert_eq!(
            serde_json::to_value(version).unwrap()["platform"],
            "windows"
        );
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn native_unix_version_memory_and_local_disk_are_populated_without_identifiers_or_writes() {
        let temp = tempfile::Builder::new()
            .prefix("private-path-canary")
            .tempdir()
            .unwrap();
        let version = os_version().expect("OS version unavailable");
        let memory = physical_memory().expect("Physical memory unavailable");
        let total: u64 = memory.total_bytes.parse().unwrap();
        assert!(total > 0 && memory.available_bytes.parse::<u64>().unwrap() <= total);
        let volume = data_volume(temp.path()).expect("Local volume unavailable");
        assert!(volume.filesystem.is_some());
        assert!(volume.total_bytes.as_ref().unwrap().parse::<u64>().unwrap() > 0);
        assert!(
            volume
                .free_bytes_available_to_user
                .as_ref()
                .unwrap()
                .parse::<u64>()
                .is_ok()
        );
        assert_eq!(volume.supports_persistent_acls, None);
        let json = serde_json::json!({ "version": version, "memory": memory, "volume": volume });
        assert_eq!(json["version"]["platform"], std::env::consts::OS);
        assert!(json["version"]["kernel_version"].as_str().is_some());
        #[cfg(target_os = "macos")]
        {
            assert!(json["version"]["product_version"].as_str().is_some());
            assert!(json["version"]["build"].as_str().is_some());
        }
        #[cfg(target_os = "linux")]
        assert!(json["version"]["distribution"].as_str().is_some());
        let json = json.to_string();
        assert!(!json.contains(&temp.path().to_string_lossy().to_string()));
        for key in [
            "canary", "serial", "label", "username", "path", "machine", "mount", "hostname",
        ] {
            assert!(!json.contains(key));
        }
        assert_eq!(std::fs::read_dir(temp.path()).unwrap().count(), 0);
    }

    #[cfg(windows)]
    #[test]
    fn native_windows_facts_and_local_space_are_read_only_and_redacted() {
        let temp = tempfile::tempdir().unwrap();
        let OsVersion::Windows { major, build, .. } =
            os_version().expect("Windows version unavailable")
        else {
            panic!("unexpected OS version");
        };
        assert!(major >= 6 && build > 0);
        let memory = physical_memory().expect("Windows physical memory unavailable");
        let total: u64 = memory.total_bytes.parse().unwrap();
        assert!(total > 0 && memory.available_bytes.parse::<u64>().unwrap() <= total);
        let volume = data_volume(temp.path()).expect("Local volume unavailable");
        assert!(volume.filesystem.is_some());
        assert!(volume.total_bytes.as_ref().unwrap().parse::<u64>().unwrap() > 0);
        assert_eq!(std::fs::read_dir(temp.path()).unwrap().count(), 0);
        let json = serde_json::to_string(&volume).unwrap();
        assert!(!json.contains(&temp.path().to_string_lossy().to_string()));
        for key in ["serial", "label", "username", "path", "machine"] {
            assert!(!json.contains(key));
        }
    }

    #[cfg(windows)]
    #[test]
    fn remote_relative_and_embedded_nul_storage_is_not_probed() {
        for value in [
            "relative",
            "\\\\server-canary\\share",
            "\\\\?\\UNC\\server-canary\\share",
            "C:\\nul\0canary",
        ] {
            assert!(data_volume(Path::new(value)).is_none());
        }
    }
}
