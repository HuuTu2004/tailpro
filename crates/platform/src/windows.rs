use std::{fs, io, os::windows::ffi::OsStrExt, path::Path, ptr};
use windows_sys::Win32::{
    Foundation::*,
    Security::{Authorization::*, *},
    System::Threading::*,
};
fn wide(path: &Path) -> Vec<u16> {
    path.as_os_str().encode_wide().chain(Some(0)).collect()
}
struct Local(*mut std::ffi::c_void);
impl Drop for Local {
    fn drop(&mut self) {
        unsafe {
            LocalFree(self.0);
        }
    }
}
fn user() -> io::Result<Vec<usize>> {
    unsafe {
        let mut token = ptr::null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return Err(io::Error::last_os_error());
        }
        let mut size = 0;
        GetTokenInformation(token, TokenUser, ptr::null_mut(), 0, &mut size);
        let mut data = vec![0usize; (size as usize).div_ceil(size_of::<usize>())];
        let ok = GetTokenInformation(token, TokenUser, data.as_mut_ptr().cast(), size, &mut size);
        CloseHandle(token);
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(data)
    }
}
pub fn protect(path: &Path, directory: bool) -> io::Result<()> {
    if fs::symlink_metadata(path)?.file_type().is_symlink() {
        return Err(io::ErrorKind::PermissionDenied.into());
    }
    unsafe {
        let user = user()?;
        let sid = (*(user.as_ptr().cast::<TOKEN_USER>())).User.Sid;
        let mut text = ptr::null_mut();
        if ConvertSidToStringSidW(sid, &mut text) == 0 {
            return Err(io::Error::last_os_error());
        }
        let _text = Local(text.cast());
        let mut len = 0;
        while *text.add(len) != 0 {
            len += 1;
        }
        let sid = String::from_utf16_lossy(std::slice::from_raw_parts(text, len));
        let sddl = format!("D:P(A;{};FA;;;{sid})", if directory { "OICI" } else { "" });
        let sddl: Vec<u16> = sddl.encode_utf16().chain(Some(0)).collect();
        let mut descriptor = ptr::null_mut();
        if ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl.as_ptr(),
            1,
            &mut descriptor,
            ptr::null_mut(),
        ) == 0
        {
            return Err(io::Error::last_os_error());
        }
        let _descriptor = Local(descriptor);
        let mut present = 0;
        let mut defaulted = 0;
        let mut acl = ptr::null_mut();
        if GetSecurityDescriptorDacl(descriptor, &mut present, &mut acl, &mut defaulted) == 0
            || present == 0
            || acl.is_null()
        {
            return Err(io::ErrorKind::PermissionDenied.into());
        }
        let code = SetNamedSecurityInfoW(
            wide(path).as_ptr(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
            ptr::null_mut(),
            ptr::null_mut(),
            acl,
            ptr::null(),
        );
        if code != 0 {
            return Err(io::Error::from_raw_os_error(code as i32));
        }
    }
    check_private(path, directory)
}
pub fn check_private(path: &Path, directory: bool) -> io::Result<()> {
    let meta = fs::symlink_metadata(path)?;
    use std::os::windows::fs::MetadataExt;
    if meta.file_attributes() & 0x400 != 0 || meta.is_dir() != directory {
        return Err(io::ErrorKind::PermissionDenied.into());
    }
    unsafe {
        let user = user()?;
        let sid = (*(user.as_ptr().cast::<TOKEN_USER>())).User.Sid;
        let mut owner = ptr::null_mut();
        let mut acl = ptr::null_mut();
        let mut descriptor = ptr::null_mut();
        let code = GetNamedSecurityInfoW(
            wide(path).as_ptr(),
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
            &mut owner,
            ptr::null_mut(),
            &mut acl,
            ptr::null_mut(),
            &mut descriptor,
        );
        if code != 0 {
            return Err(io::Error::from_raw_os_error(code as i32));
        }
        let _descriptor = Local(descriptor);
        if owner.is_null() || EqualSid(owner, sid) == 0 || acl.is_null() || (*acl).AceCount == 0 {
            return Err(io::ErrorKind::PermissionDenied.into());
        }
        for index in 0..(*acl).AceCount {
            let mut ace = ptr::null_mut();
            if GetAce(acl, index as u32, &mut ace) == 0 {
                return Err(io::Error::last_os_error());
            }
            let ace = ace.cast::<ACCESS_ALLOWED_ACE>();
            if (*ace).Header.AceType != 0
                || EqualSid(ptr::addr_of!((*ace).SidStart).cast_mut().cast(), sid) == 0
            {
                return Err(io::ErrorKind::PermissionDenied.into());
            }
        }
    }
    Ok(())
}
