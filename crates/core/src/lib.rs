// game-tool-core: 核心数据模型与通用工具
//!
//! 提供游戏存档编辑器所需的所有基础能力：
//! - 核心数据类型（存档字段、游戏状态、游戏信息）
//! - 格式无关的存档读写接口
//! - 游戏引擎自动检测
//! - TCP 行协议通信
//! - LZ-String / Base64 编码工具
//! - 存档备份管理
//! - 统一错误类型

pub mod backup;
pub mod base64;
pub mod config;
pub mod detector;
pub mod error;
pub mod integrity;
pub mod lzstring;
pub mod net;
pub mod types;

pub use error::GameToolError;
pub use types::*;
