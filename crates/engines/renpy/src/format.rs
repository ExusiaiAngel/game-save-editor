//! Ren'Py 存档格式处理器 (.save ZIP)。
//!
//! Ren'Py 将存档存储为 ZIP 压缩包，内部包含以下条目：
//! - `json`: 游戏状态数据（store 变量等）
//! - `extra_info`: 额外信息文本
//! - `log`: 日志数据
//! - `screenshot.png`: 存档截图
//! - `renpy_version`: Ren'Py 引擎版本字符串

use std::collections::HashMap;
use std::fs;
use std::io::{Cursor, Read};
use std::path::Path;

use game_tool_core::{backup, error::GameToolError, ISaveFormat, ModifiableField, SaveSummary};
use serde_json::Value;
use zip::read::ZipArchive;
use zip::write::{SimpleFileOptions, ZipWriter};

/// Ren'Py 存档格式处理器。
///
/// 解析 .save ZIP 文件，提取元数据（`_meta`）、额外信息（`_extra_info`）、
/// 日志（`_log`）、截图（`_screenshot`）和引擎版本（`_renpy_version`）。
/// 支持对存档中的 store 变量进行递归扫描和修改。
pub struct RenPyFormat;

impl Default for RenPyFormat {
    fn default() -> Self {
        Self
    }
}

impl RenPyFormat {
    /// 创建新的 Ren'Py 格式处理器
    pub fn new() -> Self {
        Self
    }
}

impl ISaveFormat for RenPyFormat {
    fn name(&self) -> &str {
        "Ren'Py"
    }
    fn extensions(&self) -> Vec<String> {
        vec![".save".into()]
    }
    fn engine_type(&self) -> &str {
        "renpy"
    }
    /// Ren'Py 存档使用 ZIP 格式，魔术字节为 `PK\x03\x04`
    fn magic_bytes(&self) -> Option<&[u8]> {
        Some(b"PK\x03\x04")
    }

    /// 加载 Ren'Py 存档（ZIP 文件）。
    ///
    /// 遍历 ZIP 中的所有条目，解析为统一的数据结构：
    /// - `json` → `_meta`
    /// - `extra_info` → `_extra_info`
    /// - `log` → `_log`（Base64 编码）
    /// - `screenshot.png` → `_screenshot`（Base64 编码）
    /// - `renpy_version` → `_renpy_version`
    fn load(&self, filepath: &str) -> Result<Value, GameToolError> {
        let file =
            fs::File::open(filepath).map_err(|e| GameToolError::ArchiveLoadError(e.to_string()))?;
        let mut archive = ZipArchive::new(file)
            .map_err(|e| GameToolError::ArchiveLoadError(format!("ZIP 打开失败: {}", e)))?;

        let mut meta = Value::Null;
        let mut extra_info = String::new();
        let mut log_bytes = Vec::new();
        let mut screenshot = Vec::new();
        let mut renpy_version = String::new();

        // 遍历 ZIP 中的所有条目
        for i in 0..archive.len() {
            let mut entry = archive
                .by_index(i)
                .map_err(|e| GameToolError::ArchiveLoadError(e.to_string()))?;
            let name = entry.name().to_string();
            let mut buf = Vec::new();
            entry.read_to_end(&mut buf).ok();

            match name.as_str() {
                "json" => {
                    meta = serde_json::from_slice(&buf).map_err(|e| {
                        GameToolError::ArchiveLoadError(format!("JSON 解析: {}", e))
                    })?;
                }
                "extra_info" => extra_info = String::from_utf8_lossy(&buf).to_string(),
                "log" => log_bytes = buf,
                "screenshot.png" => screenshot = buf,
                "renpy_version" => renpy_version = String::from_utf8_lossy(&buf).trim().to_string(),
                _ => {}
            }
        }

        // json 条目是必须的
        if meta.is_null() {
            return Err(GameToolError::ArchiveLoadError(
                "ZIP 中缺少 json 条目".into(),
            ));
        }

        let mut data = serde_json::Map::new();
        data.insert("_format".into(), Value::String("renpy".into()));
        data.insert("_meta".into(), meta);
        data.insert("_extra_info".into(), Value::String(extra_info));
        data.insert(
            "_log".into(),
            Value::String(game_tool_core::base64::encode(&log_bytes)),
        );
        data.insert(
            "_screenshot".into(),
            Value::String(game_tool_core::base64::encode(&screenshot)),
        );
        data.insert("_renpy_version".into(), Value::String(renpy_version));

        Ok(Value::Object(data))
    }

    /// 保存修改后的存档：将各组件重新打包为 ZIP 文件。
    ///
    /// 自动创建备份。二进制数据（log、screenshot）从 Base64 解码后写回。
    /// ZIP 内部条目结构：json, extra_info, log, screenshot.png, renpy_version。
    fn save(&self, filepath: &str, data: &Value) -> Result<(), GameToolError> {
        let path = Path::new(filepath);
        // 保存前创建备份（保留最多 10 份）
        let _ = backup::save_backup(path, 10);

        // 从统一结构中提取各组件数据
        let meta = data.get("_meta").cloned().unwrap_or(Value::Null);
        let extra_info = data
            .get("_extra_info")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let log_b64 = data.get("_log").and_then(|v| v.as_str()).unwrap_or("");
        let screenshot_b64 = data
            .get("_screenshot")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let renpy_version = data
            .get("_renpy_version")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        // 构建新 ZIP 文件到内存缓冲区
        let mut buf = Cursor::new(Vec::new());
        {
            let mut writer = ZipWriter::new(&mut buf);
            let options = SimpleFileOptions::default();

            // 写入 json 条目：store 变量和其他序列化数据
            writer
                .start_file("json", options)
                .map_err(|e| GameToolError::ArchiveSaveError(e.to_string()))?;
            serde_json::to_writer(&mut writer, &meta)
                .map_err(|e| GameToolError::ArchiveSaveError(e.to_string()))?;

            // 写入 extra_info 条目：存档额外文本信息
            writer
                .start_file("extra_info", options)
                .map_err(|e| GameToolError::ArchiveSaveError(e.to_string()))?;
            std::io::Write::write_all(&mut writer, extra_info.as_bytes()).ok();

            // 解码并写入 log 条目：存档操作日志的二进制数据
            if !log_b64.is_empty() {
                if let Some(decoded) = game_tool_core::base64::decode(log_b64) {
                    writer
                        .start_file("log", options)
                        .map_err(|e| GameToolError::ArchiveSaveError(e.to_string()))?;
                    std::io::Write::write_all(&mut writer, &decoded).ok();
                }
            }
            // 解码并写入截图条目：存档缩略图 PNG
            if !screenshot_b64.is_empty() {
                if let Some(decoded) = game_tool_core::base64::decode(screenshot_b64) {
                    writer
                        .start_file("screenshot.png", options)
                        .map_err(|e| GameToolError::ArchiveSaveError(e.to_string()))?;
                    std::io::Write::write_all(&mut writer, &decoded).ok();
                }
            }
            // 写入版本条目：Ren'Py 引擎版本号
            if !renpy_version.is_empty() {
                writer
                    .start_file("renpy_version", options)
                    .map_err(|e| GameToolError::ArchiveSaveError(e.to_string()))?;
                std::io::Write::write_all(&mut writer, renpy_version.as_bytes()).ok();
            }

            // 完成 ZIP 写入并刷新
            writer
                .finish()
                .map_err(|e| GameToolError::ArchiveSaveError(e.to_string()))?;
        }

        // 将内存中的 ZIP 数据写入磁盘文件
        fs::write(path, buf.into_inner())
            .map_err(|e| GameToolError::ArchiveSaveError(e.to_string()))?;
        Ok(())
    }

    /// 在游戏目录中搜索存档文件夹。
    ///
    /// 按优先级搜索：`game/saves` → `game/save` → `saves`。
    fn find_data_dir(&self, game_dir: &str) -> Option<String> {
        let dir = Path::new(game_dir);
        for sub in &["game/saves", "game/save", "saves"] {
            let d = dir.join(sub);
            if d.is_dir() {
                return Some(d.to_string_lossy().to_string());
            }
        }
        None
    }

    /// 提取存档摘要：存档名、引擎版本、是否有截图。
    fn get_summary(&self, data: &Value) -> SaveSummary {
        let meta = data.get("_meta");
        let save_name = meta
            .and_then(|m| m.get("_save_name"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let version = data
            .get("_renpy_version")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let has_screenshot = data
            .get("_screenshot")
            .and_then(|v| v.as_str())
            .map(|s| !s.is_empty())
            .unwrap_or(false);

        SaveSummary {
            members: if save_name.is_empty() {
                vec![]
            } else {
                vec![save_name.to_string()]
            },
            extra: {
                let mut m = HashMap::new();
                m.insert("version".into(), Value::String(version.to_string()));
                m.insert("has_screenshot".into(), Value::Bool(has_screenshot));
                m
            },
            ..Default::default()
        }
    }

    /// 扫描可修改字段。
    ///
    /// 包含元数据字段（存档名、额外信息、版本）以及 `_meta` 中所有
    /// 非下划线前缀的叶子字段（store 变量）。
    ///
    /// 字段分类说明：
    /// - `meta`: 存档元数据（名称、额外信息、引擎版本）
    /// - `store`: Ren'Py store 中的用户定义变量（递归展开）
    fn scan_fields(&self, data: &Value, _game_dir: &str) -> Vec<ModifiableField> {
        let mut fields = Vec::new();
        let extra_info = data
            .get("_extra_info")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let save_name = data
            .get("_meta")
            .and_then(|m| m.get("_save_name"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let version = data
            .get("_renpy_version")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        // 元数据固定字段
        fields.push(ModifiableField {
            category: "meta".into(),
            field_id: "renpy_save_name".into(),
            display_name: "存档名称".into(),
            field_type: "str".into(),
            save_value: Value::String(save_name.to_string()),
            ..Default::default()
        });
        fields.push(ModifiableField {
            category: "meta".into(),
            field_id: "renpy_extra_info".into(),
            display_name: "Extra Info".into(),
            field_type: "str".into(),
            save_value: Value::String(extra_info),
            ..Default::default()
        });
        fields.push(ModifiableField {
            category: "meta".into(),
            field_id: "renpy_version".into(),
            display_name: "Ren'Py 版本".into(),
            field_type: "str".into(),
            save_value: Value::String(version.to_string()),
            ..Default::default()
        });

        // 递归提取 _meta 中的所有叶子值（store 变量）
        if let Some(meta) = data.get("_meta").and_then(|v| v.as_object()) {
            for (key, value) in meta {
                if key.starts_with('_') {
                    continue;
                }
                collect_meta_leaves(&mut fields, key, value, "");
            }
        }

        fields
    }

    /// 应用字段修改。
    ///
    /// 支持三种字段类型：
    /// - `renpy_extra_info` / `renpy_save_name`: 直接修改对应键
    /// - `renpy_meta.*`: 按点号路径设置 `_meta` 中的嵌套值
    fn apply_field(&self, data: &mut Value, field: &ModifiableField) -> Result<(), GameToolError> {
        match field.field_id.as_str() {
            "renpy_extra_info" => {
                if let Some(obj) = data.as_object_mut() {
                    obj.insert("_extra_info".into(), field.save_value.clone());
                }
            }
            "renpy_save_name" => {
                if let Some(meta) = data.pointer_mut("/_meta") {
                    if let Some(obj) = meta.as_object_mut() {
                        obj.insert("_save_name".into(), field.save_value.clone());
                    }
                }
            }
            fid if fid.starts_with("renpy_meta.") => {
                let path = &fid["renpy_meta.".len()..];
                set_nested_meta(data, path, &field.save_value);
            }
            _ => {}
        }
        Ok(())
    }
}

/// 递归提取 JSON 对象的叶子节点，生成可编辑字段列表。
///
/// 跳过以 `_` 开头的内部键。路径格式为 `parent.child`。
/// 支持嵌套对象展开，数组类型暂跳过（不支持编辑）。
fn collect_meta_leaves(fields: &mut Vec<ModifiableField>, key: &str, value: &Value, prefix: &str) {
    // 构建当前键的完整路径（以点号分隔）
    let path = if prefix.is_empty() {
        key.to_string()
    } else {
        format!("{}.{}", prefix, key)
    };
    match value {
        Value::Object(inner) => {
            // 递归进入嵌套对象，跳过内部下划线开头的私有键
            for (k, v) in inner {
                if !k.starts_with('_') {
                    collect_meta_leaves(fields, k, v, &path);
                }
            }
        }
        Value::Array(_) => {
            // 暂不支持数组类型字段的编辑
        }
        leaf => {
            // 根据叶子值的 Rust 类型推断字段类型字符串
            let field_type = match leaf {
                Value::Bool(_) => "bool",
                Value::Number(n) => {
                    if n.is_f64() {
                        "float"
                    } else {
                        "int"
                    }
                }
                Value::String(_) => "str",
                _ => "str",
            };
            // 将叶子节点注册为可编辑字段
            fields.push(ModifiableField {
                category: "store".into(),
                field_id: format!("renpy_meta.{}", path),
                display_name: path.clone(),
                field_type: field_type.into(),
                save_value: leaf.clone(),
                ..Default::default()
            });
        }
    }
}

/// 按点号分隔的路径在 `_meta` 中设置嵌套值。
///
/// 自动创建路径上不存在的中间节点。
/// 例如: 路径 `"player.stats.hp"` 会在 `_meta` 中创建 `player` → `stats` → `hp` 的嵌套结构。
fn set_nested_meta(data: &mut Value, dotted_path: &str, value: &Value) {
    // 将点号路径拆分为路径段数组
    let parts: Vec<&str> = dotted_path.split('.').collect();

    // 确保 _meta 及所有中间节点存在（不存在的节点自动创建为空对象）
    {
        let obj = data.as_object_mut().unwrap();
        let meta = obj
            .entry("_meta".to_string())
            .or_insert_with(|| Value::Object(serde_json::Map::new()));
        let mut current = meta;
        // 遍历除最后一个段以外的所有路径段，创建中间节点
        for i in 0..parts.len() - 1 {
            let part = parts[i];
            if !current.is_object() {
                *current = Value::Object(serde_json::Map::new());
            }
            let cur_obj = current.as_object_mut().unwrap();
            current = cur_obj
                .entry(part.to_string())
                .or_insert_with(|| Value::Object(serde_json::Map::new()));
        }
    }

    // 使用 JSON Pointer 方式设置最终的叶子值
    let json_ptr = format!("/_meta/{}", parts.join("/"));
    if let Some(target) = data.pointer_mut(&json_ptr) {
        // 指针路径解析成功，直接赋值
        *target = value.clone();
    } else if let Some(obj) = data.pointer_mut("/_meta") {
        // 指针解析失败（如路径中存在特殊字符），使用递归插入替代
        if let Some(inner) = obj.as_object_mut() {
            insert_nested_value(inner, &parts, value);
        }
    }
}

/// 递归按路径段插入嵌套值（`set_nested_meta` 的辅助函数）。
///
/// 递归终止条件：路径只剩一个段时，直接插入叶子值。
/// 否则：在当前对象中创建/获取子对象，并继续递归处理剩余路径段。
fn insert_nested_value(obj: &mut serde_json::Map<String, Value>, parts: &[&str], value: &Value) {
    if parts.len() == 1 {
        // 递归终止：最后一个路径段，直接插入值
        obj.insert(parts[0].to_string(), value.clone());
    } else {
        // 获取或创建中间节点（自动初始化为空对象）
        let entry = obj
            .entry(parts[0].to_string())
            .or_insert_with(|| Value::Object(serde_json::Map::new()));
        if let Some(inner) = entry.as_object_mut() {
            // 递归处理剩余的路径段
            insert_nested_value(inner, &parts[1..], value);
        }
    }
}

// ── 单元测试 ──
#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_data() -> Value {
        let mut meta = serde_json::Map::new();
        meta.insert("_save_name".into(), Value::String("Quick Save".into()));

        let mut data = serde_json::Map::new();
        data.insert("_format".into(), Value::String("renpy".into()));
        data.insert("_meta".into(), Value::Object(meta));
        data.insert("_extra_info".into(), Value::String("测试存档".into()));
        data.insert("_log".into(), Value::String("".into()));
        data.insert("_screenshot".into(), Value::String("".into()));
        data.insert("_renpy_version".into(), Value::String("8.0.3".into()));
        Value::Object(data)
    }

    #[test]
    fn test_extensions() {
        let fmt = RenPyFormat::new();
        assert!(fmt.extensions().contains(&".save".to_string()));
    }

    #[test]
    fn test_magic_bytes() {
        let fmt = RenPyFormat::new();
        assert_eq!(fmt.magic_bytes(), Some(b"PK\x03\x04".as_ref()));
    }

    #[test]
    fn test_get_summary() {
        let fmt = RenPyFormat::new();
        let summary = fmt.get_summary(&make_test_data());
        assert!(summary.members.contains(&"Quick Save".to_string()));
    }

    #[test]
    fn test_scan_fields() {
        let fmt = RenPyFormat::new();
        let fields = fmt.scan_fields(&make_test_data(), "");
        assert!(fields.iter().any(|f| f.field_id == "renpy_save_name"));
        assert!(fields.iter().any(|f| f.field_id == "renpy_extra_info"));
    }

    #[test]
    fn test_apply_field() {
        let fmt = RenPyFormat::new();
        let mut data = make_test_data();
        let field = ModifiableField {
            category: "meta".into(),
            field_id: "renpy_extra_info".into(),
            display_name: "Extra".into(),
            field_type: "str".into(),
            save_value: Value::String("Changed".into()),
            ..Default::default()
        };
        fmt.apply_field(&mut data, &field).unwrap();
        assert_eq!(data["_extra_info"], Value::String("Changed".into()));
    }

    #[test]
    fn test_find_data_dir() {
        let fmt = RenPyFormat::new();
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("game/saves")).unwrap();
        let found = fmt.find_data_dir(&dir.path().to_string_lossy());
        assert!(found.is_some());
    }
}
