# Plan 2: RPG Maker Engine Crate

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement RPG Maker MV/MZ save format handler (RpgMakerFormat), game data scanner, and field merge logic in the `engines/rpgmaker` crate. Restore jsonex module with all tests.

**Architecture:** `rpgmaker` crate implements `SaveFormat` trait from `core`. Internal modules: `jsonex` (JsonEx parser), `format` (RpgMakerFormat + save I/O), `gamedata` (System.json scanner), `scanner` (field merge). Depends on `core` for types/traits, `infra` for networking (future).

**Tech Stack:** Rust, serde_json, lz-str, core crate types

**Dependency:** Plan 1 (foundation restructuring) must be complete.

---

## File Map

| Action | Path | Responsibility |
|--------|------|----------------|
| CREATE | `crates/engines/rpgmaker/src/jsonex.rs` | JsonEx @c/@a parser (restored from deleted core) |
| CREATE | `crates/engines/rpgmaker/tests/jsonex_roundtrip.rs` | JsonEx integration tests (restored) |
| CREATE | `crates/engines/rpgmaker/src/format.rs` | RpgMakerFormat: SaveFormat impl |
| CREATE | `crates/engines/rpgmaker/src/gamedata.rs` | Game data scanner (System.json, Actors.json etc.) |
| CREATE | `crates/engines/rpgmaker/src/scanner.rs` | Field merge (config + save + live) |
| MODIFY | `crates/engines/rpgmaker/src/lib.rs` | Module declarations + re-exports |

---

### Task 1: Restore jsonex module in rpgmaker crate

**Files:**
- Create: `crates/engines/rpgmaker/src/jsonex.rs`
- Create: `crates/engines/rpgmaker/tests/jsonex_roundtrip.rs`
- Modify: `crates/engines/rpgmaker/src/lib.rs`

- [ ] **Step 1: Write jsonex.rs**

File: `crates/engines/rpgmaker/src/jsonex.rs`

```rust
//! RPG Maker MV/MZ JsonEx 序列化/反序列化格式处理
//!
//! RPG Maker MV/MZ 使用 JsonEx 格式扩展标准 JSON：
//! - `@c`: 压缩引用数组，指向 `@a` 中的实际数据
//! - `@a`: 实际数组内容（可能是列表或稀疏字典）
//! - 纯列表: 没有 @c/@a 包装的普通 JSON 数组
//! - 稀疏字典: `{"@a": {"1": true, "5": false}}` 表示索引 1=true，索引 5=false
//!
//! ## JsonEx 元数据保留规则
//!
//! 修改非 @c 引用的字段（如 gold）时，@c 引用池必须完整保留。
//! 只有修改了 @c 直接引用的数据时才需要更新 @c 内容。

use serde_json::{Map, Value};

/// 元数据键前缀 — 以 @ 开头的键不是实际数据
const META_PREFIX: char = '@';

/// 最大递归深度，防止无限循环
const MAX_DEPTH: usize = 20;

// ── JsonEx 检测 ─────────────────────────────────────────

/// 递归检测数据中是否包含 JsonEx @c 压缩数组格式
pub fn has_json_ex_c_format(data: &Value) -> bool {
    has_json_ex_c_format_inner(data, 0)
}

fn has_json_ex_c_format_inner(data: &Value, depth: usize) -> bool {
    if depth > MAX_DEPTH {
        return false;
    }
    match data {
        Value::Object(map) => {
            if map.contains_key("@c") {
                return true;
            }
            for v in map.values() {
                if has_json_ex_c_format_inner(v, depth + 1) {
                    return true;
                }
            }
            false
        }
        Value::Array(arr) => {
            for v in arr {
                if has_json_ex_c_format_inner(v, depth + 1) {
                    return true;
                }
            }
            false
        }
        _ => false,
    }
}

// ── @a 数组解析 ─────────────────────────────────────────

/// 判断一个值是否为 JsonEx 稀疏字典格式：
/// `{"@a": {"1": true, "5": false}}`
pub fn is_sparse_dict(data: &Value) -> bool {
    match data {
        Value::Object(map) => map.contains_key("@a") && !map.contains_key("@c"),
        _ => false,
    }
}

enum ArrayOrSparse {
    Array(Vec<Value>),
    Sparse(Vec<Option<Value>>),
    Plain(Vec<Value>),
}

/// 解析 @a 数组：可能是纯列表或稀疏字典
fn resolve_array_raw(data: &Value) -> ArrayOrSparse {
    match data {
        Value::Object(map) => {
            if let Some(a) = map.get("@a") {
                match a {
                    Value::Array(arr) => ArrayOrSparse::Array(arr.clone()),
                    Value::Object(inner) => {
                        let mut indices: Vec<(usize, Value)> = inner
                            .iter()
                            .filter_map(|(k, v)| k.parse::<usize>().ok().map(|i| (i, v.clone())))
                            .collect();
                        if indices.is_empty() {
                            return ArrayOrSparse::Sparse(Vec::new());
                        }
                        indices.sort_by_key(|(i, _)| *i);
                        let max = indices.last().map(|(i, _)| *i).unwrap_or(0);
                        let mut result = vec![None; max + 1];
                        for (i, v) in indices {
                            if i < result.len() {
                                result[i] = Some(v);
                            }
                        }
                        ArrayOrSparse::Sparse(result)
                    }
                    _ => ArrayOrSparse::Plain(Vec::new()),
                }
            } else {
                ArrayOrSparse::Plain(Vec::new())
            }
        }
        Value::Array(arr) => ArrayOrSparse::Array(arr.clone()),
        _ => ArrayOrSparse::Plain(Vec::new()),
    }
}

/// 将稀疏字典展开为密集 Vec（None 填充空位）
fn sparse_dict_to_vec(data: &Value) -> Vec<Option<Value>> {
    match resolve_array_raw(data) {
        ArrayOrSparse::Sparse(v) => v,
        ArrayOrSparse::Array(arr) => arr.into_iter().map(Some).collect(),
        ArrayOrSparse::Plain(v) => v.into_iter().map(Some).collect(),
    }
}

/// 将稀疏字典展开为连续列表（跳过空位）
fn sparse_dict_to_vec_expand(data: &Value) -> Vec<Value> {
    match resolve_array_raw(data) {
        ArrayOrSparse::Sparse(v) => v.into_iter().flatten().collect(),
        ArrayOrSparse::Array(arr) => arr,
        ArrayOrSparse::Plain(v) => v,
    }
}

/// 将 @a 数组解析为平铺 Vec<Value>
pub fn resolve_array(data: &Value) -> Vec<Value> {
    sparse_dict_to_vec_expand(data)
}

/// 将 @a 数组解析为平铺列表（可包含 None）
pub fn resolve_array_flat(data: &Value) -> Vec<Option<Value>> {
    sparse_dict_to_vec(data)
}

/// 判断键是否为元数据键（以 @ 开头）
pub fn is_meta_key(key: &str) -> bool {
    key.starts_with(META_PREFIX)
}

/// 过滤掉所有元数据键（@ 开头的键）
pub fn filter_meta_keys(map: &Map<String, Value>) -> Map<String, Value> {
    map.iter()
        .filter(|(k, _)| !is_meta_key(k))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

// ── 可变操作: ensure 系列 ─────────────────────────────

/// 确保 data["switches"] 是可变的 @a 数组结构
/// 处理 `@c` + `@a` 格式和纯列表格式
pub fn ensure_switches_array(data: &mut Map<String, Value>) -> &mut Vec<Value> {
    ensure_data_array_inner(data, "switches")
}

/// 确保 data["variables"] 是可变的 @a 数组结构
pub fn ensure_variables_array(data: &mut Map<String, Value>) -> &mut Vec<Value> {
    ensure_data_array_inner(data, "variables")
}

fn ensure_data_array_inner<'a>(
    data: &'a mut Map<String, Value>,
    key: &str,
) -> &'a mut Vec<Value> {
    let entry = data.entry(key.to_string()).or_insert_with(|| Value::Array(Vec::new()));

    // 如果是数组格式，直接返回可变引用
    if let Value::Array(arr) = entry {
        return arr;
    }

    // 如果是对象格式（含 @a），从 @a 中提取数组
    if let Value::Object(map) = entry {
        let preserved: Map<String, Value> = map
            .iter()
            .filter(|(k, _)| is_meta_key(k))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();

        let array = if let Some(a) = map.get("@a") {
            match a {
                Value::Array(arr) => arr.clone(),
                Value::Object(inner) => {
                    let max_idx = inner
                        .keys()
                        .filter_map(|k| k.parse::<usize>().ok())
                        .max()
                        .unwrap_or(0);
                    let mut result = vec![Value::Null; max_idx + 1];
                    for (k, v) in inner {
                        if let Ok(idx) = k.parse::<usize>() {
                            if idx < result.len() {
                                result[idx] = v.clone();
                            }
                        }
                    }
                    result
                }
                _ => Vec::new(),
            }
        } else {
            Vec::new()
        };

        let meta = Value::Object(preserved);
        let a = Value::Array(array);

        let mut new_map = Map::new();
        // 保留其他元数据
        for (k, v) in map.iter() {
            if is_meta_key(k) && k != "@a" {
                new_map.insert(k.clone(), v.clone());
            }
        }
        new_map.insert("@a".to_string(), a);

        // 保留其他数据键（不是 @开头的）
        for (k, v) in map.iter() {
            if !is_meta_key(k) && k != "@a" {
                new_map.insert(k.clone(), v.clone());
            }
        }

        *entry = Value::Object(new_map);

        // 现在 entry 是 Object，再次提取数组引用
        if let Value::Object(m) = entry {
            if let Some(arr_val) = m.get_mut("@a") {
                if let Value::Array(arr) = arr_val {
                    return arr;
                }
            }
        }
    }

    // 兜底：创建一个新数组
    *entry = Value::Array(Vec::new());
    if let Value::Array(arr) = entry {
        arr
    } else {
        unreachable!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_has_json_ex_c_format_true() {
        let data = json!({"@c": [1, 2, 3], "@a": {"1": true}});
        assert!(has_json_ex_c_format(&data));
    }

    #[test]
    fn test_has_json_ex_c_format_nested() {
        let data = json!({"actors": {"@c": [1], "@a": {"1": {"name": "Hero"}}}});
        assert!(has_json_ex_c_format(&data));
    }

    #[test]
    fn test_has_json_ex_c_format_plain() {
        let data = json!({"gold": 1000, "switches": [true, false]});
        assert!(!has_json_ex_c_format(&data));
    }

    #[test]
    fn test_is_sparse_dict_true() {
        let data = json!({"@a": {"1": true, "5": false}});
        assert!(is_sparse_dict(&data));
    }

    #[test]
    fn test_is_sparse_dict_with_c() {
        let data = json!({"@c": [1], "@a": {"1": true}});
        assert!(!is_sparse_dict(&data));
    }

    #[test]
    fn test_resolve_array_plain() {
        let data = json!([1, 2, 3]);
        let result = resolve_array(&data);
        assert_eq!(result, vec![json!(1), json!(2), json!(3)]);
    }

    #[test]
    fn test_resolve_array_with_a_as_list() {
        let data = json!({"@a": [null, true, false]});
        let result = resolve_array(&data);
        assert_eq!(result.len(), 3);
        assert_eq!(result[0], Value::Null);
        assert_eq!(result[1], Value::Bool(true));
        assert_eq!(result[2], Value::Bool(false));
    }

    #[test]
    fn test_resolve_array_with_a_as_sparse() {
        let data = json!({"@a": {"1": true, "5": false}});
        let result = resolve_array(&data);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], Value::Bool(true));
        assert_eq!(result[1], Value::Bool(false));
    }

    #[test]
    fn test_resolve_array_flat_preserves_nulls() {
        let data = json!({"@a": {"0": "first", "2": "third"}});
        let result = resolve_array_flat(&data);
        assert_eq!(result.len(), 3);
        assert_eq!(result[0], Some(json!("first")));
        assert_eq!(result[1], None);
        assert_eq!(result[2], Some(json!("third")));
    }

    #[test]
    fn test_is_meta_key() {
        assert!(is_meta_key("@a"));
        assert!(is_meta_key("@c"));
        assert!(!is_meta_key("gold"));
        assert!(!is_meta_key("switches"));
    }

    #[test]
    fn test_filter_meta_keys() {
        let mut map = Map::new();
        map.insert("@a".into(), json!([1]));
        map.insert("gold".into(), json!(1000));
        map.insert("@c".into(), json!([0]));
        let filtered = filter_meta_keys(&map);
        assert_eq!(filtered.len(), 1);
        assert!(filtered.contains_key("gold"));
    }

    #[test]
    fn test_ensure_switches_plain_list() {
        let mut data = Map::new();
        data.insert("switches".into(), json!([true, false, false]));
        let arr = ensure_switches_array(&mut data);
        assert_eq!(arr.len(), 3);
        assert_eq!(arr[0], json!(true));
    }

    #[test]
    fn test_ensure_switches_from_sparse() {
        let mut data = Map::new();
        let mut switches = Map::new();
        switches.insert("@a".into(), json!({"1": true, "5": false}));
        data.insert("switches".into(), Value::Object(switches));
        let arr = ensure_switches_array(&mut data);
        assert_eq!(arr.len(), 6);
    }

    #[test]
    fn test_ensure_switches_create_new() {
        let mut data = Map::new();
        let arr = ensure_switches_array(&mut data);
        assert!(arr.is_empty());
    }
}
```

- [ ] **Step 2: Write jsonex integration tests**

File: `crates/engines/rpgmaker/tests/jsonex_roundtrip.rs`

```rust
//! JsonEx 往返测试：验证 @c/@a 格式的序列化/反序列化完整性

use game_tool_rpgmaker::jsonex;
use serde_json::{json, Value};

#[test]
fn test_roundtrip_a_plain_array() {
    let data = json!({"@a": [null, true, false, 42, "hello"]});
    let flat = jsonex::resolve_array_flat(&data);
    assert_eq!(flat.len(), 5);
    assert_eq!(flat[0], Some(Value::Null));
    assert_eq!(flat[1], Some(Value::Bool(true)));
    assert_eq!(flat[2], Some(Value::Bool(false)));
    assert_eq!(flat[3], Some(json!(42)));
    assert_eq!(flat[4], Some(json!("hello")));
}

#[test]
fn test_roundtrip_a_sparse_dict() {
    let data = json!({"@a": {"1": true, "5": false}});
    let flat = jsonex::resolve_array_flat(&data);
    assert_eq!(flat.len(), 6);
    assert_eq!(flat[0], None);
    assert_eq!(flat[1], Some(Value::Bool(true)));
    assert_eq!(flat[5], Some(Value::Bool(false)));
}

#[test]
fn test_has_json_ex_c_format_with_c() {
    let data = json!({"@c": [1, 2], "@a": {"1": true}});
    assert!(jsonex::has_json_ex_c_format(&data));
}

#[test]
fn test_has_json_ex_c_format_without_c() {
    let data = json!({"gold": 1000});
    assert!(!jsonex::has_json_ex_c_format(&data));
}

#[test]
fn test_is_sparse_dict_positive() {
    assert!(jsonex::is_sparse_dict(&json!({"@a": {"1": true}})));
}

#[test]
fn test_is_sparse_dict_with_c_is_false() {
    assert!(!jsonex::is_sparse_dict(&json!({"@c": [], "@a": {"1": true}})));
}

#[test]
fn test_ensure_switches_preserves_metadata() {
    let mut data = serde_json::Map::new();
    let mut switches = serde_json::Map::new();
    switches.insert("@c".into(), json!([0]));
    switches.insert("@a".into(), json!({"0": true}));
    data.insert("switches".into(), Value::Object(switches));
    let arr = jsonex::ensure_switches_array(&mut data);
    arr[0] = json!(false);
    assert_eq!(arr[0], json!(false));
    // Verify @c still exists
    if let Value::Object(m) = &data["switches"] {
        assert!(m.contains_key("@c"), "@c metadata should be preserved");
    } else {
        panic!("expected Object");
    }
}

#[test]
fn test_ensure_variables_preserves_roundtrip() {
    let mut data = serde_json::Map::new();
    data.insert("variables".into(), json!([1, 2, 3, 4, 5]));
    {
        let arr = jsonex::ensure_variables_array(&mut data);
        arr[2] = json!(99);
    }
    assert_eq!(data["variables"][2], json!(99));
    assert_eq!(data["variables"][4], json!(5));
}

#[test]
fn test_resolve_array_idempotency() {
    let data = json!({"@a": {"1": true, "3": false}});
    let first = jsonex::resolve_array(&data);
    let second = jsonex::resolve_array(&data);
    assert_eq!(first, second);
}

#[test]
fn test_filter_meta_keys_roundtrip() {
    let mut map = serde_json::Map::new();
    map.insert("@a".into(), json!([1]));
    map.insert("@c".into(), json!([0]));
    map.insert("gold".into(), json!(1000));
    map.insert("steps".into(), json!(500));
    let filtered = jsonex::filter_meta_keys(&map);
    assert_eq!(filtered.len(), 2);
    assert!(filtered.contains_key("gold"));
    assert!(filtered.contains_key("steps"));
    assert!(!filtered.contains_key("@a"));
    assert!(!filtered.contains_key("@c"));
}
```

- [ ] **Step 3: Update lib.rs**

```rust
// game-tool-rpgmaker: RPG Maker MV/MZ 引擎支持

pub mod jsonex;
pub mod format;
pub mod gamedata;
pub mod scanner;
```

- [ ] **Step 4: Verify compilation and tests**

```powershell
cargo test -p game-tool-rpgmaker
```

Expected: all jsonex tests pass (14 unit + 10 integration = 24 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/engines/rpgmaker/
git commit -m "feat(rpgmaker): restore jsonex module with all tests"
```

---

### Task 2: Create format.rs — RpgMakerFormat

**Files:**
- Create: `crates/engines/rpgmaker/src/format.rs`

- [ ] **Step 1: Write format.rs with tests**

File: `crates/engines/rpgmaker/src/format.rs`

```rust
//! RPG Maker MV/MZ 存档格式处理器
//!
//! 实现 SaveFormat trait，处理 .rpgsave / .rmmzsave 文件。

use std::fs;
use std::path::Path;

use serde_json::{Map, Value};
use game_tool_core::{
    ISaveFormat, ModifiableField, SaveSummary, GameToolError,
    backup,
};

use crate::jsonex;

/// RPG Maker MV/MZ 存档格式处理器
pub struct RpgMakerFormat;

impl RpgMakerFormat {
    pub fn new() -> Self {
        Self
    }

    /// 加载存档：读文件 → base64 解码 → LZ-String 解压 → JSON 解析
    fn load_raw(path: &Path) -> Result<Value, GameToolError> {
        let raw = fs::read_to_string(path)
            .map_err(|e| GameToolError::ArchiveLoadError(format!("无法读取文件: {}", e)))?;
        let raw = raw.trim().to_string();
        if raw.is_empty() {
            return Err(GameToolError::ArchiveLoadError("存档文件为空".into()));
        }
        let json_str = game_tool_core::lzstring::decompress_from_base64(&raw)
            .map_err(|e| GameToolError::ArchiveLoadError(format!("LZ-String 解压失败: {}", e)))?;
        if json_str.is_empty() {
            return Err(GameToolError::ArchiveLoadError("解压后数据为空".into()));
        }
        let data: Value = serde_json::from_str(&json_str)
            .map_err(|e| GameToolError::ArchiveLoadError(format!("JSON 解析失败: {}", e)))?;
        Ok(data)
    }

    fn save_raw(path: &Path, data: &Value) -> Result<(), GameToolError> {
        let json_str = serde_json::to_string(data)
            .map_err(|e| GameToolError::ArchiveSaveError(format!("JSON 序列化失败: {}", e)))?;
        let compressed = game_tool_core::lzstring::compress_to_base64(&json_str)
            .map_err(|e| GameToolError::ArchiveSaveError(format!("LZ-String 压缩失败: {}", e)))?;
        fs::write(path, &compressed)
            .map_err(|e| GameToolError::ArchiveSaveError(format!("写入文件失败: {}", e)))?;
        Ok(())
    }
}

impl ISaveFormat for RpgMakerFormat {
    fn name(&self) -> &str {
        "RPG Maker MV/MZ"
    }

    fn extensions(&self) -> Vec<String> {
        vec![".rpgsave".into(), ".rmmzsave".into()]
    }

    fn engine_type(&self) -> &str {
        "rpg_maker_mv"
    }

    fn magic_bytes(&self) -> Option<&[u8]> {
        None
    }

    fn load(&self, filepath: &str) -> Result<Value, GameToolError> {
        Self::load_raw(Path::new(filepath))
    }

    fn save(&self, filepath: &str, data: &Value) -> Result<(), GameToolError> {
        let path = Path::new(filepath);
        let _ = backup::save_backup(path, 10);
        Self::save_raw(path, data)
    }

    fn find_data_dir(&self, game_dir: &str) -> Option<String> {
        let dir = Path::new(game_dir);
        for sub in &["www/data", "data"] {
            let d = dir.join(sub);
            if d.is_dir() && d.join("System.json").is_file() {
                return Some(d.to_string_lossy().to_string());
            }
        }
        None
    }

    fn get_summary(&self, data: &Value) -> SaveSummary {
        let gold = data.get("party")
            .and_then(|p| p.get("_gold"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as i32;

        let party_size = data.get("party")
            .and_then(|p| p.get("_actors"))
            .and_then(|v| v.as_array())
            .map(|a| a.len() as i32)
            .unwrap_or(0);

        let item_count = data.get("party")
            .and_then(|p| p.get("_items"))
            .and_then(|v| v.as_object())
            .map(|m| {
                jsonex::filter_meta_keys(m)
                    .values()
                    .filter(|v| v.as_i64().unwrap_or(0) > 0)
                    .count() as i32
            })
            .unwrap_or(0);

        let save_count = data.get("system")
            .and_then(|s| s.get("_saveCount"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as i32;

        let play_time = data.get("system")
            .and_then(|s| s.get("_playtime"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as i32;

        let members = data.get("party")
            .and_then(|p| p.get("_actors"))
            .and_then(|v| v.as_array())
            .map(|actors| {
                actors.iter()
                    .filter_map(|a| {
                        let id = a.get("_actorId").and_then(|v| v.as_i64()).unwrap_or(0);
                        let name = a.get("_name").and_then(|v| v.as_str()).unwrap_or("???");
                        Some(format!("{}:{}", id, name))
                    })
                    .collect()
            })
            .unwrap_or_default();

        SaveSummary {
            gold,
            party_size,
            item_count,
            save_count,
            play_time,
            members,
            extra: std::collections::HashMap::new(),
        }
    }

    fn scan_fields(&self, data: &Value, game_dir: &str) -> Vec<ModifiableField> {
        crate::scanner::scan_all_modifiable(game_dir, Some(data), None)
            .fields
            .into_iter()
            .map(|f| crate::scanner::to_modifiable_field(&f))
            .collect()
    }

    fn apply_field(&self, data: &mut Value, field: &ModifiableField) -> Result<(), GameToolError> {
        let cat = &field.category;
        match cat.as_str() {
            "gold" => {
                if let Some(party) = data.get_mut("party") {
                    if let Some(obj) = party.as_object_mut() {
                        let amount = field.save_value.as_i64().unwrap_or(0);
                        obj.insert("_gold".into(), Value::Number(amount.into()));
                    }
                }
            }
            "switch" => {
                if let Some(obj) = data.as_object_mut() {
                    let mut map = obj.clone();
                    let arr = jsonex::ensure_switches_array(&mut map);
                    let id = field.item_id as usize;
                    if id >= arr.len() {
                        arr.resize(id + 1, Value::Bool(false));
                    }
                    arr[id] = field.save_value.clone();
                    *obj = map;
                }
            }
            "variable" => {
                if let Some(obj) = data.as_object_mut() {
                    let mut map = obj.clone();
                    let arr = jsonex::ensure_variables_array(&mut map);
                    let id = field.item_id as usize;
                    if id >= arr.len() {
                        arr.resize(id + 1, Value::Number(0.into()));
                    }
                    arr[id] = field.save_value.clone();
                    *obj = map;
                }
            }
            "item" => {
                if let Some(party) = data.pointer_mut("/party/_items") {
                    if let Some(items) = party.as_object_mut() {
                        let filtered = jsonex::filter_meta_keys(items);
                        let key = field.item_id.to_string();
                        let count = field.save_value.as_i64().unwrap_or(0);
                        items.insert(key, Value::Number(count.into()));
                    }
                }
            }
            "actor" => {
                let fid = &field.field_id;
                if fid.ends_with("_hp") {
                    Self::set_actor_stat(data, field.item_id, "_hp", &field.save_value);
                } else if fid.ends_with("_mp") {
                    Self::set_actor_stat(data, field.item_id, "_mp", &field.save_value);
                } else if fid.ends_with("_level") {
                    Self::set_actor_stat(data, field.item_id, "_level", &field.save_value);
                }
            }
            _ => {}
        }
        Ok(())
    }
}

impl RpgMakerFormat {
    fn set_actor_stat(data: &mut Value, actor_id: i32, stat: &str, value: &Value) {
        if let Some(actors) = data.pointer_mut("/party/_actors") {
            if let Some(arr) = actors.as_array_mut() {
                for actor in arr {
                    if let Some(id) = actor.get("_actorId").and_then(|v| v.as_i64()) {
                        if id as i32 == actor_id {
                            if let Some(obj) = actor.as_object_mut() {
                                let val = if value.is_boolean() {
                                    if value.as_bool().unwrap_or(false) { Value::Number(1.into()) } else { Value::Number(0.into()) }
                                } else {
                                    value.clone()
                                };
                                obj.insert(stat.to_string(), val);
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::io::Write;

    fn make_test_data() -> Value {
        json!({
            "party": {
                "_gold": 5000,
                "_actors": [
                    {"_actorId": 1, "_name": "Alice", "_hp": 100, "_mp": 50, "_level": 5},
                    {"_actorId": 2, "_name": "Bob", "_hp": 80, "_mp": 30, "_level": 4}
                ],
                "_items": {
                    "1": 10,
                    "2": 3
                }
            },
            "switches": [true, false, false, true],
            "variables": [0, 42, 0, 100],
            "system": {
                "_saveCount": 7,
                "_playtime": 3600
            }
        })
    }

    #[test]
    fn test_extensions() {
        let fmt = RpgMakerFormat::new();
        assert!(fmt.extensions().contains(&".rpgsave".to_string()));
        assert!(fmt.extensions().contains(&".rmmzsave".to_string()));
    }

    #[test]
    fn test_magic_bytes_is_none() {
        let fmt = RpgMakerFormat::new();
        assert!(fmt.magic_bytes().is_none());
    }

    #[test]
    fn test_get_summary() {
        let fmt = RpgMakerFormat::new();
        let data = make_test_data();
        let summary = fmt.get_summary(&data);
        assert_eq!(summary.gold, 5000);
        assert_eq!(summary.party_size, 2);
        assert_eq!(summary.save_count, 7);
        assert_eq!(summary.play_time, 3600);
        assert_eq!(summary.members.len(), 2);
    }

    #[test]
    fn test_apply_field_gold() {
        let fmt = RpgMakerFormat::new();
        let mut data = make_test_data();
        let field = ModifiableField {
            category: "gold".into(),
            field_id: "gold".into(),
            display_name: "金币".into(),
            save_value: json!(9999),
            ..Default::default()
        };
        fmt.apply_field(&mut data, &field).unwrap();
        assert_eq!(data["party"]["_gold"], json!(9999));
    }

    #[test]
    fn test_apply_field_switch() {
        let fmt = RpgMakerFormat::new();
        let mut data = make_test_data();
        let field = ModifiableField {
            category: "switch".into(),
            field_id: "switch_1".into(),
            display_name: "开关1".into(),
            item_id: 1,
            save_value: json!(true),
            ..Default::default()
        };
        fmt.apply_field(&mut data, &field).unwrap();
    }

    #[test]
    fn test_apply_field_variable() {
        let fmt = RpgMakerFormat::new();
        let mut data = make_test_data();
        let field = ModifiableField {
            category: "variable".into(),
            field_id: "var_2".into(),
            display_name: "变量2".into(),
            item_id: 2,
            save_value: json!(999),
            ..Default::default()
        };
        fmt.apply_field(&mut data, &field).unwrap();
    }

    #[test]
    fn test_load_save_roundtrip() {
        let fmt = RpgMakerFormat::new();
        let data = make_test_data();

        // Save to temp file
        let dir = tempfile::tempdir().unwrap();
        let save_path = dir.path().join("test.rpgsave");
        let path_str = save_path.to_string_lossy().to_string();
        fmt.save(&path_str, &data).unwrap();
        assert!(save_path.exists());

        // Load back
        let loaded = fmt.load(&path_str).unwrap();
        assert_eq!(loaded["party"]["_gold"], json!(5000));
        assert_eq!(loaded["system"]["_saveCount"], json!(7));
    }

    #[test]
    fn test_find_data_dir() {
        let fmt = RpgMakerFormat::new();
        let dir = tempfile::tempdir().unwrap();
        let www_data = dir.path().join("www/data");
        fs::create_dir_all(&www_data).unwrap();
        fs::write(www_data.join("System.json"), "{}").unwrap();

        let found = fmt.find_data_dir(&dir.path().to_string_lossy());
        assert!(found.is_some());
        assert!(found.unwrap().ends_with("www/data"));
    }

    #[test]
    fn test_save_creates_backup() {
        let fmt = RpgMakerFormat::new();
        let data = make_test_data();
        let dir = tempfile::tempdir().unwrap();
        let save_path = dir.path().join("save.rpgsave");
        let path_str = save_path.to_string_lossy().to_string();

        fmt.save(&path_str, &data).unwrap();

        // Check for backup
        let backups: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().contains(".bak."))
            .collect();
        assert!(!backups.is_empty(), "backup should be created");
    }
}
```

You need to add a Default derive to `ModifiableField` in core/types.rs. Update `crates/core/src/types.rs` — change the struct derive from `#[derive(Debug, Clone, Serialize, Deserialize)]` to:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModifiableField {
```

- [ ] **Step 2: Add Default for ModifiableField (if not using derive)**

Actually, since `ModifiableField` uses `#[serde(default)]` on most fields, we can derive `Default`. Add `Default` to the derive:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModifiableField { ... }
```

- [ ] **Step 3: Make GameToolError implement Default for ModifiableField tests**

No changes needed — GameToolError doesn't need Default.

- [ ] **Step 4: Verify compilation and tests**

```powershell
cargo test -p game-tool-rpgmaker -- format
```

Expected: 9 new tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/engines/rpgmaker/src/format.rs crates/core/src/types.rs
git commit -m "feat(rpgmaker): implement RpgMakerFormat with load/save/summary/apply_field"
```

---

### Task 3: Create gamedata.rs — Game Data Scanner

**Files:**
- Create: `crates/engines/rpgmaker/src/gamedata.rs`

- [ ] **Step 1: Write gamedata.rs**

File: `crates/engines/rpgmaker/src/gamedata.rs`

```rust
//! RPG Maker MV 游戏数据扫描器
//!
//! 扫描游戏目录的 System.json, Actors.json, Items.json 等数据文件，
//! 提取开关/变量/角色/物品/武器/防具的名称映射。

use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// 扫描结果
#[derive(Debug, Clone)]
pub struct GameConfig {
    pub game_dir: String,
    pub game_title: String,
    pub currency_unit: String,
    pub data_loaded: bool,
    pub switch_names: HashMap<usize, String>,
    pub variable_names: HashMap<usize, String>,
    pub actor_names: HashMap<usize, String>,
    pub class_names: HashMap<usize, String>,
    pub item_names: HashMap<usize, String>,
    pub weapon_names: HashMap<usize, String>,
    pub armor_names: HashMap<usize, String>,
    pub skill_names: HashMap<usize, String>,
    pub state_names: HashMap<usize, String>,
}

impl Default for GameConfig {
    fn default() -> Self {
        Self {
            game_dir: String::new(),
            game_title: String::new(),
            currency_unit: "G".into(),
            data_loaded: false,
            switch_names: HashMap::new(),
            variable_names: HashMap::new(),
            actor_names: HashMap::new(),
            class_names: HashMap::new(),
            item_names: HashMap::new(),
            weapon_names: HashMap::new(),
            armor_names: HashMap::new(),
            skill_names: HashMap::new(),
            state_names: HashMap::new(),
        }
    }
}

/// 扫描 RPG Maker MV 游戏目录
pub fn scan_game_directory(game_dir: &str) -> GameConfig {
    let mut config = GameConfig {
        game_dir: game_dir.to_string(),
        ..Default::default()
    };

    let data_dir = find_data_dir(game_dir);
    if data_dir.is_none() {
        return config;
    }
    let data_dir = data_dir.unwrap();

    load_system(&mut config, &data_dir);
    load_actors(&mut config, &data_dir);
    load_classes(&mut config, &data_dir);
    load_items(&mut config, &data_dir);
    load_weapons(&mut config, &data_dir);
    load_armors(&mut config, &data_dir);
    load_skills(&mut config, &data_dir);
    load_states(&mut config, &data_dir);

    config.data_loaded = true;
    config
}

fn find_data_dir(game_dir: &str) -> Option<String> {
    let dir = Path::new(game_dir);
    for sub in &["www/data", "data"] {
        let d = dir.join(sub);
        if d.is_dir() && d.join("System.json").is_file() {
            return Some(d.to_string_lossy().to_string());
        }
    }
    None
}

fn load_json(data_dir: &str, filename: &str) -> Option<serde_json::Value> {
    let path = Path::new(data_dir).join(filename);
    if !path.is_file() {
        return None;
    }
    let content = fs::read_to_string(&path).ok()?;
    serde_json::from_str(&content).ok()
}

fn load_system(config: &mut GameConfig, data_dir: &str) {
    let sys = match load_json(data_dir, "System.json") {
        Some(v) => v,
        None => return,
    };
    config.game_title = sys.get("gameTitle")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    config.currency_unit = sys.get("currencyUnit")
        .and_then(|v| v.as_str())
        .unwrap_or("G")
        .to_string();

    if let Some(switches) = sys.get("switches").and_then(|v| v.as_array()) {
        for (i, name) in switches.iter().enumerate() {
            if let Some(s) = name.as_str() {
                if !s.trim().is_empty() {
                    config.switch_names.insert(i, s.trim().to_string());
                }
            }
        }
    }

    if let Some(variables) = sys.get("variables").and_then(|v| v.as_array()) {
        for (i, name) in variables.iter().enumerate() {
            if let Some(s) = name.as_str() {
                if !s.trim().is_empty() {
                    config.variable_names.insert(i, s.trim().to_string());
                }
            }
        }
    }
}

fn load_actors(config: &mut GameConfig, data_dir: &str) {
    let data = match load_json(data_dir, "Actors.json") {
        Some(Value::Array(arr)) => arr,
        _ => return,
    };
    for actor in &data {
        if let (Some(id), Some(name)) = (
            actor.get("id").and_then(|v| v.as_i64()),
            actor.get("name").and_then(|v| v.as_str()),
        ) {
            if !name.trim().is_empty() {
                config.actor_names.insert(id as usize, name.trim().to_string());
            }
        }
    }
}

fn load_classes(config: &mut GameConfig, data_dir: &str) {
    let data = match load_json(data_dir, "Classes.json") {
        Some(Value::Array(arr)) => arr,
        _ => return,
    };
    for cls in &data {
        if let (Some(id), Some(name)) = (
            cls.get("id").and_then(|v| v.as_i64()),
            cls.get("name").and_then(|v| v.as_str()),
        ) {
            config.class_names.insert(id as usize, name.to_string());
        }
    }
}

fn load_items(config: &mut GameConfig, data_dir: &str) {
    let data = match load_json(data_dir, "Items.json") {
        Some(Value::Array(arr)) => arr,
        _ => return,
    };
    for item in &data {
        if let (Some(id), Some(name)) = (
            item.get("id").and_then(|v| v.as_i64()),
            item.get("name").and_then(|v| v.as_str()),
        ) {
            config.item_names.insert(id as usize, name.to_string());
        }
    }
}

fn load_weapons(config: &mut GameConfig, data_dir: &str) {
    let data = match load_json(data_dir, "Weapons.json") {
        Some(Value::Array(arr)) => arr,
        _ => return,
    };
    for item in &data {
        if let (Some(id), Some(name)) = (
            item.get("id").and_then(|v| v.as_i64()),
            item.get("name").and_then(|v| v.as_str()),
        ) {
            config.weapon_names.insert(id as usize, name.to_string());
        }
    }
}

fn load_armors(config: &mut GameConfig, data_dir: &str) {
    let data = match load_json(data_dir, "Armors.json") {
        Some(Value::Array(arr)) => arr,
        _ => return,
    };
    for item in &data {
        if let (Some(id), Some(name)) = (
            item.get("id").and_then(|v| v.as_i64()),
            item.get("name").and_then(|v| v.as_str()),
        ) {
            config.armor_names.insert(id as usize, name.to_string());
        }
    }
}

fn load_skills(config: &mut GameConfig, data_dir: &str) {
    let data = match load_json(data_dir, "Skills.json") {
        Some(Value::Array(arr)) => arr,
        _ => return,
    };
    for item in &data {
        if let (Some(id), Some(name)) = (
            item.get("id").and_then(|v| v.as_i64()),
            item.get("name").and_then(|v| v.as_str()),
        ) {
            config.skill_names.insert(id as usize, name.to_string());
        }
    }
}

fn load_states(config: &mut GameConfig, data_dir: &str) {
    let data = match load_json(data_dir, "States.json") {
        Some(Value::Array(arr)) => arr,
        _ => return,
    };
    for item in &data {
        if let (Some(id), Some(name)) = (
            item.get("id").and_then(|v| v.as_i64()),
            item.get("name").and_then(|v| v.as_str()),
        ) {
            config.state_names.insert(id as usize, name.to_string());
        }
    }
}

use serde_json::Value;

/// Helper: look up actor name by ID
pub fn actor_name(config: &GameConfig, id: usize) -> String {
    config.actor_names.get(&id).cloned().unwrap_or_else(|| format!("角色 #{}", id))
}

/// Helper: look up item name by ID
pub fn item_name(config: &GameConfig, id: usize) -> String {
    config.item_names.get(&id).cloned().unwrap_or_else(|| format!("物品 #{}", id))
}

/// Helper: look up switch name by ID
pub fn switch_name(config: &GameConfig, id: usize) -> String {
    config.switch_names.get(&id).cloned().unwrap_or_else(|| format!("开关 #{}", id))
}

/// Helper: look up variable name by ID
pub fn variable_name(config: &GameConfig, id: usize) -> String {
    config.variable_names.get(&id).cloned().unwrap_or_else(|| format!("变量 #{}", id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_scan_no_data_dir() {
        let dir = tempfile::tempdir().unwrap();
        let config = scan_game_directory(&dir.path().to_string_lossy());
        assert!(!config.data_loaded);
        assert!(config.game_title.is_empty());
    }

    #[test]
    fn test_scan_with_system_json() {
        let dir = tempfile::tempdir().unwrap();
        let data_dir = dir.path().join("www/data");
        fs::create_dir_all(&data_dir).unwrap();

        let system = serde_json::json!({
            "gameTitle": "Test Game",
            "currencyUnit": "G",
            "switches": ["", "Door Open", "", "Chest"],
            "variables": ["", "Steps", "Gold"]
        });
        fs::write(data_dir.join("System.json"), system.to_string()).unwrap();

        let config = scan_game_directory(&dir.path().to_string_lossy());
        assert!(config.data_loaded);
        assert_eq!(config.game_title, "Test Game");
        assert_eq!(config.switch_names.get(&1).unwrap(), "Door Open");
        assert_eq!(config.switch_names.get(&3).unwrap(), "Chest");
        assert_eq!(config.variable_names.get(&1).unwrap(), "Steps");
    }

    #[test]
    fn test_scan_with_actors() {
        let dir = tempfile::tempdir().unwrap();
        let data_dir = dir.path().join("www/data");
        fs::create_dir_all(&data_dir).unwrap();

        fs::write(data_dir.join("System.json"), r#"{"gameTitle":"","switches":[],"variables":[]}"#).unwrap();
        fs::write(data_dir.join("Actors.json"), r#"[{"id":1,"name":"Alice"},{"id":2,"name":"Bob"}]"#).unwrap();

        let config = scan_game_directory(&dir.path().to_string_lossy());
        assert_eq!(config.actor_names.get(&1).unwrap(), "Alice");
        assert_eq!(config.actor_names.get(&2).unwrap(), "Bob");
    }

    #[test]
    fn test_actor_name_fallback() {
        let config = GameConfig::default();
        assert_eq!(actor_name(&config, 5), "角色 #5");
    }

    #[test]
    fn test_item_name_fallback() {
        let config = GameConfig::default();
        assert_eq!(item_name(&config, 10), "物品 #10");
    }
}
```

- [ ] **Step 2: Verify compilation and tests**

```powershell
cargo test -p game-tool-rpgmaker -- gamedata
```

Expected: 5 tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/engines/rpgmaker/src/gamedata.rs
git commit -m "feat(rpgmaker): implement game data scanner (System.json, Actors.json, etc.)"
```

---

### Task 4: Create scanner.rs — Field Merge Logic

**Files:**
- Create: `crates/engines/rpgmaker/src/scanner.rs`

- [ ] **Step 1: Write scanner.rs**

File: `crates/engines/rpgmaker/src/scanner.rs`

```rust
//! 游戏数据扫描器 — 综合扫描游戏内所有可修改项目
//!
//! 从游戏数据文件 (System.json, Actors.json 等)、存档文件和实时游戏状态
//! 中收集所有可修改的项目，生成统一字段列表。

use std::collections::HashMap;
use serde_json::Value;
use game_tool_core::ModifiableField;

use crate::gamedata::{self, GameConfig};
use crate::jsonex;

/// 扫描器内部字段类型（包含 RPG Maker 特有字段）
#[derive(Debug, Clone)]
pub struct ScanField {
    pub category: String,
    pub field_id: String,
    pub display_name: String,
    pub item_id: i32,
    pub field_type: String,
    pub save_value: Value,
    pub live_value: Value,
    pub default_value: Value,
    pub min_val: i32,
    pub max_val: i32,
    pub description: String,
    pub dirty: bool,
    /// RPG Maker 特有：关联的金币变量 ID
    pub gold_var_id: i32,
}

/// 扫描结果
#[derive(Debug, Clone)]
pub struct GameScanResult {
    pub game_dir: String,
    pub game_title: String,
    pub has_save_data: bool,
    pub has_live_data: bool,
    pub fields: Vec<ScanField>,
    pub categories: HashMap<String, Vec<ScanField>>,
}

/// 将 ScanField 转换为通用的 ModifiableField
pub fn to_modifiable_field(f: &ScanField) -> ModifiableField {
    ModifiableField {
        category: f.category.clone(),
        field_id: f.field_id.clone(),
        display_name: f.display_name.clone(),
        item_id: f.item_id,
        field_type: f.field_type.clone(),
        save_value: f.save_value.clone(),
        live_value: f.live_value.clone(),
        default_value: f.default_value.clone(),
        min_val: f.min_val,
        max_val: f.max_val,
        description: f.description.clone(),
        dirty: f.dirty,
    }
}

/// 全面扫描游戏内所有可修改项目
pub fn scan_all_modifiable(
    game_dir: &str,
    save_data: Option<&Value>,
    live_state: Option<&Value>,
) -> GameScanResult {
    let config = gamedata::scan_game_directory(game_dir);
    let game_title = if config.game_title.is_empty() {
        std::path::Path::new(game_dir)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default()
    } else {
        config.game_title.clone()
    };

    let has_save_data = save_data.is_some();
    let has_live_data = live_state.is_some();

    let mut result = GameScanResult {
        game_dir: game_dir.to_string(),
        game_title,
        has_save_data,
        has_live_data,
        fields: Vec::new(),
        categories: HashMap::new(),
    };

    // ── 金币 ──
    let gold = save_data
        .and_then(|d| d.get("party"))
        .and_then(|p| p.get("_gold"))
        .and_then(|v| v.as_i64())
        .unwrap_or(0) as i32;

    let gold_var_id = find_gold_variable_id(&config, save_data);

    result.fields.push(ScanField {
        category: "gold".into(),
        field_id: "gold".into(),
        display_name: format!("金币 ({})", config.currency_unit),
        item_id: 0,
        field_type: "int".into(),
        save_value: Value::Number(gold.into()),
        live_value: Value::Null,
        default_value: Value::Number(0.into()),
        min_val: 0,
        max_val: 99_999_999,
        description: String::new(),
        dirty: false,
        gold_var_id,
    });

    // ── 开关 ──
    let switches_map = extract_switches(save_data, live_state);
    let switch_count = switches_map.keys().max().map(|k| k + 1).unwrap_or(0);
    for i in 0..switch_count {
        let val = switches_map.get(&i).copied().unwrap_or(false);
        let live_val = live_state.and_then(|s| {
            s.get("extensions")
                .and_then(|e| e.get("switches"))
                .and_then(|sw| sw.get(i.to_string()))
                .and_then(|v| v.as_bool())
        });
        let name = gamedata::switch_name(&config, i);
        result.fields.push(ScanField {
            category: "switch".into(),
            field_id: format!("switch_{}", i),
            display_name: name,
            item_id: i as i32,
            field_type: "bool".into(),
            save_value: Value::Bool(val),
            live_value: live_val.map(Value::Bool).unwrap_or(Value::Null),
            default_value: Value::Bool(false),
            min_val: 0,
            max_val: 1,
            description: String::new(),
            dirty: false,
            gold_var_id: 0,
        });
    }

    // ── 变量 ──
    let variables_map = extract_variables(save_data, live_state);
    let var_count = variables_map.keys().max().map(|k| k + 1).unwrap_or(0);
    for i in 0..var_count {
        let val = variables_map.get(&i).copied().unwrap_or(0);
        let live_val = live_state.and_then(|s| {
            s.get("extensions")
                .and_then(|e| e.get("variables"))
                .and_then(|v| v.get(i.to_string()))
                .and_then(|v| v.as_i64())
        });
        let name = if i as i32 == gold_var_id {
            format!("{} (金币变量)", gamedata::variable_name(&config, i))
        } else {
            gamedata::variable_name(&config, i)
        };
        result.fields.push(ScanField {
            category: "variable".into(),
            field_id: format!("var_{}", i),
            display_name: name,
            item_id: i as i32,
            field_type: "int".into(),
            save_value: Value::Number(val.into()),
            live_value: live_val.map(|v| Value::Number(v.into())).unwrap_or(Value::Null),
            default_value: Value::Number(0.into()),
            min_val: -9_999_999,
            max_val: 99_999_999,
            description: String::new(),
            dirty: false,
            gold_var_id: 0,
        });
    }

    // ── 角色 ──
    if let Some(actors) = save_data.and_then(|d| d.get("party")).and_then(|p| p.get("_actors")).and_then(|v| v.as_array()) {
        for actor in actors {
            let id = actor.get("_actorId").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            let name = gamedata::actor_name(&config, id as usize);
            let hp = actor.get("_hp").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            let mp = actor.get("_mp").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            let level = actor.get("_level").and_then(|v| v.as_i64()).unwrap_or(1) as i32;

            result.fields.push(ScanField {
                category: "actor".into(),
                field_id: format!("actor_{}_hp", id),
                display_name: format!("{} HP", name),
                item_id: id,
                field_type: "int".into(),
                save_value: Value::Number(hp.into()),
                live_value: Value::Null,
                default_value: Value::Number(1.into()),
                min_val: 0,
                max_val: 999_999,
                description: String::new(),
                dirty: false,
                gold_var_id: 0,
            });
            result.fields.push(ScanField {
                category: "actor".into(),
                field_id: format!("actor_{}_mp", id),
                display_name: format!("{} MP", name),
                item_id: id,
                field_type: "int".into(),
                save_value: Value::Number(mp.into()),
                live_value: Value::Null,
                default_value: Value::Number(0.into()),
                min_val: 0,
                max_val: 999_999,
                description: String::new(),
                dirty: false,
                gold_var_id: 0,
            });
            result.fields.push(ScanField {
                category: "actor".into(),
                field_id: format!("actor_{}_level", id),
                display_name: format!("{} 等级", name),
                item_id: id,
                field_type: "int".into(),
                save_value: Value::Number(level.into()),
                live_value: Value::Null,
                default_value: Value::Number(1.into()),
                min_val: 1,
                max_val: 99,
                description: String::new(),
                dirty: false,
                gold_var_id: 0,
            });
        }
    }

    // ── 物品 ──
    if let Some(items) = save_data.and_then(|d| d.get("party")).and_then(|p| p.get("_items")).and_then(|v| v.as_object()) {
        let filtered = jsonex::filter_meta_keys(items);
        for (k, v) in &filtered {
            let id: i32 = k.parse().unwrap_or(0);
            let count = v.as_i64().unwrap_or(0) as i32;
            if count > 0 {
                let name = gamedata::item_name(&config, id as usize);
                result.fields.push(ScanField {
                    category: "item".into(),
                    field_id: format!("item_{}", id),
                    display_name: name,
                    item_id: id,
                    field_type: "int".into(),
                    save_value: Value::Number(count.into()),
                    live_value: Value::Null,
                    default_value: Value::Number(0.into()),
                    min_val: 0,
                    max_val: 999,
                    description: String::new(),
                    dirty: false,
                    gold_var_id: 0,
                });
            }
        }
    }

    // ── Self Switches ──
    if let Some(self_sw) = save_data.and_then(|d| d.get("selfSwitches")).and_then(|v| v.as_object()) {
        let filtered = jsonex::filter_meta_keys(self_sw);
        for (k, v) in &filtered {
            let val = v.as_bool().unwrap_or(false);
            result.fields.push(ScanField {
                category: "self_switch".into(),
                field_id: format!("ss_{}", k),
                display_name: format!("Self Switch: {}", k),
                item_id: 0,
                field_type: "bool".into(),
                save_value: Value::Bool(val),
                live_value: Value::Null,
                default_value: Value::Bool(false),
                min_val: 0,
                max_val: 1,
                description: String::new(),
                dirty: false,
                gold_var_id: 0,
            });
        }
    }

    // Build categories index
    for field in &result.fields {
        result.categories
            .entry(field.category.clone())
            .or_default()
            .push(field.clone());
    }

    result
}

/// 提取开关值（存档值 + live state）
fn extract_switches(save_data: Option<&Value>, _live_state: Option<&Value>) -> HashMap<usize, bool> {
    let mut map = HashMap::new();
    if let Some(data) = save_data {
        if let Some(switches) = data.get("switches") {
            if let Some(arr) = switches.as_array() {
                for (i, v) in arr.iter().enumerate() {
                    map.insert(i, v.as_bool().unwrap_or(false));
                }
            } else if let Some(obj) = switches.as_object() {
                let resolved = jsonex::resolve_array_flat(switches);
                for (i, v) in resolved.iter().enumerate() {
                    if let Some(val) = v {
                        map.insert(i, val.as_bool().unwrap_or(false));
                    }
                }
            }
        }
    }
    map
}

/// 提取变量值
fn extract_variables(save_data: Option<&Value>, _live_state: Option<&Value>) -> HashMap<usize, i32> {
    let mut map = HashMap::new();
    if let Some(data) = save_data {
        if let Some(vars) = data.get("variables") {
            if let Some(arr) = vars.as_array() {
                for (i, v) in arr.iter().enumerate() {
                    map.insert(i, v.as_i64().unwrap_or(0) as i32);
                }
            } else if let Some(obj) = vars.as_object() {
                let resolved = jsonex::resolve_array_flat(vars);
                for (i, v) in resolved.iter().enumerate() {
                    if let Some(val) = v {
                        map.insert(i, val.as_i64().unwrap_or(0) as i32);
                    }
                }
            }
        }
    }
    map
}

/// 智能检测金币是否存储在变量中
fn find_gold_variable_id(config: &GameConfig, save_data: Option<&Value>) -> i32 {
    let currency_keywords = ["金币", "金钱", "gold", "money", "Gold", "Money", "GOLD", "所持金"];
    for (&i, name) in &config.variable_names {
        for kw in &currency_keywords {
            if name.to_lowercase().contains(&kw.to_lowercase()) {
                // Verify this variable exists in save data and has a reasonable value
                if let Some(data) = save_data {
                    if let Some(vars) = data.get("variables") {
                        let val = if let Some(arr) = vars.as_array() {
                            arr.get(i).and_then(|v| v.as_i64()).unwrap_or(0) as i32
                        } else {
                            0
                        };
                        if val > 0 && val < 99_999_999 {
                            return i as i32;
                        }
                    }
                }
            }
        }
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn make_save_data() -> Value {
        json!({
            "party": {
                "_gold": 3000,
                "_actors": [
                    {"_actorId": 1, "_name": "Alice", "_hp": 100, "_mp": 50, "_level": 5}
                ],
                "_items": {"1": 10, "2": 5}
            },
            "switches": [false, true, false, false, true],
            "variables": [0, 42, 0, 100, 0],
            "selfSwitches": {"1_1_A": true, "2_3_B": false}
        })
    }

    #[test]
    fn test_scan_creates_gold_field() {
        let dir = tempfile::tempdir().unwrap();
        let save = make_save_data();
        let result = scan_all_modifiable(&dir.path().to_string_lossy(), Some(&save), None);
        let gold: Vec<_> = result.fields.iter().filter(|f| f.category == "gold").collect();
        assert_eq!(gold.len(), 1);
        assert_eq!(gold[0].save_value, json!(3000));
    }

    #[test]
    fn test_scan_creates_switch_fields() {
        let dir = tempfile::tempdir().unwrap();
        let save = make_save_data();
        let result = scan_all_modifiable(&dir.path().to_string_lossy(), Some(&save), None);
        let switches: Vec<_> = result.fields.iter().filter(|f| f.category == "switch").collect();
        assert_eq!(switches.len(), 5);
        assert_eq!(switches[1].save_value, Value::Bool(true));
    }

    #[test]
    fn test_scan_creates_variable_fields() {
        let dir = tempfile::tempdir().unwrap();
        let save = make_save_data();
        let result = scan_all_modifiable(&dir.path().to_string_lossy(), Some(&save), None);
        let vars: Vec<_> = result.fields.iter().filter(|f| f.category == "variable").collect();
        assert_eq!(vars.len(), 5);
        assert_eq!(vars[1].save_value, json!(42));
    }

    #[test]
    fn test_scan_creates_actor_fields() {
        let dir = tempfile::tempdir().unwrap();
        let save = make_save_data();
        let result = scan_all_modifiable(&dir.path().to_string_lossy(), Some(&save), None);
        let actors: Vec<_> = result.fields.iter().filter(|f| f.category == "actor").collect();
        assert_eq!(actors.len(), 3); // hp, mp, level
    }

    #[test]
    fn test_scan_creates_item_fields() {
        let dir = tempfile::tempdir().unwrap();
        let save = make_save_data();
        let result = scan_all_modifiable(&dir.path().to_string_lossy(), Some(&save), None);
        let items: Vec<_> = result.fields.iter().filter(|f| f.category == "item").collect();
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn test_scan_no_save_data() {
        let dir = tempfile::tempdir().unwrap();
        let result = scan_all_modifiable(&dir.path().to_string_lossy(), None, None);
        assert!(!result.has_save_data);
        // Gold should still appear (default 0)
        let gold: Vec<_> = result.fields.iter().filter(|f| f.category == "gold").collect();
        assert_eq!(gold.len(), 1);
        assert_eq!(gold[0].save_value, json!(0));
    }

    #[test]
    fn test_to_modifiable_field() {
        let sf = ScanField {
            category: "gold".into(),
            field_id: "gold".into(),
            display_name: "金币".into(),
            item_id: 0,
            field_type: "int".into(),
            save_value: json!(1000),
            live_value: Value::Null,
            default_value: json!(0),
            min_val: 0,
            max_val: 9999,
            description: String::new(),
            dirty: false,
            gold_var_id: 0,
        };
        let mf = to_modifiable_field(&sf);
        assert_eq!(mf.category, "gold");
        assert_eq!(mf.save_value, json!(1000));
    }
}
```

- [ ] **Step 2: Verify compilation and tests**

```powershell
cargo test -p game-tool-rpgmaker -- scanner
```

Expected: 7 tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/engines/rpgmaker/src/scanner.rs
git commit -m "feat(rpgmaker): implement field scanner (config + save + live merge)"
```

---

### Task 5: Final verification

- [ ] **Step 1: Full test suite**

```powershell
cargo test -p game-tool-rpgmaker
```

Expected: all tests pass (24 jsonex + 9 format + 5 gamedata + 7 scanner = ~45 tests).

- [ ] **Step 2: Full workspace test**

```powershell
cargo test --workspace
```

Expected: all tests pass except pre-existing lzstring::test_very_large_string.

- [ ] **Step 3: Clippy**

```powershell
cargo clippy --workspace -- -D warnings
```

- [ ] **Step 4: Final commit**

```bash
git add -A
git commit -m "chore: final verification and clippy fixes for Plan 2"
```
