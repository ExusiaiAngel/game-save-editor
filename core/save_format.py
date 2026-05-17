"""存档格式抽象接口 — ISaveFormat

所有游戏存档格式处理器必须实现此接口。
编辑器（UI 层）只通过此接口操作存档，不直接依赖具体格式。
"""
from abc import ABC, abstractmethod
from dataclasses import dataclass, field


# ── 可修改字段数据类 ──────────────────────────────────

@dataclass
class ModifiableField:
    """一个可修改的存档字段（格式无关）"""
    category: str           # "gold", "switch", "variable", "actor", "item", "weapon", "armor", "self_switch"
    field_id: str           # 唯一标识符，如 "switch_12", "var_5", "actor_1_hp"
    display_name: str       # 显示名称
    item_id: int = 0        # 游戏内 ID
    field_type: str = "int" # "bool", "int", "str"
    save_value: object = None    # 存档中的值
    live_value: object = None    # 游戏实时值
    default_value: object = None
    min_val: int = 0
    max_val: int = 99999999
    description: str = ""
    dirty: bool = False     # 用户是否编辑过


@dataclass
class SaveSummary:
    """存档摘要（格式无关）"""
    gold: int = 0
    party_size: int = 0
    item_count: int = 0
    save_count: int = 0
    play_time: int = 0
    members: list[str] = field(default_factory=list)
    extra: dict = field(default_factory=dict)  # 格式特有信息


# ── 抽象接口 ──────────────────────────────────────────

class ISaveFormat(ABC):
    """游戏存档格式处理器接口

    所有具体格式（RPG Maker、Ren'Py、Unreal等）实现此接口。
    UI 层通过此接口操作存档，不关心底层格式细节。
    """

    # ── 元信息 ──

    @property
    @abstractmethod
    def name(self) -> str:
        """格式名称，如 "RPG Maker MV/MZ" """
        ...

    @property
    @abstractmethod
    def extensions(self) -> list[str]:
        """支持的文件扩展名，如 [".rpgsave", ".rmmzsave"] """
        ...

    @property
    def engine_type(self) -> str:
        """关联的引擎类型标识符（与 EngineType / bridge 的 engine_type 对应）

        子类覆盖此属性以声明所属引擎。
        常用值: "rpg_maker_mv", "rpg_maker_mz", "renpy", "unreal", "unity", "godot", "generic"
        """
        return "generic"

    @property
    def compatible_bridges(self) -> list[str]:
        """兼容的实时连接桥接类型列表

        子类覆盖以声明可用的实时数据桥接方式。
        常用值: "rpg_maker", "tcp", "cdp", "frida", "bepinex", "pymem"
        """
        return []

    @property
    def magic_bytes(self) -> bytes | None:
        """魔数字节签名，用于格式检测。无签名返回 None"""
        return None

    # ── 核心 I/O ──

    @abstractmethod
    def load(self, filepath: str) -> dict:
        """加载存档文件 → 原始数据字典"""
        ...

    @abstractmethod
    def save(self, filepath: str, data: dict) -> None:
        """保存数据字典 → 写回存档文件"""
        ...

    # ── 检测与发现 ──

    def detect(self, filepath: str) -> bool:
        """检测文件是否为此格式

        默认实现：按魔数 → 扩展名 → 内容试探的顺序检测。
        子类可覆盖以实现更精确的检测逻辑。
        """
        import os
        # 魔数检测
        if self.magic_bytes:
            try:
                with open(filepath, "rb") as f:
                    if f.read(len(self.magic_bytes)) == self.magic_bytes:
                        return True
            except OSError:
                return False
        # 扩展名检测
        ext = os.path.splitext(filepath)[1].lower()
        return ext in self.extensions

    @abstractmethod
    def find_data_dir(self, game_dir: str) -> str | None:
        """在游戏目录中查找数据文件目录

        RPG Maker: www/data/
        Ren'Py: game/
        其他引擎: 各自的数据目录
        """
        ...

    # ── 摘要 ──

    @abstractmethod
    def get_summary(self, data: dict) -> SaveSummary:
        """获取存档摘要信息"""
        ...

    # ── 字段扫描 ──

    @abstractmethod
    def scan_fields(self, data: dict, game_dir: str) -> list[ModifiableField]:
        """扫描存档中所有可修改字段

        结合游戏数据文件（如 RPG Maker 的 System.json）获取字段名称，
        生成统一的 ModifiableField 列表供 UI 展示。
        """
        ...

    # ── 字段写回 ──

    @abstractmethod
    def apply_field(self, data: dict, field: ModifiableField) -> None:
        """将单个字段的修改值写回数据字典

        UI 层修改 field.save_value 后，调用此方法将修改
        应用到实际的存档数据结构中。
        """
        ...
