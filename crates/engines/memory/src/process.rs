//! 进程枚举与句柄管理。
//!
//! 提供系统进程列表的枚举和进程句柄的打开/关闭操作。
//! Windows 平台基于 Tool Help API（CreateToolhelp32Snapshot），
//! 非 Windows 平台提供空实现（桩函数）以保持跨平台兼容。

/// 进程信息结构体。
#[derive(Debug, Clone)]
pub struct ProcessInfo {
    /// 进程标识符（PID）
    pub pid: u32,
    /// 进程可执行文件名称
    pub name: String,
}

/// Windows 平台实现：使用 Tool Help API 枚举进程和打开句柄。
#[cfg(windows)]
mod platform {
    use super::ProcessInfo;
    use std::mem;
    use windows_sys::Win32::Foundation::*;
    use windows_sys::Win32::System::Diagnostics::ToolHelp::*;
    use windows_sys::Win32::System::Threading::*;

    /// 枚举系统当前所有运行中的进程。
    ///
    /// 使用 `CreateToolhelp32Snapshot` 获取进程快照，
    /// 遍历后返回包含 PID 和可执行文件名的列表。
    pub fn enumerate_processes() -> Vec<ProcessInfo> {
        let mut result = Vec::new();
        unsafe {
            // 创建进程快照
            let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
            if snapshot == INVALID_HANDLE_VALUE {
                return result;
            }
            // 遍历进程条目
            let mut entry: PROCESSENTRY32W = mem::zeroed();
            entry.dwSize = mem::size_of::<PROCESSENTRY32W>() as u32;
            if Process32FirstW(snapshot, &mut entry) == TRUE {
                loop {
                    let pid = entry.th32ProcessID;
                    let name = String::from_utf16_lossy(&entry.szExeFile)
                        .trim_end_matches('\0')
                        .to_string();
                    if !name.is_empty() {
                        result.push(ProcessInfo { pid, name });
                    }
                    if Process32NextW(snapshot, &mut entry) != TRUE {
                        break;
                    }
                }
            }
            CloseHandle(snapshot);
        }
        result
    }

    /// 打开指定 PID 的进程句柄。
    ///
    /// 请求的访问权限：`PROCESS_VM_READ | PROCESS_VM_WRITE | PROCESS_VM_OPERATION | PROCESS_QUERY_INFORMATION`。
    /// 返回的句柄用于后续的内存读写和查询操作。
    pub fn open_process_handle(pid: u32) -> Option<*mut std::ffi::c_void> {
        unsafe {
            let handle = OpenProcess(
                PROCESS_VM_READ | PROCESS_VM_WRITE | PROCESS_VM_OPERATION | PROCESS_QUERY_INFORMATION,
                0,
                pid,
            );
            if handle.is_null() {
                None
            } else {
                Some(handle)
            }
        }
    }

    /// 关闭通过 `open_process_handle` 打开的进程句柄。
    pub fn close_process_handle(handle: *mut std::ffi::c_void) {
        unsafe { CloseHandle(handle); }
    }
}

/// 非 Windows 平台的空实现（桩函数）。
///
/// 进程枚举返回空列表，句柄打开返回 None，确保代码可编译。
#[cfg(not(windows))]
mod platform {
    use super::ProcessInfo;
    use std::ptr;
    pub fn enumerate_processes() -> Vec<ProcessInfo> { Vec::new() }
    pub fn open_process_handle(_pid: u32) -> Option<*mut std::ffi::c_void> { None }
    pub fn close_process_handle(_handle: *mut std::ffi::c_void) {}
}

/// 导出平台特定实现
pub use platform::*;
