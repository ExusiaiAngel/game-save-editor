//! 通用 JSON 存档格式处理器。
//!
//! 为未识别的游戏引擎提供通用的 JSON 存档读取/写入功能。
//! 核心流程：
//! 1. 加载时将 JSON 扁平化为"路径.键"格式存入 `_flat`
//! 2. 修改时通过 `_flat` 中的键更新对应值
//! 3. 保存时将扁平数据还原为嵌套 JSON 结构写回文件

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use game_tool_core::{backup, error::GameToolError, ISaveFormat, ModifiableField, SaveSummary};
use serde_json::{Map, Value};

/// 通用 JSON 存档格式处理器。
///
/// 支持任意 JSON 结构的存档文件，通过扁平化/反扁平化实现字段编辑。
pub struct GenericJsonFormat;

impl Default for GenericJsonFormat {
    fn default() -> Self {
        Self
    }
}

impl GenericJsonFormat {
    /// 创建新的通用 JSON 格式处理器
    pub fn new() -> Self {
        Self
    }
}

impl ISaveFormat for GenericJsonFormat {
    /// 返回格式名称："JSON (通用)"
    fn name(&self) -> &str {
        "JSON (通用)"
    }
    /// 返回支持的存档文件扩展名列表：[".json"]
    fn extensions(&self) -> Vec<String> {
        vec![".json".into()]
    }
    /// 返回引擎类型标识："generic"
    fn engine_type(&self) -> &str {
        "generic"
    }
    /// JSON 格式无固定魔术字节标识，返回 None
    fn magic_bytes(&self) -> Option<&[u8]> {
        None
    }

    /// 加载 JSON 存档文件并扁平化为 `_flat` 映射。
    ///
    /// 返回结构包含：
    /// - `_format`: 格式标识 `"generic_json"`
    /// - `_root`: 原始 JSON 根节点
    /// - `_flat`: 扁平化后的字段映射（键为"路径.键"格式）
    fn load(&self, filepath: &str) -> Result<Value, GameToolError> {
        let raw = fs::read_to_string(filepath)
            .map_err(|e| GameToolError::ArchiveLoadError(e.to_string()))?;
        let root: Value = serde_json::from_str(&raw)
            .map_err(|e| GameToolError::ArchiveLoadError(e.to_string()))?;

        // 将嵌套 JSON 展开为扁平键值对
        let flat = flatten_json(&root, "");
        let mut data = Map::new();
        data.insert("_format".into(), Value::String("generic_json".into()));
        data.insert("_root".into(), root);
        data.insert("_flat".into(), Value::Object(flat));
        Ok(Value::Object(data))
    }

    /// 保存修改后的数据：将 `_flat` 反扁平化后以美化格式写回文件。
    ///
    /// 自动创建备份（最多保留 10 个）。
    fn save(&self, filepath: &str, data: &Value) -> Result<(), GameToolError> {
        let path = Path::new(filepath);
        let _ = backup::save_backup(path, 10);
        let flat = data
            .get("_flat")
            .and_then(|v| v.as_object())
            .cloned()
            .unwrap_or_default();
        // 将扁平键值对还原为嵌套 JSON
        let root = unflatten_json(&flat);
        let json_str = serde_json::to_string_pretty(&root)
            .map_err(|e| GameToolError::ArchiveSaveError(e.to_string()))?;
        fs::write(path, &json_str).map_err(|e| GameToolError::ArchiveSaveError(e.to_string()))?;
        Ok(())
    }

    /// 在游戏目录中查找数据文件夹。
    ///
    /// 按优先级搜索 `data`、`saves`、`save`、`game` 子目录。
    fn find_data_dir(&self, game_dir: &str) -> Option<String> {
        let dir = Path::new(game_dir);
        for sub in &["data", "saves", "save", "game"] {
            let d = dir.join(sub);
            if d.is_dir() {
                return Some(d.to_string_lossy().to_string());
            }
        }
        None
    }

    /// 从扁平数据中提取存档摘要信息。
    ///
    /// 自动识别金币（gold/money/coin 等关键词）和字段总数。
    fn get_summary(&self, data: &Value) -> SaveSummary {
        let flat = data.get("_flat").and_then(|v| v.as_object());
        let gold = find_gold_like(flat).unwrap_or(0);
        let field_count = flat.map(|m| m.len() as i32).unwrap_or(0);
        SaveSummary {
            gold,
            item_count: field_count,
            ..Default::default()
        }
    }

    /// 扫描扁平数据中的所有可修改字段。
    ///
    /// 根据键名自动推断字段类别（金币/属性/背包/进度/设置/角色）。
    fn scan_fields(&self, data: &Value, _game_dir: &str) -> Vec<ModifiableField> {
        let mut fields = Vec::new();
        if let Some(flat) = data.get("_flat").and_then(|v| v.as_object()) {
            for (key, value) in flat {
                // 推断值类型
                let field_type = match value {
                    Value::Bool(_) => "bool",
                    Value::Number(n) if n.is_f64() => "float",
                    Value::Number(_) => "int",
                    Value::String(_) => "str",
                    _ => "str",
                };
                // 根据键名推断类别
                let category = guess_category(key);
                // 使用已知的中文名映射，否则使用原始键名
                let display_name = FIELD_NAME_MAP
                    .get(key.as_str())
                    .copied()
                    .unwrap_or(key.as_str());
                fields.push(ModifiableField {
                    category,
                    field_id: format!("json_{}", key),
                    display_name: display_name.to_string(),
                    field_type: field_type.into(),
                    save_value: value.clone(),
                    ..Default::default()
                });
            }
        }
        fields
    }

    /// 将修改应用到扁平数据中。
    ///
    /// 通过 `field_id` 中的 `json_` 前缀提取对应的扁平键并更新值。
    fn apply_field(&self, data: &mut Value, field: &ModifiableField) -> Result<(), GameToolError> {
        let key = field
            .field_id
            .strip_prefix("json_")
            .unwrap_or(&field.field_id)
            .to_string();
        if let Some(flat) = data.pointer_mut("/_flat") {
            if let Some(obj) = flat.as_object_mut() {
                obj.insert(key, field.save_value.clone());
            }
        }
        Ok(())
    }
}

/// 将嵌套 JSON 值扁平化为"前缀.键"格式的映射。
///
/// - 对象节点展开为 `prefix.key`（递归）
/// - 数组节点展开为 `prefix[index]`（递归）
/// - 叶子节点（数字/字符串/布尔/null）直接插入
fn flatten_json(value: &Value, prefix: &str) -> Map<String, Value> {
    let mut result = Map::new();
    match value {
        Value::Object(map) => {
            for (key, val) in map {
                let new_prefix = if prefix.is_empty() {
                    key.clone()
                } else {
                    format!("{}.{}", prefix, key)
                };
                if val.is_object() || val.is_array() {
                    // 递归展平嵌套结构
                    result.extend(flatten_json(val, &new_prefix));
                } else {
                    // 叶子节点直接插入
                    result.insert(new_prefix, val.clone());
                }
            }
        }
        Value::Array(arr) => {
            for (i, val) in arr.iter().enumerate() {
                let new_prefix = format!("{}[{}]", prefix, i);
                result.extend(flatten_json(val, &new_prefix));
            }
        }
        _ => {}
    }
    result
}

/// 将扁平键值对还原为嵌套 JSON 结构。
fn unflatten_json(flat: &Map<String, Value>) -> Value {
    let mut result = Map::new();
    for (key, value) in flat {
        insert_by_path(&mut result, key, value.clone());
    }
    Value::Object(result)
}

/// 按点号分隔的路径将值插入嵌套映射中。
///
/// 支持两种路径语法：
/// - `a.b.c` — 嵌套对象
/// - `a[0].b` — 数组索引 + 嵌套对象
fn insert_by_path(map: &mut Map<String, Value>, path: &str, value: Value) {
    // 处理括号表示法：如 "items[0].name" 或 "items[0]"
    if let Some(bracket_start) = path.find('[') {
        let array_key = &path[..bracket_start];
        let rest = &path[bracket_start..]; // "[0].name" 或 "[0]"
        let entry = map
            .entry(array_key.to_string())
            .or_insert_with(|| Value::Array(Vec::new()));
        if let Some(arr) = entry.as_array_mut() {
            if let Some(bracket_end) = rest.find(']') {
                let idx: usize = rest[1..bracket_end].parse().unwrap_or(0);
                // 扩容数组以确保索引存在
                while arr.len() <= idx {
                    arr.push(Value::Null);
                }
                let remainder = &rest[bracket_end + 1..];
                if remainder.is_empty() {
                    // 最终目标就是数组元素本身
                    arr[idx] = value;
                } else if let Some(after_dot) = remainder.strip_prefix('.') {
                    // 数组元素包含嵌套对象
                    if !arr[idx].is_object() && !arr[idx].is_array() {
                        arr[idx] = Value::Object(Map::new());
                    }
                    insert_by_path_inner(&mut arr[idx], after_dot, value);
                }
            }
        }
        return;
    }

    // 处理点号表示法：如 "player.hp"
    if let Some(dot_pos) = path.find('.') {
        let key = &path[..dot_pos];
        let rest = &path[dot_pos + 1..];
        let entry = map.entry(key.to_string()).or_insert_with(|| {
            if rest.starts_with('[') {
                // 下层是数组索引，创建数组
                Value::Array(Vec::new())
            } else {
                // 下层是对象键，创建对象
                Value::Object(Map::new())
            }
        });
        insert_by_path_inner(entry, rest, value);
    } else {
        // 只有一级键，直接插入
        map.insert(path.to_string(), value);
    }
}

/// 在任意 JSON 值节点中沿路径递归插入（`insert_by_path` 的辅助函数）。
///
/// 同样支持点号表示法和括号数组索引。
fn insert_by_path_inner(node: &mut Value, path: &str, value: Value) {
    // 处理括号表示法（在任何层级）
    if let Some(bracket_start) = path.find('[') {
        if let Some(bracket_end) = path.find(']') {
            let idx: usize = path[bracket_start + 1..bracket_end].parse().unwrap_or(0);
            let remainder = &path[bracket_end + 1..];
            if let Some(arr) = node.as_array_mut() {
                while arr.len() <= idx {
                    arr.push(Value::Null);
                }
                if remainder.is_empty() {
                    arr[idx] = value;
                } else if let Some(after_dot) = remainder.strip_prefix('.') {
                    if !arr[idx].is_object() && !arr[idx].is_array() {
                        arr[idx] = Value::Object(Map::new());
                    }
                    insert_by_path_inner(&mut arr[idx], after_dot, value);
                }
            }
        }
        return;
    }

    // 处理点号表示法
    if let Some(dot_pos) = path.find('.') {
        let key = &path[..dot_pos];
        let rest = &path[dot_pos + 1..];
        if let Some(obj) = node.as_object_mut() {
            let entry = obj
                .entry(key.to_string())
                .or_insert(Value::Object(Map::new()));
            insert_by_path_inner(entry, rest, value);
        }
    } else if let Some(obj) = node.as_object_mut() {
        obj.insert(path.to_string(), value);
    }
}

/// 在扁平数据中查找与"金币"相关的字段值。
///
/// 按中英文关键词匹配键名（不区分大小写）。
fn find_gold_like(flat: Option<&Map<String, Value>>) -> Option<i32> {
    let flat = flat?;
    let keywords = ["gold", "Gold", "money", "Money", "coin", "金币", "金钱"];
    for (key, val) in flat {
        for kw in &keywords {
            if key.to_lowercase().contains(&kw.to_lowercase()) {
                return val.as_i64().map(|v| v as i32);
            }
        }
    }
    None
}

/// 根据键名语义推断字段分类。
///
/// 分类规则（匹配不区分大小写）：
/// - **金币**: gold/money/coin/cash/credit/currency/金币/金钱
/// - **属性**: hp/health/mp/mana/atk/def/str/int/dex/vit/luk/luck/speed/spd/level/exp/stat
/// - **背包**: item/inventory/weapon/armor/equip
/// - **进度**: stage/chapter/progress/score/quest/mission
/// - **设置**: volume/language/difficulty/setting/config
/// - **角色**: name/player/character/actor
/// - **通用**: 其他所有
fn guess_category(key: &str) -> String {
    let lower = key.to_lowercase();
    if lower.contains("gold")
        || lower.contains("money")
        || lower.contains("coin")
        || lower.contains("cash")
        || lower.contains("credit")
        || lower.contains("currency")
        || lower.contains("金币")
        || lower.contains("金钱")
    {
        return "gold".into();
    }
    if lower.contains("hp")
        || lower.contains("health")
        || lower.contains("mp")
        || lower.contains("mana")
        || lower.contains("atk")
        || lower.contains("def")
        || lower.contains("str")
        || lower.contains("int")
        || lower.contains("dex")
        || lower.contains("vit")
        || lower.contains("luk")
        || lower.contains("luck")
        || lower.contains("speed")
        || lower.contains("spd")
        || lower.contains("level")
        || lower.contains("exp")
        || lower.contains("stat")
    {
        return "stats".into();
    }
    if lower.contains("item")
        || lower.contains("inventory")
        || lower.contains("weapon")
        || lower.contains("armor")
        || lower.contains("equip")
    {
        return "inventory".into();
    }
    if lower.contains("stage")
        || lower.contains("chapter")
        || lower.contains("progress")
        || lower.contains("score")
        || lower.contains("quest")
        || lower.contains("mission")
    {
        return "progress".into();
    }
    if lower.contains("volume")
        || lower.contains("language")
        || lower.contains("difficulty")
        || lower.contains("setting")
        || lower.contains("config")
    {
        return "settings".into();
    }
    if lower.contains("name")
        || lower.contains("player")
        || lower.contains("character")
        || lower.contains("actor")
    {
        return "character".into();
    }
    "general".into()
}

/// 常见字段名的中文显示名映射表。
///
/// 在 UI 中展示时，优先使用中文名而非原始 JSON 键名。
static FIELD_NAME_MAP: std::sync::LazyLock<HashMap<&str, &str>> = std::sync::LazyLock::new(|| {
    HashMap::from([
        ("gold", "金币"),
        ("money", "金钱"),
        ("coin", "硬币"),
        ("cash", "现金"),
        ("currency", "货币"),
        ("credits", "积分"),
        ("hp", "生命值"),
        ("health", "生命"),
        ("maxHp", "最大生命"),
        ("maxHealth", "最大生命"),
        ("mp", "魔力值"),
        ("mana", "魔力"),
        ("maxMp", "最大魔力"),
        ("level", "等级"),
        ("exp", "经验值"),
        ("experience", "经验"),
        ("atk", "攻击力"),
        ("def", "防御力"),
        ("spd", "速度"),
        ("speed", "速度"),
        ("str", "力量"),
        ("int", "智力"),
        ("dex", "敏捷"),
        ("vit", "体力"),
        ("luk", "运气"),
        ("luck", "运气"),
        ("name", "名称"),
        ("playerName", "玩家名"),
        ("score", "分数"),
        ("highScore", "最高分"),
        ("stage", "关卡"),
        ("chapter", "章节"),
        ("progress", "进度"),
        ("item", "物品"),
        ("items", "物品"),
        ("inventory", "背包"),
        ("weapon", "武器"),
        ("armor", "护甲"),
        ("equipment", "装备"),
        ("volume", "音量"),
        ("difficulty", "难度"),
    ])
});

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_flatten_simple() {
        let input = json!({"player": {"hp": 100, "mp": 50}, "gold": 1000});
        let flat = flatten_json(&input, "");
        assert_eq!(flat.get("gold").and_then(|v| v.as_i64()), Some(1000));
        assert_eq!(flat.get("player.hp").and_then(|v| v.as_i64()), Some(100));
        assert_eq!(flat.get("player.mp").and_then(|v| v.as_i64()), Some(50));
    }

    #[test]
    fn test_load_roundtrip() {
        let fmt = GenericJsonFormat::new();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.json");
        std::fs::write(&path, r#"{"player":{"hp":100,"mp":50},"gold":1000}"#).unwrap();

        let data = fmt.load(&path.to_string_lossy()).unwrap();
        assert_eq!(data["_flat"]["gold"], json!(1000));
        assert_eq!(data["_flat"]["player.hp"], json!(100));
    }

    #[test]
    fn test_scan_fields() {
        let fmt = GenericJsonFormat::new();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.json");
        std::fs::write(&path, r#"{"gold":1000,"name":"Hero"}"#).unwrap();

        let data = fmt.load(&path.to_string_lossy()).unwrap();
        let fields = fmt.scan_fields(&data, "");
        assert_eq!(fields.len(), 2);
    }

    #[test]
    fn test_apply_field() {
        let fmt = GenericJsonFormat::new();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.json");
        std::fs::write(&path, r#"{"gold":1000}"#).unwrap();

        let mut data = fmt.load(&path.to_string_lossy()).unwrap();
        let field = ModifiableField {
            category: "gold".into(),
            field_id: "json_gold".into(),
            display_name: "金币".into(),
            field_type: "int".into(),
            save_value: json!(9999),
            ..Default::default()
        };
        fmt.apply_field(&mut data, &field).unwrap();
        assert_eq!(data["_flat"]["gold"], json!(9999));
    }

    #[test]
    fn test_save_roundtrip() {
        let fmt = GenericJsonFormat::new();
        let dir = tempfile::tempdir().unwrap();
        let save_path = dir.path().join("save.json");
        let path_str = save_path.to_string_lossy().to_string();

        std::fs::write(&save_path, r#"{"player":{"hp":100,"mp":50}}"#).unwrap();
        let data = fmt.load(&path_str).unwrap();
        fmt.save(&path_str, &data).unwrap();

        let loaded = fmt.load(&path_str).unwrap();
        assert_eq!(loaded["_flat"]["player.hp"], json!(100));
    }
}
