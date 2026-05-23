//! Unreal Engine 支持。
//!
//! 提供 Unreal Engine GVAS 存档格式的只读解析和属性修改功能。
//! 支持解析 UE 二进制存档头部、提取属性键值对（整数/浮点数/字符串/布尔），
//! 以及对存档字段进行修改后重新序列化保存。
//!
//! # 格式说明
//!
//! GVAS（Generic Value Archive Storage）是 Unreal Engine 使用的二进制存档格式。

/// Unreal Engine GVAS 存档格式模块
pub mod format;
