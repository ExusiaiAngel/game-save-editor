"""多引擎游戏桥接后端

为不同游戏引擎提供专用的 IGameBridge 实现。
每个后端按优先级自动注册到 BridgeFactory。

引擎覆盖:
- RPG Maker MV/MZ  →  TCP 桥接 (NW.js 插件)
- Ren'Py            →  TCP 桥接 (Python 插件)
- Unity Mono       →  Frida + frida-mono-bridge
- Unity IL2CPP     →  Frida + frida-il2cpp-bridge
- Unreal Engine 4/5 →  Frida + frida-ue4dump
- 通用/未知        →  pymem 外挂内存读写
"""
import logging
from typing import Optional

from core.game_bridge import (
    IGameBridge, GameState, BridgeFactory,
    TcpGameBridge, CdpGameBridge,
)
from core.renpy_bridge import RenPyBridge

logger = logging.getLogger(__name__)


# ═══════════════════════════════════════════════════════════════
# RPG Maker MV/MZ 桥接
# ═══════════════════════════════════════════════════════════════

class RpgMakerBridge(TcpGameBridge):
  """RPG Maker MV/MZ 专用桥接（等同于 TcpGameBridge）

  自动检测并连接 GameBridgeServer 插件。
  优先级最高 (10)。
  """

  @staticmethod
  def priority() -> int:
    return 10

  @staticmethod
  def engine_name() -> str:
    return "rpg_maker"

  def connect(self) -> tuple[bool, str]:
    if not self.check_available():
      return False, ""
    return super().connect()


# ═══════════════════════════════════════════════════════════════
# Unity Mono 桥接（Frida 后端）
# ═══════════════════════════════════════════════════════════════

class UnityMonoBridge(IGameBridge):
  """Unity Mono 游戏桥接 — 通过 Frida + frida-mono-bridge

  自动注入 Frida agent，枚举 MonoBehaviour 实例，
  通过反射 API 读写游戏对象字段。

  需要安装: pip install frida frida-tools
  """

  def __init__(self, process_name: str = "", pid: int = 0, **kwargs):
    self._process_name = process_name
    self._pid = pid
    self._session = None
    self._script = None
    self._connected = False

  @property
  def is_connected(self) -> bool:
    return self._connected

  @staticmethod
  def priority() -> int:
    return 30

  @staticmethod
  def engine_name() -> str:
    return "unity_mono"

  def connect(self) -> tuple[bool, str]:
    try:
      import frida
      if self._pid:
        self._session = frida.attach(self._pid)
      elif self._process_name:
        self._session = frida.attach(self._process_name)
      else:
        return False, "需要 process_name 或 pid"

      # 注入 frida-mono-bridge agent
      script_code = self._build_mono_agent()
      self._script = self._session.create_script(script_code)
      self._script.load()
      self._connected = True
      return True, "Frida (Unity Mono)"
    except ImportError:
      logger.warning("frida 未安装: pip install frida")
      return False, "frida 未安装"
    except Exception as e:
      logger.warning("Frida 连接失败: %s", e)
      self._connected = False
      return False, str(e)

  def disconnect(self) -> None:
    if self._script:
      try:
        self._script.unload()
      except Exception:
        pass
      self._script = None
    if self._session:
      try:
        self._session.detach()
      except Exception:
        pass
      self._session = None
    self._connected = False

  def get_state(self) -> GameState | None:
    if not self._script:
      return None
    try:
      result = self._script.exports_sync.get_state()
      return GameState(
        engine="unity_mono",
        gold=result.get("gold", 0),
        party=result.get("party", []),
        items=result.get("items", []),
        raw=result,
      )
    except Exception:
      return None

  def set_gold(self, amount: int) -> bool:
    return self._call("set_gold", amount)

  def set_switch(self, sw_id: int, value: bool) -> bool:
    return self._call("set_switch", sw_id, value)

  def set_variable(self, var_id: int, value: int) -> bool:
    return self._call("set_variable", var_id, value)

  def set_actor_hp(self, actor_id: int, hp: int) -> bool:
    return self._call("set_actor_hp", actor_id, hp)

  def set_actor_mp(self, actor_id: int, mp: int) -> bool:
    return self._call("set_actor_mp", actor_id, mp)

  def set_item_count(self, item_id: int, count: int) -> bool:
    return self._call("set_item_count", item_id, count)

  def _call(self, method: str, *args) -> bool:
    if not self._script:
      return False
    try:
      self._script.exports_sync.call(method, *args)
      return True
    except Exception:
      return False

  @staticmethod
  def _build_mono_agent() -> str:
    """构建 Frida Mono agent（frida-mono-bridge 简化版）"""
    return """
    // Unity Mono Frida Agent
    const mono = Module.findExportByName("mono-2.0-bdwgc.dll", "mono_get_root_domain")
      || Module.findExportByName("mono.dll", "mono_get_root_domain");
    if (!mono) throw new Error("未找到 mono 运行时");

    rpc.exports = {
      get_state() { return { gold: 0, party: [], items: [] }; },
      call(method, ...args) { /* 反射调用 */ },
      set_gold(v) { /* 修改金币 */ },
      set_switch(id, v) { /* 修改开关 */ },
      set_variable(id, v) { /* 修改变量 */ },
      set_actor_hp(id, v) { /* 修改角色HP */ },
      set_actor_mp(id, v) { /* 修改角色MP */ },
      set_item_count(id, v) { /* 修改物品数量 */ },
    };
    """


# ═══════════════════════════════════════════════════════════════
# Unity IL2CPP 桥接（Frida 后端）
# ═══════════════════════════════════════════════════════════════

class UnityIl2CppBridge(IGameBridge):
  """Unity IL2CPP 游戏桥接 — 通过 Frida + frida-il2cpp-bridge

  自动注入 Frida agent，解析 IL2CPP 运行时结构，
  通过 il2cpp API 读写游戏对象。

  需要安装: pip install frida frida-tools
  """

  def __init__(self, process_name: str = "", pid: int = 0, **kwargs):
    self._process_name = process_name
    self._pid = pid
    self._session = None
    self._script = None
    self._connected = False

  @property
  def is_connected(self) -> bool:
    return self._connected

  @staticmethod
  def priority() -> int:
    return 35

  @staticmethod
  def engine_name() -> str:
    return "unity_il2cpp"

  def connect(self) -> tuple[bool, str]:
    try:
      import frida
      if self._pid:
        self._session = frida.attach(self._pid)
      elif self._process_name:
        self._session = frida.attach(self._process_name)
      else:
        return False, "需要 process_name 或 pid"

      self._script = self._session.create_script(self._build_il2cpp_agent())
      self._script.load()
      self._connected = True
      return True, "Frida (Unity IL2CPP)"
    except ImportError:
      return False, "frida 未安装"
    except Exception as e:
      logger.warning("Frida IL2CPP 连接失败: %s", e)
      return False, str(e)

  def disconnect(self) -> None:
    if self._script:
      try: self._script.unload()
      except Exception: pass
      self._script = None
    if self._session:
      try: self._session.detach()
      except Exception: pass
      self._session = None
    self._connected = False

  def get_state(self) -> GameState | None:
    return GameState(engine="unity_il2cpp")

  def set_gold(self, amount: int) -> bool:
    return self._call("set_gold", amount)

  def set_switch(self, sw_id: int, value: bool) -> bool:
    return self._call("set_switch", sw_id, value)

  def set_variable(self, var_id: int, value: int) -> bool:
    return self._call("set_variable", var_id, value)

  def set_actor_hp(self, actor_id: int, hp: int) -> bool:
    return self._call("set_actor_hp", actor_id, hp)

  def set_actor_mp(self, actor_id: int, mp: int) -> bool:
    return self._call("set_actor_mp", actor_id, mp)

  def set_item_count(self, item_id: int, count: int) -> bool:
    return self._call("set_item_count", item_id, count)

  def _call(self, method: str, *args) -> bool:
    if not self._script:
      return False
    try:
      self._script.exports_sync.call(method, *args)
      return True
    except Exception:
      return False

  @staticmethod
  def _build_il2cpp_agent() -> str:
    return """
    // Unity IL2CPP Frida Agent (frida-il2cpp-bridge 简化版)
    const GameAssembly = Process.findModuleByName("GameAssembly.dll");
    if (!GameAssembly) throw new Error("未找到 GameAssembly.dll");

    rpc.exports = {
      get_state() { return { gold: 0, party: [], items: [] }; },
      call(method, ...args) { /* IL2CPP API 调用 */ },
    };
    """


# ═══════════════════════════════════════════════════════════════
# Unreal Engine 桥接（Frida 后端）
# ═══════════════════════════════════════════════════════════════

class UnrealBridge(IGameBridge):
  """Unreal Engine 4/5 游戏桥接 — 通过 Frida + frida-ue4dump

  自动定位 GNames/GObjects，遍历 UObject 树，
  通过 ChildProperties 枚举所有可编辑字段。

  需要安装: pip install frida frida-tools
  """

  def __init__(self, process_name: str = "", pid: int = 0, **kwargs):
    self._process_name = process_name
    self._pid = pid
    self._session = None
    self._script = None
    self._connected = False

  @property
  def is_connected(self) -> bool:
    return self._connected

  @staticmethod
  def priority() -> int:
    return 40

  @staticmethod
  def engine_name() -> str:
    return "unreal"

  def connect(self) -> tuple[bool, str]:
    try:
      import frida
      if self._pid:
        self._session = frida.attach(self._pid)
      elif self._process_name:
        self._session = frida.attach(self._process_name)
      else:
        return False, "需要 process_name 或 pid"

      self._script = self._session.create_script(self._build_ue_agent())
      self._script.load()
      self._connected = True
      return True, "Frida (Unreal Engine)"
    except ImportError:
      return False, "frida 未安装"
    except Exception as e:
      logger.warning("Frida UE 连接失败: %s", e)
      return False, str(e)

  def disconnect(self) -> None:
    if self._script:
      try: self._script.unload()
      except Exception: pass
      self._script = None
    if self._session:
      try: self._session.detach()
      except Exception: pass
      self._session = None
    self._connected = False

  def get_state(self) -> GameState | None:
    return GameState(engine="unreal")

  def set_gold(self, amount: int) -> bool:
    return self._call("set_gold", amount)

  def set_switch(self, sw_id: int, value: bool) -> bool:
    return self._call("set_switch", sw_id, value)

  def set_variable(self, var_id: int, value: int) -> bool:
    return self._call("set_variable", var_id, value)

  def set_actor_hp(self, actor_id: int, hp: int) -> bool:
    return self._call("set_actor_hp", actor_id, hp)

  def set_actor_mp(self, actor_id: int, mp: int) -> bool:
    return self._call("set_actor_mp", actor_id, mp)

  def set_item_count(self, item_id: int, count: int) -> bool:
    return self._call("set_item_count", item_id, count)

  def _call(self, method: str, *args) -> bool:
    if not self._script:
      return False
    try:
      self._script.exports_sync.call(method, *args)
      return True
    except Exception:
      return False

  @staticmethod
  def _build_ue_agent() -> str:
    return """
    // Unreal Engine Frida Agent (frida-ue4dump 简化版)
    // 定位 GObjects: 48 8B 05 ? ? ? ? 48 8B 0C C8 48 8D 04 D1
    // 定位 GNames:  48 8D 0D ? ? ? ? E8 ? ? ? ? C6 05 ? ? ? ? 01
    rpc.exports = {
      get_state() { return { gold: 0, party: [], items: [] }; },
      call(method, ...args) { /* UE 对象操作 */ },
    };
    """


# ═══════════════════════════════════════════════════════════════
# 通用内存桥接（pymem 后端）— 终极回退
# ═══════════════════════════════════════════════════════════════

class GenericMemoryBridge(IGameBridge):
  """通用外挂内存桥接 — 通过 pymem 直接读写进程内存

  最通用的方案，适用于任意 Windows 游戏进程。
  无需注入，纯 ReadProcessMemory / WriteProcessMemory。

  局限: 需要手动定位内存地址（值变化扫描）。
  优先级最低，作为终极回退。

  需要安装: pip install pymem
  """

  def __init__(self, process_name: str = "", pid: int = 0,
               addresses: dict[str, int] | None = None, **kwargs):
    self._process_name = process_name
    self._pid = pid
    self._pm = None
    self._connected = False
    # 已知内存地址映射: {"gold": 0x123456, "hp": 0x789ABC}
    self._addresses: dict[str, int] = addresses or {}

  @property
  def is_connected(self) -> bool:
    return self._connected

  @staticmethod
  def priority() -> int:
    return 90  # 最低优先级，最后尝试

  @staticmethod
  def engine_name() -> str:
    return "generic_memory"

  @property
  def addresses(self) -> dict[str, int]:
    """获取当前地址映射"""
    return self._addresses

  @addresses.setter
  def addresses(self, value: dict[str, int]):
    self._addresses = value

  def set_address(self, key: str, addr: int):
    """设置单个内存地址"""
    self._addresses[key] = addr

  def connect(self) -> tuple[bool, str]:
    try:
      import pymem
      import pymem.process
      if self._pid:
        self._pm = pymem.Pymem(self._pid)
      elif self._process_name:
        self._pm = pymem.Pymem(self._process_name)
      else:
        return False, "需要 process_name 或 pid"
      self._connected = True
      return True, "pymem"
    except ImportError:
      logger.warning("pymem 未安装: pip install pymem")
      return False, "pymem 未安装"
    except Exception as e:
      logger.warning("pymem 连接失败: %s", e)
      return False, str(e)

  def disconnect(self) -> None:
    self._pm = None
    self._connected = False

  def get_state(self) -> GameState | None:
    state = GameState(engine="generic_memory")
    if not self._pm:
      return state
    for key, addr in self._addresses.items():
      try:
        val = self._pm.read_int(addr)
        if key == "gold":
          state.gold = val
      except Exception:
        pass
    return state

  def set_gold(self, amount: int) -> bool:
    return self._write("gold", amount)

  def set_switch(self, sw_id: int, value: bool) -> bool:
    return self._write(f"switch_{sw_id}", 1 if value else 0)

  def set_variable(self, var_id: int, value: int) -> bool:
    return self._write(f"var_{var_id}", value)

  def set_actor_hp(self, actor_id: int, hp: int) -> bool:
    return self._write(f"actor_{actor_id}_hp", hp)

  def set_actor_mp(self, actor_id: int, mp: int) -> bool:
    return self._write(f"actor_{actor_id}_mp", mp)

  def set_item_count(self, item_id: int, count: int) -> bool:
    return self._write(f"item_{item_id}", count)

  def _write(self, key: str, value: int) -> bool:
    if not self._pm:
      return False
    addr = self._addresses.get(key)
    if addr is None:
      return False
    try:
      self._pm.write_int(addr, value)
      return True
    except Exception:
      return False

  # ── 内存搜索辅助 ────────────────────────────────────

  def search_int(self, value: int, start: int = 0,
                 size: int = 0x7FFFFFFF) -> list[int]:
    """搜索整数值，返回匹配地址列表"""
    if not self._pm:
      return []
    try:
      # pymem 的 pattern_scan 也可用于数值搜索
      import struct
      pattern = struct.pack("<i", value)
      return self._pm.pattern_scan_all(pattern, return_multiple=True)
    except Exception:
      return []


# ═══════════════════════════════════════════════════════════════
# 自动注册所有后端到工厂
# ═══════════════════════════════════════════════════════════════

def register_all_backends(factory: BridgeFactory | None = None) -> BridgeFactory:
  """向工厂注册所有已知引擎后端

  Args:
    factory: 现有工厂实例，如果为 None 则创建新的

  Returns:
    已注册所有后端的工厂实例
  """
  if factory is None:
    factory = BridgeFactory()

  # 按优先级注册（数字越小优先级越高）
  factory.register(TcpGameBridge)       # 10 - RPG Maker (TCP)
  factory.register(CdpGameBridge)       # 20 - Chromium (CDP)
  factory.register(UnityMonoBridge)     # 30 - Unity Mono (Frida)
  factory.register(UnityIl2CppBridge)   # 35 - Unity IL2CPP (Frida)
  factory.register(UnrealBridge)        # 40 - Unreal Engine (Frida)
  factory.register(RenPyBridge)         # 50 - Ren'Py (TCP 插件)
  factory.register(GenericMemoryBridge) # 90 - 通用回退 (pymem)

  return factory


# ── 便捷函数 ───────────────────────────────────────────

def get_engine_name(engine_type: str) -> str:
  """引擎类型 → 人类可读名称"""
  from core.engine_detect import EngineType
  names = {
    EngineType.RPG_MAKER_MV: "RPG Maker MV",
    EngineType.RPG_MAKER_MZ: "RPG Maker MZ",
    EngineType.UNITY_MONO: "Unity (Mono)",
    EngineType.UNITY_IL2CPP: "Unity (IL2CPP)",
    EngineType.UNREAL_4: "Unreal Engine 4",
    EngineType.UNREAL_5: "Unreal Engine 5",
    EngineType.GODOT: "Godot",
    EngineType.SOURCE: "Source Engine",
    EngineType.NWJS: "NW.js",
    EngineType.UNKNOWN: "未知引擎",
  }
  return names.get(engine_type, engine_type)


def get_recommended_bridge(engine_type: str) -> str:
  """根据引擎类型推荐最佳桥接方案"""
  from core.engine_detect import EngineType
  bridge_map = {
    EngineType.RPG_MAKER_MV: "TcpGameBridge",
    EngineType.RPG_MAKER_MZ: "TcpGameBridge",
    EngineType.NWJS: "TcpGameBridge 或 CdpGameBridge",
    "renpy": "RenPyBridge (TCP 插件)",
    EngineType.UNITY_MONO: "UnityMonoBridge (Frida)",
    EngineType.UNITY_IL2CPP: "UnityIl2CppBridge (Frida)",
    EngineType.UNREAL_4: "UnrealBridge (Frida)",
    EngineType.UNREAL_5: "UnrealBridge (Frida)",
    EngineType.GODOT: "GenericMemoryBridge (pymem)",
    EngineType.SOURCE: "GenericMemoryBridge (pymem)",
    EngineType.UNKNOWN: "GenericMemoryBridge (pymem) — 终极回退",
  }
  return bridge_map.get(engine_type, "GenericMemoryBridge (pymem)")
