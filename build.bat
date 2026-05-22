@echo off
echo === GameSaveEditor Build ===
echo.

echo Cleaning old output...
if exist "dist\" rd /s /q "dist"
mkdir dist

echo Building release...
cargo build --release
if %errorlevel% neq 0 (
    echo BUILD FAILED
    pause
    exit /b 1
)

echo Copying binary...
copy /Y "target\release\GameSaveEditor.exe" "dist\GameSaveEditor.exe"
copy /Y "target\release\GameSaveEditor.pdb" "dist\GameSaveEditor.pdb" 2>nul

echo ===========================================
echo Build Complete!
echo Output: dist\GameSaveEditor.exe
echo Size:
dir "dist\GameSaveEditor.exe" | findstr "GameSaveEditor"
echo ===========================================
pause
