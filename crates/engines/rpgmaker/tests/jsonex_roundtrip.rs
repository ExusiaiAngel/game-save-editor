use game_tool_rpgmaker::jsonex;
use serde_json::{json, Value};

#[test]
fn test_roundtrip_a_plain_array() {
    let data = json!([true, false, true]);
    let resolved = jsonex::resolve_array(&data);
    assert_eq!(resolved, vec![json!(true), json!(false), json!(true)]);
}

#[test]
fn test_roundtrip_a_sparse_dict() {
    let data = json!({"@a": {"1": true, "5": false, "10": true}});
    let resolved = jsonex::resolve_array(&data);
    assert_eq!(resolved.len(), 11);
    assert_eq!(resolved[1], json!(true));
    assert_eq!(resolved[5], json!(false));
    assert_eq!(resolved[10], json!(true));
}

#[test]
fn test_has_json_ex_c_format_with_c() {
    let data = json!({"switches": {"_data": {"@c": [], "@a": {"1": true}}}});
    assert!(jsonex::has_json_ex_c_format(&data));
}

#[test]
fn test_has_json_ex_c_format_without_c() {
    let data = json!({"switches": {"_data": [false, true]}});
    assert!(!jsonex::has_json_ex_c_format(&data));
}

#[test]
fn test_is_sparse_dict_positive() {
    let data = json!({"@a": {"1": true}, "@c": []});
    assert!(jsonex::is_sparse_dict(&data));
}

#[test]
fn test_is_sparse_dict_with_c_is_false() {
    let data = json!({"@c": []});
    assert!(!jsonex::is_sparse_dict(&data));
}

#[test]
fn test_ensure_switches_preserves_metadata() {
    let mut data = json!({
        "switches": {
            "_data": {
                "@c": [1, 2, 3],
                "@a": {"1": true, "2": false}
            }
        }
    });

    {
        let arr = jsonex::ensure_switches_array(&mut data);
        while arr.len() <= 3 {
            arr.push(json!(false));
        }
        arr[3] = json!(true);
    }

    let sw_data = &data["switches"]["_data"];
    assert!(sw_data.is_object());
    assert_eq!(sw_data["@a"][3], json!(true));
}

#[test]
fn test_ensure_variables_preserves_roundtrip() {
    let mut data = json!({
        "variables": {
            "_data": {
                "@c": [42],
                "@a": {"1": 10, "2": 20, "3": 30}
            }
        }
    });

    {
        let arr = jsonex::ensure_variables_array(&mut data);
        arr[2] = json!(99);
    }

    let arr = jsonex::ensure_variables_array(&mut data);
    assert_eq!(arr[1], json!(10));
    assert_eq!(arr[2], json!(99));
    assert_eq!(arr[3], json!(30));
}

#[test]
fn test_resolve_array_idempotency() {
    let data = json!({"@a": {"3": 1, "7": 2}});
    let first = jsonex::resolve_array(&data);
    let second = jsonex::resolve_array(&data);
    assert_eq!(first, second);

    let data2 = json!([1, 2, 3]);
    let first2 = jsonex::resolve_array(&data2);
    let second2 = jsonex::resolve_array(&data2);
    assert_eq!(first2, second2);
}

#[test]
fn test_filter_meta_keys_roundtrip() {
    let data = json!({"@a": [1, 2, 3], "@c": "ref", "value": 42, "name": "test"});

    let filtered = if let Value::Object(map) = &data {
        jsonex::filter_meta_keys(map)
    } else {
        panic!("expected object");
    };

    assert_eq!(filtered.len(), 2);
    assert_eq!(filtered.get("value").unwrap(), &json!(42));
    assert_eq!(filtered.get("name").unwrap(), &json!("test"));
    assert!(!filtered.contains_key("@a"));
    assert!(!filtered.contains_key("@c"));
}
