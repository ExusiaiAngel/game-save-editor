//! Unreal Engine GVAS 存档格式处理器（只读优先）。
//!
//! GVAS (Generic Value Archive Storage) 是 Unreal Engine 使用的
//! 二进制存档格式。本模块提供头部解析和属性提取功能，
//! 支持读取和有限范围的写入（属性修改）。

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use game_tool_core::{backup, error::GameToolError, ISaveFormat, ModifiableField, SaveSummary};
use serde_json::Value;

/// GVAS 格式的魔术字节标识
const MAGIC: &[u8] = b"GVAS";

/// Unreal Engine GVAS 存档格式处理器。
///
/// 支持的属性类型：
/// - **IntProperty** (0x02): 8 字节有符号整数
/// - **FloatProperty** (0x03): 4 字节单精度浮点数
/// - **StrProperty** (0x04): 4 字节长度 + UTF-8 字符串
/// - **BoolProperty** (0x08): 1 字节布尔值
pub struct UnrealGVASFormat;

impl Default for UnrealGVASFormat {
    fn default() -> Self {
        Self
    }
}

impl UnrealGVASFormat {
    /// 创建新的 GVAS 格式处理器
    pub fn new() -> Self {
        Self
    }
}

impl ISaveFormat for UnrealGVASFormat {
    /// 返回格式名称："Unreal Engine (GVAS)"
    fn name(&self) -> &str {
        "Unreal Engine (GVAS)"
    }
    /// 返回支持的存档文件扩展名列表：[".sav"]
    fn extensions(&self) -> Vec<String> {
        vec![".sav".into()]
    }
    /// 返回引擎类型标识："unreal"
    fn engine_type(&self) -> &str {
        "unreal"
    }
    /// GVAS 格式的魔术字节：`GVAS`
    fn magic_bytes(&self) -> Option<&[u8]> {
        Some(MAGIC)
    }

    /// 加载 GVAS 存档文件。
    ///
    /// 解析流程：
    /// 1. 验证魔术字节 `GVAS`
    /// 2. 解析文件头部（引擎版本、分支、存档类型等）
    /// 3. 提取属性列表（Int/Float/Str/Bool）
    ///
    /// 返回结构包含：
    /// - `_format`: `"unreal_gvas"`
    /// - `_raw`: 原始二进制数据的 Base64 编码
    /// - `_header`: 头部字段
    /// - `_props`: 解析出的属性键值对
    fn load(&self, filepath: &str) -> Result<Value, GameToolError> {
        let raw = fs::read(filepath).map_err(|e| GameToolError::ArchiveLoadError(e.to_string()))?;
        if raw.len() < 4 || &raw[0..4] != MAGIC {
            return Err(GameToolError::ArchiveLoadError("无效的 GVAS 格式".into()));
        }

        let header = Self::parse_header(&raw);
        let data_offset = header
            .get("_data_offset")
            .and_then(|v| v.as_u64())
            .unwrap_or(36) as usize;
        let props = Self::extract_properties(&raw, data_offset);

        let mut data = serde_json::Map::new();
        data.insert("_format".into(), Value::String("unreal_gvas".into()));
        data.insert(
            "_raw".into(),
            Value::String(game_tool_core::base64::encode(&raw)),
        );
        data.insert("_header".into(), Value::Object(header));
        data.insert("_props".into(), Value::Object(props));

        Ok(Value::Object(data))
    }

    /// 保存修改后的 GVAS 存档。
    ///
    /// 基于原始二进制数据，用修改后的属性重新序列化并替换原始属性段。
    /// 除属性段外的其他数据（头部 + 尾部）保持不变。
    fn save(&self, filepath: &str, data: &Value) -> Result<(), GameToolError> {
        let path = Path::new(filepath);
        let _ = backup::save_backup(path, 10);

        // 从 Base64 解码原始数据
        let raw_bytes = data
            .get("_raw")
            .and_then(|v| v.as_str())
            .and_then(game_tool_core::base64::decode)
            .ok_or_else(|| GameToolError::ArchiveSaveError("缺少 _raw 数据".into()))?;

        let data_offset = data
            .get("_header")
            .and_then(|h| h.get("_data_offset"))
            .and_then(|v| v.as_u64())
            .unwrap_or(36) as usize;

        let props = data
            .get("_props")
            .and_then(|v| v.as_object())
            .cloned()
            .unwrap_or_default();

        // 序列化修改后的属性为二进制
        let props_binary = Self::serialize_properties(&props);
        let original_props_end = Self::find_original_props_end(&raw_bytes, data_offset);

        // 组装新文件: 头部 + 新属性 + 尾部
        let mut output = Vec::with_capacity(
            data_offset + props_binary.len() + raw_bytes.len().saturating_sub(original_props_end),
        );
        output.extend_from_slice(&raw_bytes[..data_offset]); // 头部（不变）
        output.extend_from_slice(&props_binary);              // 新属性段
        if original_props_end < raw_bytes.len() {
            output.extend_from_slice(&raw_bytes[original_props_end..]); // 尾部（不变）
        }

        fs::write(path, &output).map_err(|e| GameToolError::ArchiveSaveError(e.to_string()))?;
        Ok(())
    }

    /// 在游戏目录中查找存档文件夹。
    ///
    /// 按优先级搜索：`Saved/SaveGames` → `Saved` → `SaveGames`。
    fn find_data_dir(&self, game_dir: &str) -> Option<String> {
        let dir = Path::new(game_dir);
        for sub in &["Saved/SaveGames", "Saved", "SaveGames"] {
            let d = dir.join(sub);
            if d.is_dir() {
                return Some(d.to_string_lossy().to_string());
            }
        }
        None
    }

    /// 提取存档摘要：金币、游戏时间、属性数量。
    fn get_summary(&self, data: &Value) -> SaveSummary {
        let props = data.get("_props");
        // 优先查找 Gold，回退 Money
        let gold = props
            .and_then(|p| p.get("Gold"))
            .or_else(|| props.and_then(|p| p.get("Money")))
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as i32;
        // 优先查找 PlayTime，回退 RealTimeSeconds
        let play_time = props
            .and_then(|p| p.get("PlayTime"))
            .or_else(|| props.and_then(|p| p.get("RealTimeSeconds")))
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0) as i32;
        let prop_count = props
            .and_then(|p| p.as_object())
            .map(|m| m.len() as i32)
            .unwrap_or(0);

        SaveSummary {
            gold,
            play_time,
            item_count: prop_count,
            ..Default::default()
        }
    }

    /// 扫描属性列表中的所有可修改字段。
    fn scan_fields(&self, data: &Value, _game_dir: &str) -> Vec<ModifiableField> {
        let mut fields = Vec::new();
        if let Some(props) = data.get("_props").and_then(|v| v.as_object()) {
            for (key, value) in props {
                // 推断属性类型
                let field_type = match value {
                    Value::Bool(_) => "bool",
                    Value::Number(n) if n.is_f64() => "float",
                    Value::Number(_) => "int",
                    Value::String(_) => "str",
                    _ => "str",
                };
                // 使用已知名称映射作为显示名
                let display_name = KNOWN_NAMES
                    .get(key.as_str())
                    .copied()
                    .unwrap_or(key.as_str());
                fields.push(ModifiableField {
                    category: "gvas".into(),
                    field_id: format!("gvas_{}", key),
                    display_name: display_name.to_string(),
                    field_type: field_type.into(),
                    save_value: value.clone(),
                    ..Default::default()
                });
            }
        }
        fields
    }

    /// 应用字段修改到属性集中。
    fn apply_field(&self, data: &mut Value, field: &ModifiableField) -> Result<(), GameToolError> {
        let key = field
            .field_id
            .strip_prefix("gvas_")
            .unwrap_or(&field.field_id)
            .to_string();
        if let Some(props) = data.pointer_mut("/_props") {
            if let Some(obj) = props.as_object_mut() {
                obj.insert(key, field.save_value.clone());
            }
        }
        Ok(())
    }
}

impl UnrealGVASFormat {
    /// 解析 GVAS 文件头部（前 ~100+ 字节）。
    ///
    /// 头部结构（偏移从魔术字节后开始）：
    /// - +4: 存档版本 (i32 LE)
    /// - +8: 包版本 (i32 LE)
    /// - +12: 引擎主版本 (u16 LE)
    /// - +14: 引擎次版本 (u16 LE)
    /// - +16: 引擎补丁版本 (u16 LE)
    /// - +18: 引擎构建号 (u32 LE)
    /// - +22: 分支字符串 (CString, 最多 16 字节)
    /// - +38: 自定义格式版本 (i32 LE)
    /// - +42: 自定义版本数量 (i32 LE) → 跳过 N*16 字节
    /// - 存档类型字符串 (长度前缀 CString)
    fn parse_header(raw: &[u8]) -> serde_json::Map<String, Value> {
        let mut header = serde_json::Map::new();
        if raw.len() < 36 {
            return header;
        }

        // 读取固定字段（小端序）
        let sv = i32::from_le_bytes([raw[4], raw[5], raw[6], raw[7]]);
        let pv = i32::from_le_bytes([raw[8], raw[9], raw[10], raw[11]]);
        let em = u16::from_le_bytes([raw[12], raw[13]]);
        let e_min = u16::from_le_bytes([raw[14], raw[15]]);
        let ep = u16::from_le_bytes([raw[16], raw[17]]);
        let eb = u32::from_le_bytes([raw[18], raw[19], raw[20], raw[21]]);
        let branch = Self::read_cstring(&raw[22..38]);
        let cfv = i32::from_le_bytes([raw[38], raw[39], raw[40], raw[41]]);
        let custom_count = i32::from_le_bytes([raw[42], raw[43], raw[44], raw[45]]) as usize;

        // 跳过自定义版本条目（每条 16 字节）
        let mut offset = 46;
        for _ in 0..custom_count.min(64) {
            if offset + 16 > raw.len() {
                break;
            }
            offset += 16;
        }
        if offset + 4 > raw.len() {
            return header;
        }
        // 读取存档类型字符串（长度前缀）
        let type_len = i32::from_le_bytes([
            raw[offset],
            raw[offset + 1],
            raw[offset + 2],
            raw[offset + 3],
        ]) as usize;
        offset += 4;
        let save_type = if offset + type_len <= raw.len() {
            String::from_utf8_lossy(&raw[offset..offset + type_len])
                .trim_end_matches('\0')
                .to_string()
        } else {
            String::new()
        };
        offset += type_len;

        // 填入头部映射
        header.insert("_saveGameVersion".into(), Value::Number(sv.into()));
        header.insert("_packageVersion".into(), Value::Number(pv.into()));
        header.insert("_engineMajor".into(), Value::Number(em.into()));
        header.insert("_engineMinor".into(), Value::Number(e_min.into()));
        header.insert("_enginePatch".into(), Value::Number(ep.into()));
        header.insert("_engineBuild".into(), Value::Number(eb.into()));
        header.insert("_branch".into(), Value::String(branch));
        header.insert("_customFormatVersion".into(), Value::Number(cfv.into()));
        header.insert("_saveGameType".into(), Value::String(save_type));
        header.insert("_data_offset".into(), Value::Number((offset as u64).into()));
        header
    }

    /// 从属性段提取所有属性。
    ///
    /// 属性段格式（从 `start` 偏移开始）：
    /// - `<属性名><NUL><类型字节>` 的重复序列
    /// - 0x02: IntProperty → 名称 + \0 + 0x02 + 8字节值
    /// - 0x03: FloatProperty → 名称 + \0 + 0x03 + 4字节值
    /// - 0x04: StrProperty → 名称 + \0 + 0x04 + 4字节长度 + UTF-8内容
    /// - 0x08: BoolProperty → 名称 + \0 + 0x08 + 1字节值
    /// - 其他类型字节表示属性段结束
    fn extract_properties(raw: &[u8], start: usize) -> serde_json::Map<String, Value> {
        let mut props = serde_json::Map::new();
        let mut offset = start.min(raw.len());
        while offset + 4 < raw.len() {
            // 读取 \0 结尾的属性名
            let name_end = raw[offset..].iter().position(|&b| b == 0);
            let name = match name_end {
                Some(len) if len > 0 => {
                    String::from_utf8_lossy(&raw[offset..offset + len]).to_string()
                }
                _ => break,
            };
            offset += name.len() + 1; // 跳过名称和 \0
            if offset >= raw.len() {
                break;
            }

            // 根据类型字节分发处理
            match raw[offset] {
                0x02 => {
                    // IntProperty: 8 字节有符号整数（小端序）
                    if offset + 9 <= raw.len() {
                        let val = i64::from_le_bytes(
                            raw[offset + 1..offset + 9].try_into().unwrap_or([0; 8]),
                        );
                        props.insert(name, Value::Number(val.into()));
                        offset += 9;
                    } else {
                        break;
                    }
                }
                0x03 => {
                    // FloatProperty: 4 字节单精度浮点数（小端序）
                    if offset + 5 <= raw.len() {
                        let val = f32::from_le_bytes(
                            raw[offset + 1..offset + 5].try_into().unwrap_or([0; 4]),
                        );
                        if let Some(n) = serde_json::Number::from_f64(val as f64) {
                            props.insert(name, Value::Number(n));
                        }
                        offset += 5;
                    } else {
                        break;
                    }
                }
                0x04 => {
                    // StrProperty: 4 字节长度（i32 LE）+ UTF-8 内容
                    if offset + 5 <= raw.len() {
                        let len = i32::from_le_bytes(
                            raw[offset + 1..offset + 5].try_into().unwrap_or([0; 4]),
                        ) as usize;
                        offset += 5;
                        if offset + len <= raw.len() {
                            let s = String::from_utf8_lossy(&raw[offset..offset + len])
                                .trim_end_matches('\0')
                                .to_string();
                            props.insert(name, Value::String(s));
                            offset += if len > 0 { len } else { 1 };
                        } else {
                            break;
                        }
                    } else {
                        break;
                    }
                }
                0x08 => {
                    // BoolProperty: 1 字节值（0 = false, 非0 = true）
                    if offset + 2 <= raw.len() {
                        props.insert(name, Value::Bool(raw[offset + 1] != 0));
                        offset += 2;
                    } else {
                        break;
                    }
                }
                _ => break, // 未知类型，停止解析
            }
        }
        props
    }

    /// 读取以 \0 结尾的 C 风格字符串。
    fn read_cstring(data: &[u8]) -> String {
        let pos = data.iter().position(|&b| b == 0).unwrap_or(data.len());
        String::from_utf8_lossy(&data[..pos])
            .trim_end_matches('\0')
            .to_string()
    }

    /// 将属性映射序列化为二进制格式。
    ///
    /// 智能选择类型：
    /// - 整数（无小数部分且范围合适）→ IntProperty (0x02)
    /// - 浮点数 → FloatProperty (0x03)
    /// - 字符串 → StrProperty (0x04)
    /// - 布尔 → BoolProperty (0x08)
    fn serialize_properties(props: &serde_json::Map<String, Value>) -> Vec<u8> {
        let mut buf = Vec::new();
        for (name, value) in props {
            // 写入属性名 + \0
            buf.extend_from_slice(name.as_bytes());
            buf.push(0);
            match value {
                Value::Number(n) => {
                    if let Some(f) = n.as_f64() {
                        if f.fract() == 0.0 && f >= i64::MIN as f64 && f <= i64::MAX as f64 {
                            // 整数 → IntProperty
                            buf.push(0x02);
                            buf.extend_from_slice(&(f as i64).to_le_bytes());
                        } else {
                            // 浮点数 → FloatProperty
                            buf.push(0x03);
                            buf.extend_from_slice(&(f as f32).to_le_bytes());
                        }
                    }
                }
                Value::String(s) => {
                    buf.push(0x04);
                    let bytes = s.as_bytes();
                    // 长度前缀（i32 LE）
                    buf.extend_from_slice(&(bytes.len() as i32).to_le_bytes());
                    buf.extend_from_slice(bytes);
                }
                Value::Bool(b) => {
                    buf.push(0x08);
                    buf.push(if *b { 1 } else { 0 });
                }
                _ => {} // 不支持的类型，跳过
            }
        }
        buf
    }

    /// 扫描原始二进制数据，找到原始属性段的结束位置。
    ///
    /// 遍历算法与 `extract_properties` 一致，但仅跟踪偏移而不解析值。
    fn find_original_props_end(raw: &[u8], start: usize) -> usize {
        let mut offset = start.min(raw.len());
        while offset + 4 < raw.len() {
            let name_end = raw[offset..].iter().position(|&b| b == 0);
            match name_end {
                Some(len) if len > 0 => {
                    offset += len + 1;
                }
                _ => break,
            }
            if offset >= raw.len() {
                break;
            }
            // 根据类型跳过对应字节数
            match raw[offset] {
                0x02 => offset += 9,   // IntProperty: 1(type) + 8(value)
                0x03 => offset += 5,   // FloatProperty: 1 + 4
                0x04 => {
                    if offset + 5 > raw.len() {
                        break;
                    }
                    let len = i32::from_le_bytes(
                        raw[offset + 1..offset + 5].try_into().unwrap_or([0; 4]),
                    ) as usize;
                    offset += 5 + len.max(1); // StrProperty: 1 + 4 + len
                }
                0x08 => offset += 2,   // BoolProperty: 1 + 1
                _ => break,
            }
        }
        offset
    }
}

/// 已知 UE 属性名的中文显示名映射。
static KNOWN_NAMES: std::sync::LazyLock<HashMap<&str, &str>> = std::sync::LazyLock::new(|| {
    HashMap::from([
        ("Gold", "金币"),
        ("Money", "金钱"),
        ("Health", "生命值"),
        ("HP", "HP"),
        ("MaxHealth", "最大生命"),
        ("Level", "等级"),
        ("Experience", "经验值"),
        ("PlayTime", "游戏时间"),
        ("RealTimeSeconds", "实时秒数"),
        ("PlayerName", "玩家名"),
        ("SaveSlotName", "存档槽名"),
    ])
});

#[cfg(test)]
mod tests {
    use super::*;

    /// 构建最小化的 GVAS 测试数据
    fn make_minimal_gvas() -> Vec<u8> {
        let mut data = vec![b'G', b'V', b'A', b'S'];
        data.extend_from_slice(&0u32.to_le_bytes());
        data.extend_from_slice(&0u32.to_le_bytes());
        data.extend_from_slice(&0u16.to_le_bytes());
        data.extend_from_slice(&0u16.to_le_bytes());
        data.extend_from_slice(&0u16.to_le_bytes());
        data.extend_from_slice(&0u32.to_le_bytes());
        data.extend_from_slice(b"++UE5+Release\0\0\0");
        data.extend_from_slice(&0u32.to_le_bytes());
        data.extend_from_slice(&0u32.to_le_bytes());
        data.extend_from_slice(&(0u32).to_le_bytes());
        data
    }

    #[test]
    fn test_extensions() {
        let fmt = UnrealGVASFormat::new();
        assert!(fmt.extensions().contains(&".sav".to_string()));
    }

    #[test]
    fn test_magic_bytes() {
        let fmt = UnrealGVASFormat::new();
        assert_eq!(fmt.magic_bytes(), Some(b"GVAS".as_ref()));
    }

    #[test]
    fn test_load_minimal_gvas() {
        let fmt = UnrealGVASFormat::new();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.sav");
        std::fs::write(&path, &make_minimal_gvas()).unwrap();
        let data = fmt.load(&path.to_string_lossy()).unwrap();
        assert_eq!(data["_format"], "unreal_gvas");
    }

    #[test]
    fn test_find_data_dir() {
        let fmt = UnrealGVASFormat::new();
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("Saved/SaveGames")).unwrap();
        assert!(fmt.find_data_dir(&dir.path().to_string_lossy()).is_some());
    }
}
