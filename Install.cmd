@echo off
setlocal
cd /d "%~dp0"
if not exist "ClaudeSwitchPro-Setup.exe" (
  echo ClaudeSwitchPro-Setup.exe not found in current directory.
  pause
  exit /b 1
)
start "" "%~dp0ClaudeSwitchPro-Setup.exe" %*
