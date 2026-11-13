@echo off
setlocal
cd /d "%~dp0"
if exist "AuraBridge-Setup.exe" (
  start "" "%~dp0AuraBridge-Setup.exe" %*
  exit /b 0
)
if exist "ClaudeSwitchPro-Setup.exe" (
  start "" "%~dp0ClaudeSwitchPro-Setup.exe" %*
  exit /b 0
)
echo AuraBridge-Setup.exe not found in current directory.
pause
exit /b 1
