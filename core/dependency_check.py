"""引擎依赖检测模块

检测各引擎后端所需的 Python 库是否已安装，
返回结构化状态供 UI 展示。
"""
import importlib
from dataclasses import dataclass, field


@dataclass
class DepStatus:
  """单个依赖的状态"""
  name: str           # 包名，如 "frida"
  installed: bool     # 是否已安装
  version: str = ""   # 已安装版本
  install_cmd: str = ""  # 安装命令
  required_by: list[str] = field(default_factory=list)  # 需要此依赖的引擎


@dataclass
class EngineDepReport:
  """引擎依赖完整报告"""
  engine_name: str           # "RPG Maker MV/MZ"
  engine_type: str           # "rpg_maker"
  backend_class: str         # "TcpGameBridge"
  ready: bool                # 所有依赖就绪？
  deps: list[DepStatus] = field(default_factory=list)
  connect_hint: str = ""     # 连接提示


# ── 检测函数 ───────────────────────────────────────────

def _check_import(module_name: str) -> tuple[bool, str]:
  """检查模块是否可导入，返回 (已安装, 版本)"""
  try:
    mod = importlib.import_module(module_name)
    version = getattr(mod, "__version__", "")
    if not version:
      version = getattr(mod, "VERSION", "")
    if not version:
      try:
        from importlib.metadata import version as pkg_version
        version = pkg_version(module_name)
      except Exception:
        version = "已安装"
    return True, str(version)
  except ImportError:
    return False, ""


def check_all_dependencies() -> list[EngineDepReport]:
  """检测所有引擎后端的依赖状态

  Returns:
    按优先级排序的引擎状态报告列表
  """
  reports = []

  # ── RPG Maker MV/MZ (TCP 桥接) ──
  frida_ok, frida_ver = _check_import("frida")
  pymem_ok, pymem_ver = _check_import("pymem")

  # RPG Maker — 零额外依赖（内置 TCP）
  reports.append(EngineDepReport(
    engine_name="RPG Maker MV/MZ",
    engine_type="rpg_maker",
    backend_class="TcpGameBridge",
    ready=True,  # 纯内置，始终就绪
    deps=[
      DepStatus("socket", True, "内置", install_cmd="",
                required_by=["RPG Maker"]),
    ],
    connect_hint="注入插件 → 重启游戏 → 连接游戏",
  ))

  # Ren'Py — 零额外依赖（Python TCP）
  reports.append(EngineDepReport(
    engine_name="Ren'Py",
    engine_type="renpy",
    backend_class="RenPyBridge",
    ready=True,  # 纯内置，始终就绪
    deps=[
      DepStatus("socket", True, "内置", install_cmd="",
                required_by=["Ren'Py"]),
    ],
    connect_hint="注入插件 → 重启游戏 → 连接游戏 (localhost:19999)",
  ))

  # Chromium / NW.js (CDP WebSocket)
  ws_ok, ws_ver = _check_import("websocket")
  reports.append(EngineDepReport(
    engine_name="Chromium / NW.js",
    engine_type="chromium",
    backend_class="CdpGameBridge",
    ready=ws_ok,
    deps=[
      DepStatus("websocket-client", ws_ok, ws_ver,
                install_cmd="pip install websocket-client",
                required_by=["Chromium NW.js"]),
    ],
    connect_hint="以 --remote-debugging-port=9222 启动游戏 → 连接",
  ))

  # Unity Mono (Frida)
  reports.append(EngineDepReport(
    engine_name="Unity (Mono)",
    engine_type="unity_mono",
    backend_class="UnityMonoBridge",
    ready=frida_ok,
    deps=[
      DepStatus("frida", frida_ok, frida_ver,
                install_cmd="pip install frida frida-tools",
                required_by=["Unity Mono", "Unity IL2CPP", "Unreal Engine"]),
      DepStatus("frida-tools", frida_ok, "",
                install_cmd="pip install frida-tools",
                required_by=["Unity Mono"]),
    ],
    connect_hint="安装 Frida → 识别目标进程 → 自动注入",
  ))

  # Unity IL2CPP (Frida)
  reports.append(EngineDepReport(
    engine_name="Unity (IL2CPP)",
    engine_type="unity_il2cpp",
    backend_class="UnityIl2CppBridge",
    ready=frida_ok,
    deps=[
      DepStatus("frida", frida_ok, frida_ver,
                install_cmd="pip install frida frida-tools",
                required_by=["Unity Mono", "Unity IL2CPP", "Unreal Engine"]),
    ],
    connect_hint="安装 Frida → 识别目标进程 → 自动解析 IL2CPP 结构",
  ))

  # Unreal Engine (Frida)
  reports.append(EngineDepReport(
    engine_name="Unreal Engine 4/5",
    engine_type="unreal",
    backend_class="UnrealBridge",
    ready=frida_ok,
    deps=[
      DepStatus("frida", frida_ok, frida_ver,
                install_cmd="pip install frida frida-tools",
                required_by=["Unity Mono", "Unity IL2CPP", "Unreal Engine"]),
    ],
    connect_hint="安装 Frida → 自动定位 GNames/GObjects → 枚举 UObject",
  ))

  # 通用内存 (pymem)
  reports.append(EngineDepReport(
    engine_name="通用进程 (pymem)",
    engine_type="generic_memory",
    backend_class="GenericMemoryBridge",
    ready=pymem_ok,
    deps=[
      DepStatus("pymem", pymem_ok, pymem_ver,
                install_cmd="pip install pymem",
                required_by=["任意 Windows 进程"]),
    ],
    connect_hint="安装 pymem → 手动配置内存地址 → 直接读写",
  ))

  return reports


def get_summary(reports: list[EngineDepReport]) -> str:
  """生成人类可读的摘要"""
  ready = sum(1 for r in reports if r.ready)
  total = len(reports)
  return f"{ready}/{total} 引擎就绪"


def get_missing_deps(reports: list[EngineDepReport]) -> list[DepStatus]:
  """获取所有缺失的依赖（去重）"""
  seen = set()
  missing = []
  for report in reports:
    for dep in report.deps:
      if not dep.installed and dep.name not in seen:
        seen.add(dep.name)
        missing.append(dep)
  return missing


def get_install_commands(reports: list[EngineDepReport]) -> list[str]:
  """获取安装所有缺失依赖的命令列表"""
  missing = get_missing_deps(reports)
  return [d.install_cmd for d in missing if d.install_cmd]


def quick_check() -> dict[str, bool]:
  """快速检查所有可选依赖"""
  frida_ok, _ = _check_import("frida")
  pymem_ok, _ = _check_import("pymem")
  ws_ok, _ = _check_import("websocket")
  return {
    "frida": frida_ok,
    "pymem": pymem_ok,
    "websocket": ws_ok,
    "rpg_maker": True,  # 内置，始终可用
    "renpy": True,       # 内置，始终可用
  }
