//! Fixed-size native probes. Never export uname's hostname or statfs identifiers.
use std::ffi::CString;
use std::os::fd::{FromRawFd, OwnedFd};
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

pub(super) fn directory(root: &Path) -> Option<OwnedFd> {
    let bytes = root.as_os_str().as_bytes();
    if !root.is_absolute() || bytes.is_empty() || bytes.len() >= 4096 {
        return None;
    }
    let path = CString::new(bytes).ok()?;
    // Read-only directory handle; no file contents are read. O_NONBLOCK avoids
    // special-file waits, O_DIRECTORY excludes files, and RAII closes the handle.
    let fd = unsafe {
        libc::open(
            path.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC | libc::O_NONBLOCK,
        )
    };
    (fd >= 0).then(|| unsafe { OwnedFd::from_raw_fd(fd) })
}

pub(super) fn kernel_version() -> Option<String> {
    let mut info = std::mem::MaybeUninit::<libc::utsname>::uninit();
    // Correctly sized caller-owned output; inspect only the bounded release field.
    if unsafe { libc::uname(info.as_mut_ptr()) } != 0 {
        return None;
    }
    let info = unsafe { info.assume_init() };
    let bytes: Vec<u8> = info.release.iter().map(|byte| *byte as u8).collect();
    let end = bytes.iter().position(|byte| *byte == 0)?;
    let release = std::str::from_utf8(&bytes[..end]).ok()?;
    // Drop kernel flavor/custom suffixes rather than exporting arbitrary text.
    super::kernel_release(release)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn directory_probe_refuses_invalid_paths_and_files() {
        let temp = tempfile::tempdir().unwrap();
        assert!(directory(temp.path()).is_some());
        for path in [Path::new("relative"), Path::new("/nul\0canary")] {
            assert!(directory(path).is_none());
        }
        let file = temp.path().join("file-canary");
        std::fs::write(&file, b"unchanged-canary").unwrap();
        assert!(directory(&file).is_none());
        assert_eq!(std::fs::read(&file).unwrap(), b"unchanged-canary");
        assert!(kernel_version().is_some());
    }
}
