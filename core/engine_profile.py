"""引擎关联注册中心 — 松散耦合格式、桥接、引擎检测

通过 engine_type 字符串标识符将三套独立系统关联起来：
  格式处理器 (ISaveFormat) ─── engine_type ─── 实时桥接 (IGameBridge)
                       │                          │
                       └──── 引擎检测 ────────────┘

耦合方式: 字符串标识符映射（松耦合）
  - 格式通过 ISaveFormat.engine_type 声明所属引擎
  - 桥接通过 IGameBridge.engine_name() 声明支持的引擎
  - 注册中心通过 engine_type 将它们关联

解耦方式: 删除/替换注册项，不影响其他组件
"""
from dataclasses import dataclass, field
from core.save_format import ISaveFormat


@dataclass
class EngineProfile:
    """引擎关联档案 — 一个引擎的完整信息"""
    engine_type: str                              # "rpg_maker_mv", "unreal", ...
    engine_name: str                              # "RPG Maker MV", "Unreal Engine", ...
    bridge_types: list[str] = field(default_factory=list)   # 兼容的桥接类型
    format_handlers: list[type[ISaveFormat]] = field(default_factory=list)  # 兼容的格式处理器
    data_dir_patterns: list[str] = field(default_factory=list)   # 数据目录检测模式
    save_dir_patterns: list[str] = field(default_factory=list)   # 存档目录检测模式
    exe_patterns: list[str] = field(default_factory=list)        # 可执行文件检测模式


class EngineRegistry:
    """引擎关联注册中心

    所有组件通过 engine_type 字符串在此关联。
    任何组件可独立注册/注销，不影响其他组件。

    使用方式:
        # 格式处理器注册（自动）
        reg = EngineRegistry()
        reg.register_format(RpgMakerFormat)

        # 桥接注册
        reg.register_bridge("rpg_maker", "rpg_maker_mv")

        # 查询
        bridges = reg.get_bridges_for_format(".rpgsave")
        formats = reg.get_formats_for_engine("unreal")
    """
    def __init__(self):
        self._profiles: dict[str, EngineProfile] = {}
        self._ext_to_engine: dict[str, str] = {}       # .rpgsave → rpg_maker_mv
        self._bridge_to_engine: dict[str, list[str]] = {}  # rpg_maker → ["rpg_maker_mv"]

    # ── 注册 ──────────────────────────────────────────

    def register_format(self, handler_cls: type[ISaveFormat]):
        """注册格式处理器，自动关联到引擎类型"""
        handler = handler_cls()
        engine_type = handler.engine_type

        # 确保引擎档案存在
        if engine_type not in self._profiles:
            self._profiles[engine_type] = EngineProfile(
                engine_type=engine_type,
                engine_name=handler.name,
            )

        profile = self._profiles[engine_type]
        if handler_cls not in profile.format_handlers:
            profile.format_handlers.append(handler_cls)

        # 建立扩展名→引擎映射
        for ext in handler.extensions:
            self._ext_to_engine[ext.lower()] = engine_type

        # 建立桥接关联
        for bridge in handler.compatible_bridges:
            if bridge not in profile.bridge_types:
                profile.bridge_types.append(bridge)
            if bridge not in self._bridge_to_engine:
                self._bridge_to_engine[bridge] = []
            if engine_type not in self._bridge_to_engine[bridge]:
                self._bridge_to_engine[bridge].append(engine_type)

    def register_bridge(self, bridge_type: str, engine_type: str):
        """注册桥接类型与引擎的关联"""
        if engine_type not in self._profiles:
            self._profiles[engine_type] = EngineProfile(
                engine_type=engine_type,
                engine_name=engine_type,
            )
        if bridge_type not in self._profiles[engine_type].bridge_types:
            self._profiles[engine_type].bridge_types.append(bridge_type)

        if bridge_type not in self._bridge_to_engine:
            self._bridge_to_engine[bridge_type] = []
        if engine_type not in self._bridge_to_engine[bridge_type]:
            self._bridge_to_engine[bridge_type].append(engine_type)

    def register_profile(self, profile: EngineProfile):
        """直接注册完整的引擎档案"""
        self._profiles[profile.engine_type] = profile
        for handler_cls in profile.format_handlers:
            handler = handler_cls()
            for ext in handler.extensions:
                self._ext_to_engine[ext.lower()] = profile.engine_type
        for bridge in profile.bridge_types:
            if bridge not in self._bridge_to_engine:
                self._bridge_to_engine[bridge] = []
            if profile.engine_type not in self._bridge_to_engine[bridge]:
                self._bridge_to_engine[bridge].append(profile.engine_type)

    def setup_defaults(self):
        """注册预定义的引擎关联（应用启动时调用一次）"""
        from formats.rpgmaker import RpgMakerFormat
        from formats.renpy import RenPyFormat
        from formats.unreal_gvas import UnrealGVASFormat
        from formats.generic_json import GenericJsonFormat

        # 注册格式（自动关联引擎+桥接）
        self.register_format(RpgMakerFormat)
        self.register_format(RenPyFormat)
        self.register_format(UnrealGVASFormat)
        self.register_format(GenericJsonFormat)

        # 补充桥接关联（格式可能不知道所有桥接方式）
        self.register_bridge("rpg_maker", "rpg_maker_mv")
        self.register_bridge("rpg_maker", "rpg_maker_mz")
        self.register_bridge("renpy", "renpy")

        # 补充引擎档案的检测模式
        rpg = self._profiles.get("rpg_maker_mv")
        if rpg:
            rpg.engine_name = "RPG Maker MV/MZ"
            rpg.data_dir_patterns = ["www/data", "data"]
            rpg.save_dir_patterns = ["www/Save", "Save", "save"]
            rpg.exe_patterns = ["Game.exe", "game.exe", "nw.exe"]

        renpy_p = self._profiles.get("renpy")
        if renpy_p:
            renpy_p.engine_name = "Ren'Py"
            if "renpy" not in renpy_p.bridge_types:
                renpy_p.bridge_types.append("renpy")
            renpy_p.data_dir_patterns = ["game/saves", "game"]
            renpy_p.save_dir_patterns = ["game/saves", "saves"]
            renpy_p.exe_patterns = ["renpy.exe", "python.exe", "game.exe"]

        unreal = self._profiles.get("unreal")
        if unreal:
            unreal.engine_name = "Unreal Engine"
            unreal.data_dir_patterns = ["Saved/SaveGames", "Saved"]
            unreal.save_dir_patterns = ["Saved/SaveGames", "SaveGames"]
            unreal.exe_patterns = []

        renpy = self._profiles.get("renpy")
        if renpy:
            renpy.engine_name = "Ren'Py"
            renpy.data_dir_patterns = ["game/saves", "game"]
            renpy.save_dir_patterns = ["game/saves", "saves"]

    # ── 查询 ──────────────────────────────────────────

    def get_bridges_for_extension(self, ext: str) -> list[str]:
        """根据文件扩展名获取推荐的桥接类型列表"""
        engine_type = self._ext_to_engine.get(ext.lower())
        if engine_type and engine_type in self._profiles:
            return list(self._profiles[engine_type].bridge_types)
        return []

    def get_bridges_for_format(self, handler: ISaveFormat) -> list[str]:
        """根据格式处理器获取推荐的桥接类型"""
        return list(handler.compatible_bridges)

    def get_bridges_for_engine(self, engine_type: str) -> list[str]:
        """根据引擎类型获取兼容的桥接"""
        profile = self._profiles.get(engine_type)
        return list(profile.bridge_types) if profile else []

    def get_formats_for_engine(self, engine_type: str) -> list[type[ISaveFormat]]:
        """获取引擎兼容的格式处理器"""
        profile = self._profiles.get(engine_type)
        return list(profile.format_handlers) if profile else []

    def get_extensions_for_engine(self, engine_type: str) -> list[str]:
        """获取引擎对应的存档扩展名列表"""
        result = []
        for ext, eng in self._ext_to_engine.items():
            if eng == engine_type:
                result.append(ext)
        return result

    def get_engine_for_extension(self, ext: str) -> str | None:
        """根据扩展名获取引擎类型"""
        return self._ext_to_engine.get(ext.lower())

    def get_profile(self, engine_type: str) -> EngineProfile | None:
        """获取引擎档案"""
        return self._profiles.get(engine_type)

    def get_profile_for_extension(self, ext: str) -> EngineProfile | None:
        """根据扩展名获取引擎档案"""
        engine_type = self._ext_to_engine.get(ext.lower())
        return self._profiles.get(engine_type) if engine_type else None

    def list_engines(self) -> list[str]:
        """列出所有引擎类型"""
        return list(self._profiles.keys())

    def list_bridges(self) -> list[str]:
        """列出所有桥接类型"""
        return list(self._bridge_to_engine.keys())

    def count(self) -> int:
        """已注册的引擎数量"""
        return len(self._profiles)


# ── 全局单例 ──────────────────────────────────────

_engine_registry: EngineRegistry | None = None


def get_engine_registry() -> EngineRegistry:
    """获取全局引擎注册中心单例"""
    global _engine_registry
    if _engine_registry is None:
        _engine_registry = EngineRegistry()
        _engine_registry.setup_defaults()
    return _engine_registry
