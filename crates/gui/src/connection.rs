use crate::factory::create_bridge;
use crate::state::{BridgeJob, BridgeResult, ConnectionStatus, RealtimeConnection};
use game_tool_core::detector::EngineType;
use std::sync::mpsc;
use std::thread;

pub fn spawn_bridge_thread(
    engine_clone: EngineType,
    host: String,
    port: u16,
) -> RealtimeConnection {
    let (cmd_tx, cmd_rx) = mpsc::channel::<BridgeJob>();
    let (result_tx, result_rx) = mpsc::channel::<BridgeResult>();

    let host_clone = host.clone();
    thread::spawn(move || {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut bridge: Option<Box<dyn game_tool_core::GameBridge>> = None;

            loop {
                match cmd_rx.recv() {
                    Ok(BridgeJob::Connect) => {
                        let h = host_clone.clone();
                        if let Some(mut b) = create_bridge(&engine_clone, &h, port) {
                            match b.connect() {
                                Ok(()) => {
                                    bridge = Some(b);
                                    let _ = result_tx.send(BridgeResult::Connected);
                                }
                                Err(e) => {
                                    let _ = result_tx.send(BridgeResult::Error(e.to_string()));
                                }
                            }
                        } else {
                            let _ =
                                result_tx.send(BridgeResult::Error("该引擎不支持实时连接".into()));
                        }
                    }
                    Ok(BridgeJob::Disconnect) => {
                        if let Some(ref mut b) = bridge {
                            b.disconnect();
                        }
                        bridge = None;
                        let _ = result_tx.send(BridgeResult::Disconnected);
                    }
                    Ok(BridgeJob::Execute(cmd)) => match &mut bridge {
                        Some(b) => match b.execute(&cmd) {
                            Ok(val) => {
                                let _ = result_tx.send(BridgeResult::CommandResult(val));
                            }
                            Err(e) => {
                                let _ = result_tx.send(BridgeResult::Error(e.to_string()));
                            }
                        },
                        None => {
                            let _ = result_tx.send(BridgeResult::Error("未连接".into()));
                        }
                    },
                    Err(_) => {
                        if let Some(ref mut b) = bridge {
                            b.disconnect();
                        }
                        let _ = result_tx.send(BridgeResult::Disconnected);
                        break;
                    }
                }
            }
        }));

        if let Err(_panic) = result {
            let _ = result_tx.send(BridgeResult::Error("连接异常断开".into()));
            let _ = result_tx.send(BridgeResult::Disconnected);
        }
    });

    RealtimeConnection {
        cmd_tx,
        result_rx,
        status: ConnectionStatus::Disconnected,
    }
}

pub fn drain_results(conn: &mut RealtimeConnection) -> Vec<BridgeResult> {
    let mut results = Vec::new();
    while let Ok(r) = conn.result_rx.try_recv() {
        results.push(r);
    }
    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{BridgeJob, BridgeResult, ConnectionStatus, RealtimeConnection};
    use std::sync::mpsc;

    fn make_test_connection() -> RealtimeConnection {
        let (cmd_tx, _cmd_rx) = mpsc::channel();
        let (_result_tx, result_rx) = mpsc::channel();
        RealtimeConnection {
            cmd_tx,
            result_rx,
            status: ConnectionStatus::Disconnected,
        }
    }

    #[test]
    fn test_drain_results_empty() {
        let mut conn = make_test_connection();
        let results = drain_results(&mut conn);
        assert!(results.is_empty());
    }

    #[test]
    fn test_drain_results_single() {
        let (cmd_tx, _) = mpsc::channel();
        let (result_tx, result_rx) = mpsc::channel();
        result_tx.send(BridgeResult::Connected).unwrap();
        drop(result_tx);

        let mut conn = RealtimeConnection {
            cmd_tx,
            result_rx,
            status: ConnectionStatus::Disconnected,
        };
        let results = drain_results(&mut conn);
        assert_eq!(results.len(), 1);
        match &results[0] {
            BridgeResult::Connected => {}
            _ => panic!("Expected Connected"),
        }
    }

    #[test]
    fn test_drain_results_multiple() {
        let (cmd_tx, _) = mpsc::channel();
        let (result_tx, result_rx) = mpsc::channel();
        result_tx.send(BridgeResult::Connected).unwrap();
        result_tx
            .send(BridgeResult::CommandResult(serde_json::Value::String(
                "ok".into(),
            )))
            .unwrap();
        result_tx.send(BridgeResult::Disconnected).unwrap();
        drop(result_tx);

        let mut conn = RealtimeConnection {
            cmd_tx,
            result_rx,
            status: ConnectionStatus::Disconnected,
        };
        let results = drain_results(&mut conn);
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn test_drain_results_error() {
        let (cmd_tx, _) = mpsc::channel();
        let (result_tx, result_rx) = mpsc::channel();
        result_tx
            .send(BridgeResult::Error("connection refused".into()))
            .unwrap();
        drop(result_tx);

        let mut conn = RealtimeConnection {
            cmd_tx,
            result_rx,
            status: ConnectionStatus::Disconnected,
        };
        let results = drain_results(&mut conn);
        assert_eq!(results.len(), 1);
        match &results[0] {
            BridgeResult::Error(e) => assert_eq!(e, "connection refused"),
            _ => panic!("Expected Error"),
        }
    }

    #[test]
    fn test_spawn_bridge_thread_initial_status() {
        let conn = spawn_bridge_thread(
            game_tool_core::detector::EngineType::Unknown,
            "127.0.0.1".into(),
            19999,
        );
        assert_eq!(conn.status, ConnectionStatus::Disconnected);
    }

    #[test]
    fn test_spawn_bridge_thread_supported_engine() {
        let conn = spawn_bridge_thread(
            game_tool_core::detector::EngineType::RpgMakerMv,
            "127.0.0.1".into(),
            19999,
        );
        assert_eq!(conn.status, ConnectionStatus::Disconnected);
        // Connection should be possible (bridge created but not connected yet)
        let _ = conn.cmd_tx.send(BridgeJob::Connect);
        // Don't wait for result since no real server — just verify no panic
    }
}
