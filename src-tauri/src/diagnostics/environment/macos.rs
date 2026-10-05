//! macOS version/sysctl, Mach memory and local volume probes; no shell commands.
use super::MemoryReport;
#[cfg(target_os = "macos")]
use super::{OsVersion, VolumeReport, filesystem_name, numeric_version, unix};

fn build_version(value: &str) -> Option<String> {
    // Apple's build grammar (e.g. 24A335, 24E248a); exclude arbitrary API text.
    if value.len() > 16 {
        return None;
    }
    let major = value.bytes().take_while(u8::is_ascii_digit).count();
    let branch = value[major..]
        .bytes()
        .take_while(u8::is_ascii_uppercase)
        .count();
    let suffix = &value[major + branch..];
    let revision = suffix
        .strip_suffix(|ch: char| ch.is_ascii_lowercase())
        .unwrap_or(suffix);
    ((1..=3).contains(&major)
        && (1..=2).contains(&branch)
        && (1..=6).contains(&revision.len())
        && revision.bytes().all(|byte| byte.is_ascii_digit()))
    .then(|| value.to_owned())
}

fn memory(
    total: u64,
    free_pages: u32,
    inactive_pages: u32,
    page_size: u32,
) -> Option<MemoryReport> {
    if total == 0 || page_size == 0 {
        return None;
    }
    // Mach free_count already includes speculative pages. Do not add them again
    // or sum purgeable pages that may overlap active/inactive queues.
    let available = u64::from(free_pages)
        .checked_add(u64::from(inactive_pages))?
        .checked_mul(u64::from(page_size))?
        .min(total);
    Some(MemoryReport {
        total_bytes: total.to_string(),
        available_bytes: available.to_string(),
        availability_source: "macos_free_and_inactive_estimate",
    })
}

#[cfg(target_os = "macos")]
fn sysctl_text(name: &std::ffi::CStr) -> Option<String> {
    let mut bytes = [0u8; 64];
    let mut length = bytes.len();
    // Fixed numeric-version keys, bounded writable buffer; null newp prevents writes.
    let result = unsafe {
        libc::sysctlbyname(
            name.as_ptr(),
            bytes.as_mut_ptr().cast(),
            &mut length,
            std::ptr::null_mut(),
            0,
        )
    };
    if result != 0 || length == 0 || length > bytes.len() {
        return None;
    }
    let end = bytes[..length].iter().position(|byte| *byte == 0)?;
    std::str::from_utf8(&bytes[..end]).ok().map(str::to_owned)
}

#[cfg(target_os = "macos")]
pub(super) fn os_version() -> Option<OsVersion> {
    let product_version = sysctl_text(c"kern.osproductversion")
        .as_deref()
        .and_then(numeric_version);
    let kernel_version = unix::kernel_version();
    let build = sysctl_text(c"kern.osversion")
        .as_deref()
        .and_then(build_version);
    (product_version.is_some() || kernel_version.is_some() || build.is_some()).then_some(
        OsVersion::Macos {
            product_version,
            kernel_version,
            build,
        },
    )
}

#[cfg(target_os = "macos")]
unsafe extern "C" {
    // Public libSystem APIs not declared by the pinned libc crate.
    fn host_page_size(
        host: libc::mach_port_t,
        page_size: *mut libc::vm_size_t,
    ) -> libc::kern_return_t;
    fn mach_port_deallocate(
        task: libc::mach_port_t,
        name: libc::mach_port_t,
    ) -> libc::kern_return_t;
}

#[cfg(target_os = "macos")]
struct HostPort(libc::mach_port_t);

#[cfg(target_os = "macos")]
impl Drop for HostPort {
    fn drop(&mut self) {
        // Release this acquired host send right on every exit path.
        #[allow(deprecated)]
        unsafe {
            mach_port_deallocate(libc::mach_task_self(), self.0);
        }
    }
}

#[cfg(target_os = "macos")]
pub(super) fn physical_memory() -> Option<MemoryReport> {
    let mut total = 0u64;
    let mut length = size_of::<u64>();
    // hw.memsize is a fixed uint64 sysctl; only its numeric value is retained.
    let result = unsafe {
        libc::sysctlbyname(
            c"hw.memsize".as_ptr(),
            (&mut total as *mut u64).cast(),
            &mut length,
            std::ptr::null_mut(),
            0,
        )
    };
    if result != 0 || length != size_of::<u64>() {
        return None;
    }
    // libc recommends a separate Mach binding crate; the native API remains
    // supported. Keep this allowance confined to the pinned binding call.
    #[allow(deprecated)]
    let host = HostPort(unsafe { libc::mach_host_self() });
    if host.0 == 0 {
        return None;
    }
    // vm_size_t is pointer-sized on 64-bit Darwin; match the native output ABI.
    let mut page_size: libc::vm_size_t = 0;
    if unsafe { host_page_size(host.0, &mut page_size) } != libc::KERN_SUCCESS {
        return None;
    }
    let mut stats = std::mem::MaybeUninit::<libc::vm_statistics64>::zeroed();
    let mut count = libc::HOST_VM_INFO64_COUNT;
    // Caller-owned buffer in integer_t units. Older Darwin releases may return
    // fewer fields; only the first four page counters are needed here.
    if unsafe {
        libc::host_statistics64(
            host.0,
            libc::HOST_VM_INFO64,
            stats.as_mut_ptr().cast(),
            &mut count,
        )
    } != libc::KERN_SUCCESS
        || count < 4
    {
        return None;
    }
    let stats = unsafe { stats.assume_init() };
    memory(
        total,
        stats.free_count,
        stats.inactive_count,
        u32::try_from(page_size).ok()?,
    )
}

#[cfg(target_os = "macos")]
pub(super) fn data_volume(root: &std::path::Path) -> Option<VolumeReport> {
    use std::os::fd::AsRawFd;
    let directory = unix::directory(root)?;
    let mut stats = std::mem::MaybeUninit::<libc::statfs>::uninit();
    // Query only the selected directory. Device IDs, owners and mount paths in
    // this native structure are never read or serialized.
    if unsafe { libc::fstatfs(directory.as_raw_fd(), stats.as_mut_ptr()) } != 0 {
        return None;
    }
    let stats = unsafe { stats.assume_init() };
    if stats.f_flags & (libc::MNT_LOCAL as u32) == 0 {
        return None;
    }
    let bytes: Vec<u8> = stats.f_fstypename.iter().map(|byte| *byte as u8).collect();
    let end = bytes.iter().position(|byte| *byte == 0)?;
    let name = std::str::from_utf8(&bytes[..end]).ok()?;
    Some(VolumeReport {
        filesystem: Some(filesystem_name(name)),
        supports_persistent_acls: None,
        read_only: Some(stats.f_flags & (libc::MNT_RDONLY as u32) != 0),
        total_bytes: super::byte_count(stats.f_blocks.into(), stats.f_bsize.into()),
        free_bytes_available_to_user: super::byte_count(
            stats.f_bavail.into(),
            stats.f_bsize.into(),
        ),
        space_source: "macos_statfs_blocks",
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apple_build_filter_rejects_arbitrary_identifiers() {
        for value in ["24A335", "24E248a", "25B78"] {
            assert_eq!(build_version(value).as_deref(), Some(value));
        }
        for value in [
            "",
            "private-machine-canary",
            "24A335-private-user-canary",
            "24A335\\path",
            "\u{00e9}",
            "1234A335",
            "24A",
            "24a335",
        ] {
            assert!(build_version(value).is_none());
        }
        let version = super::super::OsVersion::Macos {
            product_version: Some("15.7".into()),
            kernel_version: Some("24.6.0".into()),
            build: build_version("24G222"),
        };
        let json = serde_json::to_string(&version).unwrap();
        assert!(!json.contains("canary"));
        assert!(json.contains("macos"));
    }

    #[test]
    fn memory_estimate_uses_native_page_size_and_does_not_double_count_speculative_pages() {
        let report = memory(1_048_576, 2, 3, 16384).unwrap();
        assert_eq!(report.available_bytes, "81920");
        assert_eq!(report.total_bytes, "1048576");
        assert_eq!(
            report.availability_source,
            "macos_free_and_inactive_estimate"
        );
        assert_eq!(
            memory(1024, 100, 100, 4096).unwrap().available_bytes,
            "1024"
        );
        assert!(memory(0, 1, 1, 4096).is_none());
        assert!(memory(1024, 1, 1, 0).is_none());
    }
}
