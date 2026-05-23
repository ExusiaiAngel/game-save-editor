//! 进程模块枚举。
//!
//! 提供目标进程中已加载 DLL/EXE 模块的查询功能。
//! Windows 平台基于 Tool Help API（Module32FirstW/Module32NextW），
//! 非 Windows 平台提供空实现。

/// 进程模块信息结构体。
#[derive(Debug, Clone)]
pub struct ModuleInfo {
    /// 模块名称（如 "UnityPlayer.dll"）
    pub name: String,
    /// 模块在目标进程中的加载基址
    pub base_addr: usize,
    /// 模块大小（字节）
    pub size: usize,
}

/// Windows 平台实现：使用 Tool Help API 枚举目标进程的已加载模块。
#[cfg(windows)]
mod platform {
    use super::ModuleInfo;
    use std::mem;
    use windows_sys::Win32::Foundation::*;
    use windows_sys::Win32::System::Diagnostics::ToolHelp::*;

    /// 获取指定进程的所有已加载模块列表。
    ///
    /// 使用 `CreateToolhelp32Snapshot` 配合 `TH32CS_SNAPMODULE` 标志
    /// 获取指定进程的模块快照，然后遍历每个模块条目。
    pub fn get_modules(pid: u32) -> Vec<ModuleInfo> {
        let mut result = Vec::new();
        unsafe {
            // 创建模块快照（同时获取 32 位和 64 位模块）
            let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPMODULE | TH32CS_SNAPMODULE32, pid);
            if snapshot == INVALID_HANDLE_VALUE {
                return result;
            }

            let mut entry: MODULEENTRY32W = mem::zeroed();
            entry.dwSize = mem::size_of::<MODULEENTRY32W>() as u32;

            // 遍历模块条目
            if Module32FirstW(snapshot, &mut entry) == TRUE {
                loop {
                    let name = String::from_utf16_lossy(&entry.szModule)
                        .trim_end_matches('\0')
                        .to_string();
                    let base = entry.modBaseAddr as usize;
                    let size = entry.modBaseSize as usize;

                    if !name.is_empty() {
                        result.push(ModuleInfo { name, base_addr: base, size });
                    }

                    if Module32NextW(snapshot, &mut entry) != TRUE {
                        break;
                    }
                }
            }

            CloseHandle(snapshot);
        }
        result
    }

    /// 获取指定进程的主模块（即 EXE 本身）。
    ///
    /// 主模块通常是模块列表中的第一个条目。
    pub fn get_main_module(pid: u32) -> Option<ModuleInfo> {
        let modules = get_modules(pid);
        modules.into_iter().next()
    }

    /// 根据模块名称查找指定进程中的模块。
    ///
    /// 名称匹配不区分大小写。
    pub fn find_module_by_name(pid: u32, name: &str) -> Option<ModuleInfo> {
        let lower = name.to_lowercase();
        get_modules(pid).into_iter().find(|m| m.name.to_lowercase() == lower)
    }
}

/// 非 Windows 平台的空实现（桩函数）。
///
/// 所有模块查询返回空结果，确保代码可编译。
#[cfg(not(windows))]
mod platform {
    use super::ModuleInfo;

    pub fn get_modules(_pid: u32) -> Vec<ModuleInfo> {
        Vec::new()
    }

    pub fn get_main_module(_pid: u32) -> Option<ModuleInfo> {
        None
    }

    pub fn find_module_by_name(_pid: u32, _name: &str) -> Option<ModuleInfo> {
        None
    }
}

/// 导出平台特定实现
pub use platform::*;
