@echo off
chcp 65001 >nul
cd /d "%~dp0"
title 游戏存档编辑器
echo.
echo   ╔═══════════════════════════════╗
echo   ║     游戏存档编辑器           ║
echo   ║    Game Save Editor          ║
echo   ╚═══════════════════════════════╝
echo.
echo   支持格式: RPG Maker MV/MZ, Ren'Py, Unreal Engine, JSON
echo.
echo   正在启动...
python main.py
if errorlevel 1 (
    echo.
    echo   ❌ 启动失败！请检查 Python 和依赖是否安装:
    echo      pip install -r requirements.txt
    echo.
    pause
)
