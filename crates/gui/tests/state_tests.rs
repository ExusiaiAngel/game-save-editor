//! 状态初始化与枚举变体测试。
//!
//! 验证 `AppState::new(None)` 的默认值是否符合预期，以及各个枚举类型的
//! 变体之间是否互不相同、Clone 行为是否正确。

mod common;

use game_tool_core::detector::EngineType;
use game_tool_gui::state::{AppState, ConnectionStatus, SavePanelMode};

/// 验证 `AppState::new(None)` 的默认初始值：游戏目录、引擎、配置、字段列表等
/// 所有字段均为空/默认状态。
#[test]
fn test_appstate_new_none_defaults() {
    let state = AppState::new(None);
    assert!(state.game_dir.is_none());
    assert_eq!(state.engine, EngineType::Unknown);
    assert!(state.game_config.is_none());
    assert_eq!(state.game_title, "");
    assert!(!state.show_unsaved_dialog);
    assert_eq!(state.status_message, "");
    assert!(state.save_panel.format.is_none());
    assert!(state.save_panel.save_files.is_empty());
    assert!(state.save_panel.selected_save.is_none());
    assert!(state.save_panel.save_data.is_none());
    assert!(state.save_panel.summary.is_none());
    assert!(state.save_panel.fields.is_empty());
    assert_eq!(state.save_panel.dirty_count, 0);
    assert_eq!(state.save_panel.selected_category, None);
    assert_eq!(state.save_panel.search_query, "");
    assert_eq!(state.save_panel.jump_id, "");
    assert!(!state.save_panel.readonly);
    assert!(state.rt_panel.conn.is_none());
    assert!(state.rt_panel.fields.is_empty());
    assert!(!state.rt_panel.plugin_installed);
    assert_eq!(state.rt_panel.host, "127.0.0.1");
    assert!(state.rt_panel.port > 0);
    assert_eq!(state.rt_panel.error_message, "");
    assert_eq!(state.rt_panel.write_feedback, "");
    assert_eq!(state.rt_panel.search_query, "");
    assert_eq!(state.rt_panel.jump_id, "");
    assert!(state.rt_panel.auto_refresh);
    assert_eq!(state.rt_panel.refresh_interval_secs, 3);
    assert!(state.rt_panel.locked_fields.is_empty());
}

/// 验证连接状态枚举的三种变体互不相同
#[test]
fn test_connection_status_variants_distinct() {
    assert_ne!(ConnectionStatus::Disconnected, ConnectionStatus::Connecting);
    assert_ne!(ConnectionStatus::Connecting, ConnectionStatus::Connected);
    assert_ne!(ConnectionStatus::Connected, ConnectionStatus::Disconnected);
}

/// 验证存档面板模式枚举的四种变体互不相同
#[test]
fn test_save_panel_mode_variants_distinct() {
    assert_ne!(SavePanelMode::RpgMaker, SavePanelMode::RenPy);
    assert_ne!(SavePanelMode::RenPy, SavePanelMode::Unreal);
    assert_ne!(SavePanelMode::Unreal, SavePanelMode::Generic);
    assert_ne!(SavePanelMode::Generic, SavePanelMode::RpgMaker);
}

/// 验证 readonly 标志的默认值为 false，且可以正确切换为 true
#[test]
fn test_save_panel_readonly_flag() {
    let mut state = AppState::new(None);
    assert!(!state.save_panel.readonly);
    state.save_panel.readonly = true;
    assert!(state.save_panel.readonly);
}

/// 验证未加载游戏时 game_title 为空字符串
#[test]
fn test_game_title_empty_with_no_game() {
    let state = AppState::new(None);
    assert_eq!(state.game_title, "");
}
