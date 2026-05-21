use serde_json::{Map, Value};

fn has_json_ex_c_format_depth(data: &Value, depth: usize) -> bool {
    if depth > 20 {
        return false;
    }
    match data {
        Value::Object(map) => {
            if map.contains_key("@c") {
                return true;
            }
            map.values().any(|v| has_json_ex_c_format_depth(v, depth + 1))
        }
        Value::Array(arr) => arr
            .iter()
            .any(|v| has_json_ex_c_format_depth(v, depth + 1)),
        _ => false,
    }
}

pub fn has_json_ex_c_format(data: &Value) -> bool {
    has_json_ex_c_format_depth(data, 0)
}

pub fn is_sparse_dict(data: &Value) -> bool {
    match data {
        Value::Object(map) => map.contains_key("@a"),
        _ => false,
    }
}

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
    let mut arr = vec![Value::Null; max_idx + 1];
    for i in indices {
        arr[i] = sparse[&i.to_string()].clone();
    }
    arr
}

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

fn expand_sparse_dict_with_default(
    sparse: &Map<String, Value>,
    default: &Value,
) -> Vec<Value> {
    let mut indices: Vec<usize> = sparse
        .keys()
        .filter_map(|k| k.parse::<usize>().ok())
        .collect();
    if indices.is_empty() {
        return Vec::new();
    }
    indices.sort_unstable();
    let max_idx = *indices.last().unwrap();
    let mut arr = vec![default.clone(); max_idx + 1];
    for i in indices {
        arr[i] = sparse[&i.to_string()].clone();
    }
    arr
}

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

pub fn is_meta_key(key: &str) -> bool {
    key.starts_with('@')
}

pub fn filter_meta_keys(map: &Map<String, Value>) -> Map<String, Value> {
    map.iter()
        .filter(|(k, _)| !k.starts_with('@'))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

pub fn ensure_switches_array(data: &mut Value) -> &mut Vec<Value> {
    if !data.is_object() {
        *data = Value::Object(Map::new());
    }

    let obj = data.as_object_mut().unwrap();
    if !obj.contains_key("switches") {
        obj.insert(
            "switches".to_string(),
            Value::Object(Map::new()),
        );
    }

    {
        let sw = obj.get_mut("switches").unwrap();
        if !sw.is_object() {
            *sw = Value::Object(Map::new());
        }
        let sw_obj = sw.as_object_mut().unwrap();
        if !sw_obj.contains_key("_data") {
            sw_obj.insert(
                "_data".to_string(),
                Value::Array(vec![Value::Bool(false)]),
            );
        }
    }

    {
        let sw = obj.get_mut("switches").unwrap();
        if !sw.is_object() {
            *sw = Value::Object(Map::new());
        }
        let sw_obj = sw.as_object_mut().unwrap();
        if !sw_obj.contains_key("_data") {
            sw_obj.insert(
                "_data".to_string(),
                Value::Array(vec![Value::Bool(false)]),
            );
        }
        let sw_data = sw_obj.get_mut("_data").unwrap();

        if !sw_data.is_array() {
            if let Some(data_obj) = sw_data.as_object_mut() {
                if let Some(a_val) = data_obj.get_mut("@a") {
                    if a_val.is_object() {
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

    let sw = obj.get_mut("switches").unwrap();
    let sw_obj = sw.as_object_mut().unwrap();
    let sw_data = sw_obj.get_mut("_data").unwrap();

    match sw_data {
        Value::Array(arr) => arr,
        Value::Object(ref mut obj) => obj.get_mut("@a").unwrap().as_array_mut().unwrap(),
        _ => unreachable!("_data was normalized to Array or Object with @a"),
    }
}

pub fn ensure_variables_array(data: &mut Value) -> &mut Vec<Value> {
    if !data.is_object() {
        *data = Value::Object(Map::new());
    }

    let obj = data.as_object_mut().unwrap();
    if !obj.contains_key("variables") {
        obj.insert(
            "variables".to_string(),
            Value::Object(Map::new()),
        );
    }

    {
        let var = obj.get_mut("variables").unwrap();
        if !var.is_object() {
            *var = Value::Object(Map::new());
        }
        let var_obj = var.as_object_mut().unwrap();
        if !var_obj.contains_key("_data") {
            var_obj.insert(
                "_data".to_string(),
                Value::Array(vec![Value::Number(0.into())]),
            );
        }
        let var_data = var_obj.get_mut("_data").unwrap();

        if !var_data.is_array() {
            if let Some(data_obj) = var_data.as_object_mut() {
                if let Some(a_val) = data_obj.get_mut("@a") {
                    if a_val.is_object() {
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
