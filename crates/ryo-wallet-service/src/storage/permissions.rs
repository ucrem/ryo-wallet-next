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
    use windows_sys::Win32::Foundation::{ERROR_ACCESS_DENIED, HANDLE, LocalFree};
    use windows_sys::Win32::Security::Authorization::{
        ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW,
        GetSecurityInfo, SDDL_REVISION_1, SE_FILE_OBJECT, SetSecurityInfo,
    };
    use windows_sys::Win32::Security::{
        ACCESS_ALLOWED_ACE, ACE_HEADER, ACL, CopySid, CreateWellKnownSid,
        DACL_SECURITY_INFORMATION, EqualSid, GetAce, GetLengthSid, GetSecurityDescriptorDacl,
        GetTokenInformation, IsValidSid, OWNER_SECURITY_INFORMATION,
        PROTECTED_DACL_SECURITY_INFORMATION, TOKEN_INFORMATION_CLASS, TOKEN_OWNER, TOKEN_QUERY,
        TOKEN_USER, TokenOwner, TokenUser, WELL_KNOWN_SID_TYPE, WinBuiltinAdministratorsSid,
        WinLocalSystemSid,
    };
    use windows_sys::Win32::Storage::FileSystem::{
        DELETE, FILE_ATTRIBUTE_REPARSE_POINT, FILE_FLAG_BACKUP_SEMANTICS,
        FILE_FLAG_OPEN_REPARSE_POINT, FILE_GENERIC_READ, FILE_READ_ATTRIBUTES, READ_CONTROL,
        WRITE_DAC, WRITE_OWNER,
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
            Self::from_token(TokenUser)
        }
        fn default_owner() -> io::Result<Self> {
            Self::from_token(TokenOwner)
        }
        fn well_known(kind: WELL_KNOWN_SID_TYPE) -> io::Result<Self> {
            let mut buffer = vec![0u32; 17]; // SECURITY_MAX_SID_SIZE (68 bytes), aligned.
            let mut length = (buffer.len() * size_of::<u32>()) as u32;
            // SAFETY: correctly aligned, bounded output buffer; no domain SID required.
            if unsafe {
                CreateWellKnownSid(kind, null_mut(), buffer.as_mut_ptr().cast(), &mut length)
            } == 0
            {
                return Err(io::Error::last_os_error());
            }
            Ok(Self(buffer))
        }
        fn from_token(kind: TOKEN_INFORMATION_CLASS) -> io::Result<Self> {
            let mut token: HANDLE = null_mut();
            // SAFETY: output handle and token buffers remain live for these calls.
            unsafe {
                if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
                    return Err(io::Error::last_os_error());
                }
                let token = OwnedHandle::from_raw_handle(token);
                let mut length = 0;
                GetTokenInformation(token.as_raw_handle(), kind, null_mut(), 0, &mut length);
                if length < size_of::<TOKEN_USER>() as u32 || length > 65536 {
                    return Err(io::Error::other("invalid process identity"));
                }
                let mut buffer = vec![0usize; (length as usize).div_ceil(size_of::<usize>())];
                if GetTokenInformation(
                    token.as_raw_handle(),
                    kind,
                    buffer.as_mut_ptr().cast(),
                    length,
                    &mut length,
                ) == 0
                {
                    return Err(io::Error::last_os_error());
                }
                let sid = if kind == TokenOwner {
                    (*buffer.as_ptr().cast::<TOKEN_OWNER>()).Owner
                } else {
                    (*buffer.as_ptr().cast::<TOKEN_USER>()).User.Sid
                };
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
        open_with_access(
            path,
            FILE_READ_ATTRIBUTES
                | READ_CONTROL
                | if write_security {
                    WRITE_DAC | WRITE_OWNER
                } else {
                    0
                },
        )
    }

    fn open_with_access(path: &Path, access: u32) -> io::Result<File> {
        let file = fs::OpenOptions::new()
            .access_mode(access)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
            .open(path)?;
        if file.metadata()?.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err(io::Error::other("private storage is a reparse point"));
        }
        Ok(file)
    }

    pub(super) fn secure(path: &Path, directory: bool) -> io::Result<()> {
        let user = UserSid::current()?;
        let (file, change_owner) = match open(path, true) {
            Ok(file) => (file, true),
            Err(error) if error.raw_os_error() == Some(ERROR_ACCESS_DENIED as i32) => {
                // Owners can edit their DACL without WRITE_OWNER. Secondary
                // drives often grant only Modify; do not request an ownership
                // change when this same validated handle is already user-owned.
                let file = open_with_access(path, FILE_READ_ATTRIBUTES | READ_CONTROL | WRITE_DAC)?;
                if !owned_by(&file, &user)? {
                    return Err(error);
                }
                (file, false)
            }
            Err(error) => return Err(error),
        };
        if file.metadata()?.is_dir() != directory {
            return Err(io::Error::other("private storage type changed"));
        }
        let inherit = if directory { "OICI" } else { "" };
        // Only the current user and LocalSystem receive access. Administrators
        // retain the OS's privileged recovery capabilities, not a broad ACE.
        let sddl = format!(
            "D:P(A;{inherit};FA;;;{})(A;{inherit};FA;;;SY)",
            user.string()?
        );
        apply_policy(&file, &user, &sddl, change_owner)
    }

    fn owned_by(file: &File, user: &UserSid) -> io::Result<bool> {
        let mut owner = null_mut();
        let mut descriptor = null_mut();
        // SAFETY: query the live, reparse-checked handle; owner points into the
        // returned descriptor and is compared before its allocation is freed.
        unsafe {
            let result = GetSecurityInfo(
                file.as_raw_handle(),
                SE_FILE_OBJECT,
                OWNER_SECURITY_INFORMATION,
                &mut owner,
                null_mut(),
                null_mut(),
                null_mut(),
                &mut descriptor,
            );
            if result != 0 {
                return Err(io::Error::from_raw_os_error(result as i32));
            }
            let _allocation = LocalMemory(descriptor);
            Ok(!owner.is_null() && IsValidSid(owner) != 0 && EqualSid(owner, user.pointer()) != 0)
        }
    }

    #[cfg(test)]
    fn apply(file: &File, user: &UserSid, sddl: &str) -> io::Result<()> {
        apply_policy(file, user, sddl, true)
    }

    fn apply_policy(file: &File, user: &UserSid, sddl: &str, change_owner: bool) -> io::Result<()> {
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
                DACL_SECURITY_INFORMATION
                    | PROTECTED_DACL_SECURITY_INFORMATION
                    | if change_owner {
                        OWNER_SECURITY_INFORMATION
                    } else {
                        0
                    },
                if change_owner {
                    user.pointer()
                } else {
                    null_mut()
                },
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
        let default_owner = UserSid::default_owner()?;
        let administrators = UserSid::well_known(WinBuiltinAdministratorsSid)?;
        let system = UserSid::well_known(WinLocalSystemSid)?;
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
                || !owner_allowed(owner, &user, &default_owner, &administrators)
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
                if !allow_allowed(
                    sid,
                    allowed.Mask,
                    &user,
                    &default_owner,
                    &administrators,
                    &system,
                ) {
                    return Err(io::Error::other("private file permits another user"));
                }
            }
        }
        Ok(())
    }

    // Ryo uses TokenOwner, which can be Administrators on an elevated Windows
    // token. This does not authorize arbitrary owner groups or broad admin ACEs.
    unsafe fn owner_allowed(
        owner: *mut c_void,
        user: &UserSid,
        default_owner: &UserSid,
        administrators: &UserSid,
    ) -> bool {
        // SAFETY: caller supplies validated, live SIDs.
        unsafe {
            EqualSid(owner, user.pointer()) != 0
                || (EqualSid(default_owner.pointer(), administrators.pointer()) != 0
                    && EqualSid(owner, default_owner.pointer()) != 0)
        }
    }

    unsafe fn allow_allowed(
        sid: *mut c_void,
        mask: u32,
        user: &UserSid,
        default_owner: &UserSid,
        administrators: &UserSid,
        system: &UserSid,
    ) -> bool {
        // SAFETY: caller supplies validated, live SIDs. The elevated exception is
        // only Ryo's documented read/query/delete mask for this token's owner.
        unsafe {
            mask == 0
                || EqualSid(sid, user.pointer()) != 0
                || EqualSid(sid, system.pointer()) != 0
                || (EqualSid(default_owner.pointer(), administrators.pointer()) != 0
                    && EqualSid(sid, default_owner.pointer()) != 0
                    && mask & !(READ_CONTROL | FILE_GENERIC_READ | DELETE) == 0)
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn owned_modify_only_directory_can_be_secured_without_write_owner() {
            let temp = tempfile::tempdir().unwrap();
            let user = UserSid::current().unwrap();
            {
                let file = open(temp.path(), true).unwrap();
                // Typical secondary-drive Modify grant: current user is owner,
                // but the DACL does not grant WRITE_OWNER or WRITE_DAC.
                apply(
                    &file,
                    &user,
                    &format!(
                        "D:P(A;OICI;0x1301bf;;;{})(A;OICI;FA;;;SY)",
                        user.string().unwrap()
                    ),
                )
                .unwrap();
            }
            assert_eq!(open(temp.path(), true).unwrap_err().raw_os_error(), Some(5));
            super::super::secure_directory(temp.path())
                .expect("owner can protect its DACL without changing ownership");
            let child = temp.path().join("inherited-login");
            fs::write(&child, b"ryo:disposable-credential").unwrap();
            super::super::verify_private_file(&File::open(&child).unwrap()).unwrap();
        }

        #[test]
        fn owned_modify_only_file_is_secured_without_changing_contents() {
            let temp = tempfile::tempdir().unwrap();
            let path = temp.path().join("existing-wallet-canary");
            fs::write(&path, b"unchanged-disposable-wallet-canary").unwrap();
            let user = UserSid::current().unwrap();
            {
                let file = open(&path, true).unwrap();
                apply(
                    &file,
                    &user,
                    &format!("D:P(A;;0x1301bf;;;{})(A;;FR;;;WD)", user.string().unwrap()),
                )
                .unwrap();
            }
            assert_eq!(open(&path, true).unwrap_err().raw_os_error(), Some(5));
            assert!(super::super::verify_private_file(&File::open(&path).unwrap()).is_err());
            super::super::secure_file(&path).unwrap();
            super::super::verify_private_file(&File::open(&path).unwrap()).unwrap();
            assert_eq!(
                fs::read(&path).unwrap(),
                b"unchanged-disposable-wallet-canary"
            );
        }

        #[tokio::test]
        #[ignore = "requires RYO_TEST_WALLET_RPC; only a disposable empty listener is started"]
        async fn reviewed_wallet_rpc_starts_from_owned_modify_only_storage() {
            use crate::domain::{Network, NodeConfig};
            use crate::process::{BinaryDigest, BinaryKind, VerifiedBinary, WalletRpcSession};
            use crate::storage::AppPaths;
            use std::net::{Ipv4Addr, TcpListener};
            use std::time::Duration;

            let manifest: serde_json::Value =
                serde_json::from_str(include_str!("../../../../src-tauri/runtime-manifest.json"))
                    .unwrap();
            let digest = manifest["platforms"]["x86_64-pc-windows-msvc"]["binarySha256"]
                .as_str()
                .unwrap();
            let path = std::path::PathBuf::from(
                std::env::var_os("RYO_TEST_WALLET_RPC")
                    .expect("set the verified test runtime path"),
            );
            let binary = VerifiedBinary::verify(
                BinaryKind::WalletRpc,
                &path,
                BinaryDigest::parse_hex(digest).unwrap(),
            )
            .unwrap();
            let temp = tempfile::tempdir().unwrap();
            let paths =
                AppPaths::new(temp.path().join("disposable-data"), Network::Mainnet).unwrap();
            paths.ensure_private_dirs().unwrap();
            let user = UserSid::current().unwrap();
            for directory in [
                paths.network_root(),
                paths.wallets_root(),
                paths.runtime_root(),
                paths.chain_root(),
            ] {
                let file = open(&directory, true).unwrap();
                apply(
                    &file,
                    &user,
                    &format!(
                        "D:P(A;OICI;0x1301bf;;;{})(A;OICI;FA;;;SY)",
                        user.string().unwrap()
                    ),
                )
                .unwrap();
            }
            let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
            let port = listener.local_addr().unwrap().port();
            drop(listener);
            let session = WalletRpcSession::start(
                &binary,
                &paths,
                &NodeConfig::managed_local(Network::Mainnet),
                port,
            )
            .await
            .expect("reviewed RPC starts after owner-only storage migration");
            assert!(!session.client().languages().await.unwrap().is_empty());
            assert_eq!(fs::read_dir(paths.wallets_root()).unwrap().count(), 0);
            session.stop(Duration::from_secs(5)).await.unwrap();
        }

        #[test]
        fn elevated_upstream_owner_is_narrowly_scoped_and_never_allows_everyone() {
            use windows_sys::Win32::Security::WinWorldSid;
            use windows_sys::Win32::Storage::FileSystem::FILE_ALL_ACCESS;
            let user = UserSid::current().unwrap();
            let administrators = UserSid::well_known(WinBuiltinAdministratorsSid).unwrap();
            let system = UserSid::well_known(WinLocalSystemSid).unwrap();
            let world = UserSid::well_known(WinWorldSid).unwrap();
            // SAFETY: the APIs above produced validated, owned SIDs.
            unsafe {
                assert!(owner_allowed(
                    administrators.pointer(),
                    &user,
                    &administrators,
                    &administrators
                ));
                assert!(!owner_allowed(
                    world.pointer(),
                    &user,
                    &world,
                    &administrators
                ));
                assert!(allow_allowed(
                    administrators.pointer(),
                    READ_CONTROL | FILE_GENERIC_READ | DELETE,
                    &user,
                    &administrators,
                    &administrators,
                    &system
                ));
                assert!(!allow_allowed(
                    administrators.pointer(),
                    FILE_ALL_ACCESS,
                    &user,
                    &administrators,
                    &administrators,
                    &system
                ));
                assert!(!allow_allowed(
                    world.pointer(),
                    FILE_GENERIC_READ,
                    &user,
                    &world,
                    &administrators,
                    &system
                ));
            }
        }

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
