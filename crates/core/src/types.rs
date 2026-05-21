//! 核心类型定义 — 数据模型与 Trait 接口
//!
//! 提供格式无关的存档数据模型和处理器抽象接口。
//! UI 层仅通过此模块的类型操作存档，不直接依赖具体格式。

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

use crate::error::GameToolError;

// ═══════════════════════════════════════════════════════════════
// 数据模型
// ═══════════════════════════════════════════════════════════════

/// 可修改的存档字段（格式无关）
///
/// 对应 Python 版 `ModifiableField`，是编辑器与格式处理器之间的
/// 统一字段表示。UI 层修改 `save_value` 后，调用 `apply_field` 写回。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModifiableField {
    /// 字段分类：如 "gold", "switch", "variable", "actor", "item", "weapon", "armor", "self_switch"
    pub category: String,
    /// 唯一标识符，如 "switch_12", "var_5", "actor_1_hp"
    pub field_id: String,
    /// 显示名称
    pub display_name: String,
    /// 游戏内 ID
    #[serde(default)]
    pub item_id: i32,
    /// 字段类型： "bool", "int", "str"
    #[serde(default = "default_field_type")]
    pub field_type: String,
    /// 存档中的值
    #[serde(default)]
    pub save_value: Value,
    /// 游戏实时值
    #[serde(default)]
    pub live_value: Value,
    /// 默认值
    #[serde(default)]
    pub default_value: Value,
    /// 最小值
    #[serde(default)]
    pub min_val: i32,
    /// 最大值
    #[serde(default = "default_max_val")]
    pub max_val: i32,
    /// 描述
    #[serde(default)]
    pub description: String,
    /// 用户是否编辑过
    #[serde(default)]
    pub dirty: bool,
}

fn default_field_type() -> String {
    "int".to_string()
}

fn default_max_val() -> i32 {
    99_999_999
}

/// 存档摘要（格式无关）
///
/// 对应 Python 版 `SaveSummary`，用于 UI 概览展示。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SaveSummary {
    #[serde(default)]
    pub gold: i32,
    #[serde(default)]
    pub party_size: i32,
    #[serde(default)]
    pub item_count: i32,
    #[serde(default)]
    pub save_count: i32,
    /// 游戏时间（秒）
    #[serde(default)]
    pub play_time: i32,
    #[serde(default)]
    pub members: Vec<String>,
    /// 格式特有的额外信息
    #[serde(default)]
    pub extra: HashMap<String, Value>,
}

/// 统一的游戏状态快照
///
/// 通用字段直接放在结构体上，引擎特定字段存入 extensions。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameState {
    /// 引擎类型
    #[serde(default = "default_engine")]
    pub engine: String,
    /// 当前地图名称
    #[serde(default)]
    pub map_name: String,
    /// 游戏时间文本
    #[serde(default)]
    pub play_time: String,
    /// 存档次数
    #[serde(default)]
    pub save_count: i32,
    /// 引擎特定扩展数据
    ///
    /// - RPG Maker: extensions["switches"], extensions["variables"],
    ///   extensions["self_switches"], extensions["party"],
    ///   extensions["items"], extensions["gold"], extensions["steps"]
    /// - Ren'Py: extensions["store"] (所有 store 变量)
    /// - Unreal: extensions["properties"] (内存读取属性)
    /// - Generic: extensions["memory_values"] (内存值快照)
    #[serde(default)]
    pub extensions: HashMap<String, Value>,
}

fn default_engine() -> String {
    "unknown".to_string()
}

/// 检测到的游戏信息
///
/// 对应 Python 版 `GameInfo`，描述一个被检测到的游戏目录。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameInfo {
    /// 游戏根目录
    #[serde(default)]
    pub game_dir: String,
    /// 游戏标题
    #[serde(default)]
    pub game_title: String,
    /// 引擎标识：如 "rpg_mv", "rpg_mz", "nwjs"
    #[serde(default = "default_engine")]
    pub engine: String,
    /// 是否 NW.js 封装
    #[serde(default)]
    pub is_nwjs: bool,
    /// 数据目录（含 JSON 配置文件）
    #[serde(default)]
    pub data_dir: String,
    /// 存档目录
    #[serde(default)]
    pub save_dir: String,
    /// www 目录（NW.js 游戏）
    #[serde(default)]
    pub www_dir: String,
    /// 游戏可执行文件路径
    #[serde(default)]
    pub exe_path: String,
    /// 可执行文件名
    #[serde(default = "default_exe_name")]
    pub exe_name: String,
    /// package.json 路径
    #[serde(default)]
    pub package_json_path: String,
    /// 存档文件列表
    #[serde(default)]
    pub save_files: Vec<String>,
    /// 存档格式："rpgsave" 或 "rmmzsave"
    #[serde(default = "default_save_format")]
    pub save_format: String,
    /// 检测来源："save", "process", "dir"
    #[serde(default)]
    pub detected_from: String,
}

fn default_exe_name() -> String {
    "Game.exe".to_string()
}

fn default_save_format() -> String {
    "rpgsave".to_string()
}

/// 桥接命令枚举 — 引擎无关的读写指令
///
/// 所有 GameBridge 实现通过此枚举进行统一调度，
/// 替代原先的 set_gold/set_switch/set_variable 等方法。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BridgeCommand {
    /// 读取指定字段 (field_id: 如 "gold", "switch_12", "actor_1_hp")
    ReadField(String),
    /// 写入指定字段
    WriteField(String, Value),
    /// 读取完整游戏状态
    ReadAll,
}

// ═══════════════════════════════════════════════════════════════
// Trait 接口
// ═══════════════════════════════════════════════════════════════

/// 游戏存档格式处理器接口
///
/// 所有具体格式（RPG Maker、Ren'Py、Unreal 等）实现此接口。
/// UI 层通过此接口操作存档，不关心底层格式细节。
///
/// 对应 Python 版 `ISaveFormat`。
///
/// 使用方式：
/// ```ignore
/// let format: Box<dyn ISaveFormat> = RpgMvSaveFormat::new();
/// let data = format.load("save.rpgsave")?;
/// ```
pub trait ISaveFormat: Send + Sync {
    /// 格式名称，如 "RPG Maker MV/MZ"
    fn name(&self) -> &str;

    /// 支持的文件扩展名列表，如 `[".rpgsave", ".rmmzsave"]`
    fn extensions(&self) -> Vec<String>;

    /// 关联的引擎类型标识符，如 "rpg_maker_mv", "rpg_maker_mz", "renpy"
    fn engine_type(&self) -> &str;

    /// 魔数字节签名，用于格式检测。无签名返回 `None`
    fn magic_bytes(&self) -> Option<&[u8]>;

    // ── 核心 I/O ──

    /// 加载存档文件 → 数据值
    fn load(&self, filepath: &str) -> Result<Value, GameToolError>;

    /// 保存数据值 → 写回存档文件
    fn save(&self, filepath: &str, data: &Value) -> Result<(), GameToolError>;

    // ── 检测与发现 ──

    /// 检测文件是否为此格式
    ///
    /// 默认实现：按魔数 → 扩展名的顺序检测。
    fn detect(&self, filepath: &str) -> bool {
        // 魔数检测
        if let Some(magic) = self.magic_bytes() {
            if let Ok(mut f) = std::fs::File::open(filepath) {
                use std::io::Read;
                let mut buf = vec![0u8; magic.len()];
                if f.read_exact(&mut buf).is_ok() && buf.as_slice() == magic {
                    return true;
                }
            }
        }
        // 扩展名检测
        let ext = std::path::Path::new(filepath)
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| format!(".{}", e.to_lowercase()))
            .unwrap_or_default();
        self.extensions().contains(&ext)
    }

    /// 在游戏目录中查找数据文件目录
    ///
    /// RPG Maker: www/data/
    /// Ren'Py: game/
    fn find_data_dir(&self, game_dir: &str) -> Option<String>;

    // ── 摘要与字段操作 ──

    /// 获取存档摘要信息
    fn get_summary(&self, data: &Value) -> SaveSummary;

    /// 扫描存档中所有可修改字段
    fn scan_fields(&self, data: &Value, game_dir: &str) -> Vec<ModifiableField>;

    /// 将单个字段的修改值写回数据值
    fn apply_field(&self, data: &mut Value, field: &ModifiableField) -> Result<(), GameToolError>;
}

/// 游戏桥接器接口（命令驱动）
///
/// 所有连接后端（TCP、CDP、进程内存）必须实现此接口。
/// 使用 BridgeCommand 枚举替代引擎特定方法，实现引擎无关。
///
/// 对应 Python 版 `IGameBridge`（重新设计）。
pub trait GameBridge: Send + Sync {
    /// 建立连接
    fn connect(&mut self) -> Result<(), crate::error::GameToolError>;

    /// 断开连接
    fn disconnect(&mut self);

    /// 是否已连接
    fn is_connected(&self) -> bool;

    /// 执行桥接命令
    ///
    /// 各引擎实现自行将 BridgeCommand 翻译为协议指令：
    /// - RPG Maker TCP: "set_gold N" 文本
    /// - Ren'Py: {"action": "set_var", ...} JSON
    /// - Windows Memory: WriteProcessMemory(addr, value)
    fn execute(&mut self, cmd: &BridgeCommand) -> Result<Value, crate::error::GameToolError>;

    /// 引擎名称（如 "rpg_maker", "renpy", "unreal", "generic"）
    fn engine_name(&self) -> &str;

    /// 优先级（数字越小越优先尝试）
    fn priority(&self) -> i32;
}

// ═══════════════════════════════════════════════════════════════
// 测试
// ═══════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ── ModifiableField roundtrip ──

    #[test]
    fn test_modifiable_field_defaults() {
        let field = ModifiableField {
            category: "switch".into(),
            field_id: "switch_1".into(),
            display_name: "开关1".into(),
            item_id: 1,
            field_type: "bool".into(),
            save_value: json!(true),
            live_value: json!(false),
            default_value: json!(false),
            min_val: 0,
            max_val: 1,
            description: "测试开关".into(),
            dirty: false,
        };

        let json = serde_json::to_string(&field).expect("序列化失败");
        let decoded: ModifiableField = serde_json::from_str(&json).expect("反序列化失败");

        assert_eq!(decoded.category, "switch");
        assert_eq!(decoded.field_id, "switch_1");
        assert_eq!(decoded.save_value, json!(true));
        assert_eq!(decoded.field_type, "bool");
    }

    #[test]
    fn test_modifiable_field_serde_defaults_on_missing() {
        // 验证 serde default 生效：缺失字段使用默认值
        let json = r#"{"category":"gold","field_id":"gold","display_name":"金币"}"#;
        let field: ModifiableField = serde_json::from_str(json).expect("反序列化失败");
        assert_eq!(field.category, "gold");
        assert_eq!(field.field_type, "int"); // 默认值
        assert_eq!(field.max_val, 99_999_999); // 默认值
        assert_eq!(field.save_value, Value::Null); // 默认值
        assert!(!field.dirty);
    }

    // ── SaveSummary roundtrip ──

    #[test]
    fn test_save_summary_roundtrip() {
        let summary = SaveSummary {
            gold: 12345,
            party_size: 4,
            item_count: 27,
            save_count: 3,
            play_time: 3600,
            members: vec!["Alice".into(), "Bob".into()],
            extra: {
                let mut m = HashMap::new();
                m.insert("actor_count".into(), json!(4));
                m
            },
        };

        let json = serde_json::to_string(&summary).expect("序列化失败");
        let decoded: SaveSummary = serde_json::from_str(&json).expect("反序列化失败");

        assert_eq!(decoded.gold, 12345);
        assert_eq!(decoded.party_size, 4);
        assert_eq!(decoded.members.len(), 2);
        assert_eq!(decoded.extra.get("actor_count").and_then(|v| v.as_i64()), Some(4));
    }

    #[test]
    fn test_save_summary_defaults() {
        let json = "{}";
        let summary: SaveSummary = serde_json::from_str(json).expect("反序列化失败");
        assert_eq!(summary.gold, 0);
        assert!(summary.members.is_empty());
        assert!(summary.extra.is_empty());
    }

    // ── GameState roundtrip ──

    #[test]
    fn test_game_state_roundtrip() {
        let mut extensions = HashMap::new();
        extensions.insert("switches".into(), json!({"1": true, "2": false}));
        extensions.insert("gold".into(), json!(9999));

        let state = GameState {
            engine: "rpg_mv".into(),
            map_name: "城镇".into(),
            play_time: "01:23:45".into(),
            save_count: 7,
            extensions,
        };

        let json = serde_json::to_string(&state).expect("序列化失败");
        let decoded: GameState = serde_json::from_str(&json).expect("反序列化失败");

        assert_eq!(decoded.engine, "rpg_mv");
        assert_eq!(decoded.map_name, "城镇");
        assert_eq!(decoded.save_count, 7);
        assert_eq!(decoded.extensions.get("gold").and_then(|v| v.as_i64()), Some(9999));
        assert_eq!(
            decoded.extensions.get("switches")
                .and_then(|v| v.get("1"))
                .and_then(|v| v.as_bool()),
            Some(true)
        );
    }

    #[test]
    fn test_game_state_defaults() {
        let json = "{}";
        let state: GameState = serde_json::from_str(json).expect("反序列化失败");
        assert_eq!(state.engine, "unknown");
        assert_eq!(state.map_name, "");
        assert!(state.extensions.is_empty());
    }

    #[test]
    fn test_game_state_extensions_roundtrip() {
        let mut extensions = HashMap::new();
        extensions.insert("raw".into(), json!({"extra": "data"}));

        let state = GameState {
            engine: "generic".into(),
            map_name: "".into(),
            play_time: "".into(),
            save_count: 0,
            extensions,
        };

        let json = serde_json::to_string(&state).expect("序列化失败");
        let decoded: GameState = serde_json::from_str(&json).expect("反序列化失败");

        assert_eq!(decoded.engine, "generic");
        assert!(decoded.extensions.contains_key("raw"));
    }

    // ── BridgeCommand serialization ──

    #[test]
    fn test_bridge_command_serialization() {
        let cmd = BridgeCommand::ReadField("gold".into());
        let json = serde_json::to_string(&cmd).expect("序列化失败");
        assert!(json.contains("ReadField"));
        assert!(json.contains("gold"));

        let cmd = BridgeCommand::WriteField("switch_12".into(), json!(true));
        let json = serde_json::to_string(&cmd).expect("序列化失败");
        assert!(json.contains("WriteField"));
        assert!(json.contains("switch_12"));

        let cmd = BridgeCommand::ReadAll;
        let json = serde_json::to_string(&cmd).expect("序列化失败");
        assert!(json.contains("ReadAll"));
    }

    #[test]
    fn test_bridge_command_deserialization() {
        let json = r#"{"ReadField":"gold"}"#;
        let cmd: BridgeCommand = serde_json::from_str(json).expect("反序列化失败");
        match cmd {
            BridgeCommand::ReadField(id) => assert_eq!(id, "gold"),
            _ => panic!("expected ReadField"),
        }

        let json = r#"{"WriteField":["switch_1",true]}"#;
        let cmd: BridgeCommand = serde_json::from_str(json).expect("反序列化失败");
        match cmd {
            BridgeCommand::WriteField(id, val) => {
                assert_eq!(id, "switch_1");
                assert_eq!(val, json!(true));
            }
            _ => panic!("expected WriteField"),
        }

        let json = r#""ReadAll""#;
        let cmd: BridgeCommand = serde_json::from_str(json).expect("反序列化失败");
        match cmd {
            BridgeCommand::ReadAll => {}
            _ => panic!("expected ReadAll"),
        }
    }

    // ── GameInfo roundtrip ──

    #[test]
    fn test_game_info_roundtrip() {
        let info = GameInfo {
            game_dir: r"D:\Games\MyGame".into(),
            game_title: "我的游戏".into(),
            engine: "rpg_mv".into(),
            is_nwjs: true,
            data_dir: r"D:\Games\MyGame\www\data".into(),
            save_dir: r"D:\Games\MyGame\www\Save".into(),
            www_dir: r"D:\Games\MyGame\www".into(),
            exe_path: r"D:\Games\MyGame\Game.exe".into(),
            exe_name: "Game.exe".into(),
            package_json_path: r"D:\Games\MyGame\package.json".into(),
            save_files: vec![
                r"D:\Games\MyGame\www\Save\file1.rpgsave".into(),
            ],
            save_format: "rpgsave".into(),
            detected_from: "dir".into(),
        };

        let json = serde_json::to_string(&info).expect("序列化失败");
        let decoded: GameInfo = serde_json::from_str(&json).expect("反序列化失败");

        assert_eq!(decoded.game_title, "我的游戏");
        assert_eq!(decoded.engine, "rpg_mv");
        assert!(decoded.is_nwjs);
        assert_eq!(decoded.save_files.len(), 1);
    }

    #[test]
    fn test_game_info_defaults() {
        let json = "{}";
        let info: GameInfo = serde_json::from_str(json).expect("反序列化失败");
        assert_eq!(info.engine, "unknown");
        assert!(!info.is_nwjs);
        assert_eq!(info.exe_name, "Game.exe");
        assert_eq!(info.save_format, "rpgsave");
        assert!(info.save_files.is_empty());
    }

    // ── HashMap< i32, _ > 的 JSON 序列化行为 ──

    #[test]
    fn test_i32_keyed_map_serialization() {
        let mut switches = HashMap::new();
        switches.insert(1, true);
        switches.insert(100, false);

        let json = serde_json::to_string(&switches).expect("序列化失败");
        // JSON key 必须是字符串，serde 会将 i32 key 转为字符串
        assert!(json.contains("\"1\""));
        assert!(json.contains("\"100\""));

        let decoded: HashMap<i32, bool> = serde_json::from_str(&json).expect("反序列化失败");
        assert_eq!(decoded.get(&1), Some(&true));
        assert_eq!(decoded.get(&100), Some(&false));
    }
}
