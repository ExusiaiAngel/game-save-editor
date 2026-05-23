//! 进程内存区域枚举与读写操作。
//!
//! 提供跨平台的内存区域枚举和内存读写函数。
//! Windows 平台基于 `VirtualQueryEx` / `ReadProcessMemory` / `WriteProcessMemory` 系统 API，
//! 非 Windows 平台提供空实现（桩函数）以保持跨平台兼容。

/// 描述进程虚拟地址空间中的一个内存区域。
#[derive(Debug, Clone)]
pub struct MemoryRegion {
    /// 区域起始基址（虚拟地址）
    pub base_addr: usize,
    /// 区域大小（字节）
    pub size: usize,
    /// 是否可读
    pub readable: bool,
    /// 是否可写
    pub writable: bool,
}

/// Windows 平台实现：使用 VirtualQueryEx / ReadProcessMemory / WriteProcessMemory API。
#[cfg(windows)]
mod platform {
    use super::MemoryRegion;

    /// 最小有效内存区域大小（64KB），小于此阈值的区域被忽略
    const MIN_REGION_SIZE: usize = 64 * 1024;

    // Windows API 外部函数声明
    extern "system" {
        /// 查询目标进程虚拟地址空间中指定地址的内存区域信息
        fn VirtualQueryEx(
            hProcess: *mut std::ffi::c_void,
            lpAddress: *const std::ffi::c_void,
            lpBuffer: *mut MEMORY_BASIC_INFORMATION,
            dwLength: usize,
        ) -> usize;

        /// 从目标进程的指定地址读取内存数据
        fn ReadProcessMemory(
            hProcess: *mut std::ffi::c_void,
            lpBaseAddress: *const std::ffi::c_void,
            lpBuffer: *mut std::ffi::c_void,
            nSize: usize,
            lpNumberOfBytesRead: *mut usize,
        ) -> i32;

        /// 向目标进程的指定地址写入内存数据
        fn WriteProcessMemory(
            hProcess: *mut std::ffi::c_void,
            lpBaseAddress: *mut std::ffi::c_void,
            lpBuffer: *const std::ffi::c_void,
            nSize: usize,
            lpNumberOfBytesWritten: *mut usize,
        ) -> i32;
    }

    /// Windows 内存基本信息结构体（对应 Win32 API 的 MEMORY_BASIC_INFORMATION）
    #[repr(C)]
    struct MEMORY_BASIC_INFORMATION {
        /// 区域基址
        base_address: *mut std::ffi::c_void,
        /// 分配基址（用于释放整个分配区域）
        allocation_base: *mut std::ffi::c_void,
        /// 初始分配时的内存保护属性
        allocation_protect: u32,
        /// 分区 ID
        partition_id: u16,
        /// 区域大小（字节）
        region_size: usize,
        /// 内存状态（MEM_COMMIT / MEM_RESERVE / MEM_FREE）
        state: u32,
        /// 当前内存保护属性
        protect: u32,
        /// 内存类型（MEM_IMAGE / MEM_MAPPED / MEM_PRIVATE）
        type_: u32,
    }

    /// 内存已提交（物理内存已分配）
    const MEM_COMMIT: u32 = 0x1000;
    /// 只读访问
    const PAGE_READONLY: u32 = 0x02;
    /// 读写访问
    const PAGE_READWRITE: u32 = 0x04;
    /// 写时复制
    const PAGE_WRITECOPY: u32 = 0x08;
    /// 执行 + 读取
    const PAGE_EXECUTE_READ: u32 = 0x20;
    /// 执行 + 读写
    const PAGE_EXECUTE_READWRITE: u32 = 0x40;
    /// 执行 + 写时复制
    const PAGE_EXECUTE_WRITECOPY: u32 = 0x80;

    /// 枚举目标进程的所有可读内存区域。
    ///
    /// 使用 `VirtualQueryEx` 从地址 0 开始遍历整个虚拟地址空间，
    /// 筛选出已提交（MEM_COMMIT）、大小达到最小阈值（64KB）的可读内存区域。
    /// 返回的区域列表可用于后续的内存扫描和读取操作。
    pub fn enumerate_regions(handle: *mut std::ffi::c_void) -> Vec<MemoryRegion> {
        let mut regions = Vec::new();
        unsafe {
            // 从地址 0 开始遍历进程虚拟地址空间
            let mut address: *mut std::ffi::c_void = std::ptr::null_mut();
            loop {
                let mut mbi: MEMORY_BASIC_INFORMATION = std::mem::zeroed();
                let result = VirtualQueryEx(
                    handle,
                    address,
                    &mut mbi,
                    std::mem::size_of::<MEMORY_BASIC_INFORMATION>(),
                );
                // VirtualQueryEx 返回 0 表示已遍历完所有区域
                if result == 0 {
                    break;
                }
                let region_size = mbi.region_size;
                let protect = mbi.protect;
                let state = mbi.state;

                // 判断是否可读：已提交且具有读取权限
                let readable = (state & MEM_COMMIT) != 0
                    && (protect & PAGE_READONLY != 0
                        || protect & PAGE_READWRITE != 0
                        || protect & PAGE_WRITECOPY != 0
                        || protect & PAGE_EXECUTE_READ != 0
                        || protect & PAGE_EXECUTE_READWRITE != 0
                        || protect & PAGE_EXECUTE_WRITECOPY != 0);
                // 判断是否可写：已提交且具有写入权限
                let writable = (state & MEM_COMMIT) != 0
                    && (protect & PAGE_READWRITE != 0
                        || protect & PAGE_WRITECOPY != 0
                        || protect & PAGE_EXECUTE_READWRITE != 0
                        || protect & PAGE_EXECUTE_WRITECOPY != 0);

                // 只保留可读且大小达最小阈值的区域
                if readable && region_size >= MIN_REGION_SIZE {
                    regions.push(MemoryRegion {
                        base_addr: address as usize,
                        size: region_size,
                        readable,
                        writable,
                    });
                }

                // 移动到下一个内存区域
                let addr_val = address as usize;
                match addr_val.checked_add(region_size) {
                    Some(next) if next > addr_val => {
                        address = next as *mut std::ffi::c_void;
                    }
                    _ => break,
                }
            }
        }
        regions
    }

    /// 从目标进程的指定地址读取指定字节数的内存数据。
    ///
    /// 使用 `ReadProcessMemory` API 读取。读取失败（如地址无效）返回 `None`。
    pub fn read_memory(handle: *mut std::ffi::c_void, address: usize, size: usize) -> Option<Vec<u8>> {
        let mut buf = vec![0u8; size];
        unsafe {
            let mut bytes_read: usize = 0;
            let result = ReadProcessMemory(
                handle,
                address as *const std::ffi::c_void,
                buf.as_mut_ptr() as *mut std::ffi::c_void,
                size,
                &mut bytes_read,
            );
            if result == 0 {
                None
            } else {
                buf.truncate(bytes_read);
                Some(buf)
            }
        }
    }

    /// 向目标进程的指定地址写入数据。
    ///
    /// 使用 `WriteProcessMemory` API 写入。
    /// 返回 `true` 表示写入成功且写入的字节数与目标长度一致。
    pub fn write_memory(handle: *mut std::ffi::c_void, address: usize, data: &[u8]) -> bool {
        unsafe {
            let mut bytes_written: usize = 0;
            let result = WriteProcessMemory(
                handle,
                address as *mut std::ffi::c_void,
                data.as_ptr() as *const std::ffi::c_void,
                data.len(),
                &mut bytes_written,
            );
            result != 0 && bytes_written == data.len()
        }
    }

    /// 从指定地址读取 4 字节有符号整数（i32，小端序）。
    pub fn read_i32(handle: *mut std::ffi::c_void, address: usize) -> Option<i32> {
        read_memory(handle, address, 4).map(|bytes| i32::from_le_bytes(bytes[..4].try_into().unwrap()))
    }

    /// 向指定地址写入 4 字节有符号整数（i32，小端序）。
    pub fn write_i32(handle: *mut std::ffi::c_void, address: usize, value: i32) -> bool {
        write_memory(handle, address, &value.to_le_bytes())
    }

    /// 从指定地址读取 4 字节单精度浮点数（f32，小端序）。
    pub fn read_f32(handle: *mut std::ffi::c_void, address: usize) -> Option<f32> {
        read_memory(handle, address, 4).map(|bytes| f32::from_le_bytes(bytes[..4].try_into().unwrap()))
    }

    /// 向指定地址写入 4 字节单精度浮点数（f32，小端序）。
    pub fn write_f32(handle: *mut std::ffi::c_void, address: usize, value: f32) -> bool {
        write_memory(handle, address, &value.to_le_bytes())
    }

    /// 从指定地址读取 8 字节有符号整数（i64，小端序）。
    pub fn read_i64(handle: *mut std::ffi::c_void, address: usize) -> Option<i64> {
        read_memory(handle, address, 8).map(|bytes| i64::from_le_bytes(bytes[..8].try_into().unwrap()))
    }

    /// 向指定地址写入 8 字节有符号整数（i64，小端序）。
    pub fn write_i64(handle: *mut std::ffi::c_void, address: usize, value: i64) -> bool {
        write_memory(handle, address, &value.to_le_bytes())
    }

    /// 从指定地址读取 8 字节双精度浮点数（f64，小端序）。
    pub fn read_f64(handle: *mut std::ffi::c_void, address: usize) -> Option<f64> {
        read_memory(handle, address, 8).map(|bytes| f64::from_le_bytes(bytes[..8].try_into().unwrap()))
    }

    /// 向指定地址写入 8 字节双精度浮点数（f64，小端序）。
    pub fn write_f64(handle: *mut std::ffi::c_void, address: usize, value: f64) -> bool {
        write_memory(handle, address, &value.to_le_bytes())
    }

    /// 从指定地址读取指定最大长度的字符串。
    ///
    /// 读取 `max_len` 字节后用 `String::from_utf8_lossy` 解码，
    /// 并去除尾部的 `\0` 填充字符。
    pub fn read_string(handle: *mut std::ffi::c_void, address: usize, max_len: usize) -> Option<String> {
        read_memory(handle, address, max_len).map(|bytes| {
            String::from_utf8_lossy(&bytes).trim_end_matches('\0').to_string()
        })
    }
}

/// 非 Windows 平台的空实现（桩函数）。
///
/// 所有操作返回空结果或 false，确保代码在非 Windows 环境能编译。
#[cfg(not(windows))]
mod platform {
    use super::MemoryRegion;
    pub fn enumerate_regions(_handle: *mut std::ffi::c_void) -> Vec<MemoryRegion> { Vec::new() }
    pub fn read_memory(_handle: *mut std::ffi::c_void, _address: usize, _size: usize) -> Option<Vec<u8>> { None }
    pub fn write_memory(_handle: *mut std::ffi::c_void, _address: usize, _data: &[u8]) -> bool { false }
    pub fn read_i32(_handle: *mut std::ffi::c_void, _address: usize) -> Option<i32> { None }
    pub fn write_i32(_handle: *mut std::ffi::c_void, _address: usize, _value: i32) -> bool { false }
    pub fn read_f32(_handle: *mut std::ffi::c_void, _address: usize) -> Option<f32> { None }
    pub fn write_f32(_handle: *mut std::ffi::c_void, _address: usize, _value: f32) -> bool { false }
    pub fn read_i64(_handle: *mut std::ffi::c_void, _address: usize) -> Option<i64> { None }
    pub fn write_f64(_handle: *mut std::ffi::c_void, _address: usize, _value: f64) -> bool { false }
    pub fn read_f64(_handle: *mut std::ffi::c_void, _address: usize) -> Option<f64> { None }
    pub fn write_i64(_handle: *mut std::ffi::c_void, _address: usize, _value: i64) -> bool { false }
    pub fn write_f64(_handle: *mut std::ffi::c_void, _address: usize, _value: f64) -> bool { false }
    pub fn read_string(_handle: *mut std::ffi::c_void, _address: usize, _max_len: usize) -> Option<String> { None }
}

/// 导出平台特定实现
pub use platform::*;
