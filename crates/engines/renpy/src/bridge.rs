//! Ren'Py TCP JSON 桥接

use std::collections::HashMap;
use std::sync::Mutex;

use serde_json::{json, Value};
use game_tool_core::{
    GameBridge, BridgeCommand, GameState,
    error::GameToolError,
};
use game_tool_infra::net::TcpLineConnection;

pub struct RenPyBridge {
    conn: Mutex<Option<TcpLineConnection>>,
    host: String,
    port: u16,
}

impl RenPyBridge {
    pub fn new(host: &str, port: u16) -> Self {
        Self { conn: Mutex::new(None), host: host.to_string(), port }
    }

    fn send_json(&self, action: &str, params: Value) -> Result<Value, GameToolError> {
        let mut guard = self.conn.lock().map_err(|e| {
            GameToolError::BridgeConnectError(e.to_string())
        })?;
        let conn = guard.as_mut().ok_or_else(|| {
            GameToolError::BridgeConnectError("未连接".into())
        })?;
        let msg = json!({"action": action, "params": params});
        let msg_str = serde_json::to_string(&msg)
            .map_err(|e| GameToolError::BridgeCommandError(e.to_string()))?;
        conn.send_line(&msg_str).map_err(|e| {
            GameToolError::BridgeCommandError(e.to_string())
        })?;
        let resp = conn.recv_line().map_err(|e| {
            GameToolError::BridgeCommandError(e.to_string())
        })?;
        serde_json::from_str(&resp)
            .map_err(|e| GameToolError::BridgeCommandError(e.to_string()))
    }
}

impl GameBridge for RenPyBridge {
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
            if let Some(ref mut conn) = *guard { conn.disconnect(); }
            *guard = None;
        }
    }

    fn is_connected(&self) -> bool {
        self.conn.lock().map(|g| g.is_some()).unwrap_or(false)
    }

    fn execute(&mut self, cmd: &BridgeCommand) -> Result<Value, GameToolError> {
        match cmd {
            BridgeCommand::ReadAll => {
                let resp = self.send_json("get_state", json!({}))?;
                let store = resp.get("store").cloned().unwrap_or(Value::Object(Default::default()));
                let mut extensions = HashMap::new();
                extensions.insert("store".into(), store);
                Ok(serde_json::to_value(GameState {
                    engine: "renpy".into(),
                    extensions,
                    ..Default::default()
                }).map_err(|e| GameToolError::BridgeCommandError(e.to_string()))?)
            }
            BridgeCommand::ReadField(field_id) => {
                let var_name = Self::field_to_var(field_id);
                let resp = self.send_json("get_var", json!({"name": var_name}))?;
                Ok(resp.get("value").cloned().unwrap_or(Value::Null))
            }
            BridgeCommand::WriteField(field_id, value) => {
                let var_name = Self::field_to_var(field_id);
                let code = format!("store.{} = {}", var_name, Self::value_to_python(value));
                self.send_json("eval", json!({"code": code}))?;
                Ok(Value::String("ok".into()))
            }
        }
    }

    fn engine_name(&self) -> &str { "renpy" }
    fn priority(&self) -> i32 { 50 }
}

impl RenPyBridge {
    fn field_to_var(field_id: &str) -> &str {
        match field_id {
            "gold" => "money",
            "gold_alt" => "gold",
            s if s.starts_with("var_") => &s[4..],
            s => s,
        }
    }

    fn value_to_python(value: &Value) -> String {
        match value {
            Value::String(s) => format!("{:?}", s),
            Value::Bool(b) => (if *b { "True" } else { "False" }).to_string(),
            Value::Number(n) => n.to_string(),
            Value::Null => "None".to_string(),
            _ => value.to_string(),
        }
    }
}
