//! RPG Maker MV 游戏数据扫描器
//!
//! 扫描游戏目录的 System.json, Actors.json, Items.json 等数据文件，
//! 提取开关/变量/角色/物品/武器/防具的名称映射。

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use serde_json::Value;

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

fn load_json(data_dir: &str, filename: &str) -> Option<Value> {
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

pub fn actor_name(config: &GameConfig, id: usize) -> String {
    config.actor_names.get(&id).cloned().unwrap_or_else(|| format!("角色 #{}", id))
}

pub fn item_name(config: &GameConfig, id: usize) -> String {
    config.item_names.get(&id).cloned().unwrap_or_else(|| format!("物品 #{}", id))
}

pub fn switch_name(config: &GameConfig, id: usize) -> String {
    config.switch_names.get(&id).cloned().unwrap_or_else(|| format!("开关 #{}", id))
}

pub fn variable_name(config: &GameConfig, id: usize) -> String {
    config.variable_names.get(&id).cloned().unwrap_or_else(|| format!("变量 #{}", id))
}

#[cfg(test)]
mod tests {
    use super::*;
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
