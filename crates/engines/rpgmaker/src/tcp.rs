//! RPG Maker TCP 桥接（文本命令协议）

use std::collections::HashMap;
use std::sync::Mutex;

use serde_json::Value;
use game_tool_core::{GameBridge, BridgeCommand, GameState, GameToolError};
use game_tool_core::net::TcpLineConnection;

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
/// NW.js 插件注入（GameBridgeServer.js）

use std::fs;
use std::path::{Path, PathBuf};

const PLUGIN_FILENAME: &str = "GameBridgeServer.js";
const PLUGINS_JS: &str = "js/plugins.js";

/// GameBridgeServer.js 插件源码（TCP 服务器，暴露 RPG Maker 游戏对象）
pub const PLUGIN_SOURCE: &str = r#"
var net = require('net');
var server = net.createServer(function(socket) {
    socket.setEncoding('utf8');
    var buffer = '';
    socket.on('data', function(data) {
        buffer += data;
        var newline = buffer.indexOf('\n');
        while (newline >= 0) {
            var cmd = buffer.substring(0, newline).trim();
            buffer = buffer.substring(newline + 1);
            try { handleCommand(socket, cmd); } catch(e) {}
            newline = buffer.indexOf('\n');
        }
    });
});
function handleCommand(socket, cmd) {
    var parts = cmd.split(' ');
    var action = parts[0];
    if (action === 'ping') { socket.write('PONG\n'); return; }
    if (action === 'get_state') {
        var state = {
            gold: $gameParty ? $gameParty._gold : 0,
            steps: $gameParty ? $gameParty._steps : 0,
            switches: {},
            variables: {},
            party: [],
            items: [],
            mapName: $gameMap ? $gameMap.displayName() : '',
            playtime: $gameSystem ? $gameSystem.playtimeText() : '',
            saveCount: $gameSystem ? $gameSystem.saveCount() : 0
        };
        for (var i = 1; i < $dataSystem.switches.length; i++) state.switches[i] = $gameSwitches.value(i);
        for (var i = 1; i < $dataSystem.variables.length; i++) state.variables[i] = $gameVariables.value(i);
        socket.write('STATE:' + JSON.stringify(state) + '\n');
        return;
    }
    if (action === 'set_gold') { $gameParty._gold = parseInt(parts[1]); socket.write('OK\n'); return; }
    if (action === 'set_switch') { $gameSwitches.setValue(parseInt(parts[1]), parts[2] === '1'); socket.write('OK\n'); return; }
    if (action === 'set_variable') { $gameVariables.setValue(parseInt(parts[1]), parseInt(parts[2])); socket.write('OK\n'); return; }
    if (action === 'set_hp') { $gameActors.actor(parseInt(parts[1]))._hp = parseInt(parts[2]); socket.write('OK\n'); return; }
    if (action === 'set_mp') { $gameActors.actor(parseInt(parts[1]))._mp = parseInt(parts[2]); socket.write('OK\n'); return; }
    if (action === 'set_item') { $gameParty._items[parseInt(parts[1])] = parseInt(parts[2]); socket.write('OK\n'); return; }
    socket.write('ERR\n');
}
server.listen(19999, '127.0.0.1');
"#;

/// 检查插件是否已安装
pub fn is_plugin_installed(game_dir: &str) -> bool {
    let plugin_path = find_plugin_file(game_dir);
    if !plugin_path.is_file() { return false; }
    let plugins_js = find_plugins_js(game_dir);
    if !plugins_js.is_file() { return false; }
    if let Ok(content) = fs::read_to_string(&plugins_js) {
        content.contains(PLUGIN_FILENAME)
    } else { false }
}

/// 注入插件到游戏目录
pub fn inject_plugin(game_dir: &str) -> Result<(), String> {
    let www_dir = Path::new(game_dir).join("www");
    let js_plugins = www_dir.join("js/plugins");
    fs::create_dir_all(&js_plugins).map_err(|e| e.to_string())?;

    let plugin_path = js_plugins.join(PLUGIN_FILENAME);
    fs::write(&plugin_path, PLUGIN_SOURCE).map_err(|e| e.to_string())?;

    let plugins_js = www_dir.join(PLUGINS_JS);
    if plugins_js.is_file() {
        let content = fs::read_to_string(&plugins_js).map_err(|e| e.to_string())?;
        if !content.contains(PLUGIN_FILENAME) {
            let mut new_content = String::new();
            for line in content.lines() {
                new_content.push_str(line);
                new_content.push('\n');
                if line.trim().starts_with("//") && line.contains("end of") {
                    new_content.push_str(r#"{"name":"GameBridgeServer","status":true,"description":"TCP Bridge","parameters":{}},"#);
                    new_content.push('\n');
                }
            }
            if !new_content.contains(PLUGIN_FILENAME) {
                new_content = content.replace("];", r#"{"name":"GameBridgeServer","status":true,"description":"TCP Bridge","parameters":{}},\n];"#);
            }
            fs::write(&plugins_js, &new_content).map_err(|e| e.to_string())?;
        }
    }

    Ok(())
}

/// 移除插件
pub fn remove_plugin(game_dir: &str) -> Result<(), String> {
    let plugin_path = find_plugin_file(game_dir);
    if plugin_path.is_file() {
        fs::remove_file(&plugin_path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn find_plugin_file(game_dir: &str) -> PathBuf {
    Path::new(game_dir).join("www/js/plugins").join(PLUGIN_FILENAME)
}

fn find_plugins_js(game_dir: &str) -> PathBuf {
    Path::new(game_dir).join("www/js/plugins.js")
}
