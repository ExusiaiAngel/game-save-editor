use game_tool_core::detector::EngineType;
use game_tool_core::{BridgeCommand, ISaveFormat, ModifiableField, SaveSummary};
use game_tool_rpgmaker::scanner::GameConfig;
use serde_json::Value;
use std::collections::HashSet;
use std::sync::mpsc::{Receiver, Sender};

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum TabMode {
    SaveEditor,
    RealtimeEditor,
    BackupManager,
    Toolbox,
    Settings,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum SavePanelMode {
    RpgMaker,
    RenPy,
    Unreal,
    Generic,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ConnectionStatus {
    Disconnected,
    Connecting,
    Connected,
}

pub enum ConfirmAction {
    DiscardAndSwitch,
    DeleteBackups(Vec<usize>),
    RestoreBackup(usize),
    ClearRecentGames,
    DeleteSingleBackup(usize),
}

pub struct ConfirmDialog {
    pub title: String,
    pub message: String,
    pub on_confirm: ConfirmAction,
}

pub enum BridgeJob {
    Connect,
    Disconnect,
    Execute(BridgeCommand),
}

pub enum BridgeResult {
    Connected,
    Disconnected,
    CommandResult(Value),
    Error(String),
}

pub struct RealtimeConnection {
    pub cmd_tx: Sender<BridgeJob>,
    pub result_rx: Receiver<BridgeResult>,
    pub status: ConnectionStatus,
}

pub struct SavePanelState {
    pub format: Option<Box<dyn ISaveFormat>>,
    pub save_files: Vec<String>,
    pub selected_save: Option<String>,
    pub save_data: Option<Value>,
    pub summary: Option<SaveSummary>,
    pub fields: Vec<ModifiableField>,
    pub dirty_count: usize,
    pub selected_category: Option<String>,
    pub search_query: String,
    pub panel_mode: SavePanelMode,
    pub readonly: bool,
    pub jump_id: String,
}

pub struct RtPanelState {
    pub conn: Option<RealtimeConnection>,
    pub fields: Vec<ModifiableField>,
    pub plugin_installed: bool,
    pub host: String,
    pub port: u16,
    pub error_message: String,
    pub error_remaining: u32,
    pub write_feedback: String,
    pub write_feedback_remaining: u32,
    pub search_query: String,
    pub jump_id: String,
    pub auto_refresh: bool,
    pub refresh_timer: u32,
    pub locked_fields: HashSet<String>,
    pub refresh_interval_secs: u64,
    pub last_refresh: Option<std::time::Instant>,
}

pub struct AppState {
    pub game_dir: Option<String>,
    pub game_title: String,
    pub engine: EngineType,
    pub game_config: Option<GameConfig>,
    pub active_tab: TabMode,
    pub dark_mode: bool,
    pub recent_games: Vec<String>,
    pub backup_paths: Vec<String>,
    pub save_panel: SavePanelState,
    pub rt_panel: RtPanelState,
    pub status_message: String,
    pub show_unsaved_dialog: bool,
    pub show_confirm_dialog: Option<ConfirmDialog>,
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
        let _discard = ConfirmAction::DiscardAndSwitch;
        let _delete = ConfirmAction::DeleteBackups(vec![0]);
        let _restore = ConfirmAction::RestoreBackup(0);
        let _clear = ConfirmAction::ClearRecentGames;
        let _single = ConfirmAction::DeleteSingleBackup(0);
    }
}
