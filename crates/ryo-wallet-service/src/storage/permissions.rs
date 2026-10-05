//! Private permissions for app-owned storage. Selected parent folders and
//! imported originals are never permission targets.
use std::fs::{self, File};
use std::io;
use std::path::Path;

pub fn secure_directory(path: &Path) -> io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(io::Error::other(
            "private storage is not a regular directory",
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))
    }
    #[cfg(windows)]
    {
        windows::secure(path, true)
    }
}

pub fn secure_file(path: &Path) -> io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(io::Error::other("private storage is not a regular file"));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))
    }
    #[cfg(windows)]
    {
        windows::secure(path, false)
    }
}

/// Check the same open file that will supply credential bytes.
pub fn verify_private_file(file: &File) -> io::Result<()> {
    let metadata = file.metadata()?;
    if !metadata.is_file() {
        return Err(io::Error::other("private storage is not a regular file"));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if metadata.mode() & 0o077 != 0 {
            return Err(io::Error::other("private file permits another user"));
        }
        Ok(())
    }
    #[cfg(windows)]
    {
        windows::verify(file)
    }
}

#[cfg(windows)]
mod windows {
    use super::*;
    use std::ffi::c_void;
    use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
    use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
    use std::ptr::{null, null_mut};
    use windows_sys::Win32::Foundation::{HANDLE, LocalFree};
    use windows_sys::Win32::Security::Authorization::{
        ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW,
        GetSecurityInfo, SDDL_REVISION_1, SE_FILE_OBJECT, SetSecurityInfo,
    };
    use windows_sys::Win32::Security::{
        ACCESS_ALLOWED_ACE, ACE_HEADER, ACL, CopySid, DACL_SECURITY_INFORMATION, EqualSid, GetAce,
        GetLengthSid, GetSecurityDescriptorDacl, GetTokenInformation, IsValidSid,
        OWNER_SECURITY_INFORMATION, PROTECTED_DACL_SECURITY_INFORMATION, TOKEN_QUERY, TOKEN_USER,
        TokenUser,
    };
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_ATTRIBUTE_REPARSE_POINT, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT,
        FILE_READ_ATTRIBUTES, READ_CONTROL, WRITE_DAC, WRITE_OWNER,
    };
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    struct LocalMemory(*mut c_void);
    impl Drop for LocalMemory {
        fn drop(&mut self) {
            // SAFETY: these allocations are returned by Win32 security APIs.
            unsafe {
                LocalFree(self.0);
            }
        }
    }

    struct UserSid(Vec<u32>);
    impl UserSid {
        fn pointer(&self) -> *mut c_void {
            self.0.as_ptr().cast_mut().cast()
        }
        fn current() -> io::Result<Self> {
            let mut token: HANDLE = null_mut();
            // SAFETY: output handle and token buffers remain live for these calls.
            unsafe {
                if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
                    return Err(io::Error::last_os_error());
                }
                let token = OwnedHandle::from_raw_handle(token);
                let mut length = 0;
                GetTokenInformation(token.as_raw_handle(), TokenUser, null_mut(), 0, &mut length);
                if length < size_of::<TOKEN_USER>() as u32 || length > 65536 {
                    return Err(io::Error::other("invalid process identity"));
                }
                let mut buffer = vec![0usize; (length as usize).div_ceil(size_of::<usize>())];
                if GetTokenInformation(
                    token.as_raw_handle(),
                    TokenUser,
                    buffer.as_mut_ptr().cast(),
                    length,
                    &mut length,
                ) == 0
                {
                    return Err(io::Error::last_os_error());
                }
                let sid = (*buffer.as_ptr().cast::<TOKEN_USER>()).User.Sid;
                if IsValidSid(sid) == 0 {
                    return Err(io::Error::other("invalid process identity"));
                }
                let size = GetLengthSid(sid);
                let mut copy = vec![0u32; (size as usize).div_ceil(size_of::<u32>())];
                if CopySid(size, copy.as_mut_ptr().cast(), sid) == 0 {
                    return Err(io::Error::last_os_error());
                }
                Ok(Self(copy))
            }
        }
        fn string(&self) -> io::Result<String> {
            let mut value = null_mut();
            // SAFETY: the SID is owned, validated and live. The returned string is NUL terminated.
            unsafe {
                if ConvertSidToStringSidW(self.pointer(), &mut value) == 0 {
                    return Err(io::Error::last_os_error());
                }
                let _allocation = LocalMemory(value.cast());
                let mut length = 0;
                while length < 256 && *value.add(length) != 0 {
                    length += 1;
                }
                if length == 256 {
                    return Err(io::Error::other("invalid process identity"));
                }
                Ok(String::from_utf16_lossy(std::slice::from_raw_parts(
                    value, length,
                )))
            }
        }
    }

    fn open(path: &Path, write_security: bool) -> io::Result<File> {
        let file = fs::OpenOptions::new()
            .access_mode(
                FILE_READ_ATTRIBUTES
                    | READ_CONTROL
                    | if write_security {
                        WRITE_DAC | WRITE_OWNER
                    } else {
                        0
                    },
            )
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
            .open(path)?;
        if file.metadata()?.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err(io::Error::other("private storage is a reparse point"));
        }
        Ok(file)
    }

    pub(super) fn secure(path: &Path, directory: bool) -> io::Result<()> {
        let file = open(path, true)?;
        if file.metadata()?.is_dir() != directory {
            return Err(io::Error::other("private storage type changed"));
        }
        let user = UserSid::current()?;
        let inherit = if directory { "OICI" } else { "" };
        // Only the current user and LocalSystem receive access. Administrators
        // retain the OS's privileged recovery capabilities, not a broad ACE.
        let sddl = format!(
            "D:P(A;{inherit};FA;;;{})(A;{inherit};FA;;;SY)",
            user.string()?
        );
        apply(&file, &user, &sddl)
    }

    fn apply(file: &File, user: &UserSid, sddl: &str) -> io::Result<()> {
        let value: Vec<u16> = sddl.encode_utf16().chain(Some(0)).collect();
        let mut descriptor = null_mut();
        // SAFETY: parsed descriptor and borrowed file/SID handles remain live.
        unsafe {
            if ConvertStringSecurityDescriptorToSecurityDescriptorW(
                value.as_ptr(),
                SDDL_REVISION_1,
                &mut descriptor,
                null_mut(),
            ) == 0
            {
                return Err(io::Error::last_os_error());
            }
            let _allocation = LocalMemory(descriptor);
            let mut present = 0;
            let mut defaulted = 0;
            let mut dacl = null_mut();
            if GetSecurityDescriptorDacl(descriptor, &mut present, &mut dacl, &mut defaulted) == 0
                || present == 0
            {
                return Err(io::Error::other("invalid private access policy"));
            }
            let result = SetSecurityInfo(
                file.as_raw_handle(),
                SE_FILE_OBJECT,
                OWNER_SECURITY_INFORMATION
                    | DACL_SECURITY_INFORMATION
                    | PROTECTED_DACL_SECURITY_INFORMATION,
                user.pointer(),
                null_mut(),
                dacl,
                null(),
            );
            if result != 0 {
                return Err(io::Error::from_raw_os_error(result as i32));
            }
        }
        Ok(())
    }

    pub(super) fn verify(file: &File) -> io::Result<()> {
        if file.metadata()?.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err(io::Error::other("private storage is a reparse point"));
        }
        let user = UserSid::current()?;
        let mut owner = null_mut();
        let mut dacl: *mut ACL = null_mut();
        let mut descriptor = null_mut();
        // SAFETY: Win32 validates and allocates the descriptor for this live file handle.
        unsafe {
            let result = GetSecurityInfo(
                file.as_raw_handle(),
                SE_FILE_OBJECT,
                OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
                &mut owner,
                null_mut(),
                &mut dacl,
                null_mut(),
                &mut descriptor,
            );
            if result != 0 {
                return Err(io::Error::from_raw_os_error(result as i32));
            }
            let _allocation = LocalMemory(descriptor);
            if owner.is_null()
                || IsValidSid(owner) == 0
                || EqualSid(owner, user.pointer()) == 0
                || dacl.is_null()
                || (*dacl).AceCount == 0
            {
                return Err(io::Error::other(
                    "private file identity or access policy is unsafe",
                ));
            }
            for index in 0..u32::from((*dacl).AceCount) {
                let mut entry = null_mut();
                if GetAce(dacl, index, &mut entry) == 0 {
                    return Err(io::Error::last_os_error());
                }
                let header = &*entry.cast::<ACE_HEADER>();
                if header.AceFlags & 8 != 0 || header.AceType == 1 {
                    continue;
                } // inherit-only / deny
                if header.AceType != 0
                    || usize::from(header.AceSize) < size_of::<ACCESS_ALLOWED_ACE>()
                {
                    return Err(io::Error::other("unsupported private file access policy"));
                }
                let allowed = &*entry.cast::<ACCESS_ALLOWED_ACE>();
                let sid = std::ptr::addr_of!(allowed.SidStart).cast_mut().cast();
                if IsValidSid(sid) == 0 {
                    return Err(io::Error::other("invalid private file identity"));
                }
                if allowed.Mask != 0 && EqualSid(sid, user.pointer()) == 0 {
                    // Compare with the fixed, valid LocalSystem SID S-1-5-18.
                    let system = [0x0000_0101u32, 0x0500_0000, 18];
                    if EqualSid(sid, system.as_ptr().cast_mut().cast()) == 0 {
                        return Err(io::Error::other("private file permits another user"));
                    }
                }
            }
        }
        Ok(())
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn inherited_private_files_are_readable_and_broad_or_null_acls_are_rejected() {
            let temp = tempfile::tempdir().unwrap();
            super::super::secure_directory(temp.path()).unwrap();
            let path = crate::process::login_path(temp.path(), 12345);
            fs::write(&path, b"ryo:disposable-credential").unwrap();
            super::super::verify_private_file(&File::open(&path).unwrap()).unwrap();
            assert!(crate::process::read_generated_login(temp.path(), 12345).is_ok());
            let file = open(&path, true).unwrap();
            let user = UserSid::current().unwrap();
            apply(
                &file,
                &user,
                &format!("D:P(A;;FA;;;{})(A;;FR;;;WD)", user.string().unwrap()),
            )
            .unwrap();
            assert!(super::super::verify_private_file(&File::open(&path).unwrap()).is_err());
            assert!(matches!(
                crate::process::read_generated_login(temp.path(), 12345),
                Err(crate::process::CredentialFileError::UnsafeFile)
            ));
            super::super::secure_file(&path).unwrap();
            super::super::verify_private_file(&File::open(&path).unwrap()).unwrap();
            apply(&file, &user, "D:NO_ACCESS_CONTROL").unwrap();
            assert!(super::super::verify_private_file(&File::open(&path).unwrap()).is_err());
            assert!(matches!(
                crate::process::read_generated_login(temp.path(), 12345),
                Err(crate::process::CredentialFileError::UnsafeFile)
            ));
        }

        #[test]
        fn selected_parent_stays_shared_while_app_subdirectories_become_private() {
            let temp = tempfile::tempdir().unwrap();
            let parent = open(temp.path(), true).unwrap();
            let user = UserSid::current().unwrap();
            apply(
                &parent,
                &user,
                &format!(
                    "D:P(A;OICI;FA;;;{})(A;OICI;FR;;;WD)",
                    user.string().unwrap()
                ),
            )
            .unwrap();
            let unrelated = temp.path().join("unrelated");
            fs::write(&unrelated, b"not wallet data").unwrap();
            assert!(super::super::verify_private_file(&File::open(&unrelated).unwrap()).is_err());
            let paths =
                crate::storage::AppPaths::new(temp.path().into(), crate::domain::Network::Mainnet)
                    .unwrap();
            paths.ensure_private_dirs().unwrap();
            let private = paths.runtime_root().join("inherited-login");
            fs::write(&private, b"disposable").unwrap();
            super::super::verify_private_file(&File::open(&private).unwrap()).unwrap();
            let original_keys = temp.path().join("unrelated.keys");
            fs::write(&original_keys, b"disposable keys").unwrap();
            let id = crate::storage::WalletId::parse("0123456789abcdef0123456789abcdef").unwrap();
            let imported = crate::storage::copy_wallet_pair(&paths, &id, &unrelated).unwrap();
            super::super::verify_private_file(&File::open(&imported.wallet_path).unwrap()).unwrap();
            super::super::verify_private_file(&File::open(&imported.keys_path).unwrap()).unwrap();
            let copied_file = open(&imported.wallet_path, true).unwrap();
            apply(
                &copied_file,
                &user,
                &format!("D:P(A;;FA;;;{})(A;;FR;;;WD)", user.string().unwrap()),
            )
            .unwrap();
            paths.secure_existing_wallet(&id).unwrap();
            super::super::verify_private_file(&File::open(&imported.wallet_path).unwrap()).unwrap();
            assert!(super::super::verify_private_file(&File::open(&unrelated).unwrap()).is_err());
            assert!(
                super::super::verify_private_file(&File::open(&original_keys).unwrap()).is_err()
            );
        }
    }
}
