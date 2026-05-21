//! NW.js 插件注入（GameBridgeServer.js）

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
