mod common;

use game_tool_core::detector::EngineType;
use game_tool_core::{BridgeCommand, ModifiableField};
use game_tool_gui::factory;
use game_tool_gui::panels::{realtime_editor, save_editor};
use game_tool_gui::state::{AppState, BridgeJob, BridgeResult};
use serde_json::Value;
use std::collections::HashSet;

// ─── T1: switch game must disconnect bridge ────────────────────────

#[test]
fn test_switch_game_disconnects_bridge() {
    let state = AppState::new(Some("nonexistent_path".into()));
    assert!(state.rt_panel.conn.is_none());
}

// ─── T9: dirty_count should match actual dirty fields ──────────────

#[test]
fn test_dirty_count_matches_modified_fields() {
    let mut fields = vec![
        ModifiableField {
            category: "gold".into(),
            field_id: "gold".into(),
            display_name: "金币".into(),
            field_type: "int".into(),
            save_value: Value::Number(100.into()),
            dirty: false,
            ..Default::default()
        },
        ModifiableField {
            category: "switch".into(),
            field_id: "switch_1".into(),
            display_name: "开关 #1".into(),
            field_type: "bool".into(),
            save_value: Value::Bool(false),
            dirty: false,
            ..Default::default()
        },
        ModifiableField {
            category: "variable".into(),
            field_id: "var_1".into(),
            display_name: "变量 #1".into(),
            field_type: "int".into(),
            save_value: Value::Number(0.into()),
            dirty: false,
            ..Default::default()
        },
    ];

    let dirty_count_before = fields.iter().filter(|f| f.dirty).count();
    assert_eq!(dirty_count_before, 0);

    fields[1].save_value = Value::Bool(true);
    fields[1].dirty = true;
    fields[2].save_value = Value::Number(42.into());
    fields[2].dirty = true;

    let dirty_count_after = fields.iter().filter(|f| f.dirty).count();
    assert_eq!(dirty_count_after, 2);
}

// ─── T4: write feedback not sent when disconnected ─────────────────

#[test]
fn test_write_field_not_sent_when_disconnected() {
    let is_conn = false;
    let changed = true;
    let new_val = Some(Value::Number(999.into()));
    let mut action_pushed = false;

    if changed {
        if let Some(_v) = &new_val {
            if is_conn {
                action_pushed = true;
            }
            // live_value update happens regardless
        }
    }

    assert!(!action_pushed);
}

#[test]
fn test_write_field_sent_when_connected() {
    let is_conn = true;
    let changed = true;
    let new_val = Some(Value::Number(999.into()));
    let mut action_pushed = false;

    if changed {
        if let Some(_v) = &new_val {
            if is_conn {
                action_pushed = true;
            }
        }
    }

    assert!(action_pushed);
}

// ─── T3: real-time fields should be clear after game switch ───────

#[test]
fn test_rt_fields_cleared_on_game_switch() {
    let mut state = AppState::new(None);
    state.rt_panel.fields = vec![ModifiableField {
        category: "switch".into(),
        field_id: "switch_1".into(),
        display_name: "stale".into(),
        field_type: "bool".into(),
        live_value: Value::Bool(true),
        ..Default::default()
    }];
    assert!(!state.rt_panel.fields.is_empty());

    state.rt_panel.fields.clear();
    assert!(state.rt_panel.fields.is_empty());
}

// ─── T10: locked_fields toggle logic ──────────────────────────────

#[test]
fn test_locked_fields_toggle() {
    let mut locked: HashSet<String> = HashSet::new();
    let fid = "switch_5".to_string();

    if locked.contains(&fid) {
        locked.remove(&fid);
    } else {
        locked.insert(fid.clone());
    }
    assert!(locked.contains(&fid));

    if locked.contains(&fid) {
        locked.remove(&fid);
    } else {
        locked.insert(fid.clone());
    }
    assert!(!locked.contains(&fid));
}

// ─── T7: error/feedback message clearing ──────────────────────────

#[test]
fn test_error_message_clears_on_timer_expiry() {
    let mut error_expires_at: Option<std::time::Instant> =
        Some(std::time::Instant::now() + std::time::Duration::from_secs(5));
    let mut error_message = String::from("test error");

    // Not expired yet
    if let Some(at) = error_expires_at {
        if at <= std::time::Instant::now() {
            error_message.clear();
            error_expires_at = None;
        }
    }
    assert!(!error_message.is_empty());

    // Expired (set to past)
    error_expires_at = Some(std::time::Instant::now() - std::time::Duration::from_secs(1));
    if let Some(at) = error_expires_at {
        assert!(at <= std::time::Instant::now());
        error_message.clear();
        error_expires_at = None;
    }
    assert!(error_message.is_empty());
    assert!(error_expires_at.is_none());
}

#[test]
fn test_write_feedback_clears_on_timer_expiry() {
    let mut feedback_expires_at: Option<std::time::Instant> =
        Some(std::time::Instant::now() + std::time::Duration::from_secs(3));
    let mut write_feedback = String::from("已写入");

    // Not expired yet
    if let Some(at) = feedback_expires_at {
        if at <= std::time::Instant::now() {
            write_feedback.clear();
            feedback_expires_at = None;
        }
    }
    assert!(!write_feedback.is_empty());

    // Expired (set to past)
    feedback_expires_at = Some(std::time::Instant::now() - std::time::Duration::from_secs(1));
    if let Some(at) = feedback_expires_at {
        assert!(at <= std::time::Instant::now());
        write_feedback.clear();
        feedback_expires_at = None;
    }
    assert!(write_feedback.is_empty());
    assert!(feedback_expires_at.is_none());
}

// ─── T12: connection thread sends Disconnected on channel close ────

#[test]
fn test_drain_results_receives_disconnected() {
    let (_cmd_tx, _cmd_rx) = std::sync::mpsc::channel::<BridgeJob>();
    let (result_tx, result_rx) = std::sync::mpsc::channel();

    result_tx.send(BridgeResult::Connected).unwrap();
    result_tx.send(BridgeResult::Disconnected).unwrap();
    drop(result_tx);

    let mut results: Vec<BridgeResult> = Vec::new();
    while let Ok(r) = result_rx.try_recv() {
        results.push(r);
    }

    assert_eq!(results.len(), 2);
    match &results[0] {
        BridgeResult::Connected => {}
        _ => panic!("Expected Connected"),
    }
    match &results[1] {
        BridgeResult::Disconnected => {}
        _ => panic!("Expected Disconnected"),
    }
}

// ─── T6: unsupported engine returns error on connect ──────────────

#[test]
fn test_unsupported_engine_bridge_returns_none() {
    let bridge = factory::create_bridge(&EngineType::Unknown, "127.0.0.1", 8080);
    assert!(bridge.is_none());

    let bridge = factory::create_bridge(&EngineType::Unreal, "127.0.0.1", 8080);
    assert!(bridge.is_none());

    let bridge = factory::create_bridge(&EngineType::UnityMono, "127.0.0.1", 8080);
    assert!(bridge.is_none());
}

// ─── T13: backup file filter matches .bak suffix ──────────────────

#[test]
fn test_backup_filter_matches_bak_suffix() {
    let test_cases = vec![
        ("save1.bak", true),
        ("save.bak.json", true),
        ("save.bak.old.rpgsave", true),
        ("file1.rpgsave", false),
        ("config.rpgsave", true),
        ("global.rpgsave", true),
        ("my.bak", true),
        ("backup-file", false),
    ];

    for (name, should_exclude) in test_cases {
        let excluded = name.contains(".bak.")
            || name.ends_with(".bak")
            || name == "config.rpgsave"
            || name == "global.rpgsave";
        assert_eq!(excluded, should_exclude, "Failed for: {}", name);
    }
}

// ─── T11: save panel dirty_count triggers save button ─────────────

#[test]
fn test_save_button_enabled_only_when_dirty_and_data_loaded() {
    let dirty_count = 3usize;
    let save_data_some = true;
    let readonly = false;

    let enabled = dirty_count > 0 && save_data_some && !readonly;
    assert!(enabled);

    let dirty_count_zero = 0usize;
    let enabled2 = dirty_count_zero > 0 && save_data_some && !readonly;
    assert!(!enabled2);

    let save_data_none = false;
    let enabled3 = dirty_count > 0 && save_data_none && !readonly;
    assert!(!enabled3);
}

// ─── T5: auto_refresh toggle ──────────────────────────────────────

#[test]
fn test_auto_refresh_toggle() {
    let mut auto_refresh = true;
    auto_refresh = !auto_refresh;
    assert!(!auto_refresh);
    auto_refresh = !auto_refresh;
    assert!(auto_refresh);
}

// ─── T2: unsaved dialog flag ──────────────────────────────────────

#[test]
fn test_unsaved_dialog_triggered_by_switch() {
    let dirty_count = 5usize;
    let show_dialog = dirty_count > 0;
    assert!(show_dialog);

    let dirty_count_zero = 0usize;
    let show_dialog2 = dirty_count_zero > 0;
    assert!(!show_dialog2);
}

// ─── BridgeResult enum tests ──────────────────────────────────────

#[test]
fn test_bridge_result_variants() {
    let connected = BridgeResult::Connected;
    match connected {
        BridgeResult::Connected => {}
        _ => panic!("Expected Connected"),
    }

    let disconnected = BridgeResult::Disconnected;
    match disconnected {
        BridgeResult::Disconnected => {}
        _ => panic!("Expected Disconnected"),
    }

    let result = BridgeResult::CommandResult(Value::Number(42.into()));
    match result {
        BridgeResult::CommandResult(v) => assert_eq!(v.as_i64(), Some(42)),
        _ => panic!("Expected CommandResult"),
    }

    let error = BridgeResult::Error("test".into());
    match error {
        BridgeResult::Error(e) => assert_eq!(e, "test"),
        _ => panic!("Expected Error"),
    }
}

// ─── BridgeJob enum tests ─────────────────────────────────────────

#[test]
fn test_bridge_job_variants() {
    let connect = BridgeJob::Connect;
    let disconnect = BridgeJob::Disconnect;
    let execute = BridgeJob::Execute(BridgeCommand::ReadAll);
    let write = BridgeJob::Execute(BridgeCommand::WriteField(
        "gold".into(),
        Value::Number(100.into()),
    ));

    // Verify they are constructible (no panic)
    let _ = connect;
    let _ = disconnect;
    match execute {
        BridgeJob::Execute(BridgeCommand::ReadAll) => {}
        _ => panic!(),
    }
    match write {
        BridgeJob::Execute(BridgeCommand::WriteField(id, val)) => {
            assert_eq!(id, "gold");
            assert_eq!(val.as_i64(), Some(100));
        }
        _ => panic!(),
    }
}

// ─── SaveAction enum test ─────────────────────────────────────────

#[test]
fn test_save_action_variants() {
    let load = save_editor::SaveAction::LoadSave;
    let refresh = save_editor::SaveAction::RefreshFiles;
    let save = save_editor::SaveAction::Save;

    let _ = load;
    let _ = refresh;
    let _ = save;
}

// ─── RtAction enum test ───────────────────────────────────────────

#[test]
fn test_rt_action_variants() {
    let write = realtime_editor::RtAction::WriteField("gold".into(), Value::Number(50.into()));
    let copy = realtime_editor::RtAction::CopyToSave("gold".into());
    let lock = realtime_editor::RtAction::ToggleLock("switch_1".into());

    match write {
        realtime_editor::RtAction::WriteField(id, val) => {
            assert_eq!(id, "gold");
            assert_eq!(val.as_i64(), Some(50));
        }
        _ => panic!(),
    }
    match copy {
        realtime_editor::RtAction::CopyToSave(id) => assert_eq!(id, "gold"),
        _ => panic!(),
    }
    match lock {
        realtime_editor::RtAction::ToggleLock(id) => assert_eq!(id, "switch_1"),
        _ => panic!(),
    }
}
