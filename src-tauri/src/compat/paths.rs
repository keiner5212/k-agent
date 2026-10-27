use std::path::{Path, PathBuf};

/// User home directory on this host.
pub fn home_dir() -> Option<PathBuf> {
    for key in ["HOME", "USERPROFILE"] {
        if let Some(home) = std::env::var_os(key) {
            if !home.is_empty() {
                return Some(PathBuf::from(home));
            }
        }
    }
    // Windows without USERPROFILE: HOMEDRIVE plus HOMEPATH.
    if let (Some(drive), Some(path)) = (std::env::var_os("HOMEDRIVE"), std::env::var_os("HOMEPATH"))
    {
        if !drive.is_empty() && !path.is_empty() {
            let rest = path
                .to_string_lossy()
                .trim_start_matches(['/', '\\'])
                .to_string();
            return Some(PathBuf::from(drive).join(rest));
        }
    }
    None
}

/// Extensions that make a file runnable on this host.
pub fn executable_extensions() -> &'static [&'static str] {
    if cfg!(windows) {
        &[".exe", ".cmd", ".bat", ".com"]
    } else {
        &[]
    }
}

/// Whether a file can be run as a program on this host.
pub fn is_executable(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(path)
            .map(|meta| meta.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        let extension = path
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| value.to_ascii_lowercase());
        match extension {
            Some(extension) => executable_extensions()
                .iter()
                .any(|candidate| candidate.trim_start_matches('.') == extension),
            None => false,
        }
    }
}

/// Give a file the right permissions to run on this host.
pub fn make_executable(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(path)
            .map_err(|error| error.to_string())?
            .permissions();
        perms.set_mode(perms.mode() | 0o755);
        std::fs::set_permissions(path, perms).map_err(|error| error.to_string())?;
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
    Ok(())
}

/// Restrict a file to the current user.
pub fn set_user_private(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    }
    #[cfg(windows)]
    {
        restrict_acl(path);
    }
}

/// Restrict a directory to the current user.
pub fn set_dir_private(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700));
    }
    #[cfg(windows)]
    {
        restrict_acl(path);
    }
}

/// Replace the inherited access control list with owner only access.
#[cfg(windows)]
fn restrict_acl(path: &Path) {
    use windows::core::{BOOL, PCWSTR};
    use windows::Win32::Foundation::{LocalFree, HLOCAL};
    use windows::Win32::Security::Authorization::{
        ConvertStringSecurityDescriptorToSecurityDescriptorW, SetNamedSecurityInfoW, SE_FILE_OBJECT,
    };
    use windows::Win32::Security::{
        GetSecurityDescriptorDacl, ACL, DACL_SECURITY_INFORMATION, OBJECT_SECURITY_INFORMATION,
        PROTECTED_DACL_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR,
    };

    let wide = path_to_wide(path);
    // Protected DACL, full access for the owner only.
    let sddl: Vec<u16> = "D:P(A;;FA;;;OW)\0".encode_utf16().collect();
    let mut descriptor = PSECURITY_DESCRIPTOR(std::ptr::null_mut());
    let built = unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            PCWSTR(sddl.as_ptr()),
            1,
            &mut descriptor,
            None,
        )
    };
    if built.is_err() {
        return;
    }
    unsafe {
        let mut dacl: *mut ACL = std::ptr::null_mut();
        let mut present = BOOL::default();
        let mut defaulted = BOOL::default();
        if GetSecurityDescriptorDacl(descriptor, &mut present, &mut dacl, &mut defaulted).is_ok()
            && present.as_bool()
        {
            let flags = OBJECT_SECURITY_INFORMATION(
                DACL_SECURITY_INFORMATION.0 | PROTECTED_DACL_SECURITY_INFORMATION.0,
            );
            let _ = SetNamedSecurityInfoW(
                PCWSTR(wide.as_ptr()),
                SE_FILE_OBJECT,
                flags,
                None,
                None,
                Some(dacl),
                None,
            );
        }
        let _ = LocalFree(Some(HLOCAL(descriptor.0)));
    }
}

#[cfg(windows)]
fn path_to_wide(path: &Path) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    path.as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn home_falls_back_to_windows_variables() {
        let home = std::env::var_os("HOME");
        let profile = std::env::var_os("USERPROFILE");
        let _drive = std::env::var_os("HOMEDRIVE");
        let _path = std::env::var_os("HOMEPATH");
        std::env::remove_var("HOME");
        std::env::remove_var("USERPROFILE");
        std::env::set_var("HOMEDRIVE", "C:");
        std::env::set_var("HOMEPATH", "\\Users\\me");
        let resolved = home_dir().expect("home directory");
        let text = resolved.to_string_lossy().to_string();
        assert!(text.contains("Users"), "home directory was {text}");
        assert!(text.ends_with("me"), "home directory was {text}");
        std::env::remove_var("HOMEDRIVE");
        std::env::remove_var("HOMEPATH");
        if let Some(value) = home {
            std::env::set_var("HOME", value);
        }
        if let Some(value) = profile {
            std::env::set_var("USERPROFILE", value);
        }
    }
}
