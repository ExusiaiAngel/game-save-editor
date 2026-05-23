# 工具箱完整性检查 + 代码优化 实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实现 4 项工具箱新功能（存档信息查看、完整性检查、批量检查、修复工具）+ 4 项代码优化

**Architecture:** 核心逻辑放入 `game-tool-core::integrity`（新建模块），GUI 通过 `ToolboxAction` 枚举驱动，`app.rs` 新增 Toolbox 动作分发分支

**Tech Stack:** Rust, game-tool-core, egui/eframe, rfd (file dialog), serde_json, lz-str, zip

---

## File Structure

| 文件 | 操作 | 职责 |
|------|:---:|------|
| `crates/core/src/integrity.rs` | 新建 | 完整性检查、文件信息、批量扫描、存档修复 全部核心逻辑 |
| `crates/core/src/lib.rs` | 修改 | 新增 `pub mod integrity` |
| `crates/gui/src/state.rs` | 修改 | 扩展 `ToolboxState` 12字段 + 新增 `ToolboxAction` 枚举 |
| `crates/gui/src/panels/toolbox.rs` | 重写 | 6 个功能区块 UI，返回 `Vec<ToolboxAction>` |
| `crates/gui/src/app.rs` | 修改 | Toolbox 动作分发 + Unicode→中文 + 拆分 `new()` + 缩进修正 |
| `crates/gui/src/factory.rs` | 修改 | 移除 `is_readonly()` 死代码 |
| `README.md` | 修改 | 更新工具箱功能描述 |
| `docs/ARCHITECTURE.md` | 修改 | 更新工具箱架构描述 |

---

### Task 1: 创建 core/src/integrity.rs 核心逻辑

**Files:**
- Create: `crates/core/src/integrity.rs`

- [ ] **Step 1: 写入完整 integrity.rs**

```rust
//! 存档完整性检查与修复工具模块。
//!
//! 提供存档文件格式检测、完整性校验、信息查看、批量扫描和修复功能。
//! 所有检测逻辑在 core 层独立实现，不依赖具体引擎 crate。

use std::fs;
use std::path::Path;
use std::time::UNIX_EPOCH;

use crate::types::SaveSummary;

/// 完整性检查结果
#[derive(Debug, Clone)]
pub struct IntegrityResult {
    /// 存档文件的绝对路径
    pub file_path: String,
    /// 识别的格式名称，如 "RPG Maker MV/MZ"
    pub format_name: String,
    /// 引擎标识符，如 "rpg_mv"
    pub engine: String,
    /// 格式是否通过校验
    pub is_valid: bool,
    /// 文件大小（字节）
    pub file_size: u64,
    /// 文件修改时间（ISO 8601 格式）
    pub modified: String,
    /// 格式/逻辑错误列表
    pub errors: Vec<String>,
    /// 数据逻辑警告列表
    pub warnings: Vec<String>,
    /// 成功解析后的摘要信息
    pub summary: Option<SaveSummary>,
    /// 发现的字段数量
    pub field_count: usize,
}

/// 存档文件快速信息（轻量级，不深入解析）
#[derive(Debug, Clone)]
pub struct SaveFileInfo {
    /// 文件路径
    pub file_path: String,
    /// 格式名称
    pub format_name: String,
    /// 引擎标识符
    pub engine: String,
    /// 文件大小（字节）
    pub file_size: u64,
    /// 修改时间（ISO 8601）
    pub modified: String,
    /// 是否为有效存档
    pub is_valid: bool,
    /// 简要错误信息（如文件为空、无法识别格式）
    pub error: Option<String>,
}

/// 存档修复结果
#[derive(Debug, Clone)]
pub struct RepairResult {
    /// 修复是否成功
    pub success: bool,
    /// 修复后文件的路径（生成 `{原名}_repaired.{ext}`）
    pub repaired_path: Option<String>,
    /// 原始文件中的错误列表
    pub original_errors: Vec<String>,
    /// 修复后残留的错误
    pub remaining_errors: Vec<String>,
}

/// 检测到的存档格式枚举（文件级检测，非目录级）
#[derive(Debug, Clone, PartialEq)]
enum SaveFormat {
    /// RPG Maker MV/MZ (.rpgsave / .rmmzsave)
    RpgMaker { is_mz: bool },
    /// Ren'Py (.save)
    RenPy,
    /// Unreal Engine (.sav)
    Unreal,
    /// 通用 JSON (.json)
    Generic,
    /// 无法识别
    Unknown,
}

impl SaveFormat {
    fn from_path(filepath: &str) -> Self {
        let path = Path::new(filepath);
        let ext = path.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .unwrap_or_default();
        match ext.as_str() {
            "rpgsave" => SaveFormat::RpgMaker { is_mz: false },
            "rmmzsave" => SaveFormat::RpgMaker { is_mz: true },
            "save" => {
                // 尝试以 ZIP 打开来判断是否 Ren'Py
                if let Ok(data) = fs::read(filepath) {
                    if data.len() >= 4 && &data[0..4] == b"GVAS" {
                        return SaveFormat::Unreal;
                    }
                    if let Ok(cursor) = std::io::Cursor::new(&data) {
                        if zip::ZipArchive::new(cursor).is_ok() {
                            return SaveFormat::RenPy;
                        }
                    }
                }
                // 尝试作为 RPG Maker 存档
                SaveFormat::RpgMaker { is_mz: false }
            }
            "sav" => SaveFormat::Unreal,
            "json" => SaveFormat::Generic,
            _ => SaveFormat::Unknown,
        }
    }

    fn name(&self) -> &str {
        match self {
            SaveFormat::RpgMaker { is_mz: true } => "RPG Maker MZ",
            SaveFormat::RpgMaker { is_mz: false } => "RPG Maker MV",
            SaveFormat::RenPy => "Ren'Py",
            SaveFormat::Unreal => "Unreal Engine (GVAS)",
            SaveFormat::Generic => "通用 JSON",
            SaveFormat::Unknown => "未知格式",
        }
    }

    fn engine(&self) -> &str {
        match self {
            SaveFormat::RpgMaker { is_mz: true } => "rpg_mz",
            SaveFormat::RpgMaker { is_mz: false } => "rpg_mv",
            SaveFormat::RenPy => "renpy",
            SaveFormat::Unreal => "unreal",
            SaveFormat::Generic => "generic",
            SaveFormat::Unknown => "unknown",
        }
    }
}

/// 获取存档文件的快速信息
pub fn get_save_info(filepath: &str) -> SaveFileInfo {
    let path = Path::new(filepath);
    let meta = fs::metadata(filepath);
    let file_size = meta.as_ref().map(|m| m.len()).unwrap_or(0);
    let modified = meta
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| {
            let secs = d.as_secs();
            chrono::DateTime::from_timestamp(secs as i64, 0)
                .map(|dt| dt.format("%Y-%m-%dT%H:%M:%S").to_string())
                .unwrap_or_default()
        })
        .unwrap_or_default();

    let format = SaveFormat::from_path(filepath);
    let (is_valid, error) = match &format {
        SaveFormat::Unknown => (false, Some("无法识别的存档格式".to_string())),
        _ => {
            if file_size == 0 {
                (false, Some("文件为空".to_string()))
            } else {
                (true, None)
            }
        }
    };

    SaveFileInfo {
        file_path: filepath.to_string(),
        format_name: format.name().to_string(),
        engine: format.engine().to_string(),
        file_size,
        modified,
        is_valid,
        error,
    }
}

/// 对单个存档文件执行完整性检查
pub fn check_save_integrity(filepath: &str) -> IntegrityResult {
    let info = get_save_info(filepath);
    let format = SaveFormat::from_path(filepath);
    let mut errors: Vec<String> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();
    let mut summary: Option<SaveSummary> = None;
    let mut field_count = 0;

    if let Some(ref e) = info.error {
        errors.push(e.clone());
    }

    if format == SaveFormat::Unknown {
        return IntegrityResult {
            file_path: info.file_path,
            format_name: info.format_name,
            engine: info.engine,
            is_valid: false,
            file_size: info.file_size,
            modified: info.modified,
            errors,
            warnings,
            summary: None,
            field_count: 0,
        };
    }

    let data = fs::read(filepath);
    match data {
        Err(e) => {
            errors.push(format!("读取文件失败: {}", e));
        }
        Ok(data) => {
            match &format {
                SaveFormat::RpgMaker { .. } => {
                    check_rpgmaker(&data, &mut errors, &mut warnings, &mut summary, &mut field_count);
                }
                SaveFormat::RenPy => {
                    check_renpy(&data, &mut errors, &mut warnings, &mut summary, &mut field_count);
                }
                SaveFormat::Unreal => {
                    check_unreal(&data, &mut errors, &mut warnings, &mut summary, &mut field_count);
                }
                SaveFormat::Generic => {
                    check_generic(&data, &mut errors, &mut warnings, &mut summary, &mut field_count);
                }
                SaveFormat::Unknown => {}
            }
        }
    }

    IntegrityResult {
        file_path: info.file_path,
        format_name: info.format_name,
        engine: info.engine,
        is_valid: errors.is_empty(),
        file_size: info.file_size,
        modified: info.modified,
        errors,
        warnings,
        summary,
        field_count,
    }
}

/// 校验 RPG Maker 存档（Base64 → LZString → JSON）
fn check_rpgmaker(
    data: &[u8],
    errors: &mut Vec<String>,
    warnings: &mut Vec<String>,
    summary: &mut Option<SaveSummary>,
    field_count: &mut usize,
) {
    let text = match std::str::from_utf8(data) {
        Ok(s) => s.trim().to_string(),
        Err(e) => {
            errors.push(format!("文件不是有效的 UTF-8 文本: {}", e));
            return;
        }
    };

    if text.is_empty() {
        errors.push("存档文件为空".to_string());
        return;
    }

    let json_str = match crate::lzstring::decompress_from_base64(&text) {
        Ok(s) => s,
        Err(e) => {
            errors.push(format!("LZString 解压失败: {}", e));
            return;
        }
    };

    let parsed: serde_json::Value = match serde_json::from_str(&json_str) {
        Ok(v) => v,
        Err(e) => {
            errors.push(format!("JSON 解析失败: {}", e));
            return;
        }
    };

    // 结构校验
    if !parsed.is_object() {
        errors.push("存档根元素不是 JSON 对象".to_string());
        return;
    }

    let obj = parsed.as_object().unwrap();
    *field_count = count_json_fields(&parsed, 0);

    // party 校验
    match obj.get("party") {
        None => errors.push("缺少 party 字段".to_string()),
        Some(party) => {
            if let Some(gold) = party.get("_gold") {
                let g = gold.as_i64().unwrap_or(-1);
                if g < 0 { errors.push(format!("金币为负数: {}", g)); }
                if g > 99_999_999 { warnings.push(format!("金币异常大: {}", g)); }
                *summary = Some(SaveSummary {
                    gold: g as i32,
                    ..Default::default()
                });
            }
            if let Some(actors) = party.get("_actors").and_then(|v| v.as_array()) {
                for actor in actors {
                    if let (Some(hp), Some(mp), Some(lv)) = (
                        actor.get("_hp").and_then(|v| v.as_i64()),
                        actor.get("_mp").and_then(|v| v.as_i64()),
                        actor.get("_level").and_then(|v| v.as_i64()),
                    ) {
                        if hp < 0 || hp > 9999 { warnings.push(format!("角色 HP 异常: {}", hp)); }
                        if mp < 0 || mp > 9999 { warnings.push(format!("角色 MP 异常: {}", mp)); }
                        if lv < 1 || lv > 99 { warnings.push(format!("角色等级异常: {}", lv)); }
                    }
                }
            }
        }
    }

    // switches 校验
    if let Some(switches) = obj.get("switches") {
        if !switches.is_array() {
            if switches.is_object() {
                // JSONEx 格式：检查 _data 和 @a
                let inner = switches.get("_data").unwrap_or(switches);
                if let Some(arr) = inner.get("@a").and_then(|v| v.as_object()) {
                    for v in arr.values() {
                        if v.is_number() && v.as_i64().map_or(true, |n| n != 0 && n != 1) {
                            warnings.push("开关值包含非 0/1 的数值".to_string());
                            break;
                        }
                    }
                }
            }
        } else if let Some(arr) = switches.as_array() {
            for v in arr {
                if !v.is_boolean() && !v.is_number() {
                    errors.push(format!("开关值类型错误: {:?}", v));
                    break;
                }
            }
        }
    } else {
        warnings.push("缺少 switches 字段".to_string());
    }

    // variables 校验
    if let Some(variables) = obj.get("variables") {
        if !variables.is_array() {
            if variables.is_object() {
                let inner = variables.get("_data").unwrap_or(variables);
                if let Some(arr) = inner.get("@a").and_then(|v| v.as_object()) {
                    for v in arr.values() {
                        if !v.is_number() {
                            warnings.push("变量值包含非数值类型".to_string());
                            break;
                        }
                    }
                }
            }
        } else if let Some(arr) = variables.as_array() {
            for v in arr {
                if !v.is_number() {
                    errors.push(format!("变量值类型错误: {:?}", v));
                    break;
                }
            }
        }
    }
}

/// 校验 Ren'Py 存档（ZIP 格式）
fn check_renpy(
    data: &[u8],
    errors: &mut Vec<String>,
    _warnings: &mut Vec<String>,
    _summary: &mut Option<SaveSummary>,
    _field_count: &mut usize,
) {
    let cursor = std::io::Cursor::new(data);
    match zip::ZipArchive::new(cursor) {
        Ok(mut archive) => {
            let names: Vec<String> = archive.file_names().map(|s| s.to_string()).collect();
            if names.is_empty() {
                errors.push("ZIP 归档内没有文件".to_string());
            }
        }
        Err(e) => {
            errors.push(format!("ZIP 解析失败: {}", e));
        }
    }
}

/// 校验 Unreal GVAS 存档
fn check_unreal(
    data: &[u8],
    errors: &mut Vec<String>,
    _warnings: &mut Vec<String>,
    _summary: &mut Option<SaveSummary>,
    field_count: &mut usize,
) {
    const MAGIC: &[u8] = b"GVAS";
    if data.len() < 4 {
        errors.push("文件太小，不是有效的 GVAS 格式".to_string());
        return;
    }
    if &data[0..4] != MAGIC {
        errors.push(format!(
            "魔术字节不匹配: 期望 'GVAS', 实际 '{:?}'",
            String::from_utf8_lossy(&data[0..4])
        ));
        return;
    }
    // 解析头部信息
    if data.len() >= 46 {
        let sv = i32::from_le_bytes([data[4], data[5], data[6], data[7]]);
        if sv < 0 { warnings_push(errors, format!("存档版本异常: {}", sv)); }
    }
    // 计算属性数量
    let mut offset = 46;
    let mut prop_count = 0;
    while offset + 4 < data.len() {
        let name_end = data[offset..].iter().position(|&b| b == 0);
        match name_end {
            Some(len) if len > 0 => { offset += len + 1; }
            _ => break,
        }
        if offset >= data.len() { break; }
        match data[offset] {
            0x02 => { offset += 9; prop_count += 1; }
            0x03 => { offset += 5; prop_count += 1; }
            0x04 => {
                if offset + 5 > data.len() { break; }
                let len = i32::from_le_bytes([data[offset+1], data[offset+2], data[offset+3], data[offset+4]]) as usize;
                offset += 5 + len.max(1);
                prop_count += 1;
            }
            0x08 => { offset += 2; prop_count += 1; }
            _ => break,
        }
    }
    *field_count = prop_count;
}

/// 校验通用 JSON 存档
fn check_generic(
    data: &[u8],
    errors: &mut Vec<String>,
    _warnings: &mut Vec<String>,
    _summary: &mut Option<SaveSummary>,
    field_count: &mut usize,
) {
    let text = match std::str::from_utf8(data) {
        Ok(s) => s,
        Err(e) => {
            errors.push(format!("文件不是有效的 UTF-8 文本: {}", e));
            return;
        }
    };
    let parsed: serde_json::Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(e) => {
            errors.push(format!("JSON 解析失败: {}", e));
            return;
        }
    };
    *field_count = count_json_fields(&parsed, 0);
}

/// 递归计算 JSON 对象/数组中叶子字段的个数
fn count_json_fields(value: &serde_json::Value, depth: usize) -> usize {
    if depth > 20 { return 0; }
    match value {
        serde_json::Value::Object(obj) => {
            obj.values().map(|v| count_json_fields(v, depth + 1)).sum()
        }
        serde_json::Value::Array(arr) => {
            arr.iter().map(|v| count_json_fields(v, depth + 1)).sum()
        }
        _ => 1,
    }
}

/// 对指定目录中的所有存档文件执行批量完整性检查
pub fn batch_check_saves(dir: &str) -> Vec<IntegrityResult> {
    let mut results = Vec::new();
    let path = Path::new(dir);
    if !path.is_dir() { return results; }

    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() { continue; }
            let fname = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            // 跳过备份和系统文件
            if fname.contains(".bak.") || fname == "config.rpgsave" || fname == "global.rpgsave" {
                continue;
            }
            let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");
            let supported = matches!(ext, "rpgsave" | "rmmzsave" | "save" | "sav" | "json");
            if supported {
                results.push(check_save_integrity(&p.to_string_lossy()));
            }
        }
    }
    results
}

/// 尝试修复损坏的 RPG Maker 存档
pub fn attempt_repair(filepath: &str) -> RepairResult {
    let data = match fs::read(filepath) {
        Ok(d) => d,
        Err(e) => {
            return RepairResult {
                success: false,
                repaired_path: None,
                original_errors: vec![format!("读取文件失败: {}", e)],
                remaining_errors: vec![],
            };
        }
    };

    let text = String::from_utf8_lossy(&data).trim().to_string();
    let mut original_errors = Vec::new();

    // 策略1: 尝试修复 Base64 padding
    let fixed_text = repair_base64_padding(&text);
    if fixed_text != text {
        if let Ok(json_str) = crate::lzstring::decompress_from_base64(&fixed_text) {
            if let Ok(_parsed) = serde_json::from_str::<serde_json::Value>(&json_str) {
                return write_repaired(filepath, &fixed_text);
            }
        }
    }

    // 策略2: 去除非法 Base64 字符后重试
    let cleaned = clean_base64(&text);
    if cleaned != text {
        if let Ok(json_str) = crate::lzstring::decompress_from_base64(&cleaned) {
            if let Ok(_parsed) = serde_json::from_str::<serde_json::Value>(&json_str) {
                return write_repaired(filepath, &cleaned);
            }
        }
    }

    // 策略3: 尝试 LZString 解压后修复不完整 JSON
    if let Ok(mut json_str) = crate::lzstring::decompress_from_base64(&text) {
        let fixed_json = repair_json(&json_str);
        if fixed_json != json_str {
            json_str = fixed_json;
        }
        if serde_json::from_str::<serde_json::Value>(&json_str).is_ok() {
            match crate::lzstring::compress_to_base64(&json_str) {
                Ok(compressed) => return write_repaired(filepath, &compressed),
                Err(_) => {},
            }
        }
    }

    original_errors.push("所有修复策略均失败".to_string());
    RepairResult {
        success: false,
        repaired_path: None,
        original_errors,
        remaining_errors: vec![],
    }
}

/// 补齐缺失的 Base64 padding
fn repair_base64_padding(text: &str) -> String {
    let trimmed = text.trim_end_matches('=');
    let missing = (4 - (trimmed.len() % 4)) % 4;
    let mut fixed = trimmed.to_string();
    for _ in 0..missing {
        fixed.push('=');
    }
    fixed
}

/// 移除字符串中的非 Base64 字符
fn clean_base64(text: &str) -> String {
    text.chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '+' || *c == '/' || *c == '=')
        .collect()
}

/// 尝试修复不完整的 JSON 字符串（补齐缺少的 } 或 ]）
fn repair_json(json_str: &str) -> String {
    let mut fixed = json_str.to_string();
    let open_braces = fixed.matches('{').count().saturating_sub(fixed.matches('}').count());
    let open_brackets = fixed.matches('[').count().saturating_sub(fixed.matches(']').count());
    for _ in 0..open_brackets { fixed.push(']'); }
    for _ in 0..open_braces { fixed.push('}'); }
    fixed
}

/// 将修复后的数据写入 `{原名}_repaired.{ext}` 文件
fn write_repaired(original_path: &str, content: &str) -> RepairResult {
    let path = Path::new(original_path);
    let stem = path.file_stem().unwrap_or_default().to_string_lossy();
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    let parent = path.parent().unwrap_or(Path::new("."));
    let repaired_name = if ext.is_empty() {
        format!("{}_repaired", stem)
    } else {
        format!("{}_repaired.{}", stem, ext)
    };
    let repaired_path = parent.join(&repaired_name);
    match fs::write(&repaired_path, content) {
        Ok(()) => RepairResult {
            success: true,
            repaired_path: Some(repaired_path.to_string_lossy().to_string()),
            original_errors: vec![],
            remaining_errors: vec![],
        },
        Err(e) => RepairResult {
            success: false,
            repaired_path: None,
            original_errors: vec![],
            remaining_errors: vec![format!("写入修复文件失败: {}", e)],
        },
    }
}

/// 内部辅助：安全地 push 到 Vec<String>（处理 borrow checker 问题）
fn warnings_push(container: &mut Vec<String>, msg: String) {
    container.push(msg);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use serde_json::json;

    // ── SaveFileInfo 测试 ──

    #[test]
    fn test_get_save_info_unknown_extension() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.xyz");
        fs::write(&path, b"hello").unwrap();
        let info = get_save_info(&path.to_string_lossy());
        assert_eq!(info.format_name, "未知格式");
        assert!(!info.is_valid);
        assert!(info.error.is_some());
    }

    #[test]
    fn test_get_save_info_rpgsave() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("save.rpgsave");
        fs::write(&path, b"AA").unwrap();
        let info = get_save_info(&path.to_string_lossy());
        assert!(info.format_name.contains("RPG Maker"));
        assert_eq!(info.engine, "rpg_mv");
    }

    // ── Integrity 测试 ──

    #[test]
    fn test_check_rpgmaker_valid() {
        let dir = tempfile::tempdir().unwrap();
        let save = json!({
            "party": {"_gold": 1000, "_actors": [{"_actorId":1,"_hp":100,"_mp":50,"_level":5}]},
            "switches": [false, true, false],
            "variables": [0, 42, 0]
        });
        let compressed = crate::lzstring::compress_to_base64(&save.to_string()).unwrap();
        let path = dir.path().join("save.rpgsave");
        fs::write(&path, &compressed).unwrap();
        let result = check_save_integrity(&path.to_string_lossy());
        assert!(result.is_valid, "应通过校验: {:?}", result.errors);
        assert_eq!(result.summary.as_ref().map(|s| s.gold), Some(1000));
    }

    #[test]
    fn test_check_rpgmaker_negative_gold() {
        let dir = tempfile::tempdir().unwrap();
        let save = json!({"party": {"_gold": -500}});
        let compressed = crate::lzstring::compress_to_base64(&save.to_string()).unwrap();
        let path = dir.path().join("save.rpgsave");
        fs::write(&path, &compressed).unwrap();
        let result = check_save_integrity(&path.to_string_lossy());
        assert!(!result.is_valid);
        assert!(result.errors.iter().any(|e| e.contains("负数")));
    }

    #[test]
    fn test_check_unreal_valid() {
        let dir = tempfile::tempdir().unwrap();
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
        data.extend_from_slice(&0u32.to_le_bytes());
        let path = dir.path().join("test.sav");
        fs::write(&path, &data).unwrap();
        let result = check_save_integrity(&path.to_string_lossy());
        assert!(result.is_valid);
        assert_eq!(result.format_name, "Unreal Engine (GVAS)");
    }

    #[test]
    fn test_check_invalid_gvas_magic() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad.sav");
        fs::write(&path, b"XXXX").unwrap();
        let result = check_save_integrity(&path.to_string_lossy());
        assert!(!result.is_valid);
        assert!(result.errors.iter().any(|e| e.contains("魔术")));
    }

    // ── Repair 测试 ──

    #[test]
    fn test_repair_base64_padding() {
        assert_eq!(repair_base64_padding("abc"), "abc=");
        assert_eq!(repair_base64_padding("abcd"), "abcd");
        assert_eq!(repair_base64_padding("ab"), "ab==");
    }

    #[test]
    fn test_clean_base64() {
        assert_eq!(clean_base64("abc^def"), "abcdef");
        assert_eq!(clean_base64("a+b/c=d"), "a+b/c=d");
    }

    #[test]
    fn test_repair_json_unclosed() {
        let fixed = repair_json(r#"{"a":1"#);
        assert_eq!(fixed, r#"{"a":1}"#);
        let fixed2 = repair_json(r#"[1,2"#);
        assert_eq!(fixed2, r#"[1,2]"#);
    }

    #[test]
    fn test_attempt_repair_nonexistent_file() {
        let result = attempt_repair("nonexistent_file.rpgsave");
        assert!(!result.success);
        assert!(result.original_errors.iter().any(|e| e.contains("读取文件失败")));
    }

    // ── Batch 测试 ──

    #[test]
    fn test_batch_check_saves_empty_dir() {
        let dir = tempfile::tempdir().unwrap();
        let results = batch_check_saves(&dir.path().to_string_lossy());
        assert!(results.is_empty());
    }
}
```

- [ ] **Step 2: 运行 core 测试验证**

```bash
cargo test -p game-tool-core -- integrity
```
Expected: 9 tests pass

- [ ] **Step 3: 提交**

```bash
git add crates/core/src/integrity.rs
git commit -m "feat(core): add save integrity check, info, batch scan, repair module"
```

---

### Task 2: 更新 core/src/lib.rs 注册新模块

**Files:**
- Modify: `crates/core/src/lib.rs`

- [ ] **Step 1: 添加模块声明**

在 `crates/core/src/lib.rs` 中，紧接 `pub mod error;` 之后添加：

```rust
/// 存档完整性检查、信息查看与修复工具模块
pub mod integrity;
```

- [ ] **Step 2: 验证编译**

```bash
cargo build -p game-tool-core
```
Expected: 编译成功

- [ ] **Step 3: 提交**

```bash
git add crates/core/src/lib.rs
git commit -m "feat(core): register integrity module in lib.rs"
```

---

### Task 3: 更新 GUI 状态 (state.rs)

**Files:**
- Modify: `crates/gui/src/state.rs`

- [ ] **Step 1: 扩展 ToolboxState**

将 `ToolboxState` 结构体（第 98-104 行）替换为：

```rust
/// 工具箱面板的状态数据
pub struct ToolboxState {
    // --- LZString 工具 ---
    pub lz_input: String,
    pub lz_output: String,
    pub lz_error: String,
    // --- Base64 工具 ---
    pub b64_input: String,
    pub b64_output: String,
    // --- 存档信息查看器 ---
    pub info_path: String,
    pub info_result: Option<game_tool_core::integrity::SaveFileInfo>,
    // --- 完整性检查 ---
    pub check_path: String,
    pub check_result: Option<game_tool_core::integrity::IntegrityResult>,
    // --- 批量检查 ---
    pub batch_dir: String,
    pub batch_results: Vec<game_tool_core::integrity::IntegrityResult>,
    // --- 存档修复 ---
    pub repair_path: String,
    pub repair_result: Option<game_tool_core::integrity::RepairResult>,
}
```

- [ ] **Step 2: 新增 ToolboxAction 枚举**

在 `RealtimeConnection` 结构体（约第 95 行）之后添加：

```rust
/// 工具箱面板的用户操作指令
pub enum ToolboxAction {
    /// 获取存档文件基本信息
    GetSaveInfo(String),
    /// 执行存档完整性检查
    IntegrityCheck(String),
    /// 对目录执行批量完整性检查
    BatchCheck(String),
    /// 尝试修复损坏的存档文件
    RepairSave(String),
    /// 清除完整性检查结果
    ClearCheck,
    /// 清除批量检查结果
    ClearBatch,
    /// 清除修复结果
    ClearRepair,
}
```

- [ ] **Step 3: 验证编译**

```bash
cargo build -p game-tool-gui
```
Expected: 编译成功

- [ ] **Step 4: 提交**

```bash
git add crates/gui/src/state.rs
git commit -m "feat(gui): expand ToolboxState and add ToolboxAction enum"
```

---

### Task 4: 重写 toolbox.rs UI

**Files:**
- Modify: `crates/gui/src/panels/toolbox.rs`

- [ ] **Step 1: 使用 Write 工具完整重写文件**

**文件路径**: `crates/gui/src/panels/toolbox.rs`

完整内容见下方：

```rust
//! 工具箱面板，提供独立于游戏的实用工具集合。
//!
//! # 工具列表
//! 1. **LZString 压缩/解压** — 处理 RPG Maker MV 存档的 LZString + Base64 格式
//! 2. **Base64 编解码** — 通用 Base64 编码/解码工具
//! 3. **存档信息查看器** — 快速查看存档文件格式、大小、修改时间等元信息
//! 4. **存档完整性检查** — 深度格式校验 + 数据逻辑检查
//! 5. **批量完整性检查** — 扫描目录中所有存档文件，批量校验
//! 6. **存档修复工具** — 尝试修复损坏的 RPG Maker 存档文件

use crate::state::{ToolboxAction, ToolboxState};
use crate::theme::colors;
use egui::Ui;

/// 渲染工具箱面板，返回用户触发的操作列表
pub fn render(ui: &mut Ui, state: &mut ToolboxState) -> Vec<ToolboxAction> {
    let mut actions = Vec::new();

    ui.heading("🧰 工具箱");
    ui.add_space(8.0);

    // ========== LZString 压缩/解压工具 ==========
    render_lzstring_section(ui, state);
    ui.add_space(8.0);

    // ========== Base64 编解码工具 ==========
    render_base64_section(ui, state);
    ui.add_space(8.0);

    // ========== 存档信息查看器 ==========
    render_save_info_section(ui, state, &mut actions);
    ui.add_space(8.0);

    // ========== 存档完整性检查 ==========
    render_integrity_section(ui, state, &mut actions);
    ui.add_space(8.0);

    // ========== 批量完整性检查 ==========
    render_batch_section(ui, state, &mut actions);
    ui.add_space(8.0);

    // ========== 存档修复工具 ==========
    render_repair_section(ui, state, &mut actions);

    actions
}

/// LZString 压缩/解压区块
fn render_lzstring_section(ui: &mut Ui, state: &mut ToolboxState) {
    egui::CollapsingHeader::new("🗜 LZString 压缩/解压")
        .default_open(true)
        .show(ui, |ui| {
            ui.colored_label(colors::TEXT_SECONDARY, "RPG Maker MV 存档使用的 LZString + Base64 格式");
            ui.add_space(4.0);
            ui.label("输入 (JSON 文本或 Base64 压缩文本):");
            ui.add_sized(
                [ui.available_width(), 100.0],
                egui::TextEdit::multiline(&mut state.lz_input),
            );
            ui.horizontal(|ui| {
                if ui.button("压缩").clicked() {
                    match game_tool_core::lzstring::compress_to_base64(&state.lz_input) {
                        Ok(r) => { state.lz_output = r; state.lz_error.clear(); }
                        Err(e) => { state.lz_error = format!("{:?}", e); }
                    }
                }
                if ui.button("解压").clicked() {
                    match game_tool_core::lzstring::decompress_from_base64(&state.lz_input) {
                        Ok(r) => { state.lz_output = r; state.lz_error.clear(); }
                        Err(e) => { state.lz_error = format!("{:?}", e); }
                    }
                }
                if !state.lz_output.is_empty() && ui.button("📋 复制").clicked() {
                    ui.ctx().copy_text(state.lz_output.clone());
                }
            });
            if !state.lz_output.is_empty() {
                ui.colored_label(colors::SUCCESS, "结果:");
                ui.label(&state.lz_output);
            }
            if !state.lz_error.is_empty() {
                ui.colored_label(colors::ERROR, &state.lz_error);
            }
        });
}

/// Base64 编解码区块
fn render_base64_section(ui: &mut Ui, state: &mut ToolboxState) {
    egui::CollapsingHeader::new("🔤 Base64 编解码")
        .default_open(false)
        .show(ui, |ui| {
            ui.label("输入:");
            ui.add_sized(
                [ui.available_width(), 100.0],
                egui::TextEdit::multiline(&mut state.b64_input),
            );
            ui.horizontal(|ui| {
                if ui.button("编码").clicked() {
                    state.b64_output = game_tool_core::base64::encode(state.b64_input.as_bytes());
                }
                if ui.button("解码").clicked() {
                    if let Some(bytes) = game_tool_core::base64::decode(&state.b64_input) {
                        state.b64_output = String::from_utf8_lossy(&bytes).to_string();
                    } else {
                        state.b64_output = "解码失败: 无效的 Base64 输入".into();
                    }
                }
                if !state.b64_output.is_empty() && ui.button("📋 复制").clicked() {
                    ui.ctx().copy_text(state.b64_output.clone());
                }
            });
            if !state.b64_output.is_empty() {
                ui.label(format!("结果: {}", state.b64_output));
            }
        });
}

/// 存档信息查看器区块
fn render_save_info_section(ui: &mut Ui, state: &mut ToolboxState, actions: &mut Vec<ToolboxAction>) {
    egui::CollapsingHeader::new("📄 存档信息查看器")
        .default_open(false)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("文件路径:");
                ui.add(egui::TextEdit::singleline(&mut state.info_path).hint_text("选择存档文件..."));
                if ui.button("选择").clicked() {
                    if let Some(path) = rfd::FileDialog::new().set_title("选择存档文件").pick_file() {
                        state.info_path = path.to_string_lossy().to_string();
                    }
                }
                if !state.info_path.is_empty() && ui.button("查看").clicked() {
                    actions.push(ToolboxAction::GetSaveInfo(state.info_path.clone()));
                }
            });
            if let Some(ref info) = state.info_result {
                ui.separator();
                ui.label(format!("格式: {}", info.format_name));
                ui.label(format!("引擎: {}", info.engine));
                ui.label(format!("大小: {} 字节", info.file_size));
                ui.label(format!("修改时间: {}", info.modified));
                if info.is_valid {
                    ui.colored_label(colors::SUCCESS, "状态: 有效");
                } else {
                    ui.colored_label(colors::ERROR, format!("状态: 无效 — {}", info.error.as_deref().unwrap_or("")));
                }
            }
        });
}

/// 存档完整性检查区块
fn render_integrity_section(ui: &mut Ui, state: &mut ToolboxState, actions: &mut Vec<ToolboxAction>) {
    egui::CollapsingHeader::new("🔍 存档完整性检查")
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("文件路径:");
                ui.add(egui::TextEdit::singleline(&mut state.check_path).hint_text("选择存档文件..."));
                if ui.button("选择").clicked() {
                    if let Some(path) = rfd::FileDialog::new().set_title("选择存档文件").pick_file() {
                        state.check_path = path.to_string_lossy().to_string();
                    }
                }
                if !state.check_path.is_empty() && ui.button("检查").clicked() {
                    actions.push(ToolboxAction::IntegrityCheck(state.check_path.clone()));
                }
            });

            if let Some(ref result) = state.check_result {
                ui.separator();
                ui.heading("检查结果");
                ui.label(format!("文件: {}", result.file_path));
                ui.label(format!("格式: {}", result.format_name));
                ui.label(format!("大小: {} 字节", result.file_size));
                ui.label(format!("字段数: {}", result.field_count));
                if let Some(ref s) = result.summary {
                    ui.label(format!("金币: {}", s.gold));
                    ui.label(format!("游玩时间: {} 秒", s.play_time));
                }

                if result.is_valid {
                    ui.colored_label(colors::SUCCESS, "✓ 格式校验通过");
                } else {
                    ui.colored_label(colors::ERROR, "✗ 格式校验失败");
                }

                // 显示错误列表
                if !result.errors.is_empty() {
                    ui.colored_label(colors::ERROR, "错误:");
                    for e in &result.errors {
                        ui.colored_label(colors::ERROR, format!("  • {}", e));
                    }
                }

                // 显示警告列表
                if !result.warnings.is_empty() {
                    ui.colored_label(colors::WARNING, "警告:");
                    for w in &result.warnings {
                        ui.colored_label(colors::WARNING, format!("  • {}", w));
                    }
                }

                if ui.button("清除结果").clicked() {
                    actions.push(ToolboxAction::ClearCheck);
                }
            }
        });
}

/// 批量完整性检查区块
fn render_batch_section(ui: &mut Ui, state: &mut ToolboxState, actions: &mut Vec<ToolboxAction>) {
    egui::CollapsingHeader::new("📂 批量完整性检查")
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("目录:");
                ui.add(egui::TextEdit::singleline(&mut state.batch_dir).hint_text("选择包含存档的目录..."));
                if ui.button("选择").clicked() {
                    if let Some(dir) = rfd::FileDialog::new().set_title("选择存档目录").pick_folder() {
                        state.batch_dir = dir.to_string_lossy().to_string();
                    }
                }
                if !state.batch_dir.is_empty() && ui.button("扫描").clicked() {
                    actions.push(ToolboxAction::BatchCheck(state.batch_dir.clone()));
                }
            });

            if !state.batch_results.is_empty() {
                ui.separator();
                ui.label(format!("扫描结果: 共 {} 个文件", state.batch_results.len()));

                // 表格头部
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("文件名").strong());
                    ui.add_space(20.0);
                    ui.label(egui::RichText::new("格式").strong());
                    ui.add_space(20.0);
                    ui.label(egui::RichText::new("状态").strong());
                });
                ui.separator();

                // 表格行
                for result in &state.batch_results {
                    let fname = std::path::Path::new(&result.file_path)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or(&result.file_path);
                    ui.horizontal(|ui| {
                        ui.label(fname);
                        ui.add_space(20.0);
                        ui.label(&result.format_name);
                        ui.add_space(20.0);
                        if result.is_valid {
                            ui.colored_label(colors::SUCCESS, "✓");
                        } else {
                            ui.colored_label(colors::ERROR, "✗");
                        }
                    });
                }

                if ui.button("清除结果").clicked() {
                    actions.push(ToolboxAction::ClearBatch);
                }
            }
        });
}

/// 存档修复工具区块
fn render_repair_section(ui: &mut Ui, state: &mut ToolboxState, actions: &mut Vec<ToolboxAction>) {
    egui::CollapsingHeader::new("🔧 存档修复工具")
        .show(ui, |ui| {
            ui.colored_label(colors::TEXT_SECONDARY, "适用于 RPG Maker MV/MZ 存档 (.rpgsave/.rmmzsave)");
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label("文件路径:");
                ui.add(egui::TextEdit::singleline(&mut state.repair_path).hint_text("选择损坏的存档文件..."));
                if ui.button("选择").clicked() {
                    if let Some(path) = rfd::FileDialog::new().set_title("选择损坏的存档文件").pick_file() {
                        state.repair_path = path.to_string_lossy().to_string();
                    }
                }
                if !state.repair_path.is_empty() && ui.button("修复").clicked() {
                    actions.push(ToolboxAction::RepairSave(state.repair_path.clone()));
                }
            });

            if let Some(ref result) = state.repair_result {
                ui.separator();
                if result.success {
                    ui.colored_label(colors::SUCCESS, "✓ 修复成功");
                    if let Some(ref p) = result.repaired_path {
                        ui.label(format!("修复后文件: {}", p));
                    }
                } else {
                    ui.colored_label(colors::ERROR, "✗ 修复失败");
                    if !result.original_errors.is_empty() {
                        for e in &result.original_errors {
                            ui.colored_label(colors::ERROR, format!("  • {}", e));
                        }
                    }
                }
                if !result.remaining_errors.is_empty() {
                    ui.colored_label(colors::WARNING, "残留问题:");
                    for e in &result.remaining_errors {
                        ui.colored_label(colors::WARNING, format!("  • {}", e));
                    }
                }
                if ui.button("清除结果").clicked() {
                    actions.push(ToolboxAction::ClearRepair);
                }
            }
        });
}
```

- [ ] **Step 2: 编译验证**

```bash
cargo build -p game-tool-gui
```
Expected: 编译成功（可能因 app.rs 尚未更新而有未使用导入的警告，Task 5 修复）

- [ ] **Step 3: 提交**

```bash
git add crates/gui/src/panels/toolbox.rs
git commit -m "feat(gui): rewrite toolbox panel with 6 functional sections"
```

---

### Task 5: 更新 app.rs（工具箱分发 + 代码优化）

**Files:**
- Modify: `crates/gui/src/app.rs`

此任务分 4 个子步骤。

- [ ] **Step 5a: Unicode 转义 → 实际中文**

将 `app.rs` 中所有 `\u{xxxx}` 序列替换为实际中文字符。使用编辑器全局替换以下模式：

| Unicode 转义 | 替换为 | 出现位置 |
|---|---|---|
| `\u{52a0}\u{8f7d}\u{5b58}\u{6863}\u{5931}\u{8d25}` | 加载存档失败 | `load_save_file()` |
| `\u{672a}\u{9009}\u{62e9}\u{5b58}\u{6863}\u{6587}\u{4ef6}` | 未选择存档文件 | `save_current()` |
| `\u{5b58}\u{6863}\u{6570}\u{636e}\u{4e3a}\u{7a7a}` | 存档数据为空 | `save_current()` |
| `\u{5199}\u{5165}\u{5b57}\u{6bb5}` | 写入字段 | `save_current()` |
| `\u{5931}\u{8d25}` | 失败 | `save_current()` 多处 |
| `\u{5b58}\u{6863}\u{5df2}\u{4fdd}\u{5b58}` | 存档已保存 | `save_current()` |
| `\u{4fdd}\u{5b58}\u{5931}\u{8d25}` | 保存失败 | `save_current()` |
| `\u{9009}\u{62e9}\u{6e38}\u{620f}\u{76ee}\u{5f55}` | 选择游戏目录 | `switch_game()` |
| `\u{672a}\u{9009}\u{62e9}\u{6e38}\u{620f}\u{76ee}\u{5f55}` | 未选择游戏目录 | `inject_plugin()` |
| `\u{6ce8}\u{5165}\u{5931}\u{8d25}` | 注入失败 | `inject_plugin()` |
| `\u{4e0d}\u{652f}\u{6301}` | 不支持 | `inject_plugin()` |
| `\u{5907}\u{4efd}` 相关 | 备份 | `create_backup()` 等 |
| `\u{521b}\u{5efa}\u{5907}\u{4efd}\u{5931}\u{8d25}` | 创建备份失败 | `create_backup()` |
| `\u{6062}\u{590d}\u{5931}\u{8d25}` | 恢复失败 | `restore_backup()` |
| `\u{5220}\u{9664}\u{5931}\u{8d25}` | 删除失败 | `delete_backup()` |
| `\u{8fde}\u{63a5}\u{5931}\u{8d25}` | 连接失败 | `drain_rt_results()` |
| `\u{672a}\u{8fde}\u{63a5}` | 未连接 | `drain_rt_results()` |
| `\u{2713} \u{5df2}\u{5199}\u{5165}` | ✓ 已写入 | `drain_rt_results()` |
| 全部 `\u{xxxx}` 序列 | 实际中文 | GUI 面板按钮/标签 |

**关键**：这是纯文本替换，不改变代码逻辑。把所有 `\u{xxxx}` 序列替换为实际中文字符。可以使用 rust 编译器或在线工具解码这些码点。

- [ ] **Step 5b: 拆分 AppState::new()**

提取 3 个私有辅助函数以简化 `new()` 方法。

在 `impl AppState` 块中，在 `pub fn new()` 之前添加：

```rust
    /// 根据游戏目录检测引擎类型
    fn detect_engine(game_dir: &Option<String>) -> EngineType {
        game_dir
            .as_ref()
            .map(|d| detect_by_filesystem(d))
            .unwrap_or(EngineType::Unknown)
    }

    /// 扫描游戏目录获取游戏配置
    fn load_game_config(game_dir: &Option<String>, engine: &EngineType) -> (Option<game_tool_rpgmaker::scanner::GameConfig>, String) {
        let (config, title) = if let Some(ref dir) = game_dir {
            if *engine != EngineType::Unknown {
                let gc = game_tool_rpgmaker::scanner::scan_game_directory(dir);
                let title = if gc.data_loaded { gc.game_title.clone() } else { String::new() };
                (if gc.data_loaded { Some(gc) } else { None }, title)
            } else {
                (None, String::new())
            }
        } else {
            (None, String::new())
        };
        (config, title)
    }

    /// 初始化存档编辑面板
    fn init_save_panel(game_dir: &Option<String>, engine: &EngineType) -> SavePanelState {
        let panel_mode = factory::engine_to_panel_mode(engine);
        let readonly = false;
        let format = create_format(engine);
        let save_files = if let (Some(ref dir), Some(ref fmt)) = (game_dir, &format) {
            discovery::find_save_files(dir, &**fmt)
        } else {
            Vec::new()
        };
        SavePanelState {
            format,
            save_files,
            panel_mode,
            readonly,
            selected_save: None,
            save_data: None,
            summary: None,
            fields: Vec::new(),
            dirty_count: 0,
            selected_category: None,
            search_query: String::new(),
            jump_id: String::new(),
        }
    }

    /// 初始化实时编辑面板
    fn init_rt_panel(game_dir: &Option<String>, engine: &EngineType, config: &game_tool_core::config::AppConfig) -> RtPanelState {
        let port = config.tcp_port;
        let plugin_installed = if factory::supports_realtime(engine) {
            if let Some(ref dir) = game_dir {
                match engine {
                    EngineType::RpgMakerMv | EngineType::RpgMakerMz | EngineType::NwJs => {
                        game_tool_rpgmaker::tcp::is_plugin_installed(dir)
                    }
                    EngineType::RenPy => game_tool_renpy::bridge::is_plugin_installed(dir),
                    _ => false,
                }
            } else { false }
        } else { false };

        let bridge_mode = if matches!(engine, EngineType::Unreal | EngineType::UnityMono | EngineType::UnityIl2Cpp | EngineType::Godot) {
            BridgeMode::Memory
        } else {
            BridgeMode::Tcp
        };

        RtPanelState {
            conn: None,
            fields: Vec::new(),
            plugin_installed,
            host: "127.0.0.1".into(),
            port,
            error_message: String::new(),
            error_expires_at: None,
            write_feedback: String::new(),
            write_feedback_expires_at: None,
            search_query: String::new(),
            selected_category: None,
            jump_id: String::new(),
            auto_refresh: true,
            locked_fields: std::collections::HashSet::new(),
            refresh_interval_secs: 3,
            last_refresh: None,
            bridge_mode,
            process_list: Vec::new(),
            selected_process: None,
            scan_value: String::new(),
            scan_value_type: game_tool_memory::ValueType::I32,
            scan_results: Vec::new(),
            scan_count: 0,
            next_scan_mode: 0,
            scan_in_progress: false,
            field_seeds: Vec::new(),
            save_fields_snapshot: Vec::new(),
        }
    }
```

然后替换 `pub fn new()` 的完整实现为：

```rust
    pub fn new(game_dir: Option<String>) -> Self {
        let config = load_config().unwrap_or_default();
        let engine = Self::detect_engine(&game_dir);
        let (game_config, game_title) = Self::load_game_config(&game_dir, &engine);

        let port = config.tcp_port;
        let dark_mode = config.dark_mode;
        let save_panel = Self::init_save_panel(&game_dir, &engine);
        let rt_panel = Self::init_rt_panel(&game_dir, &engine, &config);

        let toolbox = ToolboxState {
            lz_input: String::new(),
            lz_output: String::new(),
            lz_error: String::new(),
            b64_input: String::new(),
            b64_output: String::new(),
            info_path: String::new(),
            info_result: None,
            check_path: String::new(),
            check_result: None,
            batch_dir: String::new(),
            batch_results: Vec::new(),
            repair_path: String::new(),
            repair_result: None,
        };

        Self {
            game_dir,
            game_title,
            engine,
            game_config,
            active_tab: TabMode::SaveEditor,
            dark_mode,
            recent_games: config.recent_games.clone(),
            backup_paths: Vec::new(),
            backup_selection: std::collections::HashSet::new(),
            save_panel,
            rt_panel,
            toolbox,
            status_message: String::new(),
            show_unsaved_dialog: false,
            show_confirm_dialog: None,
        }
    }
```

- [ ] **Step 5c: 修复缩进问题**

找到 `engine: engine.clone(),`（当前约第 114 行，缩进异常为 20 空格），修正为 12 空格（与 `game_config` 同级）。

在新的 `new()` 方法中，确保所有字段对齐正确。

- [ ] **Step 5d: 添加 Toolbox 动作分发**

找到 `TabMode::Toolbox` 分支（update 方法中），将其替换为完整的动作分发：

```rust
                    TabMode::Toolbox => {
                        let actions = toolbox::render(ui, &mut self.toolbox);
                        for action in actions {
                            match action {
                                ToolboxAction::GetSaveInfo(path) => {
                                    self.toolbox.info_result =
                                        Some(game_tool_core::integrity::get_save_info(&path));
                                }
                                ToolboxAction::IntegrityCheck(path) => {
                                    self.toolbox.check_result =
                                        Some(game_tool_core::integrity::check_save_integrity(&path));
                                }
                                ToolboxAction::BatchCheck(dir) => {
                                    self.toolbox.batch_results =
                                        game_tool_core::integrity::batch_check_saves(&dir);
                                }
                                ToolboxAction::RepairSave(path) => {
                                    self.toolbox.repair_result =
                                        Some(game_tool_core::integrity::attempt_repair(&path));
                                }
                                ToolboxAction::ClearCheck => {
                                    self.toolbox.check_result = None;
                                }
                                ToolboxAction::ClearBatch => {
                                    self.toolbox.batch_results.clear();
                                }
                                ToolboxAction::ClearRepair => {
                                    self.toolbox.repair_result = None;
                                }
                            }
                        }
                    }
```

**同时**: 确保 `app.rs` 文件头部导入了必要的类型：
```rust
use crate::state::{
    ..., ToolboxAction, ...  // 新增 ToolboxAction
};
```

已存在 `use crate::panels::toolbox;` 导入。

- [ ] **Step 5e: 编译验证**

```bash
cargo build -p game-tool-gui
```
Expected: 编译成功

- [ ] **Step 5f: 提交**

```bash
git add crates/gui/src/app.rs
git commit -m "refactor(gui): add toolbox actions, unicode to chinese, split new(), fix indent"
```

---

### Task 6: 移除 is_readonly() 死代码 (factory.rs)

**Files:**
- Modify: `crates/gui/src/factory.rs`

- [ ] **Step 1: 移除 is_readonly 函数及其测试**

在 `factory.rs` 中删除 `is_readonly()` 函数（第 60-65 行）。

在 `factory.rs` 的测试模块中删除 `test_is_readonly_none` 测试函数（第 335-345 行）。

修改 `init_rt_panel` 中已不再使用 `is_readonly`（在新 `new()` 中已用 `false` 替代）。

- [ ] **Step 2: 编译 + 测试验证**

```bash
cargo build -p game-tool-gui
cargo test -p game-tool-gui
```
Expected: 所有测试通过

- [ ] **Step 3: 提交**

```bash
git add crates/gui/src/factory.rs
git commit -m "refactor(gui): remove dead is_readonly() function"
```

---

### Task 7: 更新文档

**Files:**
- Modify: `README.md`
- Modify: `docs/ARCHITECTURE.md`

- [ ] **Step 1: 更新 README.md 工具列表**

将 README 中的 `- **工具箱** — LZString 压缩/解压、Base64 编解码、存档完整性检查` 替换为：

```markdown
- **工具箱** — LZString 压缩/解压、Base64 编解码、存档信息查看、完整性检查、批量扫描、存档修复
```

- [ ] **Step 2: 更新 ARCHITECTURE.md 工具箱描述**

将 `| | toolbox.rs | 工具箱标签（LZString/Base64/完整性检查/目录扫描） |` 替换为：

```markdown
| | toolbox.rs | 工具箱标签（LZString/Base64/存档信息/完整性检查/批量扫描/修复） |
```

- [ ] **Step 3: 提交**

```bash
git add README.md docs/ARCHITECTURE.md
git commit -m "docs: update toolbox features to reflect implemented functions"
```

---

### Task 8: 构建验证 + 测试 + 推送

- [ ] **Step 1: 完整测试**

```bash
cargo test -p game-tool-core
cargo test -p game-tool-gui
cargo build --release -p game-tool-gui
```
Expected: 所有测试通过，Release 构建成功

- [ ] **Step 2: 推送代码**

```bash
git push origin master
```

- [ ] **Step 3: 打 tag + Release**

```bash
git tag v1.2.0
git push origin v1.2.0
gh release create v1.2.0 "dist\GameSaveEditor.exe" "dist\GameSaveEditor.pdb" --title "v1.2.0 - 工具箱完整性检查与代码优化" --notes "## v1.2.0 更新

### 新功能（工具箱）
- 存档信息查看器：快速查看存档格式、大小、修改时间
- 存档完整性检查：格式校验 + 8 条数据逻辑检查
- 批量完整性检查：扫描目录中所有存档文件
- 存档修复工具：Base64 补齐/去噪/JSON 修复

### 代码优化
- app.rs：Unicode 转义替换为实际中文
- 拆分 AppState::new() 为 4 个辅助函数
- 移除 dead code: is_readonly()
- 修复缩进问题

### 文档
- 更新 README 和 ARCHITECTURE 文档"
```

---

### Task 9: 最终验证清单

- [ ] `cargo build -p game-tool-gui` 编译成功
- [ ] `cargo test -p game-tool-core` 全部通过
- [ ] `cargo test -p game-tool-gui` 全部通过
- [ ] `cargo build --release -p game-tool-gui` Release 构建成功
- [ ] `dist/GameSaveEditor.exe` 可正常启动
- [ ] 工具箱 6 个功能在 UI 中显示正常
- [ ] 档案信息查看器可正常使用
- [ ] 完整性检查可正常使用
- [ ] 批量检查可正常使用
- [ ] 修复工具可正常使用
```
