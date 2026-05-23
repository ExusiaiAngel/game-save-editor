//! 内存值扫描器。
//!
//! 提供基于内存模式匹配的数值扫描功能。支持多种数值类型
//!（32/64 位整数、单/双精度浮点数、定长字符串和字节序列）的
//! 首次全内存扫描和连续条件过滤扫描。
//!
//! # 工作流程
//!
//! 1. 调用 `first_scan` 搜索所有与目标值匹配的内存地址
//! 2. 修改游戏中的值（如打怪后金币减少）
//! 3. 调用 `next_scan` 使用过滤模式（增大/减小/未变/已变）缩小范围
//! 4. 重复步骤 2-3 直至候选地址数量收敛到可管理的范围
//!
//! 基于 Rayon 实现并行扫描以提高性能。

use crate::region::{enumerate_regions, read_memory};
use game_tool_core::{FieldScanSeed, ModifiableField, ScannedAddr};
use rayon::prelude::*;
use std::ffi::c_void;
use serde_json::Value;
use std::collections::HashMap;

/// 内存值类型枚举。
///
/// 定义了扫描器支持的所有数据类型及其内存表示。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ValueType {
    /// 32 位有符号整数（小端序）
    I32,
    /// 64 位有符号整数（小端序）
    I64,
    /// 32 位单精度浮点数（IEEE 754，小端序）
    F32,
    /// 64 位双精度浮点数（IEEE 754，小端序）
    F64,
    /// 定长 UTF-8 字符串（参数为最大字节数）
    String(usize),
    /// 定长字节序列（参数为字节数）
    Bytes(usize),
}

/// 连续扫描过滤模式。
///
/// 定义第二次及后续扫描时如何与快照数据比较以过滤候选地址。
#[derive(Debug, Clone)]
pub enum NextScanMode {
    /// 精确匹配指定的值
    Exact(Value),
    /// 值相比上次快照增大
    Increased,
    /// 值相比上次快照减小
    Decreased,
    /// 值相比上次快照未发生变化
    Unchanged,
    /// 值相比上次快照发生了变化
    Changed,
}

/// 内存扫描器。
///
/// 管理候选地址列表和快照数据，支持首次全内存扫描和后续的条件过滤扫描。
/// 每次扫描后更新快照，用于下一次的"已变/未变/增大/减小"模式比较。
pub struct MemoryScanner {
    /// 当前的候选地址列表
    candidates: Vec<ScannedAddr>,
    /// 各候选地址的上次扫描快照数据（地址 → 原始字节）
    snapshot: HashMap<usize, Vec<u8>>,
    /// 当前扫描的值类型
    value_type: ValueType,
    /// 当前扫描的值大小（字节数）
    value_size: usize,
}

impl MemoryScanner {
    /// 创建新的扫描器实例，初始状态下无候选地址。
    pub fn new() -> Self {
        Self {
            candidates: Vec::new(),
            snapshot: HashMap::new(),
            value_type: ValueType::I32,
            value_size: 4,
        }
    }

    /// 返回当前候选地址列表的引用。
    pub fn candidates(&self) -> &[ScannedAddr] {
        &self.candidates
    }

    /// 返回当前候选地址的数量。
    pub fn candidates_count(&self) -> usize {
        self.candidates.len()
    }

    /// 根据值类型返回对应的内存占用大小（字节数）。
    fn value_size_for_type(vt: ValueType) -> usize {
        match vt {
            ValueType::I32 => 4,
            ValueType::I64 => 8,
            ValueType::F32 => 4,
            ValueType::F64 => 8,
            ValueType::String(max) => max,
            ValueType::Bytes(size) => size,
        }
    }

    /// 执行首次全内存扫描。
    ///
    /// 遍历目标进程的所有可写内存区域，搜索与目标值字节表示匹配的地址。
    /// 使用 Rayon 实现区域级别的并行扫描。
    ///
    /// - `handle`: 目标进程句柄
    /// - `value`: 待搜索的目标值
    /// - `value_type`: 值类型（决定了匹配的字节宽度和比较方式）
    pub fn first_scan(&mut self, handle: *mut c_void, value: &Value, value_type: ValueType) -> Vec<ScannedAddr> {
        self.value_type = value_type;
        self.value_size = Self::value_size_for_type(value_type);
        self.candidates.clear();
        self.snapshot.clear();

        // 枚举所有内存区域
        let regions = enumerate_regions(handle);
        let size = self.value_size;
        let target = value_to_bytes(value, value_type);
        // 将句柄转为 usize 以便跨线程传递
        let h = handle as usize;

        // 并行扫描每个内存区域
        let scan_results: Vec<ScannedAddr> = regions
            .par_iter()
            .filter_map(|region| {
                // 只扫描可写区域（游戏数据通常位于可写内存）
                if !region.writable { return None; }
                let handle = h as *mut std::ffi::c_void;
                let mut local_results = Vec::new();
                let mut offset = 0usize;
                // 按步长扫描整个区域
                while offset + size <= region.size {
                    let addr = region.base_addr + offset;
                    let bytes = read_memory(handle, addr, size);
                    if let Some(ref data) = bytes {
                        if let Some(ref tgt) = target {
                            // 字节完全匹配则加入结果
                            if data == tgt {
                                local_results.push(ScannedAddr {
                                    address: addr,
                                    current_value: bytes_to_value(data, value_type),
                                });
                            }
                        }
                    }
                    offset += size;
                }
                Some(local_results)
            })
            .flatten()
            .collect();

        // 将首次扫描结果加入候选列表
        for addr in &scan_results {
            self.candidates.push(addr.clone());
        }
        scan_results
    }

    /// 执行连续扫描（二次过滤）。
    ///
    /// 在首次扫描或上次扫描的候选结果基础上，根据指定的过滤模式进一步筛选。
    /// 过滤前会读取各候选地址的当前值并与快照数据进行比较。
    ///
    /// - `handle`: 目标进程句柄
    /// - `mode`: 过滤模式（精确值/增大/减小/未变/已变）
    pub fn next_scan(&mut self, handle: *mut c_void, mode: &NextScanMode) -> Vec<ScannedAddr> {
        let size = self.value_size;
        let mut new_candidates = Vec::new();
        let mut new_snapshot = HashMap::new();

        // 遍历所有候选地址，逐条判断是否满足过滤条件
        for candidate in &self.candidates {
            let addr = candidate.address;
            let bytes = read_memory(handle, addr, size);
            let matches = match (bytes.clone(), mode) {
                (Some(ref data), NextScanMode::Exact(val)) => {
                    value_to_bytes(val, self.value_type).map_or(false, |target| data == &target)
                }
                (Some(ref data), NextScanMode::Increased) => {
                    if let Some(old_bytes) = self.snapshot.get(&addr) {
                        compare_values(data, old_bytes, self.value_type) == Some(std::cmp::Ordering::Greater)
                    } else {
                        false
                    }
                }
                (Some(ref data), NextScanMode::Decreased) => {
                    if let Some(old_bytes) = self.snapshot.get(&addr) {
                        compare_values(data, old_bytes, self.value_type) == Some(std::cmp::Ordering::Less)
                    } else {
                        false
                    }
                }
                (Some(ref data), NextScanMode::Unchanged) => {
                    if let Some(old_bytes) = self.snapshot.get(&addr) {
                        data == old_bytes
                    } else {
                        false
                    }
                }
                (Some(ref data), NextScanMode::Changed) => {
                    if let Some(old_bytes) = self.snapshot.get(&addr) {
                        data != old_bytes
                    } else {
                        false
                    }
                }
                (None, _) => false,
            };

                    if matches {
                if let Some(ref data) = bytes {
                    new_snapshot.insert(addr, data.clone());
                    let current_val = bytes_to_value(data, self.value_type);
                    new_candidates.push(ScannedAddr {
                        address: addr,
                        current_value: current_val,
                    });
                }
            }
        }

        // 更新候选列表和快照数据
        self.candidates = new_candidates;
        self.snapshot = new_snapshot;
        self.candidates.clone()
    }

    /// 根据存档字段值生成内存扫描种子。
    ///
    /// 对给定的每个存档字段，扫描目标进程内存中所有与该字段保存值匹配的地址，
    /// 将每个字段的匹配地址列表作为扫描种子返回。
    /// 这些种子可用于后续的交叉验证以精确定位字段对应的内存地址。
    ///
    /// - `handle`: 目标进程句柄
    /// - `fields`: 存档字段列表
    pub fn seed_from_save(
        &mut self,
        handle: *mut c_void,
        fields: &[ModifiableField],
    ) -> Vec<FieldScanSeed> {
        let mut seeds = Vec::new();
        for field in fields {
            // 根据字段值推断合适的内存值类型
            let field_type = infer_value_type(&field.save_value);
            let size = Self::value_size_for_type(field_type);
            self.value_type = field_type;
            self.value_size = size;
            self.candidates.clear();
            self.snapshot.clear();

            // 枚举可写内存区域，查找与该字段值匹配的所有地址
            let regions = enumerate_regions(handle);
            let target_bytes = value_to_bytes(&field.save_value, field_type);
            let target = match target_bytes {
                Some(ref t) => t.clone(),
                None => continue,
            };

            let mut candidates = Vec::new();
            for region in &regions {
                if region.writable {
                    let mut offset = 0usize;
                    while offset + size <= region.size {
                        let addr = region.base_addr + offset;
                        if let Some(data) = read_memory(handle, addr, size) {
                            if data == target {
                                candidates.push(addr);
                            }
                        }
                        offset += size;
                    }
                }
            }

            // 构建扫描种子
            seeds.push(FieldScanSeed {
                field_id: field.field_id.clone(),
                display_name: field.display_name.clone(),
                save_value: field.save_value.clone(),
                candidates,
                confirmed_addrs: Vec::new(),
                confidence: 0.0,
            });
        }
        seeds
    }

    /// 交叉验证扫描种子。
    ///
    /// 利用修改前后两组存档字段值，对每个种子的候选地址进行验证：
    /// 读取各候选地址的当前值，检查哪些地址的值已按预期从旧值变为新值。
    /// 通过验证的地址加入确认列表，并根据结果计算置信度。
    ///
    /// - `handle`: 目标进程句柄
    /// - `old_seeds`: 修改前的扫描种子（包含候选地址列表）
    /// - `new_fields`: 修改后的存档字段（提供新值用于比对）
    pub fn cross_validate(
        &self,
        handle: *mut c_void,
        old_seeds: &[FieldScanSeed],
        new_fields: &[ModifiableField],
    ) -> Vec<FieldScanSeed> {
        let mut results = Vec::new();
        for seed in old_seeds {
            // 查找该字段在修改后的新值
            let new_field_val = new_fields
                .iter()
                .find(|f| f.field_id == seed.field_id)
                .map(|f| &f.save_value);

            // 如果字段值已修改，验证候选地址是否已更新为新值
            let confirmed = match new_field_val {
                Some(new_val) if *new_val != seed.save_value => {
                    let field_type = infer_value_type(new_val);
                    let size = Self::value_size_for_type(field_type);
                    let target_bytes = value_to_bytes(new_val, field_type);

                    match target_bytes {
                        Some(ref target) => {
                            // 读取每个候选地址的当前值，检查是否为新值
                            let mut matching = Vec::new();
                            for addr in &seed.candidates {
                                if let Some(data) = read_memory(handle, *addr, size) {
                                    if data == *target {
                                        matching.push(*addr);
                                    }
                                }
                            }
                            matching
                        }
                        None => Vec::new(),
                    }
                }
                _ => seed.candidates.clone(),
            };

            // 根据确认地址的数量和占比计算置信度
            let confidence = if confirmed.is_empty() {
                0.0
            } else if confirmed.len() == 1 {
                0.95
            } else if confirmed.len() <= 5 {
                0.7
            } else if confirmed.len() as f64 / (seed.candidates.len().max(1) as f64) < 0.3 {
                0.5
            } else {
                0.2
            };

            results.push(FieldScanSeed {
                confirmed_addrs: confirmed,
                confidence,
                ..seed.clone()
            });
        }
        results
    }
}

/// 从 JSON Value 值推断最适合的内存扫描值类型。
///
/// 推断规则：
/// - 布尔值 → I32（内存中通常以 32 位整数形式存储）
/// - 整数型 Number → I32（值在 ±2^31 内）或 I64（超出范围）
/// - 浮点型 Number → I64（整数化存储）或 I32（小整数）
/// - 字符串 → String（长度至少 16 字节）
fn infer_value_type(value: &Value) -> ValueType {
    match value {
        Value::Number(n) => {
            if n.is_f64() {
                let f = n.as_f64().unwrap_or(0.0);
                // 有小数部分或超出 32 位范围的用 I64 表示
                if f.fract() != 0.0 || f.abs() > i32::MAX as f64 {
                    ValueType::I64
                } else {
                    ValueType::I32
                }
            } else if n.as_i64().map_or(false, |v| v > i32::MAX as i64 || v < i32::MIN as i64) {
                ValueType::I64
            } else {
                ValueType::I32
            }
        }
        Value::Bool(_) => ValueType::I32,
        Value::String(s) => ValueType::String(s.len().max(16)),
        _ => ValueType::I32,
    }
}

/// 将 JSON Value 按指定值类型转换为内存中的字节序列（小端序）。
fn value_to_bytes(value: &Value, vt: ValueType) -> Option<Vec<u8>> {
    match vt {
        ValueType::I32 => {
            let v = value.as_i64().unwrap_or(0) as i32;
            Some(v.to_le_bytes().to_vec())
        }
        ValueType::I64 => {
            let v = value.as_i64().unwrap_or(0);
            Some(v.to_le_bytes().to_vec())
        }
        ValueType::F32 => {
            let v = value.as_f64().unwrap_or(0.0) as f32;
            Some(v.to_le_bytes().to_vec())
        }
        ValueType::F64 => {
            let v = value.as_f64().unwrap_or(0.0);
            Some(v.to_le_bytes().to_vec())
        }
        ValueType::String(max) => {
            let s = value.as_str().unwrap_or("");
            let mut bytes = s.as_bytes().to_vec();
            // 补齐或截断到指定长度（使用 \0 填充）
            bytes.resize(max, 0);
            Some(bytes)
        }
        ValueType::Bytes(size) => {
            let data = value.as_str().map(|s| s.as_bytes().to_vec());
            data.map(|mut d| {
                d.resize(size, 0);
                d
            })
        }
    }
}

/// 将内存中的字节序列按值类型解析为 JSON Value。
fn bytes_to_value(data: &[u8], vt: ValueType) -> Value {
    match vt {
        ValueType::I32 => {
            let v = i32::from_le_bytes(data[..4].try_into().unwrap_or([0; 4]));
            Value::Number(v.into())
        }
        ValueType::I64 => {
            let v = i64::from_le_bytes(data[..8].try_into().unwrap_or([0; 8]));
            Value::Number(v.into())
        }
        ValueType::F32 => {
            let v = f32::from_le_bytes(data[..4].try_into().unwrap_or([0; 4]));
            serde_json::Number::from_f64(v as f64).map_or(Value::Null, Value::Number)
        }
        ValueType::F64 => {
            let v = f64::from_le_bytes(data[..8].try_into().unwrap_or([0; 8]));
            serde_json::Number::from_f64(v).map_or(Value::Null, Value::Number)
        }
        ValueType::String(_) => {
            // 去除尾部的 \0 填充字符
            let s = String::from_utf8_lossy(data)
                .trim_end_matches('\0')
                .to_string();
            Value::String(s)
        }
        ValueType::Bytes(_) => Value::String(base64_encode(data)),
    }
}

/// 比较两个字节序列的数值大小（仅支持整数类型 I32/I64）。
///
/// 对于数值类型（I32/I64）将字节解析为有符号整数后比较大小，
/// 浮点数和字符串类型不支持数值比较，返回 None。
fn compare_values(a: &[u8], b: &[u8], vt: ValueType) -> Option<std::cmp::Ordering> {
    match vt {
        ValueType::I32 => {
            let va = i32::from_le_bytes(a[..4].try_into().ok()?);
            let vb = i32::from_le_bytes(b[..4].try_into().ok()?);
            Some(va.cmp(&vb))
        }
        ValueType::I64 => {
            let va = i64::from_le_bytes(a[..8].try_into().ok()?);
            let vb = i64::from_le_bytes(b[..8].try_into().ok()?);
            Some(va.cmp(&vb))
        }
        _ => None,
    }
}

/// 对字节数据进行 Base64 编码（纯 Rust 实现，无外部依赖）。
///
/// 用于将二进制内存数据以可读的字符串形式返回给上层。
/// 遵循标准 Base64 编码表（RFC 4648）。
fn base64_encode(data: &[u8]) -> String {
    // Base64 标准编码表（A-Z, a-z, 0-9, +, /）
    const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::new();
    // 每 3 个字节编码为 4 个 Base64 字符
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = chunk.get(1).copied().unwrap_or(0) as u32;
        let b2 = chunk.get(2).copied().unwrap_or(0) as u32;
        let triple = (b0 << 16) | (b1 << 8) | b2;
        result.push(TABLE[((triple >> 18) & 0x3F) as usize] as char);
        result.push(TABLE[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            result.push(TABLE[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            // 输入不足 3 字节时用 = 填充
            result.push('=');
        }
        if chunk.len() > 2 {
            result.push(TABLE[(triple & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
    }
    result
}
