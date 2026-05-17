@echo off
chcp 65001 >nul
cd /d "%~dp0"
title 构建 GameSaveEditor.exe

echo.
echo   ╔═══════════════════════════════╗
echo   ║   构建 GameSaveEditor.exe     ║
echo   ╚═══════════════════════════════╝
echo.

REM 清理旧构建
if exist build rmdir /s /q build
if exist dist rmdir /s /q dist

echo   [1/2] 正在构建单文件 EXE...
pyinstaller ^
    --onefile ^
    --windowed ^
    --name GameSaveEditor ^
    --add-data "profiles;profiles" ^
    --hidden-import PySide6.QtCore ^
    --hidden-import PySide6.QtWidgets ^
    --hidden-import PySide6.QtGui ^
    --hidden-import core.rpgmv_save ^
    --hidden-import core.lzstring ^
    --hidden-import core.save_format ^
    --hidden-import core.format_registry ^
    --hidden-import core.engine_profile ^
    --hidden-import core.game_bridge ^
    --hidden-import core.game_data_scanner ^
    --hidden-import core.game_config ^
    --hidden-import core.game_detector ^
    --hidden-import core.engine_detect ^
    --hidden-import core.bridge_backends ^
    --hidden-import core.renpy_bridge ^
    --hidden-import core.renpy_injector ^
    --hidden-import core.plugin_injector ^
    --hidden-import core.dependency_check ^
    --hidden-import core.game_profile ^
    --hidden-import formats.rpgmaker ^
    --hidden-import formats.renpy ^
    --hidden-import formats.unreal_gvas ^
    --hidden-import formats.generic_json ^
    --hidden-import ui.main_window ^
    --hidden-import ui.modify_panel ^
    --collect-data PySide6 ^
    main.py

if errorlevel 1 (
    echo.
    echo   ❌ 构建失败！请检查上方错误信息
    echo.
    pause
    exit /b 1
)

echo.
echo   [2/2] 构建完成！
echo.
echo   ✅ 输出: dist\GameSaveEditor.exe
echo.
echo   双击 dist\GameSaveEditor.exe 即可启动
echo.
pause
