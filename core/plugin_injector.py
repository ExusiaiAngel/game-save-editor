"""插件注入器 — 自动注入/检测/移除 GameBridgeServer 插件

将 TCP 桥接服务器插件注入到 NW.js / RPG Maker MV 游戏中，
使外部工具能够实时读写游戏内存。无需 SDK 构建。
"""

import os
import shutil

from core.game_detector import GameInfo, detect_game

# ── 插件内容 ──────────────────────────────────────────

PLUGIN_NAME = "GameBridgeServer"
PLUGIN_FILENAME = "GameBridgeServer.js"
PLUGIN_PORT = 19999

PLUGIN_SOURCE = r"""/*:
 * @plugindesc 游戏数据桥接服务器 - 允许外部工具通过 TCP 读写游戏内存
 * @author GameTool
 *
 * @help
 * 启动 TCP 服务器监听 localhost:{port}
 * 支持命令（每行一条，以 \n 结尾）:
 *   get_state      - 返回完整 JSON 游戏状态
 *   set_gold N     - 设置金币为 N
 *   set_switch ID V - 设置开关 (V=0/1)
 *   set_variable ID V - 设置变量
 *   set_hp ID V    - 设置角色 HP
 *   set_mp ID V    - 设置角色 MP
 *   set_item ID V  - 设置物品数量
 *   close          - 关闭连接
 */

(function() {{
  if (typeof require === 'undefined') return;
  var net = require('net');
  var PORT = {port};
  var server = null;

  function startServer() {{
    server = net.createServer(function(socket) {{
      var buffer = '';
      socket.on('data', function(data) {{
        buffer += data.toString('utf8');
        var lines = buffer.split('\n');
        buffer = lines.pop();
        for (var i = 0; i < lines.length; i++) {{
          var line = lines[i].trim();
          if (!line) continue;
          try {{
            var response = handleCommand(line);
            if (response !== null) socket.write(response + '\n');
          }} catch (e) {{ socket.write('ERROR:' + e.message + '\n'); }}
          if (line === 'close') {{ socket.end(); return; }}
        }}
      }});
      socket.on('error', function() {{}});
    }});
    server.on('error', function(e) {{}});
    server.listen(PORT, '127.0.0.1', function() {{
      console.log('[GameBridge] TCP 服务器已启动: localhost:' + PORT);
    }});
  }}

  function handleCommand(cmd) {{
    var parts = cmd.split(' ');
    var action = parts[0];
    switch (action) {{
      case 'get_state': return getFullState();
      case 'set_gold':
        $gameParty._gold = Math.max(0, parseInt(parts[1]) || 0);
        return 'OK:gold=' + $gameParty._gold;
      case 'set_switch':
        var sid = parseInt(parts[1]);
        var sv = parts[2] === '1' || parts[2] === 'true';
        $gameSwitches.setValue(sid, sv);
        return 'OK:switch_' + sid + '=' + $gameSwitches.value(sid);
      case 'set_variable':
        var vid = parseInt(parts[1]);
        var vv = parseInt(parts[2]) || 0;
        $gameVariables.setValue(vid, vv);
        return 'OK:var_' + vid + '=' + $gameVariables.value(vid);
      case 'set_hp':
        var aid = parseInt(parts[1]);
        var hv = parseInt(parts[2]) || 0;
        var a = $gameActors.actor(aid);
        if (a) {{ a._hp = Math.max(0, hv); a.refresh(); return 'OK:hp_' + aid + '=' + a._hp; }}
        return 'ERROR:actor_not_found';
      case 'set_mp':
        var aid2 = parseInt(parts[1]);
        var mv = parseInt(parts[2]) || 0;
        var a2 = $gameActors.actor(aid2);
        if (a2) {{ a2._mp = Math.max(0, mv); a2.refresh(); return 'OK:mp_' + aid2 + '=' + a2._mp; }}
        return 'ERROR:actor_not_found';
      case 'set_item':
        var iid = parseInt(parts[1]);
        var iv = parseInt(parts[2]) || 0;
        $gameParty._items[iid] = Math.max(0, iv);
        return 'OK:item_' + iid + '=' + iv;
      case 'close': return 'BYE';
      case 'ping': return 'PONG';
      default: return 'ERROR:unknown_command';
    }}
  }}

  function getFullState() {{
    try {{
      var state = {{
        gold: $gameParty._gold, steps: $gameParty._steps,
        partySize: $gameParty.members().length,
        party: $gameParty.members().map(function(a) {{
          return {{ id: a.actorId(), name: a.name(), level: a._level, hp: a._hp, mp: a._mp, mhp: a._mhp, mmp: a._mmp }};
        }}),
        switches: {{}}, variables: {{}}, items: [],
        mapName: $gameMap ? $gameMap.displayName() : '',
        playtime: $gameSystem ? $gameSystem.playtimeText() : '',
        saveCount: $gameSystem ? $gameSystem.saveCount() : 0
      }};
      for (var i = 1; i <= 300; i++) {{
        var v = $gameSwitches.value(i);
        if (v === true || $gameSwitches._data[i] === true) state.switches[i] = true;
      }}
      for (var i = 1; i <= 300; i++) {{
        var vv = $gameVariables.value(i);
        if (vv !== 0) state.variables[i] = vv;
      }}
      var itemIds = Object.keys($gameParty._items);
      for (var j = 0; j < itemIds.length; j++) {{
        var key = itemIds[j];
        if (key.indexOf('@') === 0) continue;
        var cnt = $gameParty._items[key];
        if (cnt && cnt > 0) {{
          var item = $dataItems[parseInt(key)];
          state.items.push({{ id: parseInt(key), name: item ? item.name : '#' + key, count: cnt }});
        }}
      }}
      return 'STATE:' + JSON.stringify(state);
    }} catch (e) {{ return 'ERROR:' + e.message; }}
  }}

  var _origSceneBootStart = Scene_Boot.prototype.start;
  Scene_Boot.prototype.start = function() {{
    _origSceneBootStart.call(this);
    setTimeout(startServer, 2000);
  }};
}})();
"""


# ── 检测 ──────────────────────────────────────────────

def is_plugin_installed(game_dir: str) -> bool:
  """检查 GameBridgeServer 插件是否已安装

  Args:
    game_dir: 游戏根目录

  Returns:
    True 如果插件已安装
  """
  js_dir = _get_plugins_dir(game_dir)
  if not js_dir:
    return False
  # 检查插件文件是否存在
  plugin_path = os.path.join(js_dir, PLUGIN_FILENAME)
  if not os.path.isfile(plugin_path):
    return False
  # 检查 plugins.js 中是否已注册
  plugins_js = _find_plugins_js(game_dir)
  if not plugins_js:
    return True  # 文件存在但无法检查注册，假设已安装
  try:
    content = open(plugins_js, "r", encoding="utf-8").read()
    return PLUGIN_NAME in content
  except Exception:
    return True


def _get_plugins_dir(game_dir: str) -> str:
  """获取插件目录"""
  for sub in ["www/js/plugins", "js/plugins"]:
    d = os.path.join(game_dir, sub)
    if os.path.isdir(d):
      return d
  return ""


def _find_plugins_js(game_dir: str) -> str:
  """查找 plugins.js 文件"""
  for sub in ["www/js/plugins.js", "js/plugins.js",
              "www/js/plugins_JP.js", "js/plugins_JP.js"]:
    p = os.path.join(game_dir, sub)
    if os.path.isfile(p):
      return p
  return ""


# ── 注入 ──────────────────────────────────────────────

def inject_plugin(game_dir: str, port: int = PLUGIN_PORT) -> bool:
  """将 GameBridgeServer 插件注入游戏

  步骤:
  1. 复制 GameBridgeServer.js 到 www/js/plugins/
  2. 在 plugins.js 中注册插件

  Args:
    game_dir: 游戏根目录
    port: TCP 服务器端口 (默认 19999)

  Returns:
    True 表示注入成功
  """
  if is_plugin_installed(game_dir):
    return True  # 已安装，跳过

  # 1. 复制插件文件
  js_dir = _get_plugins_dir(game_dir)
  if not js_dir:
    # 尝试创建目录
    for sub in ["www/js/plugins", "js/plugins"]:
      d = os.path.join(game_dir, sub)
      www_parent = os.path.dirname(os.path.dirname(d))
      if os.path.isdir(www_parent) or os.path.isdir(os.path.dirname(d)):
        os.makedirs(d, exist_ok=True)
        js_dir = d
        break
  if not js_dir:
    return False

  plugin_content = PLUGIN_SOURCE.format(port=port)
  plugin_path = os.path.join(js_dir, PLUGIN_FILENAME)
  try:
    with open(plugin_path, "w", encoding="utf-8") as f:
      f.write(plugin_content)
  except OSError:
    return False

  # 2. 注册到 plugins.js
  plugins_js = _find_plugins_js(game_dir)
  if not plugins_js:
    # 没有 plugins.js，这可能不是 RPG Maker 游戏
    # 但对于纯 NW.js 游戏，插件文件存在就够了
    return True

  try:
    with open(plugins_js, "r", encoding="utf-8") as f:
      content = f.read()

    # 检查是否已注册
    if PLUGIN_NAME in content:
      return True

    # 在最后一个 ]; 之前插入
    plugin_entry = (
      '{"name":"' + PLUGIN_NAME + '","status":true,'
      '"description":"游戏数据桥接服务器 - 允许外部工具实时读写游戏内存数据",'
      '"parameters":{}}'
    )
    if content.rstrip().endswith("];"):
      new_content = content.rstrip()[:-2] + ",\n" + plugin_entry + "\n];"
    elif "]" in content:
      # 找到最后一个 ] 并在此之前插入
      last_bracket = content.rfind("]")
      if last_bracket > 0:
        new_content = (content[:last_bracket] +
                       ",\n" + plugin_entry + "\n" +
                       content[last_bracket:])
      else:
        return False
    else:
      return False

    # 备份原文件
    backup_path = plugins_js + ".bak"
    shutil.copy2(plugins_js, backup_path)

    with open(plugins_js, "w", encoding="utf-8") as f:
      f.write(new_content)

    return True
  except Exception:
    return False


# ── 移除 ──────────────────────────────────────────────

def remove_plugin(game_dir: str) -> bool:
  """从游戏中移除 GameBridgeServer 插件

  Args:
    game_dir: 游戏根目录

  Returns:
    True 表示移除成功
  """
  # 删除插件文件
  js_dir = _get_plugins_dir(game_dir)
  if js_dir:
    plugin_path = os.path.join(js_dir, PLUGIN_FILENAME)
    if os.path.isfile(plugin_path):
      try:
        os.remove(plugin_path)
      except OSError:
        pass

  # 从 plugins.js 中移除注册
  plugins_js = _find_plugins_js(game_dir)
  if not plugins_js:
    return True

  try:
    with open(plugins_js, "r", encoding="utf-8") as f:
      content = f.read()

    if PLUGIN_NAME not in content:
      return True

    # 备份
    backup_path = plugins_js + ".bak"
    shutil.copy2(plugins_js, backup_path)

    # 移除包含 PLUGIN_NAME 的行
    import re
    # 匹配整个插件条目: {"name":"GameBridgeServer",...}
    pattern = r',?\s*\{"name":"' + re.escape(PLUGIN_NAME) + r'"[^}]*\}'
    new_content = re.sub(pattern, "", content)

    # 清理可能残留的 ,] 或 [,
    new_content = new_content.replace(",\n]", "\n]").replace("[\n,", "[")

    with open(plugins_js, "w", encoding="utf-8") as f:
      f.write(new_content)

    return True
  except Exception:
    return False


# ── 便捷函数 ──────────────────────────────────────────

def ensure_plugin(game_dir: str) -> tuple[bool, str]:
  """确保插件已安装，返回 (是否成功, 状态消息)

  如果游戏正在运行，提示需要重启。
  """
  if is_plugin_installed(game_dir):
    return True, "插件已安装"

  if inject_plugin(game_dir):
    return True, "插件已注入，请重启游戏生效"

  return False, "插件注入失败"


def get_plugin_status_text(game_dir: str) -> str:
  """获取插件状态的友好描述"""
  if not game_dir:
    return "未设置游戏目录"
  if is_plugin_installed(game_dir):
    return "✓ 已安装 (端口 {})".format(PLUGIN_PORT)
  return "✗ 未安装 — 点击「注入插件」安装"
