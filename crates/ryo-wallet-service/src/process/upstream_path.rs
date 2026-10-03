use std::path::{Path, PathBuf};

/// Upstream Ryo's Windows filesystem libraries do not support Rust's canonical
/// verbatim prefixes. Keep canonical paths in storage and convert only at the
/// C++ process boundary, rejecting names whose Win32 interpretation would differ.
pub fn upstream_path(path: &Path) -> std::io::Result<PathBuf> {
    #[cfg(not(windows))]
    {
        Ok(path.to_owned())
    }
    #[cfg(windows)]
    {
        use std::ffi::OsString;
        use std::os::windows::ffi::OsStrExt;
        use std::path::{Component, Prefix};

        let invalid = || {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "data path is not supported by the bundled Windows runtime",
            )
        };
        if !path.is_absolute() {
            return Err(invalid());
        }
        let mut components = path.components();
        let mut result = match components.next() {
            Some(Component::Prefix(prefix)) => match prefix.kind() {
                Prefix::VerbatimDisk(drive) => PathBuf::from(format!("{}:", char::from(drive))),
                Prefix::VerbatimUNC(server, share) => {
                    let mut unc = OsString::from("\\\\");
                    unc.push(server);
                    unc.push("\\");
                    unc.push(share);
                    PathBuf::from(unc)
                }
                Prefix::Disk(_) | Prefix::UNC(_, _) => return Ok(path.to_owned()),
                _ => return Err(invalid()),
            },
            _ => return Err(invalid()),
        };
        for component in components {
            match component {
                Component::RootDir => result.push("\\"),
                Component::Normal(name) => {
                    let text = name.to_string_lossy();
                    let stem = text
                        .split('.')
                        .next()
                        .unwrap_or_default()
                        .to_ascii_uppercase();
                    let reserved = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
                        || (stem.len() == 4
                            && (stem.starts_with("COM") || stem.starts_with("LPT"))
                            && matches!(stem.as_bytes()[3], b'1'..=b'9'));
                    if text.ends_with(['.', ' ']) || text.contains(['/', ':']) || reserved {
                        return Err(invalid());
                    }
                    result.push(name);
                }
                _ => return Err(invalid()),
            }
        }
        // Leave room for the upstream database's own filenames below a directory.
        if result.as_os_str().encode_wide().count() >= 248 {
            return Err(invalid());
        }
        Ok(result)
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn canonical_disk_and_unc_paths_keep_the_same_location() {
        for (canonical, ordinary) in [
            (r"\\?\G:\mainnet\chain", r"G:\mainnet\chain"),
            (r"\\?\G:\", r"G:\"),
            (r"\\?\UNC\server\share\chain", r"\\server\share\chain"),
        ] {
            assert_eq!(
                upstream_path(Path::new(canonical)).unwrap(),
                Path::new(ordinary)
            );
        }
        let temp = tempfile::tempdir().unwrap();
        let canonical = std::fs::canonicalize(temp.path()).unwrap();
        let ordinary = upstream_path(&canonical).unwrap();
        assert_eq!(std::fs::canonicalize(ordinary).unwrap(), canonical);
    }

    #[test]
    fn conversion_rejects_ambiguous_names_device_paths_and_long_paths() {
        for path in [
            r"\\?\G:\folder.\chain".to_owned(),
            r"\\?\G:\folder \chain".to_owned(),
            r"\\?\G:\CON\chain".to_owned(),
            r"\\?\G:\folder/chain".to_owned(),
            r"\\?\G:\folder\..\chain".to_owned(),
            r"\\.\PhysicalDrive0".to_owned(),
            format!(r"\\?\G:\{}", "x".repeat(250)),
        ] {
            assert!(upstream_path(Path::new(&path)).is_err(), "{path}");
        }
    }
}
