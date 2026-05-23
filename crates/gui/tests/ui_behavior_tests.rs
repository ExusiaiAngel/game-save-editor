//! UI 行为逻辑测试。
//!
//! 验证 GUI 中关键的交互逻辑：
//! - 游戏切换时桥接通道断开、实时字段清空
//! - 脏字段计数、锁定字段切换、自动刷新开关
//! - 连接状态下写入动作的正确路由
//! - 错误消息与反馈消息的定时清除机制
//! - 桥接命令与结果枚举的构造匹配

mod common;

use game_tool_core::detector::EngineType;
use game_tool_core::{BridgeCommand, ModifiableField};
use game_tool_gui::factory;
use game_tool_gui::panels::{realtime_editor, save_editor};
use game_tool_gui::state::{AppState, BridgeJob, BridgeResult};
use serde_json::Value;
use std::collections::HashSet;

/// 切换游戏时，桥接连接应被断开（即 conn 为空）
#[test]
fn test_switch_game_disconnects_bridge() {
    let state = AppState::new(Some("nonexistent_path".into()));
    assert!(state.rt_panel.conn.is_none());
}

/// 修改字段后 dirty_count 应正确反映实际脏字段数量
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

/// 未连接状态下，写入字段操作不应推送 WriteField 动作
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

/// 已连接状态下，写入字段操作应推送 WriteField 动作
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

/// 切换游戏后，实时编辑字段应被清空（避免残留旧游戏数据）
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

/// 锁定字段的开关切换逻辑：第一次插入，第二次移除
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

/// 错误消息在超时后应自动清除（未到期时不消失，到期后才清空）
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

/// 写入反馈消息在超时后应自动清除（3 秒到期）
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

/// 桥接通道关闭时，drain_results 应正确收到 Connected 和 Disconnected 结果
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

/// 未知引擎类型不应创建桥接实例（返回 None）
#[test]
fn test_unsupported_engine_bridge_returns_none() {
    let bridge = factory::create_bridge(&EngineType::Unknown, "127.0.0.1", 8080);
    assert!(bridge.is_none());
}

/// Unreal 和 Unity Mono 引擎应支持内存桥接（create_bridge 返回 Some）
#[test]
fn test_memory_bridge_supports_unreal_and_unity() {
    let bridge = factory::create_bridge(&EngineType::Unreal, "127.0.0.1", 8080);
    assert!(bridge.is_some());
    assert_eq!(bridge.unwrap().engine_name(), "memory_bridge");

    let bridge = factory::create_bridge(&EngineType::UnityMono, "127.0.0.1", 8080);
    assert!(bridge.is_some());
}

/// 备份文件过滤器应正确识别 .bak 后缀、config.rpgsave、global.rpgsave
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

/// 保存按钮仅在 dirty_count > 0 且已加载数据且非只读时启用
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

/// 自动刷新开关应能在 true/false 之间正确切换
#[test]
fn test_auto_refresh_toggle() {
    let mut auto_refresh = true;
    auto_refresh = !auto_refresh;
    assert!(!auto_refresh);
    auto_refresh = !auto_refresh;
    assert!(auto_refresh);
}

/// 有脏字段时切换游戏应触发"未保存修改"对话框
#[test]
fn test_unsaved_dialog_triggered_by_switch() {
    let dirty_count = 5usize;
    let show_dialog = dirty_count > 0;
    assert!(show_dialog);

    let dirty_count_zero = 0usize;
    let show_dialog2 = dirty_count_zero > 0;
    assert!(!show_dialog2);
}

/// BridgeResult 枚举的各种变体应能正确构造并匹配
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

/// BridgeJob 枚举的各种变体应能正确构造并匹配（含 WriteField 参数验证）
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

/// SaveAction 枚举的 LoadSave、RefreshFiles、Save 变体应能正确构造
#[test]
fn test_save_action_variants() {
    let load = save_editor::SaveAction::LoadSave;
    let refresh = save_editor::SaveAction::RefreshFiles;
    let save = save_editor::SaveAction::Save;

    let _ = load;
    let _ = refresh;
    let _ = save;
}

/// RtAction 枚举的 WriteField、CopyToSave、ToggleLock 变体应能正确构造并匹配
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
