@echo off
rem Launch ytlrc from Command Prompt / Windows Terminal.
rem Prefers a native Windows build (ytlrc.exe); falls back to WSL.
setlocal
set "HERE=%~dp0"

if exist "%HERE%ytlrc.exe" (
  "%HERE%ytlrc.exe" %*
  exit /b %ERRORLEVEL%
)

rem WSL fallback. The daemon must listen on 0.0.0.0 so Windows can reach it.
set "PROJECT=/mnt/d/Project/ytlrc"
set "BIN=%PROJECT%/target/x86_64-unknown-linux-musl/release/ytlrc"
wsl.exe -e bash -lc "exec '%BIN%' --bind 0.0.0.0 \"$@\"" ytlrc %*
endlocal
