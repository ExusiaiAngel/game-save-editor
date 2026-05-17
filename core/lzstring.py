"""LZ-String 压缩/解压 (封装 lzstring 包)

与 JS 版 lz-string 1.4.4 兼容，用于 RPG Maker MV 存档解析。
"""
from lzstring import LZString as _LZString

_deflater = _LZString()


def decompress_from_base64(data):
  return _deflater.decompressFromBase64(data)


def compress_to_base64(data):
  return _deflater.compressToBase64(data)


class LZString:
  @staticmethod
  def compressToBase64(s):
    return _deflater.compressToBase64(s)

  @staticmethod
  def decompressFromBase64(s):
    return _deflater.decompressFromBase64(s)
