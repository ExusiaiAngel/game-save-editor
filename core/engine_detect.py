"""游戏引擎自动检测器

通过进程模块列表、文件系统特征、内存特征识别游戏引擎类型。
支持: RPG Maker, Unity (Mono/IL2CPP), Unreal Engine, Godot, Source, 通用 NW.js
"""
import os
import ctypes
from ctypes import wintypes
from dataclasses import dataclass


# ── 引擎类型枚举 ──────────────────────────────────────

class EngineType:
  RPG_MAKER_MV = "rpg_maker_mv"
  RPG_MAKER_MZ = "rpg_maker_mz"
  UNITY_MONO = "unity_mono"
  UNITY_IL2CPP = "unity_il2cpp"
  UNREAL_4 = "unreal_4"
  UNREAL_5 = "unreal_5"
  GODOT = "godot"
  SOURCE = "source"
  NWJS = "nwjs"
  UNKNOWN = "unknown"


@dataclass
class EngineInfo:
  """检测到的引擎信息"""
  engine_type: str = EngineType.UNKNOWN
  engine_name: str = "未知引擎"
  game_dir: str = ""
  exe_path: str = ""
  is_64bit: bool = True
  modules: list[str] = None

  def __post_init__(self):
    if self.modules is None:
      self.modules = []


# ── 主检测函数 ─────────────────────────────────────────

def detect_engine(process_name: str = "", pid: int = 0,
                  game_dir: str = "") -> EngineInfo:
  """自动检测游戏引擎类型

  Args:
    process_name: 进程名（如 "Game.exe"）
    pid: 进程 ID
    game_dir: 游戏根目录

  Returns:
    EngineInfo 包含引擎类型和详细信息
  """
  info = EngineInfo(game_dir=game_dir)

  # 1. 从进程模块检测（最准确）
  if pid > 0:
    modules = _get_process_modules(pid)
    info.modules = modules
    result = _detect_by_modules(modules, info)
    if result:
      return result

  # 2. 从游戏目录文件检测
  if game_dir and os.path.isdir(game_dir):
    result = _detect_by_files(game_dir, info)
    if result:
      return result

  return info


def detect_engine_from_dir(game_dir: str) -> EngineInfo:
  """仅从游戏目录检测引擎"""
  return detect_engine(game_dir=game_dir)


def detect_engine_from_pid(pid: int) -> EngineInfo:
  """仅从进程 ID 检测引擎"""
  return detect_engine(pid=pid)


# ── 模块检测 ───────────────────────────────────────────

def _detect_by_modules(modules: list[str], info: EngineInfo) -> EngineInfo | None:
  """通过加载的 DLL/SO 列表识别引擎"""
  lower = [m.lower() for m in modules]

  # Unity IL2CPP
  if any("gameassembly.dll" in m for m in lower):
    info.engine_type = EngineType.UNITY_IL2CPP
    info.engine_name = "Unity (IL2CPP)"
    return info

  # Unity Mono
  if any(m in lower for m in ["mono.dll", "mono-2.0-bdwgc.dll"]):
    info.engine_type = EngineType.UNITY_MONO
    info.engine_name = "Unity (Mono)"
    return info

  # Unreal Engine
  if any("core.dll" in m for m in lower) and \
     any("coreuobject.dll" in m for m in lower):
    # 检测 UE4 还是 UE5
    if any("unrealengine5" in m or "ue5" in m for m in lower):
      info.engine_type = EngineType.UNREAL_5
      info.engine_name = "Unreal Engine 5"
    else:
      info.engine_type = EngineType.UNREAL_4
      info.engine_name = "Unreal Engine 4"
    return info

  # Source Engine
  if any(m in lower for m in ["engine.dll", "vstdlib.dll"]):
    info.engine_type = EngineType.SOURCE
    info.engine_name = "Source Engine"
    return info

  # NW.js (通用)
  if any("nw.dll" in m for m in lower) or any("nw_elf.dll" in m for m in lower):
    info.engine_type = EngineType.NWJS
    info.engine_name = "NW.js"
    return info

  # Godot
  if any("godot" in m for m in lower):
    info.engine_type = EngineType.GODOT
    info.engine_name = "Godot"
    return info

  return None


# ── 文件系统检测 ───────────────────────────────────────

def _is_rpg_maker_mv(game_dir: str) -> bool:
  """检测 RPG Maker MV"""
  return os.path.isfile(os.path.join(game_dir, "www", "data", "System.json"))

def _is_rpg_maker_mz(game_dir: str) -> bool:
  """检测 RPG Maker MZ"""
  return os.path.isfile(os.path.join(game_dir, "data", "System.json")) and \
         os.path.isfile(os.path.join(game_dir, "data", "Traits.json"))

def _detect_by_files(game_dir: str, info: EngineInfo) -> EngineInfo | None:
  """通过游戏目录文件结构识别引擎"""
  # RPG Maker MV
  if _is_rpg_maker_mv(game_dir):
    info.engine_type = EngineType.RPG_MAKER_MV
    info.engine_name = "RPG Maker MV"
    return info

  # RPG Maker MZ
  if _is_rpg_maker_mz(game_dir):
    info.engine_type = EngineType.RPG_MAKER_MZ
    info.engine_name = "RPG Maker MZ"
    return info

  # 通用 NW.js 游戏
  if os.path.isfile(os.path.join(game_dir, "nw.dll")) or \
     os.path.isfile(os.path.join(game_dir, "nw_elf.dll")):
    info.engine_type = EngineType.NWJS
    info.engine_name = "NW.js"
    return info

  # Unity — 查找 *_Data/Managed/ 或 *_Data/il2cpp_data/
  for item in os.listdir(game_dir):
    item_path = os.path.join(game_dir, item)
    if os.path.isdir(item_path) and item.endswith("_Data"):
      if os.path.isdir(os.path.join(item_path, "Managed")):
        info.engine_type = EngineType.UNITY_MONO
        info.engine_name = "Unity (Mono)"
        return info
      if os.path.isdir(os.path.join(item_path, "il2cpp_data")):
        info.engine_type = EngineType.UNITY_IL2CPP
        info.engine_name = "Unity (IL2CPP)"
        return info

  return None


# ── 进程模块枚举（Windows）─────────────────────────────

def _get_process_modules(pid: int) -> list[str]:
  """获取进程加载的模块列表（仅 Windows）"""
  try:
    kernel32 = ctypes.WinDLL('kernel32', use_last_error=True)

    CreateToolhelp32Snapshot = kernel32.CreateToolhelp32Snapshot
    CreateToolhelp32Snapshot.argtypes = [wintypes.DWORD, wintypes.DWORD]
    CreateToolhelp32Snapshot.restype = wintypes.HANDLE

    class MODULEENTRY32(ctypes.Structure):
      _fields_ = [
        ("dwSize", wintypes.DWORD),
        ("th32ModuleID", wintypes.DWORD),
        ("th32ProcessID", wintypes.DWORD),
        ("GlblcntUsage", wintypes.DWORD),
        ("ProccntUsage", wintypes.DWORD),
        ("modBaseAddr", ctypes.POINTER(ctypes.c_byte)),
        ("modBaseSize", wintypes.DWORD),
        ("hModule", wintypes.HANDLE),
        ("szModule", ctypes.c_char * 256),
        ("szExePath", ctypes.c_char * 260),
      ]

    Module32First = kernel32.Module32First
    Module32First.argtypes = [wintypes.HANDLE, ctypes.POINTER(MODULEENTRY32)]
    Module32First.restype = wintypes.BOOL

    Module32Next = kernel32.Module32Next
    Module32Next.argtypes = [wintypes.HANDLE, ctypes.POINTER(MODULEENTRY32)]
    Module32Next.restype = wintypes.BOOL

    CloseHandle = kernel32.CloseHandle
    CloseHandle.argtypes = [wintypes.HANDLE]
    CloseHandle.restype = wintypes.BOOL

    snapshot = CreateToolhelp32Snapshot(0x00000008, pid)
    if not snapshot or snapshot == wintypes.HANDLE(-1).value:
      return []

    me = MODULEENTRY32()
    me.dwSize = ctypes.sizeof(MODULEENTRY32)
    modules = []

    if Module32First(snapshot, ctypes.byref(me)):
      while True:
        try:
          name = me.szModule.decode('utf-8', errors='replace')
          path = me.szExePath.decode('utf-8', errors='replace')
          modules.append(name)
          modules.append(path)
        except Exception:
          pass
        if not Module32Next(snapshot, ctypes.byref(me)):
          break

    CloseHandle(snapshot)
    return modules
  except Exception:
    return []


# ── 便捷函数 ───────────────────────────────────────────

ENGINE_CONNECT_HINTS: dict[str, str] = {
  EngineType.RPG_MAKER_MV: "使用「注入插件」→ 重启游戏 → 「连接游戏」",
  EngineType.RPG_MAKER_MZ: "与 RPG Maker MV 相同",
  "renpy": "使用「注入插件」→ 重启游戏 → 「连接游戏」",
  EngineType.NWJS: "如果是 RPG Maker 游戏，使用 TCP 桥接；否则尝试 CDP 调试端口",
  EngineType.UNITY_MONO: "推荐安装 BepInEx + 自定义插件，或使用 Frida 注入",
  EngineType.UNITY_IL2CPP: "推荐使用 Frida + frida-il2cpp-bridge 注入",
  EngineType.UNREAL_4: "推荐使用 Frida + frida-ue4dump 注入",
  EngineType.UNREAL_5: "推荐使用 Frida + frida-ue4dump 注入（UE5 兼容）",
  EngineType.GODOT: "使用 Frida 通用注入",
  EngineType.SOURCE: "使用 pymem 外挂内存读写",
  EngineType.UNKNOWN: "尝试外挂内存扫描（pymem）",
}


def get_engine_connect_hint(engine_type: str) -> str:
  """获取引擎对应的连接建议"""
  return ENGINE_CONNECT_HINTS.get(engine_type, "未知引擎，请联系开发者")
