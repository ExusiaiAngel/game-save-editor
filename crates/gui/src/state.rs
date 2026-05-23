//! GUI 状态定义：所有 UI 枚举类型、数据结构与全局应用状态。
//!
//! 本模块定义了：
//! - 导航与面板相关的枚举（TabMode, SavePanelMode, ConnectionStatus）
//! - 桥接线程通信相关的枚举与结构体（BridgeJob, BridgeResult, RealtimeConnection）
//! - 各面板的独立状态结构体（SavePanelState, RtPanelState, ToolboxState）
//! - 全局聚合状态 AppState（含对话框状态）
//!
//! 约定：所有状态结构体仅存储数据，不包含业务逻辑。业务逻辑在 app.rs 中实现。

use game_tool_core::detector::EngineType;
use game_tool_core::{BridgeCommand, ISaveFormat, MemoryCommand, ModifiableField, SaveSummary};
use game_tool_memory::ValueType;
use game_tool_rpgmaker::scanner::GameConfig;
use serde_json::Value;
use std::collections::HashSet;
use std::sync::mpsc::{Receiver, Sender};

/// GUI 标签页模式：决定主区域显示哪个面板
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum TabMode {
    SaveEditor,      // 存档编辑面板
    RealtimeEditor,  // 实时修改面板（需游戏运行中）
    BackupManager,   // 备份管理面板
    Toolbox,         // 工具箱（LZ/Base64等小工具）
    Settings,        // 设置面板
}

/// 存档面板模式：根据游戏引擎决定存档编辑器的行为
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum SavePanelMode {
    RpgMaker,  // RPG Maker 系列（MV/MZ/NW.js）
    RenPy,     // Ren'Py 引擎
    Unreal,    // Unreal 引擎（GVA存档结构）
    Generic,   // 通用 JSON 格式
}

/// 实时连接的当前状态
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ConnectionStatus {
    Disconnected,  // 未连接
    Connecting,    // 正在连接中
    Connected,     // 已连接
}

/// 确认对话框执行的动作类型
pub enum ConfirmAction {
    DeleteBackups(Vec<usize>),    // 批量删除备份（传入索引列表）
    RestoreBackup(usize),         // 恢复指定备份到当前存档
    ClearRecentGames,             // 清空最近游戏记录
    DeleteSingleBackup(usize),    // 删除单个备份
}

/// 确认对话框的数据：标题、提示信息与对应的操作
pub struct ConfirmDialog {
    pub title: String,           // 对话框标题
    pub message: String,         // 提示信息
    pub on_confirm: ConfirmAction, // 确认后执行的动作
}

/// 桥接模式
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum BridgeMode {
    Tcp,   // TCP 桥接（RPG Maker / Ren'Py）
    Memory, // 内存桥接（Unreal / Unity / Godot）
}

/// 发送给桥接线程的任务指令
pub enum BridgeJob {
    Connect,                // 建立连接
    Disconnect,             // 断开连接
    Execute(BridgeCommand), // 执行具体命令（如读取/写入字段）
    /// 执行内存桥特殊命令（扫描/跟踪等）
    MemoryCommand(MemoryCommand),
}

/// 桥接线程返回的结果
pub enum BridgeResult {
    Connected,                  // 连接成功
    Disconnected,               // 已断开
    CommandResult(Value),       // 命令执行结果（JSON值）
    Error(String),              // 错误信息
    // 内存模式专用结果
    Attached,                   // 进程已附加
    ScanResult(Value),          // 扫描结果（候选人 JSON 数组）
    SeedResult(Value),          // 存档种子结果
    // 内存模式：桥接模式
}

/// 实时连接的通道集合：命令发送端与结果接收端
pub struct RealtimeConnection {
    pub cmd_tx: Sender<BridgeJob>,       // 向桥接线程发送命令
    pub result_rx: Receiver<BridgeResult>, // 从桥接线程接收结果
    pub status: ConnectionStatus,        // 当前连接状态
}

/// 工具箱面板的用户操作指令
pub enum ToolboxAction {
    /// 获取存档文件基本信息
    GetSaveInfo(String),
    /// 执行存档完整性检查
    IntegrityCheck(String),
    /// 对目录执行批量完整性检查
    BatchCheck(String),
    /// 尝试修复损坏的存档文件
    RepairSave(String),
    /// 清除完整性检查结果
    ClearCheck,
    /// 清除批量检查结果
    ClearBatch,
    /// 清除修复结果
    ClearRepair,
}

/// 工具箱面板的状态数据
pub struct ToolboxState {
    pub lz_input: String,    // LZ 压缩/解压输入文本
    pub lz_output: String,   // LZ 压缩/解压输出结果
    pub lz_error: String,    // LZ 操作错误信息
    pub b64_input: String,   // Base64 编解码输入文本
    pub b64_output: String,  // Base64 编解码输出结果
    // --- 存档信息查看器 ---
    pub info_path: String,
    pub info_result: Option<game_tool_core::integrity::SaveFileInfo>,
    // --- 完整性检查 ---
    pub check_path: String,
    pub check_result: Option<game_tool_core::integrity::IntegrityResult>,
    // --- 批量检查 ---
    pub batch_dir: String,
    pub batch_results: Vec<game_tool_core::integrity::IntegrityResult>,
    // --- 存档修复 ---
    pub repair_path: String,
    pub repair_result: Option<game_tool_core::integrity::RepairResult>,
}

/// 存档编辑面板的完整状态
pub struct SavePanelState {
    pub format: Option<Box<dyn ISaveFormat>>, // 当前引擎的存档格式处理器
    pub save_files: Vec<String>,              // 扫描到的存档文件路径列表
    pub selected_save: Option<String>,        // 当前选中的存档文件路径
    pub save_data: Option<Value>,             // 已加载的存档原始数据（JSON）
    pub summary: Option<SaveSummary>,         // 存档摘要信息（存档时间、进度等）
    pub fields: Vec<ModifiableField>,         // 可修改的字段列表
    pub dirty_count: usize,                   // 已修改但未保存的字段数量
    pub selected_category: Option<String>,    // 当前筛选的字段类别
    pub search_query: String,                 // 搜索关键词
    pub panel_mode: SavePanelMode,            // 面板模式
    pub readonly: bool,                       // 是否只读（如Unreal引擎）
    pub jump_id: String,                      // 跳转到指定ID的字段
}

/// 实时修改面板的完整状态
pub struct RtPanelState {
    pub conn: Option<RealtimeConnection>,          // 实时连接通道
    pub fields: Vec<ModifiableField>,              // 从游戏读取的实时字段列表
    pub plugin_installed: bool,                    // 插件是否已注入到游戏目录
    pub host: String,                              // 连接目标主机地址
    pub port: u16,                                 // 连接目标端口
    pub error_message: String,                     // 当前错误信息（带自动消失）
    pub error_expires_at: Option<std::time::Instant>, // 错误信息过期时间
    pub write_feedback: String,                    // 写入操作反馈信息
    pub write_feedback_expires_at: Option<std::time::Instant>, // 写入反馈过期时间
    pub search_query: String,                      // 搜索关键词
    pub selected_category: Option<String>,         // 当前筛选的字段类别
    pub jump_id: String,                           // 跳转到指定ID的字段
    pub auto_refresh: bool,                        // 是否启用自动刷新
    pub locked_fields: HashSet<String>,            // 锁定字段集合（自动刷新时保留当前值）
    pub refresh_interval_secs: u64,                // 自动刷新间隔（秒）
    pub last_refresh: Option<std::time::Instant>,  // 上次刷新时间
    // 内存模式特有字段
    pub bridge_mode: BridgeMode,                   // 桥接模式
    pub process_list: Vec<game_tool_memory::ProcessInfo>, // 枚举到的进程列表
    pub selected_process: Option<String>,          // 选中的进程名
    pub scan_value: String,                        // 扫描输入值
    pub scan_value_type: ValueType,                // 扫描值类型
    pub scan_results: Vec<game_tool_core::ScannedAddr>, // 扫描候选地址
    pub scan_count: usize,                         // 候选数量
    pub next_scan_mode: u32,                       // 下次扫描模式 0=exact/1=inc/2=dec/3=unchanged/4=changed
    pub scan_in_progress: bool,                    // 扫描中
    pub field_seeds: Vec<game_tool_core::FieldScanSeed>, // 存档辅助种子
    pub save_fields_snapshot: Vec<ModifiableField>, // 存档字段快照（供种子使用）
}

/// 应用程序全局状态：所有面板、配置与UI状态的聚合根
pub struct AppState {
    pub game_dir: Option<String>,             // 当前打开的游戏目录路径
    pub game_title: String,                   // 游戏标题（从配置扫描获得）
    pub engine: EngineType,                   // 检测到的游戏引擎类型
    pub game_config: Option<GameConfig>,      // 游戏配置数据（开关名、变量名等）
    pub active_tab: TabMode,                  // 当前激活的标签页
    pub dark_mode: bool,                      // 是否使用暗色主题
    pub recent_games: Vec<String>,            // 最近打开的游戏目录列表
    pub backup_paths: Vec<String>,            // 当前存档的备份文件路径列表
    pub backup_selection: HashSet<usize>,     // 备份管理器中选中的备份索引
    pub save_panel: SavePanelState,           // 存档编辑面板状态
    pub rt_panel: RtPanelState,               // 实时修改面板状态
    pub toolbox: ToolboxState,                // 工具箱面板状态
    pub status_message: String,               // 状态栏提示信息
    pub show_unsaved_dialog: bool,            // 是否显示"未保存修改"对话框
    pub show_confirm_dialog: Option<ConfirmDialog>, // 当前显示的确认对话框
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_save_panel_mode_variants_distinct() {
        assert_ne!(SavePanelMode::RpgMaker, SavePanelMode::RenPy);
        assert_ne!(SavePanelMode::RenPy, SavePanelMode::Unreal);
        assert_ne!(SavePanelMode::Unreal, SavePanelMode::Generic);
        assert_ne!(SavePanelMode::Generic, SavePanelMode::RpgMaker);
    }

    #[test]
    fn test_save_panel_mode_clone() {
        let mode = SavePanelMode::RpgMaker;
        let cloned = mode;
        assert_eq!(mode, cloned);
    }

    #[test]
    fn test_connection_status_variants_distinct() {
        assert_ne!(ConnectionStatus::Disconnected, ConnectionStatus::Connecting);
        assert_ne!(ConnectionStatus::Connecting, ConnectionStatus::Connected);
        assert_ne!(ConnectionStatus::Connected, ConnectionStatus::Disconnected);
    }

    #[test]
    fn test_connection_status_clone() {
        let status = ConnectionStatus::Connecting;
        let cloned = status;
        assert_eq!(status, cloned);
    }

    #[test]
    fn test_bridge_job_constructible() {
        let _connect = BridgeJob::Connect;
        let _disconnect = BridgeJob::Disconnect;
        let _exec = BridgeJob::Execute(game_tool_core::BridgeCommand::ReadAll);
    }

    #[test]
    fn test_bridge_result_constructible() {
        let _conn = BridgeResult::Connected;
        let _disc = BridgeResult::Disconnected;
        let _res = BridgeResult::CommandResult(serde_json::Value::Number(1.into()));
        let _err = BridgeResult::Error("test error".into());
    }

    #[test]
    fn test_tab_mode_variants_distinct() {
        assert_ne!(TabMode::SaveEditor, TabMode::RealtimeEditor);
        assert_ne!(TabMode::RealtimeEditor, TabMode::BackupManager);
        assert_ne!(TabMode::BackupManager, TabMode::Toolbox);
        assert_ne!(TabMode::Toolbox, TabMode::Settings);
        assert_ne!(TabMode::Settings, TabMode::SaveEditor);
    }

    #[test]
    fn test_tab_mode_clone() {
        let v = TabMode::SaveEditor;
        assert_eq!(v, v);
    }

    #[test]
    fn test_confirm_action_constructible() {
        let _delete = ConfirmAction::DeleteBackups(vec![0]);
        let _restore = ConfirmAction::RestoreBackup(0);
        let _clear = ConfirmAction::ClearRecentGames;
        let _single = ConfirmAction::DeleteSingleBackup(0);
    }
}
