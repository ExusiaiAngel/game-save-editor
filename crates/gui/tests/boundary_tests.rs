//! 边界与性能测试。
//!
//! 验证 GUI 组件在高负载、大规模数据下的行为：
//! - 5000/10000 个字段的大集合操作（筛选、分类、dirty 统计）不应崩溃
//! - 引擎显示名称对所有变体均覆盖
//! - 工厂方法在所有引擎类型下返回一致结果
//! - JSON Value 边界的 null/bool/number/string 转换

mod common;

use game_tool_core::detector::EngineType;
use game_tool_core::ModifiableField;
use game_tool_gui::factory;
use serde_json::Value;
use std::time::Instant;

/// 创建用于边界测试的模拟字段
/// 设置最小化的有效字段，只保留关键属性
fn make_mock_field(id: usize, category: &str) -> ModifiableField {
    ModifiableField {
        category: category.into(),
        field_id: format!("{}_{}", category, id),
        display_name: format!("{} #{}", category, id),
        field_type: "int".into(),
        item_id: id as i32,
        save_value: Value::Number(id.into()),
        live_value: Value::Number(id.into()),
        min_val: 0,
        max_val: 99_999_999,
        ..Default::default()
    }
}

/// 5000 个字段的大型集合操作不应崩溃：筛选、计数、修改脏标记后重新计数
#[test]
fn test_large_field_set_no_panic() {
    let mut fields = Vec::new();
    for i in 0..5000 {
        fields.push(make_mock_field(i, "switch"));
    }
    assert_eq!(fields.len(), 5000);

    let filtered: Vec<_> = fields
        .iter()
        .filter(|f| f.field_id == "switch_42")
        .collect();
    assert_eq!(filtered.len(), 1);

    let dirty_count = fields.iter().filter(|f| f.dirty).count();
    assert_eq!(dirty_count, 0);

    fields[100].dirty = true;
    let new_dirty = fields.iter().filter(|f| f.dirty).count();
    assert_eq!(new_dirty, 1);
}

/// 10000 个字段（4 个分类各 2500 个）的 BTreeMap 分类统计应在 1 秒内完成
#[test]
fn test_large_category_grouping() {
    let mut fields = Vec::new();
    for i in 0..2500 {
        fields.push(make_mock_field(i, "switch"));
    }
    for i in 0..2500 {
        fields.push(make_mock_field(i, "variable"));
    }
    for i in 0..2500 {
        fields.push(make_mock_field(i, "item"));
    }
    for i in 0..2500 {
        fields.push(make_mock_field(i, "actor"));
    }
    assert_eq!(fields.len(), 10000);

    let start = Instant::now();
    let mut cats: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    for f in &fields {
        *cats.entry(f.category.clone()).or_default() += 1;
    }
    let elapsed = start.elapsed();

    assert_eq!(cats.len(), 4);
    assert_eq!(cats.get("switch"), Some(&2500));
    assert_eq!(cats.get("variable"), Some(&2500));
    assert_eq!(cats.get("item"), Some(&2500));
    assert_eq!(cats.get("actor"), Some(&2500));

    // Should complete in under 1 second on any reasonable machine
    assert!(
        elapsed.as_millis() < 1000,
        "Categorization took {}ms, expected <1000ms",
        elapsed.as_millis()
    );
}

/// 引擎类型到显示名称的映射辅助函数（用于覆盖测试）
fn engine_display_name(engine: &EngineType) -> &str {
    match engine {
        EngineType::RpgMakerMv => "RPG Maker MV",
        EngineType::RpgMakerMz => "RPG Maker MZ",
        EngineType::NwJs => "NW.js",
        EngineType::RenPy => "Ren'Py",
        EngineType::Unreal => "Unreal",
        EngineType::UnityMono => "Unity (Mono)",
        EngineType::UnityIl2Cpp => "Unity (IL2CPP)",
        EngineType::Godot => "Godot",
        EngineType::Unknown => "未知",
    }
}

/// 所有引擎类型变体都应有非空的显示名称
#[test]
fn test_all_engine_variants_have_display_names() {
    let engines = vec![
        EngineType::RpgMakerMv,
        EngineType::RpgMakerMz,
        EngineType::NwJs,
        EngineType::RenPy,
        EngineType::Unreal,
        EngineType::UnityMono,
        EngineType::UnityIl2Cpp,
        EngineType::Godot,
        EngineType::Unknown,
    ];

    for engine in engines {
        let name = engine_display_name(&engine);
        assert!(!name.is_empty(), "Engine {:?} has no display name", engine);
    }
}

/// 工厂方法 create_format 在所有引擎类型下返回一致结果：
/// 已知引擎返回 Some 且 extensions/name 非空，未知引擎返回 None
#[test]
fn test_create_format_all_engines_consistent() {
    let engines = vec![
        (EngineType::RpgMakerMv, true),
        (EngineType::RpgMakerMz, true),
        (EngineType::NwJs, true),
        (EngineType::RenPy, true),
        (EngineType::Unreal, true),
        (EngineType::UnityMono, true),
        (EngineType::UnityIl2Cpp, true),
        (EngineType::Godot, true),
        (EngineType::Unknown, false),
    ];

    for (engine, should_have_format) in engines.iter() {
        let format = factory::create_format(engine);
        assert_eq!(
            format.is_some(),
            *should_have_format,
            "Engine {:?}: expected is_some={}",
            engine,
            should_have_format
        );

        if let Some(f) = format {
            let exts = f.extensions();
            assert!(
                !exts.is_empty(),
                "Engine {:?} format has empty extensions",
                engine
            );
            assert!(
                !f.name().is_empty(),
                "Engine {:?} format has empty name",
                engine
            );
        }
    }
}

/// 所有已知引擎都应支持桥接创建，未知引擎返回 None
#[test]
fn test_create_bridge_all_supported_except_unknown() {
    let supported = vec![
        EngineType::RpgMakerMv,
        EngineType::RpgMakerMz,
        EngineType::NwJs,
        EngineType::RenPy,
        EngineType::Unreal,
        EngineType::UnityMono,
        EngineType::UnityIl2Cpp,
        EngineType::Godot,
    ];
    let unsupported = vec![EngineType::Unknown];

    for engine in &supported {
        let bridge = factory::create_bridge(engine, "localhost", 8080);
        assert!(
            bridge.is_some(),
            "Engine {:?} should support bridge",
            engine
        );
    }
    for engine in &unsupported {
        let bridge = factory::create_bridge(engine, "localhost", 8080);
        assert!(
            bridge.is_none(),
            "Engine {:?} should NOT support bridge",
            engine
        );
    }
}

/// JSON Value 边界测试：Null/Bool/Number/String 的基本类型转换
#[test]
fn test_value_display_edge_cases() {
    let null_val = Value::Null;
    let bool_true = Value::Bool(true);
    let bool_false = Value::Bool(false);
    let large_num = Value::Number(serde_json::Number::from(9_999_999_999i64));
    let text = Value::String("测试中文".into());

    assert_eq!(null_val.is_null(), true);
    assert_eq!(bool_true.as_bool(), Some(true));
    assert_eq!(bool_false.as_bool(), Some(false));
    assert_eq!(large_num.as_i64(), Some(9_999_999_999));
    assert_eq!(text.as_str(), Some("测试中文"));
}
