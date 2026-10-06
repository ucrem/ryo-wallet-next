//! Bounded Linux system facts; os-release is parsed as data, never sourced.
use super::{MemoryReport, numeric_version};
#[cfg(target_os = "linux")]
use super::{OsVersion, VolumeReport, unix};

fn distribution(value: &str) -> &'static str {
    match value {
        "ubuntu" => "ubuntu",
        "debian" => "debian",
        "fedora" => "fedora",
        "rhel" => "rhel",
        "centos" => "centos",
        "rocky" => "rocky",
        "almalinux" => "almalinux",
        "arch" => "arch",
        "manjaro" => "manjaro",
        "linuxmint" => "linuxmint",
        "pop" => "pop",
        "elementary" => "elementary",
        "opensuse-leap" => "opensuse-leap",
        "opensuse-tumbleweed" => "opensuse-tumbleweed",
        "sles" => "sles",
        "nixos" => "nixos",
        "gentoo" => "gentoo",
        "alpine" => "alpine",
        "void" => "void",
        "kali" => "kali",
        "raspbian" => "raspbian",
        _ => "other",
    }
}

fn release(text: &str) -> (Option<&'static str>, Option<String>) {
    if text.len() > 16384 {
        return (None, None);
    }
    let mut id = None;
    let mut version = None;
    for line in text.lines() {
        let Some((key, value)) = line.trim().split_once('=') else {
            continue;
        };
        // Recognize only simple quoted/unquoted scalars. Do not expand escapes,
        // shell substitutions, PRETTY_NAME, URLs, vendor or custom image fields.
        let value = value.trim();
        let value = value
            .strip_prefix('"')
            .and_then(|value| value.strip_suffix('"'))
            .or_else(|| {
                value
                    .strip_prefix('\'')
                    .and_then(|value| value.strip_suffix('\''))
            })
            .unwrap_or(value);
        match key {
            "ID" => id = Some(distribution(value)),
            "VERSION_ID" => version = numeric_version(value),
            _ => {}
        }
    }
    (id, version)
}

fn memory(text: &str) -> Option<MemoryReport> {
    if text.len() > 16384 {
        return None;
    }
    let mut total = None;
    let mut available = None;
    for line in text.lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let target = match key {
            "MemTotal" => &mut total,
            "MemAvailable" => &mut available,
            _ => continue,
        };
        let mut fields = value.split_whitespace();
        let number = fields.next()?.parse::<u64>().ok()?;
        if fields.next() != Some("kB") || fields.next().is_some() {
            return None;
        }
        *target = Some(number.checked_mul(1024)?);
    }
    let (total, available) = (total?, available?);
    if total == 0 || available > total {
        return None;
    }
    Some(MemoryReport {
        total_bytes: total.to_string(),
        available_bytes: available.to_string(),
        availability_source: "linux_mem_available",
    })
}

fn local_filesystem(magic: i128) -> Option<&'static str> {
    // Kernel magic numbers, not mount names. Unknown, network and FUSE filesystems
    // are omitted; FUSE can wrap network storage, so do not assume it is local.
    match magic {
        0xef53 => Some("ext"),
        0x9123_683e => Some("Btrfs"),
        0x5846_5342 => Some("XFS"),
        0x2fc1_2fc1 => Some("ZFS"),
        0xf2f5_2010 => Some("F2FS"),
        0x0102_1994 => Some("tmpfs"),
        0x8584_58f6 => Some("ramfs"),
        0x794c_7630 => Some("overlay"),
        0x4d44 => Some("FAT"),
        0x2011_bab0 => Some("exFAT"),
        0x5346_544e => Some("NTFS"),
        0x9660 => Some("CDFS"),
        0x1501_3346 => Some("UDF"),
        0x7371_7368 => Some("SquashFS"),
        0x5265_4973 => Some("ReiserFS"),
        0x3153_464a => Some("JFS"),
        _ => None,
    }
}

#[cfg(target_os = "linux")]
fn bounded_system_file(path: &str) -> Option<String> {
    use std::io::Read;
    let file = std::fs::File::open(path).ok()?;
    let mut contents = String::new();
    file.take(16385).read_to_string(&mut contents).ok()?;
    (contents.len() <= 16384).then_some(contents)
}

#[cfg(target_os = "linux")]
pub(super) fn os_version() -> Option<OsVersion> {
    let text = bounded_system_file("/etc/os-release")
        .or_else(|| bounded_system_file("/usr/lib/os-release"));
    let (distribution, distribution_version) = text.as_deref().map(release).unwrap_or_default();
    let kernel_version = unix::kernel_version();
    (distribution.is_some() || distribution_version.is_some() || kernel_version.is_some())
        .then_some(OsVersion::Linux {
            distribution,
            distribution_version,
            kernel_version,
        })
}

#[cfg(target_os = "linux")]
pub(super) fn physical_memory() -> Option<MemoryReport> {
    memory(&bounded_system_file("/proc/meminfo")?)
}

#[cfg(target_os = "linux")]
pub(super) fn data_volume(root: &std::path::Path) -> Option<VolumeReport> {
    use std::os::fd::AsRawFd;
    let directory = unix::directory(root)?;
    let mut filesystem = std::mem::MaybeUninit::<libc::statfs>::uninit();
    // Query only the selected directory, never enumerate mounts or wallet files.
    if unsafe { libc::fstatfs(directory.as_raw_fd(), filesystem.as_mut_ptr()) } != 0 {
        return None;
    }
    let filesystem = unsafe { filesystem.assume_init() };
    let category = local_filesystem(filesystem.f_type.into())?;
    let mut space = std::mem::MaybeUninit::<libc::statvfs>::uninit();
    // The same open directory handle prevents switching mounts between probes.
    if unsafe { libc::fstatvfs(directory.as_raw_fd(), space.as_mut_ptr()) } != 0 {
        return None;
    }
    let space = unsafe { space.assume_init() };
    Some(VolumeReport {
        filesystem: Some(category),
        supports_persistent_acls: None,
        read_only: Some(space.f_flag & libc::ST_RDONLY != 0),
        total_bytes: super::byte_count(space.f_blocks.into(), space.f_frsize.into()),
        free_bytes_available_to_user: super::byte_count(
            space.f_bavail.into(),
            space.f_frsize.into(),
        ),
        space_source: "linux_statvfs_blocks",
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_only_exports_allowlisted_id_and_numeric_version() {
        assert_eq!(
            release("ID=ubuntu\nVERSION_ID=\"24.04\"\nPRETTY_NAME=private-name-canary"),
            (Some("ubuntu"), Some("24.04".into()))
        );
        assert_eq!(
            release("ID='fedora'\nVERSION_ID='43'"),
            (Some("fedora"), Some("43".into()))
        );
        assert_eq!(
            release("ID=private-vendor-canary\nVERSION_ID=private-version-canary"),
            (Some("other"), None)
        );
        assert_eq!(release("ID=arch"), (Some("arch"), None));
        assert_eq!(
            release("ID=$(private-command-canary)\nVERSION_ID=\"1.2-private-canary\""),
            (Some("other"), None)
        );
        assert_eq!(release(&"x".repeat(16385)), (None, None));
        let (distribution, distribution_version) = release(
            "ID=debian\nVERSION_ID=13\nHOME_URL=https://private-url-canary\nBUILD_ID=private-image-canary",
        );
        let report = super::super::OsVersion::Linux {
            distribution,
            distribution_version,
            kernel_version: Some("6.12.1".into()),
        };
        let json = serde_json::to_string(&report).unwrap();
        assert!(!json.contains("canary"));
        assert!(!json.contains("HOME_URL") && !json.contains("BUILD_ID"));
    }

    #[test]
    fn mem_available_is_not_replaced_with_free_memory_or_unchecked_units() {
        let value = memory(
            "MemTotal: 8192 kB\nMemFree: 1 kB\nMemAvailable: 2048 kB\nprivate-canary: omitted",
        )
        .unwrap();
        assert_eq!(value.total_bytes, "8388608");
        assert_eq!(value.available_bytes, "2097152");
        assert_eq!(value.availability_source, "linux_mem_available");
        for text in [
            "MemTotal: 8192 kB\nMemFree: 2048 kB",
            "MemTotal: 10 kB\nMemAvailable: 11 kB",
            "MemTotal: 10 MB\nMemAvailable: 1 kB",
            "MemTotal: 10 kB private-canary\nMemAvailable: 1 kB",
            "MemTotal: 18446744073709551615 kB\nMemAvailable: 1 kB",
            "MemTotal: 0 kB\nMemAvailable: 0 kB",
        ] {
            assert!(memory(text).is_none(), "accepted invalid memory snapshot");
        }
        assert_eq!(
            memory("MemTotal: 10 kB\nMemAvailable: 0 kB")
                .unwrap()
                .available_bytes,
            "0"
        );
    }

    #[test]
    fn local_filesystem_filter_skips_network_unknown_and_fuse() {
        assert_eq!(local_filesystem(0xef53), Some("ext"));
        for magic in [
            0x6969,
            0xff53_4d42,
            0x6573_5546,
            0x0102_1997,
            0x00c3_6400,
            0,
            -1,
        ] {
            assert!(local_filesystem(magic).is_none());
        }
    }
}
