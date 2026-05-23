//! 内存引擎模块。
//!
//! 提供进程内存的扫描、读取和写入能力，支持通过内存地址直接修改游戏数据。
//!
//! # 核心组件
//!
//! - `bridge`: 统一内存桥接，整合扫描、读写与跟踪功能
//! - `module`: 目标进程已加载模块枚举
//! - `process`: 进程列表枚举与进程句柄管理
//! - `region`: Windows 内存区域枚举与内存读写操作
//! - `scanner`: 内存值扫描器，支持首次全内存扫描和多轮条件过滤

/// 统一内存桥接接口
pub mod bridge;

/// 进程模块信息
pub mod module;

/// 进程信息和句柄管理
pub mod process;

/// 内存区域枚举与读写
pub mod region;

/// 内存值扫描器
pub mod scanner;

/// 导出：统一内存桥接
pub use bridge::UniversalMemoryBridge;

/// 导出：进程信息（PID、名称）
pub use process::ProcessInfo;

/// 导出：内存区域描述结构体
pub use region::MemoryRegion;

/// 导出：内存扫描器、扫描模式枚举、值类型枚举
pub use scanner::{MemoryScanner, NextScanMode, ValueType};
