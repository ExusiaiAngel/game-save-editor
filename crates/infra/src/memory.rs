//! Windows 进程内存操作
//!
//! 提供 ReadProcessMemory / WriteProcessMemory 封装 + 进程枚举。

/// 进程信息
#[derive(Debug, Clone)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub exe_path: String,
}

/// 模块信息
#[derive(Debug, Clone)]
pub struct ModuleInfo {
    pub name: String,
    pub base_address: usize,
    pub size: usize,
}

/// 进程句柄
#[derive(Debug)]
pub struct ProcessHandle {
    pid: u32,
}

impl ProcessHandle {
    /// 通过进程名打开（取第一个匹配）
    pub fn open_by_name(_name: &str) -> Result<Self, std::io::Error> {
        todo!("P4 实施")
    }

    /// 通过 PID 打开
    pub fn open_by_pid(pid: u32) -> Result<Self, std::io::Error> {
        Ok(Self { pid })
    }

    pub fn read<T: Sized>(&self, _address: usize) -> Result<T, std::io::Error> {
        todo!("P4 实施")
    }

    pub fn write<T: Sized>(&self, _address: usize, _value: &T) -> Result<(), std::io::Error> {
        todo!("P4 实施")
    }

    pub fn read_bytes(&self, _address: usize, _len: usize) -> Result<Vec<u8>, std::io::Error> {
        todo!("P4 实施")
    }

    pub fn write_bytes(&self, _address: usize, _data: &[u8]) -> Result<(), std::io::Error> {
        todo!("P4 实施")
    }
}

pub fn enumerate_processes(_name_filter: &str) -> Result<Vec<ProcessInfo>, std::io::Error> {
    todo!("P6 实施")
}

pub fn enumerate_modules(_pid: u32) -> Result<Vec<ModuleInfo>, std::io::Error> {
    todo!("P6 实施")
}
