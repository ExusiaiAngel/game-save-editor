"""格式注册表 — 多格式存档自动检测与处理器发现

FormatRegistry 管理所有已注册的格式处理器，提供自动检测流水线：
  1. 魔数匹配（最高优先级，精确）
  2. 扩展名匹配
  3. 内容试探（检测文件内部格式特征）
"""
import os
from core.save_format import ISaveFormat


class FormatRegistry:
    """格式处理器注册表

    使用方式:
        # 注册格式（应用启动时一次性注册）
        FormatRegistry.register(RpgMakerFormat)
        FormatRegistry.register(RenPyFormat)

        # 自动检测并获取处理器
        handler = FormatRegistry.detect("path/to/save.rpgsave")
        data = handler.load("path/to/save.rpgsave")
    """
    _handlers: list[type[ISaveFormat]] = []
    _ext_map: dict[str, type[ISaveFormat]] = {}
    _magic_map: list[tuple[bytes, type[ISaveFormat]]] = []

    # ── 注册 ──────────────────────────────────────────

    @classmethod
    def register(cls, handler_cls: type[ISaveFormat]):
        """注册一个格式处理器

        格式处理器必须实现 ISaveFormat 接口。
        注册时会自动按魔数长度降序排列（长魔数优先匹配）。
        """
        if handler_cls in cls._handlers:
            return
        cls._handlers.append(handler_cls)

        # 创建临时实例获取元信息
        handler = handler_cls()

        # 建立扩展名索引
        for ext in handler.extensions:
            ext_lower = ext.lower()
            if ext_lower not in cls._ext_map:
                cls._ext_map[ext_lower] = handler_cls

        # 建立魔数索引（长魔数优先）
        magic = handler.magic_bytes
        if magic:
            cls._magic_map.append((magic, handler_cls))
            cls._magic_map.sort(key=lambda x: -len(x[0]))

    @classmethod
    def unregister(cls, handler_cls: type[ISaveFormat]):
        """注销格式处理器"""
        if handler_cls in cls._handlers:
            cls._handlers.remove(handler_cls)
        # 重建索引
        cls._ext_map = {}
        cls._magic_map = []
        for h_cls in cls._handlers:
            handler = h_cls()
            for ext in handler.extensions:
                cls._ext_map[ext.lower()] = h_cls
            magic = handler.magic_bytes
            if magic:
                cls._magic_map.append((magic, h_cls))
                cls._magic_map.sort(key=lambda x: -len(x[0]))

    # ── 检测 ──────────────────────────────────────────

    @classmethod
    def detect(cls, filepath: str) -> ISaveFormat:
        """自动检测存档格式并返回对应处理器

        检测优先级：魔数 → 扩展名 → 内容试探

        Raises:
            ValueError: 无法识别的存档格式
        """
        if not os.path.isfile(filepath):
            raise FileNotFoundError(f"存档文件不存在: {filepath}")

        # 读取文件头部用于魔数检测
        header = b""
        try:
            with open(filepath, "rb") as f:
                header = f.read(256)
        except OSError:
            pass

        # 1. 魔数匹配
        for magic, handler_cls in cls._magic_map:
            if header.startswith(magic):
                return handler_cls()

        # 2. 扩展名匹配
        ext = os.path.splitext(filepath)[1].lower()
        if ext in cls._ext_map:
            return cls._ext_map[ext]()

        # 3. 内容试探（每个已注册格式的 detect() 方法）
        for handler_cls in cls._handlers:
            try:
                handler = handler_cls()
                if handler.detect(filepath):
                    return handler
            except Exception:
                continue

        raise ValueError(
            f"无法识别的存档格式: {os.path.basename(filepath)}\n"
            f"已注册格式: {cls.list_formats()}"
        )

    @classmethod
    def try_detect(cls, filepath: str) -> ISaveFormat | None:
        """尝试检测格式，失败返回 None（不抛出异常）"""
        try:
            return cls.detect(filepath)
        except (ValueError, FileNotFoundError):
            return None

    # ── 查询 ──────────────────────────────────────────

    @classmethod
    def get_supported_extensions(cls) -> list[str]:
        """获取所有支持的扩展名"""
        return list(cls._ext_map.keys())

    @classmethod
    def get_file_filter(cls) -> str:
        """获取文件对话框的过滤器字符串

        返回: "存档文件 (*.rpgsave *.rmmzsave *.sav);;所有文件 (*.*)"
        """
        extensions = cls.get_supported_extensions()
        if not extensions:
            return "所有文件 (*.*)"
        ext_patterns = " ".join(f"*{e}" for e in sorted(extensions))
        return f"游戏存档文件 ({ext_patterns});;所有文件 (*.*)"

    @classmethod
    def list_formats(cls) -> list[str]:
        """列出所有已注册的格式名称"""
        result = []
        for h_cls in cls._handlers:
            h = h_cls()
            exts = ", ".join(h.extensions)
            result.append(f"{h.name} ({exts})")
        return result

    @classmethod
    def count(cls) -> int:
        """已注册的格式数量"""
        return len(cls._handlers)
