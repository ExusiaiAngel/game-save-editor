"""游戏桥接模块 — 通用游戏实时连接架构

通过抽象接口 IGameBridge 统一多种连接后端:
- TCP 桥接 (NW.js / RPG Maker MV/MZ)
- CDP WebSocket (Chrome DevTools Protocol)
- Frida 动态插桩 (Unity / Unreal 等)
- 外挂内存读写 (pymem, 通用回退)
"""

import json
import logging
import socket
import urllib.request
import urllib.error
from abc import ABC, abstractmethod
from dataclasses import dataclass, field

import websocket

logger = logging.getLogger(__name__)

# CDP 消息递增 ID
_counter = [1]


def _next_id() -> int:
  _counter[0] += 1
  return _counter[0]


# ═══════════════════════════════════════════════════════════════
# 抽象接口层
# ═══════════════════════════════════════════════════════════════

@dataclass
class GameState:
  """统一的游戏状态快照，所有后端返回此格式"""
  engine: str = "unknown"
  gold: int = 0
  steps: int = 0
  party: list[dict] = field(default_factory=list)
  items: list[dict] = field(default_factory=list)
  weapons: list[dict] = field(default_factory=list)
  armors: list[dict] = field(default_factory=list)
  switches: dict[int, bool] = field(default_factory=dict)
  variables: dict[int, int] = field(default_factory=dict)
  self_switches: dict[str, bool] = field(default_factory=dict)
  map_name: str = ""
  play_time: str = ""
  save_count: int = 0
  raw: dict | None = None


class IGameBridge(ABC):
  """游戏桥接器抽象接口

  所有连接后端（TCP、CDP、Frida、pymem）必须实现此接口。
  UI 层只依赖此接口，不感知具体实现。
  """

  @abstractmethod
  def connect(self) -> tuple[bool, str]:
    """建立连接。返回 (是否成功, 连接方式描述)"""
    ...

  @abstractmethod
  def disconnect(self) -> None:
    """断开连接，释放资源"""
    ...

  @property
  @abstractmethod
  def is_connected(self) -> bool:
    """是否已连接"""
    ...

  @abstractmethod
  def get_state(self) -> GameState | None:
    """获取完整游戏状态"""
    ...

  @abstractmethod
  def set_gold(self, amount: int) -> bool:
    """设置金币"""
    ...

  @abstractmethod
  def set_switch(self, sw_id: int, value: bool) -> bool:
    """设置开关"""
    ...

  @abstractmethod
  def set_variable(self, var_id: int, value: int) -> bool:
    """设置变量"""
    ...

  @abstractmethod
  def set_actor_hp(self, actor_id: int, hp: int) -> bool:
    """设置角色 HP"""
    ...

  @abstractmethod
  def set_actor_mp(self, actor_id: int, mp: int) -> bool:
    """设置角色 MP"""
    ...

  @abstractmethod
  def set_item_count(self, item_id: int, count: int) -> bool:
    """设置物品数量"""
    ...

  @staticmethod
  def priority() -> int:
    """返回优先级（数字越小越优先），子类可覆盖"""
    return 100

  @staticmethod
  def engine_name() -> str:
    """返回支持的引擎名称"""
    return "generic"


# ═══════════════════════════════════════════════════════════════
# 工厂模式 — 自动选择最佳连接方式
# ═══════════════════════════════════════════════════════════════

class BridgeFactory:
  """连接工厂，按优先级自动探测并创建最佳连接

  使用方式:
    factory = BridgeFactory()
    factory.register(TcpGameBridge)
    factory.register(CdpGameBridge)
    bridge = factory.create(game_dir="D:/MyGame")
  """

  def __init__(self):
    self._registry: list[type[IGameBridge]] = []

  def register(self, bridge_cls: type[IGameBridge]) -> None:
    """注册一个桥接器类（自动去重）"""
    if bridge_cls not in self._registry:
      self._registry.append(bridge_cls)
      self._registry.sort(key=lambda cls: cls.priority())

  def create(self, **kwargs) -> IGameBridge | None:
    """按优先级尝试创建连接，返回第一个成功的

    根据 bridge 类的 engine_name 传递对应参数：
      - rpg_maker: host, port (TCP)
      - chromium:  port (CDP)
    """
    for bridge_cls in self._registry:
      try:
        # 根据引擎类型构造合适的参数
        ename = bridge_cls.engine_name()
        if ename == "rpg_maker":
          bridge = bridge_cls(
            host=kwargs.get("host", "127.0.0.1"),
            port=kwargs.get("port", 19999),
          )
        elif ename == "chromium":
          bridge = bridge_cls(port=kwargs.get("cdp_port", 9222))
        else:
          bridge = bridge_cls(**kwargs)
        ok, _ = bridge.connect()
        if ok:
          logger.info("桥接成功: %s (引擎: %s)", bridge_cls.__name__, ename)
          return bridge
        bridge.disconnect()
      except Exception as e:
        logger.debug("桥接失败 %s: %s", bridge_cls.__name__, e)
    return None

  def create_all_candidates(self, **kwargs) -> list[IGameBridge]:
    """尝试所有已注册桥接器，返回所有候选（不自动连接）"""
    candidates = []
    for bridge_cls in self._registry:
      try:
        bridge = bridge_cls(**kwargs)
        candidates.append(bridge)
      except Exception:
        pass
    return candidates

  @property
  def registered_engines(self) -> list[str]:
    """返回所有已注册引擎名称"""
    return [cls.engine_name() for cls in self._registry]


# ── TCP 桥接连接（主方案，不需要 SDK 构建）────────────

class TcpGameBridge(IGameBridge):
  """通过 TCP Socket 连接游戏内 GameBridgeServer 插件"""

  def __init__(self, host: str = "127.0.0.1", port: int = 19999):
    self._host = host
    self._port = port
    self._sock: socket.socket | None = None
    self._connected = False

  @property
  def is_connected(self) -> bool:
    return self._connected

  @staticmethod
  def priority() -> int:
    return 10  # TCP 优先级最高（RPG Maker 专用）

  @staticmethod
  def engine_name() -> str:
    return "rpg_maker"

  def connect(self) -> tuple[bool, str]:
    """连接 TCP 服务器"""
    try:
      self._sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
      self._sock.settimeout(5)
      self._sock.connect((self._host, self._port))
      self._sock.settimeout(10)
      self._connected = True
      logger.info("TCP 桥接已连接: %s:%s", self._host, self._port)
      return True, "TCP"
    except Exception as e:
      logger.debug("TCP 连接失败: %s", e)
      self._sock = None
      self._connected = False
      return False, ""

  def disconnect(self) -> None:
    """断开连接"""
    if self._sock:
      try:
        self._send_cmd("close")
      except Exception:
        pass
      try:
        self._sock.close()
      except Exception:
        pass
      self._sock = None
    self._connected = False

  def check_available(self) -> bool:
    """检查 TCP 服务器是否可用（快速探测）"""
    try:
      s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
      s.settimeout(1)
      s.connect((self._host, self._port))
      s.close()
      return True
    except Exception:
      return False

  def _send_cmd(self, cmd: str) -> str | None:
    """发送命令并读取响应"""
    if not self._sock:
      return None
    try:
      self._sock.sendall((cmd + "\n").encode("utf-8"))
      # 读取响应直到遇到换行符
      data = b""
      while True:
        chunk = self._sock.recv(4096)
        if not chunk:
          break
        data += chunk
        if b"\n" in data:
          break
      return data.decode("utf-8").strip()
    except Exception as e:
      logger.warning("TCP 命令失败 [%s]: %s", cmd, e)
      return None

  def send_cmd(self, cmd: str) -> str | None:
    """发送命令并返回结果"""
    return self._send_cmd(cmd)

  def get_state(self) -> GameState | None:
    """获取完整游戏状态（返回统一格式）"""
    resp = self._send_cmd("get_state")
    if resp and resp.startswith("STATE:"):
      try:
        raw = json.loads(resp[6:])
        return GameState(
          engine="rpg_maker",
          gold=raw.get("gold", 0),
          steps=raw.get("steps", 0),
          party=raw.get("party", []),
          items=raw.get("items", []),
          switches={int(k): v for k, v in raw.get("switches", {}).items()},
          variables={int(k): v for k, v in raw.get("variables", {}).items()},
          map_name=raw.get("mapName", ""),
          play_time=raw.get("playtime", ""),
          save_count=raw.get("saveCount", 0),
          raw=raw,
        )
      except json.JSONDecodeError:
        return None
    return None

  def get_raw_state(self) -> dict | None:
    """获取原始游戏状态字典（兼容旧接口）"""
    resp = self._send_cmd("get_state")
    if resp and resp.startswith("STATE:"):
      try:
        return json.loads(resp[6:])
      except json.JSONDecodeError:
        return None
    return None

  def set_gold(self, amount: int) -> bool:
    resp = self._send_cmd(f"set_gold {amount}")
    return resp is not None and resp.startswith("OK")

  def set_switch(self, switch_id: int, value: bool) -> bool:
    v = 1 if value else 0
    resp = self._send_cmd(f"set_switch {switch_id} {v}")
    return resp is not None and resp.startswith("OK")

  def set_variable(self, var_id: int, value: int) -> bool:
    resp = self._send_cmd(f"set_variable {var_id} {value}")
    return resp is not None and resp.startswith("OK")

  def set_actor_hp(self, actor_id: int, hp: int) -> bool:
    resp = self._send_cmd(f"set_hp {actor_id} {hp}")
    return resp is not None and resp.startswith("OK")

  def set_actor_mp(self, actor_id: int, mp: int) -> bool:
    resp = self._send_cmd(f"set_mp {actor_id} {mp}")
    return resp is not None and resp.startswith("OK")

  def set_item_count(self, item_id: int, count: int) -> bool:
    resp = self._send_cmd(f"set_item {item_id} {count}")
    return resp is not None and resp.startswith("OK")

  def ping(self) -> bool:
    resp = self._send_cmd("ping")
    return resp == "PONG"


def auto_connect_tcp(port: int = 19999) -> TcpGameBridge | None:
  """一键 TCP 连接：检测 → 连接 → 验证

  Returns:
    TcpGameBridge 实例，失败返回 None
  """
  bridge = TcpGameBridge(port=port)
  if not bridge.check_available():
    logger.debug("TCP 桥接端口 %s 不可用", port)
    return None
  ok, _ = bridge.connect()
  if not ok:
    return None
  if not bridge.ping():
    bridge.disconnect()
    return None
  return bridge


# ── CDP WebSocket 桥接（回退方案）─────────────────────

class CdpGameBridge(IGameBridge):
  """通过 Chrome DevTools Protocol WebSocket 连接游戏

  用于以 --remote-debugging-port 启动的 NW.js/Chromium 游戏。
  """

  def __init__(self, port: int = 9222):
    self._port = port
    self._ws = None
    self._connected = False

  @property
  def is_connected(self) -> bool:
    return self._connected

  @staticmethod
  def priority() -> int:
    return 20  # CDP 为回退方案

  @staticmethod
  def engine_name() -> str:
    return "chromium"

  def connect(self) -> tuple[bool, str]:
    self.disconnect()
    if not check_debug_port(self._port):
      return False, ""
    ws = connect_to_game(self._port)
    if ws:
      self._ws = ws
      self._connected = True
      return True, "CDP"
    return False, ""

  def disconnect(self) -> None:
    if self._ws:
      try:
        disconnect(self._ws)
      except Exception:
        pass
      self._ws = None
    self._connected = False

  def get_state(self) -> GameState | None:
    if not self._ws:
      return None
    try:
      state = read_all_game_state(self._ws)
      basic = state.get("basic", {})
      return GameState(
        engine="chromium",
        gold=basic.get("gold", 0),
        steps=basic.get("steps", 0),
        party=basic.get("party", []),
        items=basic.get("items", []),
        weapons=basic.get("weapons", []),
        armors=basic.get("armors", []),
        switches=state.get("switches", {}),
        variables=state.get("variables", {}),
        self_switches=state.get("selfSwitches", {}),
        map_name=basic.get("mapName", ""),
        play_time=basic.get("playtime", ""),
        save_count=basic.get("saveCount", 0),
        raw=state,
      )
    except Exception:
      return None

  def set_gold(self, amount: int) -> bool:
    return write_game_gold(self._ws, amount) if self._ws else False

  def set_switch(self, sw_id: int, value: bool) -> bool:
    return write_game_switch(self._ws, sw_id, value) if self._ws else False

  def set_variable(self, var_id: int, value: int) -> bool:
    return write_game_variable(self._ws, var_id, value) if self._ws else False

  def set_actor_hp(self, actor_id: int, hp: int) -> bool:
    return write_game_actor_hp(self._ws, actor_id, hp) if self._ws else False

  def set_actor_mp(self, actor_id: int, mp: int) -> bool:
    return write_game_actor_mp(self._ws, actor_id, mp) if self._ws else False

  def set_item_count(self, item_id: int, count: int) -> bool:
    return write_game_item_count(self._ws, item_id, count) if self._ws else False


# ── CDP 工具函数（保留，用于支持 SDK 构建的游戏）───────

def check_debug_port(port: int = 9222) -> bool:
  """检查指定端口是否有调试目标（游戏是否以调试模式运行）

  Args:
    port: 调试端口号，默认 9222

  Returns:
    True 表示找到至少一个调试目标
  """
  try:
    url = f"http://localhost:{port}/json"
    req = urllib.request.Request(url, method="GET")
    with urllib.request.urlopen(req, timeout=3) as resp:
      body = resp.read().decode("utf-8")
      targets = json.loads(body)
      return len(targets) > 0
  except (urllib.error.URLError, ConnectionRefusedError, OSError,
          TimeoutError, json.JSONDecodeError) as e:
    logger.debug("检查调试端口 %s 失败: %s", port, e)
    return False


def get_game_target(port: int = 9222) -> dict | None:
  """获取游戏页面的调试目标信息

  Args:
    port: 调试端口号，默认 9222

  Returns:
    包含 webSocketDebuggerUrl 等字段的 dict，如果未找到则返回 None
  """
  try:
    url = f"http://localhost:{port}/json"
    req = urllib.request.Request(url, method="GET")
    with urllib.request.urlopen(req, timeout=3) as resp:
      body = resp.read().decode("utf-8")
      targets = json.loads(body)
  except Exception as e:
    logger.debug("获取调试目标失败: %s", e)
    return None

  for target in targets:
    if target.get("type") == "page":
      return target

  return None


def connect_to_game(port: int = 9222) -> websocket.WebSocket | None:
  """通过 CDP WebSocket 连接到游戏页面

  Args:
    port: 调试端口号，默认 9222

  Returns:
    WebSocket 连接对象，失败返回 None
  """
  target = get_game_target(port)
  if target is None:
    logger.warning("未找到 game 页面调试目标")
    return None

  ws_url = target.get("webSocketDebuggerUrl")
  if not ws_url:
    logger.warning("调试目标中缺少 webSocketDebuggerUrl")
    return None

  try:
    ws = websocket.create_connection(ws_url, timeout=5)
    logger.info("已连接到游戏 WebSocket: %s", ws_url)
    return ws
  except Exception as e:
    logger.warning("WebSocket 连接失败: %s", e)
    return None


def execute_js(ws: websocket.WebSocket, expression: str) -> dict:
  """通过 CDP Runtime.evaluate 在游戏上下文中执行 JavaScript

  Args:
    ws: WebSocket 连接对象
    expression: 要执行的 JavaScript 表达式

  Returns:
    解析后的 JSON 结果字典
  """
  msg_id = _next_id()
  msg = json.dumps({
    "id": msg_id,
    "method": "Runtime.evaluate",
    "params": {
      "expression": expression,
      "returnByValue": True,
    },
  })
  ws.send(msg)

  # 读取响应，跳过非结果消息（如事件通知）
  try:
    while True:
      raw = ws.recv()
      response = json.loads(raw)
      if "result" in response and response.get("id") == msg_id:
        result = response["result"]
        # CDP 返回的 result.value 可能是 JSON 字符串或直接的对象
        val = result.get("result", {}).get("value")
        if isinstance(val, str):
          return json.loads(val)
        elif isinstance(val, dict):
          return val
        return {}
  except Exception as e:
    logger.warning("执行 JS 失败: %s", e)
    raise


def execute_js_raw(ws: websocket.WebSocket, expression: str) -> str | None:
  """执行 JavaScript 并返回原始结果字符串（不做 JSON 解析）

  Args:
    ws: WebSocket 连接对象
    expression: 要执行的 JavaScript 表达式

  Returns:
    原始结果字符串，失败返回 None
  """
  msg_id = _next_id()
  msg = json.dumps({
    "id": msg_id,
    "method": "Runtime.evaluate",
    "params": {
      "expression": expression,
      "returnByValue": True,
    },
  })
  ws.send(msg)

  try:
    while True:
      raw = ws.recv()
      response = json.loads(raw)
      if "result" in response and response.get("id") == msg_id:
        result = response["result"]
        val = result.get("result", {}).get("value")
        return str(val) if val is not None else None
  except Exception as e:
    logger.warning("执行 JS 失败: %s", e)
    return None


def write_game_value(ws: websocket.WebSocket, expression: str) -> bool:
  """在游戏上下文中执行 JavaScript 赋值语句，修改游戏内存数值

  Args:
    ws: WebSocket 连接对象
    expression: JavaScript 赋值表达式，如 "$gameParty._gold = 99999"

  Returns:
    True 表示执行成功
  """
  try:
    execute_js_raw(ws, expression)
    return True
  except Exception as e:
    logger.warning("写入游戏数值失败: %s", e)
    return False


# ── 游戏状态读取 ──────────────────────────────────────

def read_game_state(ws: websocket.WebSocket) -> dict:
  """读取基本游戏实时状态（金币、队伍、物品、地图等）

  一次性获取金币、队伍、物品、武器、防具、地图、游戏时间等信息。

  Args:
    ws: WebSocket 连接对象

  Returns:
    游戏状态字典
  """
  expression = (
    "JSON.stringify({"
    "gold: $gameParty._gold,"
    "steps: $gameParty._steps,"
    "partySize: $gameParty.members().length,"
    "party: $gameParty.members().map(function(a) {"
    "  return {id: a.actorId(), name: a.name(), level: a._level, hp: a._hp, mp: a._mp}"
    "}),"
    "items: $dataItems.filter(function(i) {"
    "  return i && $gameParty.numItems($dataItems[i.id]) > 0"
    "}).map(function(i) {"
    "  return {id: i.id, name: i.name, count: $gameParty.numItems($dataItems[i.id])}"
    "}),"
    "weapons: $dataWeapons.filter(function(w) {"
    "  return w && $gameParty.numItems($dataWeapons[w.id]) > 0"
    "}).map(function(w) {"
    "  return {id: w.id, name: w.name, count: $gameParty.numItems($dataWeapons[w.id])}"
    "}),"
    "armors: $dataArmors.filter(function(a) {"
    "  return a && $gameParty.numItems($dataArmors[a.id]) > 0"
    "}).map(function(a) {"
    "  return {id: a.id, name: a.name, count: $gameParty.numItems($dataArmors[a.id])}"
    "}),"
    "mapName: $gameMap.displayName(),"
    "playtime: $gameSystem.playtimeText(),"
    "saveCount: $gameSystem.saveCount()"
    "})"
  )
  return execute_js(ws, expression)


def read_game_switches(ws: websocket.WebSocket, max_count: int = 200) -> dict[int, bool]:
  """读取游戏中所有开关的当前状态

  Args:
    ws: WebSocket 连接对象
    max_count: 最大读取开关数量 (默认 200)

  Returns:
    开关字典 {id: bool}
  """
  expression = (
    "JSON.stringify(function(){"
    "  var result={};"
    f" for(var i=1;i<={max_count};i++){{"
    "    var v=$gameSwitches.value(i);"
    "    if(v!==false || $gameSwitches._data[i]===true) result[i]=v;"
    "  }"
    "  return result;"
    "})()"
  )
  try:
    return execute_js(ws, expression)
  except Exception:
    return {}


def read_game_variables(ws: websocket.WebSocket, max_count: int = 200) -> dict[int, int]:
  """读取游戏中所有变量的当前值

  Args:
    ws: WebSocket 连接对象
    max_count: 最大读取变量数量 (默认 200)

  Returns:
    变量字典 {id: int}
  """
  expression = (
    "JSON.stringify(function(){"
    "  var result={};"
    f" for(var i=1;i<={max_count};i++){{"
    "    var v=$gameVariables.value(i);"
    "    if(v!=0) result[i]=v;"
    "  }"
    "  return result;"
    "})()"
  )
  try:
    return execute_js(ws, expression)
  except Exception:
    return {}


def read_game_self_switches(ws: websocket.WebSocket) -> dict[str, bool]:
  """读取游戏中所有自开关状态

  Args:
    ws: WebSocket 连接对象

  Returns:
    自开关字典 {"mapId,eventId,switchKey": bool}
  """
  expression = (
    "JSON.stringify(function(){"
    "  var result={};"
    "  var data=$gameSelfSwitches._data;"
    "  for(var key in data){"
    "    if(data.hasOwnProperty(key) && data[key]===true) result[key]=true;"
    "  }"
    "  return result;"
    "})()"
  )
  try:
    return execute_js(ws, expression)
  except Exception:
    return {}


def read_all_game_state(ws: websocket.WebSocket) -> dict:
  """全面读取游戏状态：基础状态 + 开关 + 变量 + 自开关

  Args:
    ws: WebSocket 连接对象

  Returns:
    完整的游戏状态字典:
    {
      "basic": {...},       # 基础状态（金币、队伍、物品等）
      "switches": {...},    # 开关 {id: bool}
      "variables": {...},   # 变量 {id: int}
      "selfSwitches": {...} # 自开关 {key: bool}
    }
  """
  result = {"basic": {}, "switches": {}, "variables": {}, "selfSwitches": {}}
  try:
    result["basic"] = read_game_state(ws)
  except Exception as e:
    logger.warning("读取基础状态失败: %s", e)
  try:
    result["switches"] = read_game_switches(ws)
  except Exception as e:
    logger.warning("读取开关失败: %s", e)
  try:
    result["variables"] = read_game_variables(ws)
  except Exception as e:
    logger.warning("读取变量失败: %s", e)
  try:
    result["selfSwitches"] = read_game_self_switches(ws)
  except Exception as e:
    logger.warning("读取自开关失败: %s", e)
  return result


# ── 游戏内存写入 ──────────────────────────────────────

def write_game_gold(ws: websocket.WebSocket, amount: int) -> bool:
  """修改游戏中的金币"""
  return write_game_value(ws, f"$gameParty._gold = {int(amount)}")


def write_game_switch(ws: websocket.WebSocket, switch_id: int, value: bool) -> bool:
  """修改游戏中的开关状态"""
  val = "true" if value else "false"
  return write_game_value(ws, f"$gameSwitches.setValue({int(switch_id)}, {val})")


def write_game_variable(ws: websocket.WebSocket, var_id: int, value: int) -> bool:
  """修改游戏中的变量值"""
  return write_game_value(ws, f"$gameVariables.setValue({int(var_id)}, {int(value)})")


def write_game_actor_hp(ws: websocket.WebSocket, actor_id: int, hp: int) -> bool:
  """修改游戏中的角色 HP"""
  return write_game_value(ws, (
    f"var a=$gameActors.actor({int(actor_id)});"
    f"if(a){{a._hp={int(hp)};a.refresh();}}"
  ))


def write_game_actor_mp(ws: websocket.WebSocket, actor_id: int, mp: int) -> bool:
  """修改游戏中的角色 MP"""
  return write_game_value(ws, (
    f"var a=$gameActors.actor({int(actor_id)});"
    f"if(a){{a._mp={int(mp)};a.refresh();}}"
  ))


def write_game_item_count(ws: websocket.WebSocket, item_id: int, count: int) -> bool:
  """修改游戏中的物品数量"""
  return write_game_value(ws, (
    f"$gameParty._items[{int(item_id)}] = {int(count)}"
  ))


def write_game_weapon_count(ws: websocket.WebSocket, weapon_id: int, count: int) -> bool:
  """修改游戏中的武器数量"""
  return write_game_value(ws, (
    f"$gameParty._weapons[{int(weapon_id)}] = {int(count)}"
  ))


def write_game_armor_count(ws: websocket.WebSocket, armor_id: int, count: int) -> bool:
  """修改游戏中的防具数量"""
  return write_game_value(ws, (
    f"$gameParty._armors[{int(armor_id)}] = {int(count)}"
  ))


# ── 连接管理 ──────────────────────────────────────────

def auto_connect(port: int = 9222) -> tuple[websocket.WebSocket | None, dict | None]:
  """一键连接：检查端口 → 获取目标 → 连接 → 读取状态

  Args:
    port: 调试端口号，默认 9222

  Returns:
    (ws, state) 元组，任一环节失败返回 (None, None)
  """
  if not check_debug_port(port):
    logger.debug("未检测到调试端口 %s", port)
    return None, None

  ws = connect_to_game(port)
  if ws is None:
    return None, None

  try:
    state = read_game_state(ws)
    return ws, state
  except Exception as e:
    logger.warning("读取游戏状态失败: %s", e)
    disconnect(ws)
    return None, None


def disconnect(ws: websocket.WebSocket | None) -> None:
  """关闭 WebSocket 连接

  Args:
    ws: WebSocket 连接对象
  """
  if ws is not None:
    try:
      ws.close()
      logger.info("已断开游戏 WebSocket 连接")
    except Exception as e:
      logger.debug("关闭 WebSocket 时出错: %s", e)


# ── 统一连接管理器 ────────────────────────────────────

class GameConnection:
  """统一游戏连接管理器

  包装单个 IGameBridge 实例，提供统一的读写接口。
  支持自动回退：TCP → CDP → Frida → pymem。
  """

  def __init__(self, tcp_port: int = 19999, cdp_port: int = 9222):
    self._bridge: IGameBridge | None = None
    self._factory = BridgeFactory()
    # 注册内置后端
    self._factory.register(TcpGameBridge)
    self._factory.register(CdpGameBridge)
    self._tcp_port = tcp_port
    self._cdp_port = cdp_port

  # ── 属性 ──────────────────────────────────────────

  @property
  def is_connected(self) -> bool:
    return self._bridge is not None and self._bridge.is_connected

  @property
  def connection_type(self) -> str:
    if self._bridge:
      return self._bridge.engine_name()
    return ""

  @property
  def bridge(self) -> IGameBridge | None:
    """获取当前桥接器"""
    return self._bridge

  # ── 连接 ──────────────────────────────────────────

  def connect(self) -> tuple[bool, str]:
    """自动连接游戏，按优先级回退"""
    self.disconnect()
    bridge = self._factory.create(
      host="127.0.0.1", port=self._tcp_port,
      cdp_port=self._cdp_port,
    )
    if bridge:
      self._bridge = bridge
      return True, bridge.engine_name()
    return False, ""

  def disconnect(self) -> None:
    if self._bridge:
      try:
        self._bridge.disconnect()
      except Exception:
        pass
      self._bridge = None

  # ── 读取 ──────────────────────────────────────────

  def get_state(self) -> GameState | None:
    if self._bridge:
      return self._bridge.get_state()
    return None

  def get_all_state(self) -> dict:
    """获取原始游戏状态（兼容旧接口）"""
    state = self.get_state()
    if state and state.raw:
      return state.raw
    return {"basic": {}, "switches": {}, "variables": {}, "selfSwitches": {}}

  # ── 写入 ──────────────────────────────────────────

  def set_gold(self, amount: int) -> bool:
    return self._bridge.set_gold(amount) if self._bridge else False

  def set_switch(self, sw_id: int, value: bool) -> bool:
    return self._bridge.set_switch(sw_id, value) if self._bridge else False

  def set_variable(self, var_id: int, value: int) -> bool:
    return self._bridge.set_variable(var_id, value) if self._bridge else False

  def set_actor_hp(self, actor_id: int, hp: int) -> bool:
    return self._bridge.set_actor_hp(actor_id, hp) if self._bridge else False

  def set_actor_mp(self, actor_id: int, mp: int) -> bool:
    return self._bridge.set_actor_mp(actor_id, mp) if self._bridge else False

  def set_item_count(self, item_id: int, count: int) -> bool:
    return self._bridge.set_item_count(item_id, count) if self._bridge else False

  # ── 扩展 ──────────────────────────────────────────

  def register_backend(self, bridge_cls: type[IGameBridge]) -> None:
    """注册额外的连接后端"""
    self._factory.register(bridge_cls)

  @property
  def registered_engines(self) -> list[str]:
    """已注册的引擎名称列表"""
    return self._factory.registered_engines

  def register_all_known_backends(self) -> None:
    """注册所有已知引擎后端"""
    from core.bridge_backends import register_all_backends
    register_all_backends(self._factory)
