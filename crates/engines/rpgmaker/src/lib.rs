//! RPG Maker MV/MZ 引擎支持。
//!
//! 提供 RPG Maker 存档（.rpgsave/.rmmzsave）的完整读写、
//! 游戏数据目录扫描、JSONEx 特殊格式解析以及 TCP 桥接实时修改功能。

/// RPG Maker 存档格式模块：处理 .rpgsave / .rmmzsave 文件的
/// LZ-String 压缩、Base64 编解码、JSON 解析与字段修改。
pub mod format;

/// JSONEx 扩展格式工具：解析 RPG Maker 使用的 `@a`（稀疏数组）、
/// `@c`（类型引用）等元键，提供数组展开和格式规范化功能。
pub mod jsonex;

/// 游戏目录和存档数据扫描器：遍历游戏配置文件（System.json 等）
/// 和存档数据，汇总所有可修改字段（金币、开关、变量、角色、物品）。
pub mod scanner;

/// TCP 桥接（文本命令协议）：通过 TCP 连接与 NW.js 游戏进程通信，
/// 使用纯文本命令格式（get_state、set_gold 等）实现实时读写。
pub mod tcp;
