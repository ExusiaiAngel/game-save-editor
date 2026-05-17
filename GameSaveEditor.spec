# -*- mode: python ; coding: utf-8 -*-
from PyInstaller.utils.hooks import collect_data_files
import os

datas = []
datas += collect_data_files('PySide6')

# 包含 profiles 目录
profiles_dir = os.path.join(SPECPATH, 'profiles')
if os.path.isdir(profiles_dir):
    for f in os.listdir(profiles_dir):
        if f.endswith('.json'):
            src = os.path.join(profiles_dir, f)
            datas.append((src, 'profiles'))

hidden_imports = [
    'PySide6.QtCore', 'PySide6.QtWidgets', 'PySide6.QtGui',
    'core.rpgmv_save', 'core.lzstring',
    'core.save_format', 'core.format_registry', 'core.engine_profile',
    'core.game_bridge', 'core.game_data_scanner', 'core.game_config',
    'core.game_detector', 'core.engine_detect', 'core.game_profile',
    'core.bridge_backends', 'core.plugin_injector', 'core.dependency_check',
    'core.renpy_bridge', 'core.renpy_injector',
    'formats.rpgmaker', 'formats.renpy',
    'formats.unreal_gvas', 'formats.generic_json',
    'ui.main_window', 'ui.modify_panel',
]

a = Analysis(
    ['main.py'],
    pathex=[],
    binaries=[],
    datas=datas,
    hiddenimports=hidden_imports,
    hookspath=[],
    hooksconfig={},
    runtime_hooks=[],
    excludes=[],
    noarchive=False,
    optimize=0,
)
pyz = PYZ(a.pure)

exe = EXE(
    pyz,
    a.scripts,
    a.binaries,
    a.datas,
    [],
    name='GameSaveEditor',
    debug=False,
    bootloader_ignore_signals=False,
    strip=False,
    upx=True,
    upx_exclude=[],
    runtime_tmpdir=None,
    console=False,
    disable_windowed_traceback=False,
    argv_emulation=False,
    target_arch=None,
    codesign_identity=None,
    entitlements_file=None,
)
