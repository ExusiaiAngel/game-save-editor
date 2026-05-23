//! 统一内存桥接接口。
//!
//! 整合内存扫描、地址读写和字段跟踪功能，通过 `GameBridge` trait
//! 向上层提供统一的内存操作接口。支持附加/分离进程、内存扫描、
//! 字段值读写和地址跟踪。
//!
//! # 工作流程
//!
//! 1. 使用 `attach(pid)` 附加到目标进程
//! 2. 执行 `first_scan` / `next_scan` 搜索目标值的内存地址
//! 3. 使用 `add_watch` 跟踪确认的地址
//! 4. 通过 `BridgeCommand::ReadField` / `WriteField` 读写跟踪的字段
//! 5. 使用 `detach` 分离进程

use crate::process;
use crate::region;
use crate::scanner::{MemoryScanner, NextScanMode, ValueType};
use game_tool_core::error::GameToolError;
use game_tool_core::{
    BridgeCommand, FieldScanSeed, GameBridge, GameState, MemoryCommand, ModifiableField, ScannedAddr,
};
use serde_json::Value;
use std::collections::HashMap;

/// 跟踪的内存地址信息。
///
/// 记录一个被跟踪字段的地址和值类型，用于后续的读取和写入操作。
struct TrackedAddress {
    /// 在目标进程中的绝对虚拟地址
    pub address: usize,
    /// 该地址处存储的值类型
    pub value_type: ValueType,
}

/// 统一内存桥接器。
///
/// 管理进程附加状态、已跟踪字段列表和内存扫描器实例。
/// 是实现 `GameBridge` trait 的核心结构，向上层 CLI/UI 提供完整的内存操作能力。
pub struct UniversalMemoryBridge {
    /// 当前附加的进程 PID
    pid: Option<u32>,
    /// 进程句柄（以 isize 形式存储，避免裸指针的 Send/Sync 问题）
    handle: Option<isize>,
    /// 跟踪的字段映射（field_id → 地址信息）
    tracked: HashMap<String, TrackedAddress>,
    /// 内存扫描器实例
    scanner: Option<MemoryScanner>,
}

impl UniversalMemoryBridge {
    /// 创建新的内存桥接器，初始为未附加状态。
    pub fn new() -> Self {
        Self {
            pid: None,
            handle: None,
            tracked: HashMap::new(),
            scanner: None,
        }
    }

    /// 附加到指定 PID 的进程。
    ///
    /// 打开进程句柄并初始化扫描器。成功后可通过 `is_attached` 检查状态。
    pub fn attach(&mut self, pid: u32) -> Result<(), GameToolError> {
        let handle = process::open_process_handle(pid)
            .ok_or_else(|| GameToolError::BridgeError("无法打开进程句柄".into()))?;
        self.pid = Some(pid);
        self.handle = Some(handle as isize);
        self.scanner = Some(MemoryScanner::new());
        Ok(())
    }

    /// 分离当前附加的进程。
    ///
    /// 关闭进程句柄，清空跟踪字段列表和扫描器。
    pub fn detach(&mut self) {
        if let Some(handle) = self.handle.take() {
            process::close_process_handle(handle as *mut std::ffi::c_void);
        }
        self.pid = None;
        self.tracked.clear();
        self.scanner = None;
    }

    /// 检查当前是否已附加到某个进程。
    pub fn is_attached(&self) -> bool {
        self.handle.is_some()
    }

    /// 返回当前附加的进程 PID（如已附加）。
    pub fn pid(&self) -> Option<u32> {
        self.pid
    }

    /// 返回当前正在跟踪的字段数量。
    pub fn tracked_count(&self) -> usize {
        self.tracked.len()
    }

    /// 获取进程句柄的裸指针形式，未附加时返回错误。
    fn handle(&self) -> Result<*mut std::ffi::c_void, GameToolError> {
        let h = self.handle
            .ok_or_else(|| GameToolError::BridgeError("未附加进程".into()))?;
        Ok(h as *mut std::ffi::c_void)
    }

    /// 添加一个要跟踪的内存地址。
    ///
    /// 被跟踪的地址可通过 `BridgeCommand::ReadField` / `WriteField` 读写。
    pub fn add_watch(&mut self, field_id: String, address: usize, value_type: ValueType) {
        self.tracked.insert(
            field_id,
            TrackedAddress {
                address,
                value_type,
            },
        );
    }

    /// 移除一个被跟踪的字段。
    pub fn remove_watch(&mut self, field_id: &str) {
        self.tracked.remove(field_id);
    }

    /// 执行首次全内存扫描。
    ///
    /// 在目标进程中搜索所有与目标值匹配的地址。
    pub fn first_scan(
        &mut self,
        value: Value,
        value_type: ValueType,
    ) -> Result<Vec<ScannedAddr>, GameToolError> {
        let handle = self.handle()?;
        let scanner = self
            .scanner
            .as_mut()
            .ok_or_else(|| GameToolError::BridgeError("扫描器未初始化".into()))?;
        Ok(scanner.first_scan(handle, &value, value_type))
    }

    /// 执行连续扫描（二次过滤）。
    ///
    /// 在当前候选地址基础上按指定模式进一步筛选。
    pub fn next_scan(&mut self, mode: &NextScanMode) -> Result<Vec<ScannedAddr>, GameToolError> {
        let handle = self.handle()?;
        let scanner = self
            .scanner
            .as_mut()
            .ok_or_else(|| GameToolError::BridgeError("扫描器未初始化".into()))?;
        Ok(scanner.next_scan(handle, mode))
    }

    /// 根据存档字段值生成内存扫描种子。
    ///
    /// 对每个存档字段，扫描进程内存中所有匹配的地址。
    pub fn seed_from_save(
        &mut self,
        fields: &[ModifiableField],
    ) -> Result<Vec<FieldScanSeed>, GameToolError> {
        let handle = self.handle()?;
        let scanner = self
            .scanner
            .as_mut()
            .ok_or_else(|| GameToolError::BridgeError("扫描器未初始化".into()))?;
        Ok(scanner.seed_from_save(handle, fields))
    }

    /// 交叉验证扫描种子。
    ///
    /// 利用修改后的新字段值验证扫描结果，提高地址匹配的准确率。
    pub fn cross_validate(
        &mut self,
        seeds: &[FieldScanSeed],
        fields: &[ModifiableField],
    ) -> Result<Vec<FieldScanSeed>, GameToolError> {
        let handle = self.handle()?;
        let scanner = self
            .scanner
            .as_mut()
            .ok_or_else(|| GameToolError::BridgeError("扫描器未初始化".into()))?;
        Ok(scanner.cross_validate(handle, seeds, fields))
    }
}

impl GameBridge for UniversalMemoryBridge {
    /// 连接（通用接口）。内存桥使用 `attach` 而不是此方法连接。
    fn connect(&mut self) -> Result<(), GameToolError> {
        Err(GameToolError::BridgeConnectError(
            "内存桥通过 attach 连接".into(),
        ))
    }

    /// 断开连接。等同于 `detach`。
    fn disconnect(&mut self) {
        self.detach();
    }

    /// 检查是否已连接（已附加到进程）。
    fn is_connected(&self) -> bool {
        self.is_attached()
    }

    /// 执行桥接命令。
    ///
    /// 支持的命令：
    /// - `ReadAll`: 读取所有已跟踪字段的当前值
    /// - `ReadField(id)`: 读取指定跟踪字段的当前值
    /// - `WriteField(id, value)`: 向指定跟踪字段写入新值
    fn execute(&mut self, cmd: &BridgeCommand) -> Result<Value, GameToolError> {
        let handle = self.handle()?;
        match cmd {
            BridgeCommand::ReadAll => {
                // 读取所有已跟踪字段的值
                let mut extensions = HashMap::new();
                for (field_id, ta) in &self.tracked {
                    let val = read_value(handle, ta.address, &ta.value_type);
                    extensions.insert(field_id.clone(), val);
                }
                let state = GameState {
                    engine: "generic".into(),
                    map_name: String::new(),
                    play_time: String::new(),
                    save_count: 0,
                    extensions,
                };
                serde_json::to_value(state).map_err(GameToolError::JsonError)
            }
            BridgeCommand::ReadField(field_id) => {
                // 读取单个跟踪字段的值
                let ta = self.tracked.get(field_id).ok_or_else(|| {
                    GameToolError::BridgeCommandError(format!("未跟踪字段: {}", field_id))
                })?;
                Ok(read_value(handle, ta.address, &ta.value_type))
            }
            BridgeCommand::WriteField(field_id, value) => {
                // 向单个跟踪字段写入新值
                let ta = self.tracked.get(field_id).ok_or_else(|| {
                    GameToolError::BridgeCommandError(format!("未跟踪字段: {}", field_id))
                })?;
                let ok = write_value(handle, ta.address, &ta.value_type, value);
                if ok {
                    Ok(Value::String("ok".into()))
                } else {
                    Err(GameToolError::BridgeCommandError("内存写入失败".into()))
                }
            }
        }
    }

    /// 返回引擎名称标识："memory_bridge"
    fn engine_name(&self) -> &str {
        "memory_bridge"
    }

    /// 返回桥接器的优先级（数值越大优先级越高）。
    fn priority(&self) -> i32 {
        100
    }

    /// 处理内存相关的专用命令。
    ///
    /// 包括附加/分离进程、内存扫描、种子生成和地址跟踪等操作。
    fn handle_memory_command(
        &mut self,
        cmd: &MemoryCommand,
    ) -> Result<Value, GameToolError> {
        match cmd {
            MemoryCommand::Attach(pid) => {
                self.attach(*pid)?;
                Ok(Value::String("attached".into()))
            }
            MemoryCommand::Detach => {
                self.detach();
                Ok(Value::String("detached".into()))
            }
            MemoryCommand::FirstScan { value, value_type_id } => {
                let vtype = value_type_from_id(*value_type_id);
                let results = self.first_scan(value.clone(), vtype)?;
                let addrs: Vec<ScannedAddr> = results
                    .into_iter()
                    .map(|r| ScannedAddr {
                        address: r.address,
                        current_value: r.current_value,
                    })
                    .collect();
                serde_json::to_value(addrs).map_err(GameToolError::JsonError)
            }
            MemoryCommand::NextScan { scan_mode_id, value } => {
                let mode = scan_mode_from_id(*scan_mode_id, value.clone());
                let results = self.next_scan(&mode)?;
                let addrs: Vec<ScannedAddr> = results
                    .into_iter()
                    .map(|r| ScannedAddr {
                        address: r.address,
                        current_value: r.current_value,
                    })
                    .collect();
                serde_json::to_value(addrs).map_err(GameToolError::JsonError)
            }
            MemoryCommand::SeedFromSave(fields) => {
                let seeds = self.seed_from_save(fields)?;
                serde_json::to_value(seeds).map_err(GameToolError::JsonError)
            }
            MemoryCommand::CrossValidate { ref seeds_data, new_fields } => {
                let seeds: Vec<FieldScanSeed> = serde_json::from_slice(seeds_data)
                    .map_err(GameToolError::JsonError)?;
                let results = self.cross_validate(&seeds, new_fields)?;
                serde_json::to_value(results).map_err(GameToolError::JsonError)
            }
            MemoryCommand::AddWatch { field_id, address, value_type_id } => {
                let vtype = value_type_from_id(*value_type_id);
                self.add_watch(field_id.clone(), *address, vtype);
                Ok(Value::String("added".into()))
            }
            MemoryCommand::RemoveWatch(field_id) => {
                self.remove_watch(field_id);
                Ok(Value::String("removed".into()))
            }
        }
    }
}

/// 从跟踪地址读取当前值，按值类型转换为 JSON Value。
fn read_value(handle: *mut std::ffi::c_void, address: usize, vt: &ValueType) -> Value {
    match vt {
        ValueType::I32 => region::read_i32(handle, address)
            .map(|v| Value::Number(v.into()))
            .unwrap_or(Value::Null),
        ValueType::I64 => region::read_i64(handle, address)
            .map(|v| Value::Number(v.into()))
            .unwrap_or(Value::Null),
        ValueType::F32 => region::read_f32(handle, address)
            .and_then(|v| serde_json::Number::from_f64(v as f64))
            .map(Value::Number)
            .unwrap_or(Value::Null),
        ValueType::F64 => region::read_f64(handle, address)
            .and_then(serde_json::Number::from_f64)
            .map(Value::Number)
            .unwrap_or(Value::Null),
        ValueType::String(max) => region::read_string(handle, address, *max)
            .map(Value::String)
            .unwrap_or(Value::Null),
        ValueType::Bytes(size) => region::read_memory(handle, address, *size)
            .map(|d| Value::String(game_tool_core::base64::encode(&d)))
            .unwrap_or(Value::Null),
    }
}

/// 向跟踪地址写入新值，按值类型将 JSON Value 转换为字节序列后写入。
fn write_value(handle: *mut std::ffi::c_void, address: usize, vt: &ValueType, value: &Value) -> bool {
    match vt {
        ValueType::I32 => {
            let v = value.as_i64().unwrap_or(0) as i32;
            region::write_i32(handle, address, v)
        }
        ValueType::I64 => {
            let v = value.as_i64().unwrap_or(0);
            region::write_i64(handle, address, v)
        }
        ValueType::F32 => {
            let v = value.as_f64().unwrap_or(0.0) as f32;
            region::write_f32(handle, address, v)
        }
        ValueType::F64 => {
            let v = value.as_f64().unwrap_or(0.0);
            region::write_f64(handle, address, v)
        }
        ValueType::String(max) => {
            let s = value.as_str().unwrap_or("");
            let mut bytes = s.as_bytes().to_vec();
            bytes.resize(*max, 0);
            region::write_memory(handle, address, &bytes)
        }
        ValueType::Bytes(_) => {
            let raw = value.as_str().and_then(|s| game_tool_core::base64::decode(s));
            match raw {
                Some(data) => region::write_memory(handle, address, &data),
                None => false,
            }
        }
    }
}

/// 将数值 ID 转换为 `ValueType` 枚举。
///
/// 用于序列化通信场景（如与前端的数据交换）。
fn value_type_from_id(id: u32) -> ValueType {
    match id {
        0 => ValueType::I32,
        1 => ValueType::I64,
        2 => ValueType::F32,
        3 => ValueType::F64,
        4 => ValueType::String(256),
        _ => ValueType::I32,
    }
}

/// 将数值 ID 和可选的 Value 转换为 `NextScanMode` 枚举。
///
/// 用于序列化通信场景。
fn scan_mode_from_id(id: u32, value: Option<Value>) -> NextScanMode {
    match id {
        0 => NextScanMode::Exact(value.unwrap_or(Value::Null)),
        1 => NextScanMode::Increased,
        2 => NextScanMode::Decreased,
        3 => NextScanMode::Unchanged,
        4 => NextScanMode::Changed,
        _ => NextScanMode::Exact(Value::Null),
    }
}
