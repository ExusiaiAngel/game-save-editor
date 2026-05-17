"""格式处理器包

每种游戏引擎的存档格式各实现一个 ISaveFormat 处理器。

使用方式:
    from formats.rpgmaker import RpgMakerFormat
    from core.format_registry import FormatRegistry
    FormatRegistry.register(RpgMakerFormat)
"""
