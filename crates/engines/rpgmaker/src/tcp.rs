//! RPG Maker TCP 桥接（文本命令协议）

use std::collections::HashMap;
use std::sync::Mutex;

use serde_json::Value;
use game_tool_core::{GameBridge, BridgeCommand, GameState};
use game_tool_core::error::GameToolError;
use game_tool_infra::net::TcpLineConnection;

pub struct RpgMakerTcpBridge {
    conn: Mutex<Option<TcpLineConnection>>,
    host: String,
    port: u16,
}

impl RpgMakerTcpBridge {
    pub fn new(host: &str, port: u16) -> Self {
        Self { conn: Mutex::new(None), host: host.to_string(), port }
    }

    fn send_cmd(&self, cmd: &str) -> Result<String, GameToolError> {
        let mut guard = self.conn.lock().map_err(|e| {
            GameToolError::BridgeConnectError(e.to_string())
        })?;
        let conn = guard.as_mut().ok_or_else(|| {
            GameToolError::BridgeConnectError("未连接".into())
        })?;
        conn.send_line(cmd).map_err(|e| {
            GameToolError::BridgeCommandError(e.to_string())
        })?;
        conn.recv_line().map_err(|e| {
            GameToolError::BridgeCommandError(e.to_string())
        })
    }

    fn parse_state_response(resp: &str) -> Result<GameState, GameToolError> {
        let json_str = resp.strip_prefix("STATE:")
            .ok_or_else(|| GameToolError::BridgeCommandError("无效响应".into()))?;
        let raw: Value = serde_json::from_str(json_str)
            .map_err(|e| GameToolError::BridgeCommandError(e.to_string()))?;
        let mut extensions = HashMap::new();
        if let Some(sw) = raw.get("switches") { extensions.insert("switches".into(), sw.clone()); }
        if let Some(vars) = raw.get("variables") { extensions.insert("variables".into(), vars.clone()); }
        if let Some(party) = raw.get("party") { extensions.insert("party".into(), party.clone()); }
        if let Some(items) = raw.get("items") { extensions.insert("items".into(), items.clone()); }
        extensions.insert("gold".into(), Value::Number(
            raw.get("gold").and_then(|v| v.as_i64()).unwrap_or(0).into()
        ));
        Ok(GameState {
            engine: "rpg_maker".into(),
            map_name: raw.get("mapName").and_then(|v| v.as_str()).unwrap_or("").into(),
            play_time: raw.get("playtime").and_then(|v| v.as_str()).unwrap_or("").into(),
            save_count: raw.get("saveCount").and_then(|v| v.as_i64()).unwrap_or(0) as i32,
            extensions,
        })
    }
}

impl GameBridge for RpgMakerTcpBridge {
    fn connect(&mut self) -> Result<(), GameToolError> {
        let conn = TcpLineConnection::connect(&format!("{}:{}", self.host, self.port))
            .map_err(|e| GameToolError::BridgeConnectError(e.to_string()))?;
        *self.conn.lock().map_err(|e| {
            GameToolError::BridgeConnectError(e.to_string())
        })? = Some(conn);
        Ok(())
    }

    fn disconnect(&mut self) {
        if let Ok(mut guard) = self.conn.lock() {
            if let Some(ref mut conn) = *guard {
                let _ = conn.send_line("close");
                conn.disconnect();
            }
            *guard = None;
        }
    }

    fn is_connected(&self) -> bool {
        self.conn.lock()
            .map(|g| g.as_ref().is_some_and(|c| c.is_connected()))
            .unwrap_or(false)
    }

    fn execute(&mut self, cmd: &BridgeCommand) -> Result<Value, GameToolError> {
        match cmd {
            BridgeCommand::ReadAll => {
                let resp = self.send_cmd("get_state")?;
                let state = Self::parse_state_response(&resp)?;
                serde_json::to_value(state)
                    .map_err(|e| GameToolError::BridgeCommandError(e.to_string()))
            }
            BridgeCommand::ReadField(field_id) => {
                let resp = self.send_cmd("get_state")?;
                let json_str = resp.strip_prefix("STATE:")
                    .ok_or_else(|| GameToolError::BridgeCommandError("无效响应".into()))?;
                let raw: Value = serde_json::from_str(json_str)
                    .map_err(|e| GameToolError::BridgeCommandError(e.to_string()))?;
                if field_id == "gold" {
                    Ok(raw.get("gold").cloned().unwrap_or(Value::Null))
                } else if let Some(id_str) = field_id.strip_prefix("switch_") {
                    let id = id_str.to_string();
                    Ok(raw.get("switches").and_then(|s| s.get(&id)).cloned().unwrap_or(Value::Null))
                } else if let Some(id_str) = field_id.strip_prefix("var_") {
                    let id = id_str.to_string();
                    Ok(raw.get("variables").and_then(|v| v.get(&id)).cloned().unwrap_or(Value::Null))
                } else {
                    Ok(Value::Null)
                }
            }
            BridgeCommand::WriteField(field_id, value) => {
                let check_ok = |resp: &str| {
                    if resp.starts_with("OK") { Ok(Value::String("ok".into())) }
                    else { Err(GameToolError::BridgeCommandError(resp.into())) }
                };
                if field_id == "gold" {
                    let n = value.as_i64().unwrap_or(0);
                    check_ok(&self.send_cmd(&format!("set_gold {}", n))?)
                } else if let Some(id_str) = field_id.strip_prefix("switch_") {
                    let id: i32 = id_str.parse().unwrap_or(0);
                    let v = if value.as_bool().unwrap_or(false) { 1 } else { 0 };
                    check_ok(&self.send_cmd(&format!("set_switch {} {}", id, v))?)
                } else if let Some(id_str) = field_id.strip_prefix("var_") {
                    let id: i32 = id_str.parse().unwrap_or(0);
                    let v = value.as_i64().unwrap_or(0);
                    check_ok(&self.send_cmd(&format!("set_variable {} {}", id, v))?)
                } else if let Some(rest) = field_id.strip_prefix("actor_") {
                    let parts: Vec<&str> = rest.rsplitn(2, '_').collect();
                    if parts.len() == 2 {
                        let id: i32 = parts[0].parse().unwrap_or(0);
                        let v = value.as_i64().unwrap_or(0);
                        let cmd = match parts[1] {
                            "hp" => format!("set_hp {} {}", id, v),
                            "mp" => format!("set_mp {} {}", id, v),
                            _ => return Err(GameToolError::BridgeCommandError(format!("未知属性: {}", parts[1]))),
                        };
                        check_ok(&self.send_cmd(&cmd)?)
                    } else {
                        Err(GameToolError::BridgeCommandError("无效actor字段".into()))
                    }
                } else if let Some(id_str) = field_id.strip_prefix("item_") {
                    let id: i32 = id_str.parse().unwrap_or(0);
                    let count = value.as_i64().unwrap_or(0);
                    check_ok(&self.send_cmd(&format!("set_item {} {}", id, count))?)
                } else {
                    Err(GameToolError::BridgeCommandError(format!("不支持的字段: {}", field_id)))
                }
            }
        }
    }

    fn engine_name(&self) -> &str { "rpg_maker_tcp" }
    fn priority(&self) -> i32 { 10 }
}
