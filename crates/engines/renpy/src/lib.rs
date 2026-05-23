//! Ren'Py 引擎支持。
//!
//! 提供 Ren'Py 存档（.save ZIP）的完整读写、元数据提取、
//! store 变量的扫描与修改，以及基于 TCP JSON 协议的游戏实时桥接功能。

/// TCP JSON 桥接模块：通过 TCP 连接与 Ren'Py 进程实时通信，
/// 实现 store 变量的读取、写入和任意 Python 代码执行。
pub mod bridge;

/// 存档格式模块：处理 .save ZIP 文件的解析、保存、字段扫描与修改，
/// 支持 Ren'Py store 变量的递归提取与回写。
pub mod format;
