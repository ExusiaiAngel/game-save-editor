@echo off
echo === GameSaveEditor Build ===
echo.

echo Cleaning old output...
if exist "dist\" rd /s /q "dist"
mkdir dist

echo Building release (GUI)...
cargo build --release -p game-tool-gui
if %errorlevel% neq 0 (
    echo BUILD FAILED
    pause
    exit /b 1
)

echo Copying binary...
copy /Y "target\release\GameSaveEditor.exe" "dist\GameSaveEditor.exe"

echo ===========================================
echo Build Complete!
echo Output: dist\GameSaveEditor.exe
echo Size:
dir "dist\GameSaveEditor.exe" | findstr "GameSaveEditor"
echo ===========================================
pause
