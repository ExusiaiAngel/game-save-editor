//! RPG Maker JSONEx 扩展格式工具。
//!
//! JSONEx 是 RPG Maker 用于压缩存档的可扩展 JSON 格式，
//! 使用 `@a` 表示稀疏数组、`@c` 表示类型引用等元键。
//! 本模块提供 JSONEx 数据的检测、解析和规范化功能。

use serde_json::{Map, Value};

/// 递归检查 JSON 树是否包含 JSONEx 的 `@c` 元键（类型声明）。
///
/// `depth` 参数限制递归深度以避免栈溢出。
fn has_json_ex_c_format_depth(data: &Value, depth: usize) -> bool {
    if depth > 20 {
        return false;
    }
    match data {
        Value::Object(map) => {
            if map.contains_key("@c") {
                return true;
            }
            map.values()
                .any(|v| has_json_ex_c_format_depth(v, depth + 1))
        }
        Value::Array(arr) => arr.iter().any(|v| has_json_ex_c_format_depth(v, depth + 1)),
        _ => false,
    }
}

/// 检测数据是否使用了 JSONEx 格式（包含 `@c` 元键）。
pub fn has_json_ex_c_format(data: &Value) -> bool {
    has_json_ex_c_format_depth(data, 0)
}

/// 检测数据是否为 JSONEx 稀疏字典（包含 `@a` 键）。
///
/// 稀疏字典使用数字字符串键表示索引，配合 `@a` 标记。
pub fn is_sparse_dict(data: &Value) -> bool {
    match data {
        Value::Object(map) => map.contains_key("@a"),
        _ => false,
    }
}

/// 将 JSONEx 稀疏字典展开为标准数组。
///
/// 稀疏字典使用数字字符串键（如 `"1"`, `"3"`）表示非默认值索引，
/// 未列出的索引填充 `null`。
fn expand_sparse_dict(sparse: &Map<String, Value>) -> Vec<Value> {
    let mut indices: Vec<usize> = sparse
        .keys()
        .filter_map(|k| k.parse::<usize>().ok())
        .collect();
    if indices.is_empty() {
        return Vec::new();
    }
    indices.sort_unstable();
    let max_idx = *indices.last().unwrap();
    // 创建满数组，未指定的索引填充 null
    let mut arr = vec![Value::Null; max_idx + 1];
    for i in indices {
        arr[i] = sparse[&i.to_string()].clone();
    }
    arr
}

/// 将 JSONEx 稀疏字典展开为 `Option<Value>` 数组。
///
/// 与 `expand_sparse_dict` 的区别在于未设置的索引用 `None` 而非 `Value::Null` 表示。
fn expand_sparse_dict_flat(sparse: &Map<String, Value>) -> Vec<Option<Value>> {
    let mut indices: Vec<usize> = sparse
        .keys()
        .filter_map(|k| k.parse::<usize>().ok())
        .collect();
    if indices.is_empty() {
        return Vec::new();
    }
    indices.sort_unstable();
    let max_idx = *indices.last().unwrap();
    let mut arr = vec![None; max_idx + 1];
    for i in indices {
        arr[i] = Some(sparse[&i.to_string()].clone());
    }
    arr
}

/// 将 JSONEx 稀疏字典展开为标准数组，未指定索引用默认值填充。
fn expand_sparse_dict_with_default(sparse: &Map<String, Value>, default: &Value) -> Vec<Value> {
    let mut indices: Vec<usize> = sparse
        .keys()
        .filter_map(|k| k.parse::<usize>().ok())
        .collect();
    if indices.is_empty() {
        return Vec::new();
    }
    indices.sort_unstable();
    let max_idx = *indices.last().unwrap();
    // 创建数组并全部填充默认值
    let mut arr = vec![default.clone(); max_idx + 1];
    for i in indices {
        arr[i] = sparse[&i.to_string()].clone();
    }
    arr
}

/// 将 JSON 值解析为标准数组。
///
/// 支持三种格式：
/// - 标准 JSON 数组 → 直接返回
/// - JSONEx `@a` 数组 → 返回内部数组
/// - JSONEx `@a` 稀疏字典 → 展开后返回
pub fn resolve_array(data: &Value) -> Vec<Value> {
    match data {
        Value::Array(arr) => arr.clone(),
        Value::Object(map) => {
            if let Some(a) = map.get("@a") {
                match a {
                    Value::Array(arr) => arr.clone(),
                    Value::Object(sparse) => expand_sparse_dict(sparse),
                    _ => Vec::new(),
                }
            } else {
                Vec::new()
            }
        }
        _ => Vec::new(),
    }
}

/// 将 JSON 值解析为 `Option<Value>` 数组（保留稀疏信息）。
///
/// 与 `resolve_array` 类似，但保留哪些索引是稀疏填充的信息。
pub fn resolve_array_flat(data: &Value) -> Vec<Option<Value>> {
    match data {
        Value::Array(arr) => arr.iter().map(|v| Some(v.clone())).collect(),
        Value::Object(map) => {
            if let Some(a) = map.get("@a") {
                match a {
                    Value::Array(arr) => arr.iter().map(|v| Some(v.clone())).collect(),
                    Value::Object(sparse) => expand_sparse_dict_flat(sparse),
                    _ => Vec::new(),
                }
            } else {
                Vec::new()
            }
        }
        _ => Vec::new(),
    }
}

/// 判断键是否为 JSONEx 元键（以 `@` 开头）。
pub fn is_meta_key(key: &str) -> bool {
    key.starts_with('@')
}

/// 过滤掉所有 JSONEx 元键（以 `@` 开头），返回纯数据映射。
pub fn filter_meta_keys(map: &Map<String, Value>) -> Map<String, Value> {
    map.iter()
        .filter(|(k, _)| !k.starts_with('@'))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

/// 确保数据中存在 `switches` 结构并返回其内部开关数组的可变引用。
///
/// 处理三种格式：
/// - 标准格式：`switches: [_data: [...]]` 或直接 `switches: [...]`
/// - JSONEx 格式：`switches: {_data: {@a: {0: true, 1: false, ...}}}`
/// - 完全缺失：自动创建默认结构 `[false]`
///
/// 如果是 JSONEx 稀疏格式，会自动展开为标准数组。
pub fn ensure_switches_array(data: &mut Value) -> &mut Vec<Value> {
    if !data.is_object() {
        *data = Value::Object(Map::new());
    }

    let obj = data.as_object_mut().unwrap();
    // 确保 "switches" 键存在
    if !obj.contains_key("switches") {
        obj.insert("switches".to_string(), Value::Object(Map::new()));
    }

    {
        let sw = obj.get_mut("switches").unwrap();
        if !sw.is_object() {
            *sw = Value::Object(Map::new());
        }
        let sw_obj = sw.as_object_mut().unwrap();
        // 确保 _data 存在
        if !sw_obj.contains_key("_data") {
            sw_obj.insert("_data".to_string(), Value::Array(vec![Value::Bool(false)]));
        }
    }

    {
        let sw = obj.get_mut("switches").unwrap();
        if !sw.is_object() {
            *sw = Value::Object(Map::new());
        }
        let sw_obj = sw.as_object_mut().unwrap();
        if !sw_obj.contains_key("_data") {
            sw_obj.insert("_data".to_string(), Value::Array(vec![Value::Bool(false)]));
        }
        let sw_data = sw_obj.get_mut("_data").unwrap();

        // 处理 _data 的格式转换
        if !sw_data.is_array() {
            if let Some(data_obj) = sw_data.as_object_mut() {
                if let Some(a_val) = data_obj.get_mut("@a") {
                    if a_val.is_object() {
                        // 将稀疏字典展开为标准数组
                        let expanded = expand_sparse_dict_with_default(
                            a_val.as_object().unwrap(),
                            &Value::Bool(false),
                        );
                        *a_val = Value::Array(expanded);
                    }
                    if !a_val.is_array() {
                        *a_val = Value::Array(vec![Value::Bool(false)]);
                    }
                } else {
                    data_obj.insert("@a".to_string(), Value::Array(vec![Value::Bool(false)]));
                }
            } else {
                *sw_data = Value::Array(vec![Value::Bool(false)]);
            }
        }
    }

    // 再次获取规范化后的开关数组引用
    let sw = obj.get_mut("switches").unwrap();
    let sw_obj = sw.as_object_mut().unwrap();
    let sw_data = sw_obj.get_mut("_data").unwrap();

    match sw_data {
        Value::Array(arr) => arr,
        Value::Object(ref mut obj) => obj.get_mut("@a").unwrap().as_array_mut().unwrap(),
        _ => unreachable!("_data was normalized to Array or Object with @a"),
    }
}

/// 确保数据中存在 `variables` 结构并返回其内部变量数组的可变引用。
///
/// 与 `ensure_switches_array` 逻辑相同，但默认值为数字 `0` 而非布尔 `false`。
pub fn ensure_variables_array(data: &mut Value) -> &mut Vec<Value> {
    if !data.is_object() {
        *data = Value::Object(Map::new());
    }

    let obj = data.as_object_mut().unwrap();
    // 确保 "variables" 键存在
    if !obj.contains_key("variables") {
        obj.insert("variables".to_string(), Value::Object(Map::new()));
    }

    {
        let var = obj.get_mut("variables").unwrap();
        if !var.is_object() {
            *var = Value::Object(Map::new());
        }
        let var_obj = var.as_object_mut().unwrap();
        // 确保 _data 存在
        if !var_obj.contains_key("_data") {
            var_obj.insert(
                "_data".to_string(),
                Value::Array(vec![Value::Number(0.into())]),
            );
        }
        let var_data = var_obj.get_mut("_data").unwrap();

        // 处理 _data 格式转换
        if !var_data.is_array() {
            if let Some(data_obj) = var_data.as_object_mut() {
                if let Some(a_val) = data_obj.get_mut("@a") {
                    if a_val.is_object() {
                        // 将稀疏字典展开为标准数组（默认值 0）
                        let expanded = expand_sparse_dict_with_default(
                            a_val.as_object().unwrap(),
                            &Value::Number(0.into()),
                        );
                        *a_val = Value::Array(expanded);
                    }
                    if !a_val.is_array() {
                        *a_val = Value::Array(vec![Value::Number(0.into())]);
                    }
                } else {
                    data_obj.insert(
                        "@a".to_string(),
                        Value::Array(vec![Value::Number(0.into())]),
                    );
                }
            } else {
                *var_data = Value::Array(vec![Value::Number(0.into())]);
            }
        }
    }

    let var = obj.get_mut("variables").unwrap();
    let var_obj = var.as_object_mut().unwrap();
    let var_data = var_obj.get_mut("_data").unwrap();

    match var_data {
        Value::Array(arr) => arr,
        Value::Object(ref mut obj) => obj.get_mut("@a").unwrap().as_array_mut().unwrap(),
        _ => unreachable!("_data was normalized to Array or Object with @a"),
    }
}

// ── 单元测试 ──
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_has_json_ex_c_format_true() {
        let data = json!({"@c": []});
        assert!(has_json_ex_c_format(&data));
    }

    #[test]
    fn test_has_json_ex_c_format_nested() {
        let data = json!({"system": {"switches": {"@c": []}}});
        assert!(has_json_ex_c_format(&data));
    }

    #[test]
    fn test_has_json_ex_c_format_plain() {
        let data = json!({"name": "test", "value": 42});
        assert!(!has_json_ex_c_format(&data));
    }

    #[test]
    fn test_is_sparse_dict_true() {
        let data = json!({"@a": [1, 2, 3], "@c": "ref"});
        assert!(is_sparse_dict(&data));
    }

    #[test]
    fn test_is_sparse_dict_with_c_false() {
        let data = json!({"@c": "ref"});
        assert!(!is_sparse_dict(&data));
    }

    #[test]
    fn test_resolve_array_plain() {
        let data = json!([1, 2, 3]);
        let result = resolve_array(&data);
        assert_eq!(result, vec![json!(1), json!(2), json!(3)]);
    }

    #[test]
    fn test_resolve_array_a_as_list() {
        let data = json!({"@a": [true, false, true]});
        let result = resolve_array(&data);
        assert_eq!(result, vec![json!(true), json!(false), json!(true)]);
    }

    #[test]
    fn test_resolve_array_a_as_sparse() {
        let data = json!({"@a": {"1": true, "3": false}});
        let result = resolve_array(&data);
        assert_eq!(result.len(), 4);
        assert_eq!(result[0], Value::Null);
        assert_eq!(result[1], json!(true));
        assert_eq!(result[2], Value::Null);
        assert_eq!(result[3], json!(false));
    }

    #[test]
    fn test_resolve_array_flat_preserves_nulls() {
        let data = json!({"@a": {"2": true, "5": false}});
        let result = resolve_array_flat(&data);
        assert_eq!(result.len(), 6);
        assert!(result[0].is_none());
        assert!(result[1].is_none());
        assert_eq!(result[2], Some(json!(true)));
        assert!(result[3].is_none());
        assert!(result[4].is_none());
        assert_eq!(result[5], Some(json!(false)));
    }

    #[test]
    fn test_is_meta_key() {
        assert!(is_meta_key("@a"));
        assert!(is_meta_key("@c"));
        assert!(!is_meta_key("switches"));
        assert!(!is_meta_key(""));
    }

    #[test]
    fn test_filter_meta_keys() {
        let mut map = Map::new();
        map.insert("@a".to_string(), json!([1, 2]));
        map.insert("@c".to_string(), json!("ref"));
        map.insert("data".to_string(), json!(42));
        map.insert("name".to_string(), json!("test"));

        let filtered = filter_meta_keys(&map);
        assert_eq!(filtered.len(), 2);
        assert!(filtered.contains_key("data"));
        assert!(filtered.contains_key("name"));
        assert!(!filtered.contains_key("@a"));
        assert!(!filtered.contains_key("@c"));
    }

    #[test]
    fn test_ensure_switches_plain_list() {
        let mut data = json!({
            "switches": {
                "_data": [false, true, false]
            }
        });
        let arr = ensure_switches_array(&mut data);
        assert_eq!(arr.len(), 3);
        assert_eq!(arr[1], json!(true));

        arr.push(json!(true));
        assert_eq!(arr.len(), 4);
        assert_eq!(arr[3], json!(true));
    }

    #[test]
    fn test_ensure_switches_from_sparse() {
        let mut data = json!({
            "switches": {
                "_data": {
                    "@c": [],
                    "@a": {"1": true, "3": false}
                }
            }
        });
        let arr = ensure_switches_array(&mut data);
        assert!(arr.len() >= 4);
        assert_eq!(arr[1], json!(true));
        assert_eq!(arr[3], json!(false));
    }

    #[test]
    fn test_ensure_switches_create_new() {
        let mut data = json!({});
        let arr = ensure_switches_array(&mut data);
        assert!(arr.len() >= 1);
        assert_eq!(arr[0], json!(false));

        arr.push(json!(true));
        assert_eq!(arr[1], json!(true));
    }

    #[test]
    fn test_ensure_variables_from_sparse() {
        let mut data = json!({
            "variables": {
                "_data": {
                    "@a": {"2": 100, "5": 200}
                }
            }
        });
        let arr = ensure_variables_array(&mut data);
        assert!(arr.len() >= 6);
        assert_eq!(arr[0], json!(0));
        assert_eq!(arr[2], json!(100));
        assert_eq!(arr[5], json!(200));
    }

    #[test]
    fn test_resolve_array_c_only_returns_empty() {
        let data = json!({"@c": ["ref"]});
        let result = resolve_array(&data);
        assert!(result.is_empty());
    }

    #[test]
    fn test_resolve_array_flat_c_only_returns_empty() {
        let data = json!({"@c": ["ref"]});
        let result = resolve_array_flat(&data);
        assert!(result.is_empty());
    }
}
