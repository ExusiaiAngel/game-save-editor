//! Ren'Py TCP JSON 桥接

use std::collections::HashMap;
use std::sync::Mutex;

use game_tool_core::net::TcpLineConnection;
use game_tool_core::{BridgeCommand, GameBridge, GameState, GameToolError};
use serde_json::{json, Value};

pub struct RenPyBridge {
    conn: Mutex<Option<TcpLineConnection>>,
    host: String,
    port: u16,
}

impl RenPyBridge {
    pub fn new(host: &str, port: u16) -> Self {
        Self {
            conn: Mutex::new(None),
            host: host.to_string(),
            port,
        }
    }

    fn send_json(&self, action: &str, params: Value) -> Result<Value, GameToolError> {
        let mut guard = self
            .conn
            .lock()
            .map_err(|e| GameToolError::BridgeConnectError(e.to_string()))?;
        let conn = guard
            .as_mut()
            .ok_or_else(|| GameToolError::BridgeConnectError("未连接".into()))?;
        let msg = json!({"action": action, "params": params});
        let msg_str = serde_json::to_string(&msg)
            .map_err(|e| GameToolError::BridgeCommandError(e.to_string()))?;
        conn.send_line(&msg_str)
            .map_err(|e| GameToolError::BridgeCommandError(e.to_string()))?;
        let resp = conn
            .recv_line()
            .map_err(|e| GameToolError::BridgeCommandError(e.to_string()))?;
        serde_json::from_str(&resp).map_err(|e| GameToolError::BridgeCommandError(e.to_string()))
    }
}

impl GameBridge for RenPyBridge {
    fn connect(&mut self) -> Result<(), GameToolError> {
        let conn = TcpLineConnection::connect(&format!("{}:{}", self.host, self.port))
            .map_err(|e| GameToolError::BridgeConnectError(e.to_string()))?;
        *self
            .conn
            .lock()
            .map_err(|e| GameToolError::BridgeConnectError(e.to_string()))? = Some(conn);
        Ok(())
    }

    fn disconnect(&mut self) {
        if let Ok(mut guard) = self.conn.lock() {
            if let Some(ref mut conn) = *guard {
                conn.disconnect();
            }
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
                let store = resp
                    .get("store")
                    .cloned()
                    .unwrap_or(Value::Object(Default::default()));
                let mut extensions = HashMap::new();
                extensions.insert("store".into(), store);
                Ok(serde_json::to_value(GameState {
                    engine: "renpy".into(),
                    extensions,
                    ..Default::default()
                })
                .map_err(|e| GameToolError::BridgeCommandError(e.to_string()))?)
            }
            BridgeCommand::ReadField(field_id) => {
                let var_name = Self::field_to_var(field_id);
                let resp = self.send_json("get_var", json!({"name": var_name}))?;
                Ok(resp.get("value").cloned().unwrap_or(Value::Null))
            }
            BridgeCommand::WriteField(field_id, value) => {
                let var_name = Self::field_to_var(field_id);
                self.send_json("set_var", json!({"name": var_name, "value": value}))?;
                Ok(Value::String("ok".into()))
            }
        }
    }

    fn engine_name(&self) -> &str {
        "renpy"
    }
    fn priority(&self) -> i32 {
        50
    }
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
}
/// Ren'Py 插件注入（TCP Bridge Python 模块）
use std::fs;
use std::path::{Path, PathBuf};

pub const PLUGIN_CODE: &str = r#"
import socket
import threading
import json

class GameBridgeServer:
    def __init__(self, host='127.0.0.1', port=19999):
        self.host = host
        self.port = port
        self.running = True
        self.server = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        self.server.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        try:
            self.server.bind((host, port))
            self.server.listen(1)
            self.server.settimeout(1)
        except:
            pass

    def start(self):
        thread = threading.Thread(target=self._run, daemon=True)
        thread.start()

    def _run(self):
        while self.running:
            try:
                client, addr = self.server.accept()
                self._handle(client)
            except socket.timeout:
                continue
            except:
                break

    def _handle(self, client):
        client.settimeout(10)
        data = b''
        while True:
            try:
                chunk = client.recv(4096)
                if not chunk: break
                data += chunk
                if b'\n' in data: break
            except: break
        try:
            msg = json.loads(data.decode('utf-8').strip())
            action = msg.get('action', '')
            params = msg.get('params', {})
            if action == 'ping':
                client.send(json.dumps({'status':'ok'}).encode())
            elif action == 'get_state':
                import renpy
                store_vars = {k:v for k,v in renpy.store.__dict__.items() if not k.startswith('_')}
                client.send(json.dumps({'store':store_vars}).encode())
            elif action == 'get_var':
                import renpy
                name = params.get('name','')
                val = getattr(renpy.store, name, None)
                client.send(json.dumps({'value':val}).encode())
            elif action == 'set_var':
                import renpy
                name = params.get('name','')
                val = params.get('value')
                setattr(renpy.store, name, val)
                client.send(json.dumps({'status':'ok'}).encode())
            elif action == 'eval':
                code = params.get('code','')
                exec(code, {'store':renpy.store})
                client.send(json.dumps({'status':'ok'}).encode())
        except Exception as e:
            client.send(json.dumps({'error':str(e)}).encode())
        finally:
            client.close()

try:
    bridge = GameBridgeServer()
    bridge.start()
except:
    pass
"#;

/// 检查插件是否已安装
pub fn is_plugin_installed(game_dir: &str) -> bool {
    get_plugin_target(game_dir).is_file()
}

/// 注入插件
pub fn inject_plugin(game_dir: &str) -> Result<(), String> {
    let target = get_plugin_target(game_dir);
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(&target, PLUGIN_CODE).map_err(|e| e.to_string())?;
    Ok(())
}

/// 移除插件
pub fn remove_plugin(game_dir: &str) -> Result<(), String> {
    let target = get_plugin_target(game_dir);
    if target.is_dir() {
        fs::remove_dir_all(&target).map_err(|e| e.to_string())?;
    } else if target.is_file() {
        fs::remove_file(&target).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn get_plugin_target(game_dir: &str) -> PathBuf {
    Path::new(game_dir).join("game/python-packages/tcp_bridge/__init__.py")
}
