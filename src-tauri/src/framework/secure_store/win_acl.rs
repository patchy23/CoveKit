//! Windows 专用：把文件 DACL 收紧为「仅当前用户」
//!
//! 用于降级主密钥文件（`vault-master.key` / `credentials-master.key`）：这类文件与密文同目录，
//! 至少不应因为继承来的宽权限而被同机其他账户直接读取。
//!
//! 边界说明：**这不提供抗离线解密能力**——密钥与密文放在一起被整目录拷走时，攻击者仍可解密；
//! 本函数只减少「同机其它账户顺手读到密钥文件」这一层暴露。

use std::os::windows::ffi::OsStrExt;
use std::path::Path;

use windows_sys::Win32::Foundation::{CloseHandle, LocalFree, ERROR_SUCCESS, HANDLE};
use windows_sys::Win32::Security::Authorization::{
    GetNamedSecurityInfoW, SetEntriesInAclW, SetNamedSecurityInfoW, EXPLICIT_ACCESS_W,
    GRANT_ACCESS, NO_MULTIPLE_TRUSTEE, SE_FILE_OBJECT, TRUSTEE_IS_SID, TRUSTEE_IS_USER, TRUSTEE_W,
};
use windows_sys::Win32::Security::{
    GetTokenInformation, TokenUser, ACL, DACL_SECURITY_INFORMATION,
    PROTECTED_DACL_SECURITY_INFORMATION, PSID, TOKEN_QUERY, TOKEN_USER,
};
use windows_sys::Win32::Storage::FileSystem::FILE_ALL_ACCESS;
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

/// 取当前进程令牌的用户 SID 缓冲（缓冲需保持存活到调用结束，SID 指针指向其中）
fn current_user_token_buffer() -> Result<Vec<u8>, String> {
    // SAFETY: 全部参数按 Win32 约定传入；token 句柄在函数内创建、在本函数内关闭，
    // 返回的缓冲由调用方持有，缓冲内 SID 指针生命周期不超过该缓冲。
    unsafe {
        let mut token: HANDLE = std::ptr::null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return Err("打开当前进程令牌失败（无法收紧密钥文件权限）".into());
        }
        let mut needed: u32 = 0;
        // 第一次调用只取所需长度，失败属预期
        GetTokenInformation(token, TokenUser, std::ptr::null_mut(), 0, &mut needed);
        if needed == 0 {
            CloseHandle(token);
            return Err("查询令牌用户信息长度失败（无法收紧密钥文件权限）".into());
        }
        let mut buffer = vec![0u8; needed as usize];
        let ok = GetTokenInformation(
            token,
            TokenUser,
            buffer.as_mut_ptr() as *mut core::ffi::c_void,
            needed,
            &mut needed,
        );
        CloseHandle(token);
        if ok == 0 {
            return Err("读取令牌用户信息失败（无法收紧密钥文件权限）".into());
        }
        Ok(buffer)
    }
}

/// 当前用户 SID 指针（生命周期绑定提供的令牌缓冲）
fn user_sid(buffer: &[u8]) -> PSID {
    // SAFETY: buffer 由 GetTokenInformation(TokenUser) 填充且长度足够容纳 TOKEN_USER，
    // 按 Win32 契约其首字段即 TOKEN_USER。
    unsafe { (*(buffer.as_ptr() as *const TOKEN_USER)).User.Sid }
}

/// 把文件 DACL 替换为「仅当前用户完全控制」，并断开继承（保护性 DACL）
pub(crate) fn restrict_to_current_user(path: &Path) -> Result<(), String> {
    let buffer = current_user_token_buffer()?;
    let sid = user_sid(&buffer);
    let mut wide: Vec<u16> = path.as_os_str().encode_wide().collect();
    wide.push(0);

    let entry = EXPLICIT_ACCESS_W {
        grfAccessPermissions: FILE_ALL_ACCESS,
        grfAccessMode: GRANT_ACCESS,
        grfInheritance: 0,
        Trustee: TRUSTEE_W {
            pMultipleTrustee: std::ptr::null_mut(),
            MultipleTrusteeOperation: NO_MULTIPLE_TRUSTEE,
            TrusteeForm: TRUSTEE_IS_SID,
            TrusteeType: TRUSTEE_IS_USER,
            ptstrName: sid as *mut u16,
        },
    };
    let mut acl: *mut ACL = std::ptr::null_mut();
    // SAFETY: entry 是一个合法的 EXPLICIT_ACCESS_W；oldacl 传空表示不合并；acl 由 API 分配，
    // 失败路径不会写出该指针，成功后立刻用 LocalFree 释放。
    let mut status = unsafe { SetEntriesInAclW(1, &entry, std::ptr::null(), &mut acl) };
    if status != ERROR_SUCCESS || acl.is_null() {
        return Err(format!("构造密钥文件访问控制表失败（错误码 {status}）"));
    }
    // SAFETY: wide 是以 NUL 结尾的合法宽字符串；acl 是上一步分配的合法 ACL；
    // owner/group/sacl 传空表示本次只改 DACL。
    status = unsafe {
        SetNamedSecurityInfoW(
            wide.as_ptr(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            acl,
            std::ptr::null(),
        )
    };
    // SAFETY: acl 由 SetEntriesInAclW 分配，按文档必须用 LocalFree 释放一次。
    unsafe {
        LocalFree(acl as *mut core::ffi::c_void);
    }
    if status != ERROR_SUCCESS {
        return Err(format!("设置密钥文件访问控制表失败（错误码 {status}）"));
    }
    match acl_ace_count(path)? {
        Some(count) if count >= 1 => Ok(()),
        Some(_) => Err("密钥文件访问控制表设置后为空（已放弃收紧）".into()),
        None => Ok(()),
    }
}

/// 读回文件的 DACL 并返回 ACE 数量（读不到返回 None，用于收紧后的自校验）
fn acl_ace_count(path: &Path) -> Result<Option<u32>, String> {
    let mut wide: Vec<u16> = path.as_os_str().encode_wide().collect();
    wide.push(0);
    let mut acl: *mut ACL = std::ptr::null_mut();
    let mut descriptor: *mut core::ffi::c_void = std::ptr::null_mut();
    // SAFETY: wide 为 NUL 结尾宽字符串；输出指针均在本函数内使用并在成功后由 LocalFree 释放描述符。
    let status = unsafe {
        GetNamedSecurityInfoW(
            wide.as_ptr(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut acl,
            std::ptr::null_mut(),
            &mut descriptor,
        )
    };
    if status != ERROR_SUCCESS || acl.is_null() {
        if !descriptor.is_null() {
            // SAFETY: descriptor 由 GetNamedSecurityInfoW 分配，仅释放一次。
            unsafe {
                LocalFree(descriptor);
            }
        }
        return Ok(None);
    }
    // SAFETY: ACL 由 API 返回且非空，AceCount 为结构体内的固定字段。
    let count = unsafe { (*acl).AceCount as u32 };
    // SAFETY: descriptor 由 GetNamedSecurityInfoW 分配，仅释放一次（acl 是其内部指针，不单独释放）。
    unsafe {
        LocalFree(descriptor);
    }
    Ok(Some(count))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// 临时目录（进程 id + 随机后缀唯一）
    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "secure-store-acl-{name}-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// 收紧后 DACL 只剩一条 ACE，且当前用户仍能正常读写改删
    #[test]
    fn acl_restricted_to_single_ace_and_file_still_usable() {
        let dir = temp_dir("single-ace");
        let path = dir.join("vault-master.key");
        std::fs::write(&path, [7u8; 32]).unwrap();

        restrict_to_current_user(&path).unwrap();
        assert_eq!(
            acl_ace_count(&path).unwrap(),
            Some(1),
            "DACL 应只剩当前用户一条 ACE"
        );

        // 收紧不能把自己锁在门外：读写改删都要照常
        assert_eq!(std::fs::read(&path).unwrap(), [7u8; 32].to_vec());
        std::fs::write(&path, [8u8; 32]).unwrap();
        std::fs::rename(&path, dir.join("renamed.key")).unwrap();
        std::fs::remove_file(dir.join("renamed.key")).unwrap();
        std::fs::remove_dir_all(&dir).ok();
    }

    /// 收紧是幂等的：重复调用不报错，仍是单条 ACE
    #[test]
    fn acl_restriction_is_idempotent() {
        let dir = temp_dir("idempotent");
        let path = dir.join("credentials-master.key");
        std::fs::write(&path, [1u8; 32]).unwrap();
        restrict_to_current_user(&path).unwrap();
        restrict_to_current_user(&path).unwrap();
        assert_eq!(acl_ace_count(&path).unwrap(), Some(1));
        std::fs::remove_dir_all(&dir).ok();
    }
}
