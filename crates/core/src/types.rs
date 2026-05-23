//! 核心类型定义 —— 数据模型与 Trait 接口
//!
//! 提供格式无关的存档数据模型和桥接器抽象接口。
//! UI 层仅通过此模块的类型操作存档和连接游戏，不直接依赖具体格式实现。

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

use crate::error::GameToolError;

// ═══════════════════════════════════════════════════════════════
// 数据模型
// ═══════════════════════════════════════════════════════════════

/// 可修改的存档字段（格式无关）
///
/// 对应 Python 版 `ModifiableField`，是编辑器 UI 与格式处理器之间的
/// 统一字段表示。UI 层在表格中展示字段列表，用户修改 `save_value` 后，
/// 调用 `apply_field` 将修改写回存档数据。
///
/// # 字段分类示例
/// - **gold**: 金币/货币类
/// - **switch**: RPG Maker 开关（布尔值）
/// - **variable**: RPG Maker 变量（整数值）
/// - **actor**: 角色属性（HP、MP、等级等）
/// - **item/weapon/armor**: 道具/武器/防具数量
/// - **self_switch**: RPG Maker 独立开关
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModifiableField {
    /// 字段分类，如 "gold", "switch", "variable", "actor", "item", "weapon", "armor", "self_switch"
    pub category: String,
    /// 字段唯一标识符，如 "switch_12", "var_5", "actor_1_hp"
    pub field_id: String,
    /// 字段在 UI 中展示的名称
    pub display_name: String,
    /// 游戏内数据项 ID（如物品 ID、开关编号）
    #[serde(default)]
    pub item_id: i32,
    /// 字段值类型："bool", "int", "str"
    #[serde(default = "default_field_type")]
    pub field_type: String,
    /// 存档文件中存储的当前值
    #[serde(default)]
    pub save_value: Value,
    /// 从游戏进程实时读取的当前值
    #[serde(default)]
    pub live_value: Value,
    /// 字段的默认值（重置时使用）
    #[serde(default)]
    pub default_value: Value,
    /// 字段允许的最小值（用于数值类字段）
    #[serde(default)]
    pub min_val: i32,
    /// 字段允许的最大值（用于数值类字段）
    #[serde(default = "default_max_val")]
    pub max_val: i32,
    /// 字段描述文本
    #[serde(default)]
    pub description: String,
    /// 标记用户是否在 UI 中编辑过此字段
    #[serde(default)]
    pub dirty: bool,
    /// RPG Maker 专用：关联的金币变量 ID
    /// 某些 RPG Maker 游戏使用变量而非内置 gold 字段存储金币
    #[serde(default)]
    pub gold_var_id: i32,
}

/// `field_type` 的默认值：字符串 "int"
fn default_field_type() -> String {
    "int".to_string()
}

/// `max_val` 的默认值：99,999,999
fn default_max_val() -> i32 {
    99_999_999
}

/// 存档摘要信息（格式无关）
///
/// 对应 Python 版 `SaveSummary`，用于 UI 概览面板展示
/// 存档文件的简要统计信息。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SaveSummary {
    /// 金币数量
    #[serde(default)]
    pub gold: i32,
    /// 当前队伍角色数
    #[serde(default)]
    pub party_size: i32,
    /// 道具总数
    #[serde(default)]
    pub item_count: i32,
    /// 存档次数
    #[serde(default)]
    pub save_count: i32,
    /// 游戏时间（秒）
    #[serde(default)]
    pub play_time: i32,
    /// 队伍成员名称列表
    #[serde(default)]
    pub members: Vec<String>,
    /// 格式特有的额外信息（引擎特定字段）
    #[serde(default)]
    pub extra: HashMap<String, Value>,
}

/// 统一的游戏状态快照
///
/// 通用字段直接放在结构体上，引擎特定数据存入 `extensions` 映射。
/// 不同引擎的扩展数据格式如下：
///
/// - **RPG Maker**: extensions["switches"], extensions["variables"],
///   extensions["self_switches"], extensions["party"], extensions["items"]
/// - **Ren'Py**: extensions["store"]（所有 Python store 变量）
/// - **Unreal**: extensions["properties"]（内存读取的属性快照）
/// - **Generic**: extensions["memory_values"]（通用内存值快照）
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GameState {
    /// 引擎类型标识符，如 "rpg_mv", "renpy", "unreal"
    #[serde(default = "default_engine")]
    pub engine: String,
    /// 当前地图/场景名称
    #[serde(default)]
    pub map_name: String,
    /// 游戏时间文本（格式化的时间字符串）
    #[serde(default)]
    pub play_time: String,
    /// 存档次数
    #[serde(default)]
    pub save_count: i32,
    /// 引擎特定扩展数据映射
    #[serde(default)]
    pub extensions: HashMap<String, Value>,
}

/// `engine` 字段的默认值："unknown"
fn default_engine() -> String {
    "unknown".to_string()
}

/// 检测到的游戏信息
///
/// 对应 Python 版 `GameInfo`，描述一次引擎检测扫描得到的
/// 游戏目录结构、引擎类型及相关路径信息。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameInfo {
    /// 游戏根目录的绝对路径
    #[serde(default)]
    pub game_dir: String,
    /// 游戏标题（从 package.json 或目录名推断）
    #[serde(default)]
    pub game_title: String,
    /// 引擎标识：如 "rpg_mv", "rpg_mz", "nwjs", "renpy"
    #[serde(default = "default_engine")]
    pub engine: String,
    /// 是否为 NW.js 封装运行的游戏
    #[serde(default)]
    pub is_nwjs: bool,
    /// 游戏数据目录（含 JSON 配置文件，如 System.json）
    #[serde(default)]
    pub data_dir: String,
    /// 存档文件目录
    #[serde(default)]
    pub save_dir: String,
    /// www 目录（NW.js 游戏的数据根目录）
    #[serde(default)]
    pub www_dir: String,
    /// 游戏可执行文件的完整路径
    #[serde(default)]
    pub exe_path: String,
    /// 可执行文件名（如 "Game.exe", "nw.exe"）
    #[serde(default = "default_exe_name")]
    pub exe_name: String,
    /// package.json 文件的绝对路径
    #[serde(default)]
    pub package_json_path: String,
    /// 检测到的存档文件路径列表
    #[serde(default)]
    pub save_files: Vec<String>,
    /// 存档格式标识："rpgsave"（MV）或 "rmmzsave"（MZ）
    #[serde(default = "default_save_format")]
    pub save_format: String,
    /// 检测来源途径："save"（通过存档路径）, "process"（通过进程）, "dir"（通过目录扫描）
    #[serde(default)]
    pub detected_from: String,
}

/// `exe_name` 的默认值："Game.exe"
fn default_exe_name() -> String {
    "Game.exe".to_string()
}

/// `save_format` 的默认值："rpgsave"
fn default_save_format() -> String {
    "rpgsave".to_string()
}

/// 桥接命令枚举 —— 引擎无关的读写指令
///
/// 所有 `GameBridge` 实现通过此枚举进行统一调度，
/// 替代原先每种操作一个方法（如 set_gold、set_switch）的设计。
/// 新增引擎只需实现命令 → 协议消息的翻译逻辑即可。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BridgeCommand {
    /// 读取指定字段的当前值
    /// field_id 示例: "gold", "switch_12", "actor_1_hp"
    ReadField(String),
    /// 向指定字段写入新值
    WriteField(String, Value),
    /// 读取完整游戏状态快照
    ReadAll,
}

/// 内存桥特殊操作命令（用于桥接线程向 UniversalMemoryBridge 下发扫描/跟踪操作）
#[derive(Debug, Clone)]
pub enum MemoryCommand {
    /// 附加到指定 PID 的游戏进程，获取进程句柄用于内存读写
    Attach(u32),
    /// 断开与游戏进程的连接，释放内存句柄及相关资源
    Detach,
    /// 首次内存扫描：根据指定值和值类型搜索所有匹配的地址
    FirstScan { value: Value, value_type_id: u32 },
    /// 后续扫描：基于上次扫描结果缩小范围，支持多种扫描模式（精确、模糊、增大、减小等）
    NextScan { scan_mode_id: u32, value: Option<Value> },
    /// 从存档字段生成内存扫描种子：将存档值作为线索，在内存中定位对应地址
    SeedFromSave(Vec<ModifiableField>),
    /// 交叉验证：将内存候选地址与存档数据进行双向比对，确认地址准确性
    CrossValidate { seeds_data: Vec<u8>, new_fields: Vec<ModifiableField> },
    /// 添加内存地址监视：持续跟踪指定地址的值变化并实时推送到 UI
    AddWatch { field_id: String, address: usize, value_type_id: u32 },
    /// 移除指定 field_id 的内存地址监视
    RemoveWatch(String),
}

/// 内存扫描候选地址（格式无关）
///
/// 表示一次内存扫描操作找到的一个候选内存地址及其当前值。
/// 经过多轮筛选后，候选地址将被确认为目标字段的真实内存位置。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScannedAddr {
    /// 候选内存地址（进程虚拟地址空间中的绝对地址）
    pub address: usize,
    /// 该地址当前存储的值（实时从进程内存中读取）
    pub current_value: Value,
}

/// 存档辅助扫描种子（格式无关）
///
/// 从存档字段生成的扫描种子，用于在游戏进程内存中定位
/// 该字段对应的内存地址。每个种子对应一个 `ModifiableField`，
/// 记录其存档值以及已发现的候选/确认地址。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldScanSeed {
    /// 对应的字段唯一标识符（与 ModifiableField.field_id 一致）
    pub field_id: String,
    /// 字段在 UI 中展示的名称
    pub display_name: String,
    /// 该字段在存档文件中的值（作为内存搜索的参考值）
    pub save_value: Value,
    /// 初次扫描/后续扫描找到的候选内存地址列表
    pub candidates: Vec<usize>,
    /// 经过交叉验证确认的精确内存地址列表
    pub confirmed_addrs: Vec<usize>,
    /// 当前确认结果的置信度（0.0 ~ 1.0），基于验证匹配率计算
    pub confidence: f64,
}

// ═══════════════════════════════════════════════════════════════
// Trait 接口
// ═══════════════════════════════════════════════════════════════

/// 游戏存档格式处理器接口
///
/// 所有具体格式（RPG Maker MV/MZ、Ren'Py、Unreal 等）实现此接口。
/// UI 层通过此 trait 对象操作存档，不关心底层格式细节。
///
/// 对应 Python 版 `ISaveFormat`。
///
/// # 使用示例
/// ```ignore
/// let format: Box<dyn ISaveFormat> = RpgMvSaveFormat::new();
/// let data = format.load("save.rpgsave")?;
/// let summary = format.get_summary(&data);
/// ```
pub trait ISaveFormat: Send + Sync {
    /// 返回格式的显示名称，如 "RPG Maker MV/MZ"
    fn name(&self) -> &str;

    /// 返回支持的文件扩展名列表，如 `[".rpgsave", ".rmmzsave"]`
    fn extensions(&self) -> Vec<String>;

    /// 返回关联的引擎类型标识符，如 "rpg_maker_mv", "rpg_maker_mz", "renpy"
    fn engine_type(&self) -> &str;

    /// 返回文件的魔数字节签名，用于格式识别
    /// 无签名的格式返回 `None`
    fn magic_bytes(&self) -> Option<&[u8]>;

    // ── 核心 I/O 操作 ──

    /// 从指定路径加载存档文件，返回解析后的 JSON 数据
    fn load(&self, filepath: &str) -> Result<Value, GameToolError>;

    /// 将修改后的数据写回存档文件
    fn save(&self, filepath: &str, data: &Value) -> Result<(), GameToolError>;

    // ── 检测与发现 ──

    /// 检测指定文件是否属于此格式
    ///
    /// 默认实现按以下优先级检测：
    /// 1. 魔数字节签名匹配（精确识别）
    /// 2. 文件扩展名匹配（容错后备）
    fn detect(&self, filepath: &str) -> bool {
        // 第一步：魔数检测 —— 读取文件头部字节与签名比对
        if let Some(magic) = self.magic_bytes() {
            if let Ok(mut f) = std::fs::File::open(filepath) {
                use std::io::Read;
                let mut buf = vec![0u8; magic.len()];
                if f.read_exact(&mut buf).is_ok() && buf.as_slice() == magic {
                    return true;
                }
            }
        }
        // 第二步：扩展名检测 —— 提取文件扩展名与支持列表比对
        let ext = std::path::Path::new(filepath)
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| format!(".{}", e.to_lowercase()))
            .unwrap_or_default();
        self.extensions().contains(&ext)
    }

    /// 在游戏根目录中查找数据文件目录
    ///
    /// - RPG Maker: 返回 `www/data/` 路径
    /// - Ren'Py: 返回 `game/` 路径
    /// - 其他引擎: 返回引擎特定的数据目录
    fn find_data_dir(&self, game_dir: &str) -> Option<String>;

    // ── 摘要与字段操作 ──

    /// 从存档数据中提取摘要信息（金币、队伍、道具统计等）
    fn get_summary(&self, data: &Value) -> SaveSummary;

    /// 扫描存档中的所有可修改字段
    /// `game_dir` 用于加载游戏数据定义（如 RPG Maker 的物品/武器名称）
    fn scan_fields(&self, data: &Value, game_dir: &str) -> Vec<ModifiableField>;

    /// 将单个字段的修改值应用到存档数据中
    /// 修改后的数据通过 `save()` 方法写回文件
    fn apply_field(&self, data: &mut Value, field: &ModifiableField) -> Result<(), GameToolError>;
}

/// 游戏桥接器接口（命令驱动）
///
/// 所有实时连接后端（TCP、CDP 协议、Windows 进程内存）必须实现此接口。
/// 使用 `BridgeCommand` 枚举替代引擎特定的方法集合，实现引擎无关的
/// 读写调度。
///
/// 对应 Python 版 `IGameBridge`（重新设计）。
pub trait GameBridge: Send + Sync {
    /// 建立与游戏进程的连接
    /// 各实现负责各自的连接逻辑（TCP 握手、CDP 附加、内存映射等）
    fn connect(&mut self) -> Result<(), crate::error::GameToolError>;

    /// 断开与游戏进程的连接并释放资源
    fn disconnect(&mut self);

    /// 检查当前是否处于已连接状态
    fn is_connected(&self) -> bool;

    /// 执行桥接命令并返回结果
    ///
    /// 各引擎实现自行将 `BridgeCommand` 翻译为对应的协议指令：
    /// - **RPG Maker TCP**: 发送 `"set_gold 9999"` 文本行
    /// - **Ren'Py**: 发送 `{"action": "set_var", "name": "gold", "value": 9999}` JSON
    /// - **Windows 内存**: 调用 `WriteProcessMemory(addr, value)` 写入进程内存
    fn execute(&mut self, cmd: &BridgeCommand) -> Result<Value, crate::error::GameToolError>;

    /// 返回引擎名称标识符（如 "rpg_maker", "renpy", "unreal", "generic"）
    fn engine_name(&self) -> &str;

    /// 桥接器优先级，数值越小越优先尝试连接
    /// 当同时检测到多个引擎时，按优先级排序依次尝试
    fn priority(&self) -> i32;

    /// 处理内存桥特殊命令（扫描/跟踪等）
    /// 非内存桥实现返回 Err
    fn handle_memory_command(
        &mut self,
        _cmd: &MemoryCommand,
    ) -> Result<Value, crate::error::GameToolError> {
        Err(crate::error::GameToolError::BridgeCommandError(
            "不支持内存操作".into(),
        ))
    }
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
            gold_var_id: 0,
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
        assert_eq!(
            decoded.extra.get("actor_count").and_then(|v| v.as_i64()),
            Some(4)
        );
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
        assert_eq!(
            decoded.extensions.get("gold").and_then(|v| v.as_i64()),
            Some(9999)
        );
        assert_eq!(
            decoded
                .extensions
                .get("switches")
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
            save_files: vec![r"D:\Games\MyGame\www\Save\file1.rpgsave".into()],
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
