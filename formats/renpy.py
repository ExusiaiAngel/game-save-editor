"""Ren'Py 格式处理器

Ren'Py (.save) 存档是 ZIP 归档，内含:
  - screenshot.png  — 存档截图
  - extra_info      — 存档显示名称 (UTF-8)
  - json            — 元数据 JSON (_save_name, _renpy_version, _ctime 等)
  - log             — pickle 序列化的游戏状态 (游戏特定，保留为不透明二进制)
  - renpy_version   — Ren'Py 版本号

本处理器可编辑元数据（存档名称、时间戳），完整保留游戏状态。
"""
import io
import json
import os
import struct
import zipfile
import time
from core.save_format import ISaveFormat, ModifiableField, SaveSummary


# ── 元数据键映射 ────────────────────────────────────

META_FIELD_MAP = {
    "_save_name": ("save_name", "存档名称", "str"),
    "_renpy_version": ("renpy_version", "Ren'Py 版本", "str"),
    "_game_runtime": ("game_runtime", "游戏运行时间", "str"),
    "_ctime": ("ctime", "创建时间戳", "int"),
}


class RenPyFormat(ISaveFormat):
    """Ren'Py 存档格式处理器

    文件格式: ZIP 归档，包含元数据 JSON + 游戏状态 pickle。
    元数据可编辑；游戏状态 (log) 以不透明二进制保留。
    """

    # ZIP 文件魔数
    MAGIC = b"PK\x03\x04"

    @property
    def name(self) -> str:
        return "Ren'Py"

    @property
    def extensions(self) -> list[str]:
        return [".save"]

    @property
    def engine_type(self) -> str:
        return "renpy"

    @property
    def compatible_bridges(self) -> list[str]:
        # Ren'Py 使用 Python pickle，暂无实时桥接方案
        return []

    @property
    def magic_bytes(self) -> bytes | None:
        return self.MAGIC

    # ── 检测 ────────────────────────────────────────

    def detect(self, filepath: str) -> bool:
        """检测是否为 Ren'Py 存档 (ZIP + 包含 json/log/screenshot)"""
        ext = os.path.splitext(filepath)[1].lower()
        # Ren'Py 存档通常命名为 1-1-LT1.save 等格式
        if ext != ".save":
            return False
        # 验证 ZIP 内部结构
        try:
            if not zipfile.is_zipfile(filepath):
                return False
            with zipfile.ZipFile(filepath, "r") as zf:
                names = zf.namelist()
                # Ren'Py 存档 ZIP 至少包含 json 或 log
                has_json = "json" in names
                has_log = "log" in names
                has_extra = "extra_info" in names
                return has_json and (has_log or has_extra)
        except Exception:
            return False

    # ── 核心 I/O ────────────────────────────────────

    def load(self, filepath: str) -> dict:
        """加载 Ren'Py 存档 → 内部数据字典

        返回结构:
          _format: "renpy"
          _meta: dict          — json 元数据
          _extra_info: str     — 存档显示名称
          _screenshot: bytes   — PNG 截图 (可能缺失)
          _log: bytes          — pickle 游戏状态
          _renpy_version: str  — 版本号
        """
        if not zipfile.is_zipfile(filepath):
            raise ValueError("不是有效的 Ren'Py 存档 (非 ZIP 格式)")

        data = {"_format": "renpy", "_meta": {}}

        with zipfile.ZipFile(filepath, "r") as zf:
            # 读取 json 元数据
            if "json" in zf.namelist():
                try:
                    meta = json.loads(zf.read("json").decode("utf-8"))
                    data["_meta"] = meta
                except (json.JSONDecodeError, UnicodeDecodeError):
                    pass

            # 读取 extra_info (存档显示名称)
            if "extra_info" in zf.namelist():
                try:
                    data["_extra_info"] = zf.read("extra_info").decode("utf-8")
                except UnicodeDecodeError:
                    pass

            # 读取 log (pickle 游戏状态 — 保留为不透明二进制)
            if "log" in zf.namelist():
                data["_log"] = zf.read("log")

            # 读取 screenshot
            if "screenshot.png" in zf.namelist():
                data["_screenshot"] = zf.read("screenshot.png")

            # 读取 renpy_version
            if "renpy_version" in zf.namelist():
                try:
                    data["_renpy_version"] = zf.read("renpy_version").decode("utf-8").strip()
                except UnicodeDecodeError:
                    pass

        return data

    def save(self, filepath: str, data: dict) -> None:
        """保存 Ren'Py 存档 — 重构 ZIP 归档"""
        import shutil
        from datetime import datetime

        # 备份
        if os.path.isfile(filepath):
            timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
            backup_path = f"{filepath}.{timestamp}.bak"
            shutil.copy2(filepath, backup_path)

        buf = io.BytesIO()
        with zipfile.ZipFile(buf, "w", zipfile.ZIP_DEFLATED) as zf:
            # 写回 json 元数据
            meta = data.get("_meta", {})
            if meta:
                zf.writestr("json", json.dumps(meta, ensure_ascii=False, indent=2))

            # 写回 extra_info
            extra = data.get("_extra_info", "")
            if extra:
                zf.writestr("extra_info", extra.encode("utf-8"))

            # 写回 screenshot
            screenshot = data.get("_screenshot")
            if screenshot:
                zf.writestr("screenshot.png", screenshot)

            # 写回 log (不透明二进制)
            log = data.get("_log")
            if log:
                zf.writestr("log", log)

            # 写回 renpy_version
            ver = data.get("_renpy_version", "")
            if ver:
                zf.writestr("renpy_version", ver.encode("utf-8"))

        os.makedirs(os.path.dirname(filepath) or ".", exist_ok=True)
        with open(filepath, "wb") as f:
            f.write(buf.getvalue())

    # ── 摘要 ────────────────────────────────────────

    def get_summary(self, data: dict) -> SaveSummary:
        meta = data.get("_meta", {})
        save_name = meta.get("_save_name", data.get("_extra_info", "?"))
        ctime = meta.get("_ctime", 0)

        # 格式化时间
        time_str = ""
        if ctime:
            try:
                t = time.localtime(ctime)
                time_str = time.strftime("%Y-%m-%d %H:%M", t)
            except Exception:
                time_str = str(ctime)

        return SaveSummary(
            gold=0,  # Ren'Py 存档无统一金币概念
            party_size=0,
            item_count=0,
            save_count=1,
            play_time=0,
            members=[],
            extra={
                "engine": "Ren'Py",
                "save_name": save_name,
                "version": meta.get("_renpy_version", ""),
                "timestamp": time_str,
                "has_screenshot": "_screenshot" in data,
            },
        )

    # ── 字段扫描 ────────────────────────────────────

    def scan_fields(self, data: dict, game_dir: str) -> list[ModifiableField]:
        """扫描 Ren'Py 存档中可编辑的元数据字段"""
        fields = []
        meta = data.get("_meta", {})

        for meta_key, (field_key, display_name, field_type) in META_FIELD_MAP.items():
            value = meta.get(meta_key)
            if value is None:
                continue

            f = ModifiableField(
                category="renpy_meta",
                field_id=f"renpy_{field_key}",
                display_name=display_name,
                item_id=0,
                field_type=field_type,
                save_value=value,
                min_val=0 if field_type == "int" else 0,
                max_val=9999999999 if field_type == "int" else 1,
                description=f"Ren'Py 元数据: {meta_key}",
            )
            fields.append(f)

        # 添加 extra_info 字段
        extra = data.get("_extra_info", "")
        if extra:
            fields.append(ModifiableField(
                category="renpy_meta",
                field_id="renpy_extra_info",
                display_name="存档显示名",
                item_id=0,
                field_type="str",
                save_value=extra,
                description="游戏中显示的存档名称",
            ))

        # 添加存档信息摘要字段（只读参考）
        ctime = meta.get("_ctime", 0)
        if ctime:
            try:
                t = time.localtime(ctime)
                time_str = time.strftime("%Y-%m-%d %H:%M:%S", t)
            except Exception:
                time_str = str(ctime)
            fields.append(ModifiableField(
                category="renpy_meta",
                field_id="renpy_time_display",
                display_name="存档时间",
                item_id=0,
                field_type="str",
                save_value=time_str,
                description="存档创建时间 (只读参考，修改请编辑「创建时间戳」字段)",
            ))

        return fields

    # ── 字段写回 ────────────────────────────────────

    def apply_field(self, data: dict, field: ModifiableField) -> None:
        """将字段修改写回 Ren'Py 存档数据

        元数据字段写入 _meta 字典；extra_info 单独写入。
        """
        fid = field.field_id

        if fid == "renpy_extra_info":
            data["_extra_info"] = str(field.save_value)
            return

        if fid == "renpy_time_display":
            # 只读参考字段，不写回
            return

        # 映射 field_id → _meta 键
        reverse_map = {
            "renpy_save_name": "_save_name",
            "renpy_renpy_version": "_renpy_version",
            "renpy_game_runtime": "_game_runtime",
            "renpy_ctime": "_ctime",
        }
        meta_key = reverse_map.get(fid)
        if meta_key and "_meta" in data:
            val = field.save_value
            if field.field_type == "int":
                val = int(val)
            data["_meta"][meta_key] = val

    # ── 游戏目录发现 ────────────────────────────────

    def find_data_dir(self, game_dir: str) -> str | None:
        """查找 Ren'Py 游戏数据目录 (game/saves/)"""
        for sub in ["game/saves", "game/save", "saves"]:
            d = os.path.join(game_dir, sub)
            if os.path.isdir(d):
                return d
        return None
