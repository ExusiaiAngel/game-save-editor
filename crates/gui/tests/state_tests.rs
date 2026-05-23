mod common;

use game_tool_core::detector::EngineType;
use game_tool_gui::state::{AppState, ConnectionStatus, SavePanelMode};

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
    assert_eq!(state.rt_panel.refresh_timer, 0);
    assert!(state.rt_panel.locked_fields.is_empty());
}

#[test]
fn test_connection_status_variants_distinct() {
    assert_ne!(ConnectionStatus::Disconnected, ConnectionStatus::Connecting);
    assert_ne!(ConnectionStatus::Connecting, ConnectionStatus::Connected);
    assert_ne!(ConnectionStatus::Connected, ConnectionStatus::Disconnected);
}

#[test]
fn test_save_panel_mode_variants_distinct() {
    assert_ne!(SavePanelMode::RpgMaker, SavePanelMode::RenPy);
    assert_ne!(SavePanelMode::RenPy, SavePanelMode::Unreal);
    assert_ne!(SavePanelMode::Unreal, SavePanelMode::Generic);
    assert_ne!(SavePanelMode::Generic, SavePanelMode::RpgMaker);
}

#[test]
fn test_save_panel_readonly_flag() {
    let mut state = AppState::new(None);
    assert!(!state.save_panel.readonly);
    state.save_panel.readonly = true;
    assert!(state.save_panel.readonly);
}

#[test]
fn test_game_title_empty_with_no_game() {
    let state = AppState::new(None);
    assert_eq!(state.game_title, "");
}
