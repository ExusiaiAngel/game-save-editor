"""RPG Maker MV/MZ 格式处理器

实现 ISaveFormat 接口，封装现有 rpgmv_save.py 的全部功能。
这是第一个格式处理器，也是从硬编码到插件架构的桥梁。
"""
import os
import json
from core.save_format import ISaveFormat, ModifiableField, SaveSummary
from core.rpgmv_save import (
    load_save, save_save,
    get_gold, set_gold,
    get_items, set_item_count,
    get_weapons, get_armors,
    get_party_info,
    set_actor_hp, set_actor_mp, set_actor_level,
    set_actor_max_hp, set_actor_max_mp,
    get_switches, set_switch,
    get_variables, set_variable,
    get_self_switches, set_self_switch,
    get_save_summary, clone_data,
)
from core.game_config import scan_game_directory


class RpgMakerFormat(ISaveFormat):
    """RPG Maker MV/MZ 存档格式处理器

    支持格式:
      - .rpgsave  (RPG Maker MV  — LZ-String 压缩的 Base64 JSON)
      - .rmmzsave (RPG Maker MZ — 纯 JSON 或 Base64 JSON)

    JsonEx @c/@a 压缩数组由底层 rpgmv_save 模块透明处理。
    """

    @property
    def name(self) -> str:
        return "RPG Maker MV/MZ"

    @property
    def extensions(self) -> list[str]:
        return [".rpgsave", ".rmmzsave"]

    @property
    def engine_type(self) -> str:
        return "rpg_maker_mv"

    @property
    def compatible_bridges(self) -> list[str]:
        return ["rpg_maker"]

    @property
    def magic_bytes(self) -> bytes | None:
        # RPG Maker 存档无固定魔数（Base64 文本开头）
        return None

    # ── 核心 I/O ──────────────────────────────────────

    def load(self, filepath: str) -> dict:
        return load_save(filepath)

    def save(self, filepath: str, data: dict) -> None:
        save_save(filepath, data)

    # ── 检测 ──────────────────────────────────────────

    def detect(self, filepath: str) -> bool:
        """检测文件是否为 RPG Maker MV/MZ 存档

        先检查扩展名，再通过内容试探验证。
        """
        ext = os.path.splitext(filepath)[1].lower()
        if ext not in self.extensions:
            return False
        try:
            load_save(filepath)
            return True
        except Exception:
            return False

    # ── 游戏目录发现 ────────────────────────────────

    def find_data_dir(self, game_dir: str) -> str | None:
        """查找 RPG Maker MV/MZ 数据目录 (www/data/)"""
        for sub in ["www/data", "data"]:
            d = os.path.join(game_dir, sub)
            if os.path.isdir(d) and os.path.isfile(os.path.join(d, "System.json")):
                return d
        return None

    # ── 摘要 ──────────────────────────────────────────

    def get_summary(self, data: dict) -> SaveSummary:
        raw = get_save_summary(data)
        return SaveSummary(
            gold=raw.get("gold", 0),
            party_size=raw.get("party_size", 0),
            item_count=raw.get("item_count", 0),
            save_count=raw.get("save_count", 0),
            play_time=raw.get("play_time", 0),
            members=raw.get("members", []),
        )

    # ── 字段扫描 ──────────────────────────────────────

    def scan_fields(self, data: dict, game_dir: str) -> list[ModifiableField]:
        """扫描存档中所有可修改字段

        委托给 game_data_scanner，返回统一字段列表。
        """
        from core.game_data_scanner import scan_all_modifiable
        result = scan_all_modifiable(game_dir=game_dir, save_data=data)
        # 将 game_data_scanner 的 ModifiableField 转换为接口版本
        fields = []
        for f in result.fields:
            fields.append(ModifiableField(
                category=f.category,
                field_id=f.field_id,
                display_name=f.display_name,
                item_id=f.item_id,
                field_type=f.field_type,
                save_value=f.save_value,
                live_value=f.live_value,
                default_value=f.default_value,
                min_val=f.min_val,
                max_val=f.max_val,
                description=f.description,
                dirty=f.dirty,
            ))
        return fields

    # ── 字段写回 ──────────────────────────────────────

    def apply_field(self, data: dict, field: ModifiableField) -> None:
        """将单个字段修改写回 RPG Maker 数据结构"""
        cat = field.category
        val = field.save_value

        if cat == "gold":
            set_gold(data, int(val))
        elif cat == "switch":
            set_switch(data, field.item_id, bool(val))
        elif cat == "variable":
            set_variable(data, field.item_id, int(val))
        elif cat == "actor":
            fid = field.field_id
            if fid.endswith("_hp"):
                set_actor_hp(data, field.item_id, int(val))
            elif fid.endswith("_mp"):
                set_actor_mp(data, field.item_id, int(val))
            elif fid.endswith("_level"):
                set_actor_level(data, field.item_id, int(val))
        elif cat == "item":
            set_item_count(data, field.item_id, int(val))
        elif cat == "self_switch":
            key = field.field_id[3:]  # 去掉 "ss_" 前缀
            set_self_switch(data, key, bool(val))
