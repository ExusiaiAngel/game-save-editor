use crate::state::SavePanelMode;
use game_tool_core::detector::EngineType;
use game_tool_core::{GameBridge, GameState, ISaveFormat, ModifiableField};
use game_tool_generic::format::GenericJsonFormat;
use game_tool_renpy::bridge::RenPyBridge;
use game_tool_renpy::format::RenPyFormat;
use game_tool_rpgmaker::format::RpgMakerFormat;
use game_tool_rpgmaker::scanner::GameConfig;
use game_tool_rpgmaker::tcp::RpgMakerTcpBridge;
use game_tool_unreal::format::UnrealGVASFormat;
use serde_json::Value;

pub fn create_format(engine: &EngineType) -> Option<Box<dyn ISaveFormat>> {
    match engine {
        EngineType::RpgMakerMv | EngineType::RpgMakerMz | EngineType::NwJs => {
            Some(Box::new(RpgMakerFormat::new()))
        }
        EngineType::RenPy => Some(Box::new(RenPyFormat::new())),
        EngineType::Unreal => Some(Box::new(UnrealGVASFormat::new())),
        EngineType::UnityMono | EngineType::UnityIl2Cpp | EngineType::Godot => {
            Some(Box::new(GenericJsonFormat::new()))
        }
        EngineType::Unknown => None,
    }
}

pub fn create_bridge(engine: &EngineType, host: &str, port: u16) -> Option<Box<dyn GameBridge>> {
    match engine {
        EngineType::RpgMakerMv | EngineType::RpgMakerMz | EngineType::NwJs => {
            Some(Box::new(RpgMakerTcpBridge::new(host, port)))
        }
        EngineType::RenPy => Some(Box::new(RenPyBridge::new(host, port))),
        _ => None,
    }
}

pub fn is_readonly(engine: &EngineType) -> bool {
    matches!(engine, EngineType::Unreal)
}

pub fn supports_realtime(engine: &EngineType) -> bool {
    matches!(
        engine,
        EngineType::RpgMakerMv | EngineType::RpgMakerMz | EngineType::NwJs | EngineType::RenPy
    )
}

pub fn engine_to_panel_mode(engine: &EngineType) -> SavePanelMode {
    match engine {
        EngineType::RpgMakerMv | EngineType::RpgMakerMz | EngineType::NwJs => {
            SavePanelMode::RpgMaker
        }
        EngineType::RenPy => SavePanelMode::RenPy,
        EngineType::Unreal => SavePanelMode::Unreal,
        _ => SavePanelMode::Generic,
    }
}

pub fn game_state_to_fields(
    state: &GameState,
    engine: &EngineType,
    config: Option<&GameConfig>,
) -> Vec<ModifiableField> {
    match engine {
        EngineType::RpgMakerMv | EngineType::RpgMakerMz | EngineType::NwJs => {
            rpgmaker_state_to_fields(state, config)
        }
        EngineType::RenPy => renpy_state_to_fields(state),
        _ => vec![],
    }
}

fn rpgmaker_state_to_fields(
    state: &GameState,
    config: Option<&GameConfig>,
) -> Vec<ModifiableField> {
    let mut fields = Vec::new();
    let ext = &state.extensions;

    if let Some(gold_val) = ext.get("gold") {
        let gold = gold_val
            .as_i64()
            .or_else(|| gold_val.as_str().and_then(|s| s.parse::<i64>().ok()))
            .unwrap_or(0);
        let display_name = config
            .map(|c| {
                if c.currency_unit.is_empty() {
                    "金币".into()
                } else {
                    format!("金币 ({})", c.currency_unit)
                }
            })
            .unwrap_or_else(|| "金币".into());
        fields.push(ModifiableField {
            category: "gold".into(),
            field_id: "gold".into(),
            display_name,
            field_type: "int".into(),
            live_value: Value::Number(gold.into()),
            min_val: 0,
            max_val: 99_999_999,
            ..Default::default()
        });
    }

    if let Some(switches) = ext.get("switches").and_then(|v| v.as_object()) {
        for (k, val) in switches {
            if let Ok(i) = k.parse::<usize>() {
                let display_name = config
                    .map(|c| game_tool_rpgmaker::scanner::switch_name(c, i))
                    .unwrap_or_else(|| format!("开关 #{}", i));
                fields.push(ModifiableField {
                    category: "switch".into(),
                    field_id: format!("switch_{}", i),
                    display_name,
                    item_id: i as i32,
                    field_type: "bool".into(),
                    live_value: val.clone(),
                    min_val: 0,
                    max_val: 1,
                    ..Default::default()
                });
            }
        }
    }

    if let Some(vars) = ext.get("variables").and_then(|v| v.as_object()) {
        for (k, val) in vars {
            if let Ok(i) = k.parse::<usize>() {
                let v = val.as_i64().unwrap_or(0) as i32;
                let display_name = config
                    .map(|c| game_tool_rpgmaker::scanner::variable_name(c, i))
                    .unwrap_or_else(|| format!("变量 #{}", i));
                fields.push(ModifiableField {
                    category: "variable".into(),
                    field_id: format!("var_{}", i),
                    display_name,
                    item_id: i as i32,
                    field_type: "int".into(),
                    live_value: Value::Number(v.into()),
                    min_val: -9_999_999,
                    max_val: 99_999_999,
                    ..Default::default()
                });
            }
        }
    }

    if let Some(items) = ext.get("items").and_then(|v| v.as_object()) {
        for (k, count) in items {
            if let Ok(i) = k.parse::<usize>() {
                let c = count.as_i64().unwrap_or(0) as i32;
                if c > 0 {
                    let display_name = config
                        .map(|c| game_tool_rpgmaker::scanner::item_name(c, i))
                        .unwrap_or_else(|| format!("物品 #{}", i));
                    fields.push(ModifiableField {
                        category: "item".into(),
                        field_id: format!("item_{}", i),
                        display_name,
                        item_id: i as i32,
                        field_type: "int".into(),
                        live_value: Value::Number(c.into()),
                        min_val: 0,
                        max_val: 999,
                        ..Default::default()
                    });
                }
            }
        }
    }

    if let Some(party) = ext.get("party").and_then(|v| v.as_array()) {
        for (fallback_idx, actor) in party.iter().enumerate() {
            let id = actor
                .get("_actorId")
                .and_then(|v| v.as_i64())
                .map(|v| v as i32)
                .filter(|&v| v > 0)
                .unwrap_or((fallback_idx + 1) as i32);
            let hp = actor.get("_hp").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            let mp = actor.get("_mp").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            let name = config
                .map(|c| game_tool_rpgmaker::scanner::actor_name(c, id as usize))
                .unwrap_or_else(|| format!("角色 #{}", id));
            fields.push(ModifiableField {
                category: "actor".into(),
                field_id: format!("actor_{}_hp", id),
                display_name: format!("{} HP", name),
                item_id: id,
                field_type: "int".into(),
                live_value: Value::Number(hp.into()),
                min_val: 0,
                max_val: 999_999,
                ..Default::default()
            });
            fields.push(ModifiableField {
                category: "actor".into(),
                field_id: format!("actor_{}_mp", id),
                display_name: format!("{} MP", name),
                item_id: id,
                field_type: "int".into(),
                live_value: Value::Number(mp.into()),
                min_val: 0,
                max_val: 999_999,
                ..Default::default()
            });
            let level = actor.get("_level").and_then(|v| v.as_i64()).unwrap_or(1) as i32;
            fields.push(ModifiableField {
                category: "actor".into(),
                field_id: format!("actor_{}_level", id),
                display_name: format!("{} 等级", name),
                item_id: id,
                field_type: "int".into(),
                live_value: Value::Number(level.into()),
                min_val: 1,
                max_val: 99,
                ..Default::default()
            });
        }
    }

    if let Some(ss) = ext.get("selfSwitches").and_then(|v| v.as_object()) {
        for (k, v) in ss {
            fields.push(ModifiableField {
                category: "self_switch".into(),
                field_id: format!("ss_{}", k),
                display_name: format!("Self Switch: {}", k),
                field_type: "bool".into(),
                live_value: v.clone(),
                min_val: 0,
                max_val: 1,
                ..Default::default()
            });
        }
    }

    fields
}

fn renpy_state_to_fields(state: &GameState) -> Vec<ModifiableField> {
    let mut fields = Vec::new();
    if let Some(store) = state.extensions.get("store").and_then(|v| v.as_object()) {
        for (key, val) in store {
            let field_type = match val {
                Value::Bool(_) => "bool",
                Value::Number(_) => "int",
                Value::String(_) => "str",
                _ => "str",
            };
            fields.push(ModifiableField {
                category: "store".into(),
                field_id: format!("var_{}", key),
                display_name: key.clone(),
                field_type: field_type.into(),
                live_value: val.clone(),
                ..Default::default()
            });
        }
    }
    fields
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_readonly_only_unreal() {
        assert!(!is_readonly(&EngineType::RpgMakerMv));
        assert!(!is_readonly(&EngineType::RpgMakerMz));
        assert!(!is_readonly(&EngineType::NwJs));
        assert!(!is_readonly(&EngineType::RenPy));
        assert!(is_readonly(&EngineType::Unreal));
        assert!(!is_readonly(&EngineType::UnityMono));
        assert!(!is_readonly(&EngineType::UnityIl2Cpp));
        assert!(!is_readonly(&EngineType::Godot));
        assert!(!is_readonly(&EngineType::Unknown));
    }

    #[test]
    fn test_supports_realtime_rpg_and_renpy() {
        assert!(supports_realtime(&EngineType::RpgMakerMv));
        assert!(supports_realtime(&EngineType::RpgMakerMz));
        assert!(supports_realtime(&EngineType::NwJs));
        assert!(supports_realtime(&EngineType::RenPy));
    }

    #[test]
    fn test_supports_realtime_others_false() {
        assert!(!supports_realtime(&EngineType::Unreal));
        assert!(!supports_realtime(&EngineType::UnityMono));
        assert!(!supports_realtime(&EngineType::UnityIl2Cpp));
        assert!(!supports_realtime(&EngineType::Godot));
        assert!(!supports_realtime(&EngineType::Unknown));
    }

    #[test]
    fn test_engine_to_panel_mode_rpgmaker() {
        assert_eq!(
            engine_to_panel_mode(&EngineType::RpgMakerMv),
            SavePanelMode::RpgMaker
        );
        assert_eq!(
            engine_to_panel_mode(&EngineType::RpgMakerMz),
            SavePanelMode::RpgMaker
        );
        assert_eq!(
            engine_to_panel_mode(&EngineType::NwJs),
            SavePanelMode::RpgMaker
        );
    }

    #[test]
    fn test_engine_to_panel_mode_others() {
        assert_eq!(
            engine_to_panel_mode(&EngineType::RenPy),
            SavePanelMode::RenPy
        );
        assert_eq!(
            engine_to_panel_mode(&EngineType::Unreal),
            SavePanelMode::Unreal
        );
        assert_eq!(
            engine_to_panel_mode(&EngineType::UnityMono),
            SavePanelMode::Generic
        );
        assert_eq!(
            engine_to_panel_mode(&EngineType::Godot),
            SavePanelMode::Generic
        );
        assert_eq!(
            engine_to_panel_mode(&EngineType::Unknown),
            SavePanelMode::Generic
        );
    }

    #[test]
    fn test_create_format_unknown_returns_none() {
        assert!(create_format(&EngineType::Unknown).is_none());
    }

    #[test]
    fn test_create_format_all_known_have_format() {
        for e in &[
            EngineType::RpgMakerMv,
            EngineType::RpgMakerMz,
            EngineType::NwJs,
            EngineType::RenPy,
            EngineType::Unreal,
            EngineType::UnityMono,
            EngineType::UnityIl2Cpp,
            EngineType::Godot,
        ] {
            let f = create_format(e);
            assert!(f.is_some(), "{:?} should have format", e);
            let fmt = f.unwrap();
            assert!(!fmt.name().is_empty());
            assert!(!fmt.extensions().is_empty());
        }
    }

    #[test]
    fn test_create_bridge_supported() {
        for e in &[
            EngineType::RpgMakerMv,
            EngineType::RpgMakerMz,
            EngineType::NwJs,
            EngineType::RenPy,
        ] {
            assert!(
                create_bridge(e, "127.0.0.1", 19999).is_some(),
                "{:?} should support bridge",
                e
            );
        }
    }

    #[test]
    fn test_create_bridge_unsupported() {
        for e in &[
            EngineType::Unreal,
            EngineType::UnityMono,
            EngineType::UnityIl2Cpp,
            EngineType::Godot,
            EngineType::Unknown,
        ] {
            assert!(
                create_bridge(e, "127.0.0.1", 19999).is_none(),
                "{:?} should NOT support bridge",
                e
            );
        }
    }

    #[test]
    fn test_game_state_to_fields_unknown_empty() {
        let state = GameState::default();
        let fields = game_state_to_fields(&state, &EngineType::Unknown, None);
        assert!(fields.is_empty());
    }

    #[test]
    fn test_rpgmaker_gold_field() {
        let mut state = GameState::default();
        state
            .extensions
            .insert("gold".into(), Value::Number(5000.into()));
        let fields = game_state_to_fields(&state, &EngineType::RpgMakerMv, None);
        let gf = fields.iter().find(|f| f.field_id == "gold").unwrap();
        assert_eq!(gf.live_value.as_i64(), Some(5000));
        assert_eq!(gf.field_type, "int");
    }

    #[test]
    fn test_rpgmaker_switches() {
        let mut state = GameState::default();
        let mut sw = serde_json::Map::new();
        sw.insert("1".into(), Value::Bool(true));
        sw.insert("2".into(), Value::Bool(false));
        state
            .extensions
            .insert("switches".into(), Value::Object(sw));
        let fields = game_state_to_fields(&state, &EngineType::RpgMakerMv, None);
        let sf: Vec<_> = fields.iter().filter(|f| f.category == "switch").collect();
        assert_eq!(sf.len(), 2);
    }

    #[test]
    fn test_rpgmaker_variables() {
        let mut state = GameState::default();
        let mut vars = serde_json::Map::new();
        vars.insert("1".into(), Value::Number(42.into()));
        state
            .extensions
            .insert("variables".into(), Value::Object(vars));
        let fields = game_state_to_fields(&state, &EngineType::RpgMakerMv, None);
        assert!(fields.iter().any(|f| f.field_id == "var_1"));
    }

    #[test]
    fn test_rpgmaker_items_zero_count_excluded() {
        let mut state = GameState::default();
        let mut items = serde_json::Map::new();
        items.insert("1".into(), Value::Number(5.into()));
        items.insert("2".into(), Value::Number(0.into()));
        state
            .extensions
            .insert("items".into(), Value::Object(items));
        let fields = game_state_to_fields(&state, &EngineType::RpgMakerMv, None);
        let if_: Vec<_> = fields.iter().filter(|f| f.category == "item").collect();
        assert_eq!(if_.len(), 1);
        assert_eq!(if_[0].field_id, "item_1");
    }

    #[test]
    fn test_rpgmaker_party_actors() {
        let mut state = GameState::default();
        let party = vec![serde_json::json!({"_actorId": 1, "_hp": 100, "_mp": 50, "_level": 5})];
        state.extensions.insert("party".into(), Value::Array(party));
        let fields = game_state_to_fields(&state, &EngineType::RpgMakerMv, None);
        assert!(fields.iter().any(|f| f.field_id == "actor_1_hp"));
        assert!(fields.iter().any(|f| f.field_id == "actor_1_mp"));
        assert!(fields.iter().any(|f| f.field_id == "actor_1_level"));
    }

    #[test]
    fn test_renpy_store_fields() {
        let mut state = GameState::default();
        let mut store = serde_json::Map::new();
        store.insert("money".into(), Value::Number(1000.into()));
        store.insert("alive".into(), Value::Bool(true));
        store.insert("name".into(), Value::String("Hero".into()));
        state
            .extensions
            .insert("store".into(), Value::Object(store));
        let fields = game_state_to_fields(&state, &EngineType::RenPy, None);
        assert_eq!(fields.len(), 3);
    }

    #[test]
    fn test_renpy_empty_store() {
        let state = GameState::default();
        let fields = game_state_to_fields(&state, &EngineType::RenPy, None);
        assert!(fields.is_empty());
    }
}
