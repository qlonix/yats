@echo off
set "CARGO_TARGET_DIR=C:\temp\yats-target"
if not exist "C:\temp\yats-target" mkdir "C:\temp\yats-target"
call "C:\Program Files\Microsoft Visual Studio\18\Community\Common7\Tools\VsDevCmd.bat"
if "%~1"=="build" (
    npm run tauri build
) else (
    cd src-tauri
    cargo check
)
