@echo off
setlocal
echo Stopping Claude Switch Pro...
taskkill /F /IM claude-switch-pro.exe >nul 2>&1
timeout /t 1 /nobreak >nul

echo Removing installation directory, shortcuts, and registry entries...
powershell -NoProfile -ExecutionPolicy Bypass -Command ^
  "$d = [Environment]::GetFolderPath('Desktop'); Remove-Item -Force -ErrorAction SilentlyContinue ($d + '\Claude Switch Pro.lnk'); " ^
  "$sm = [Environment]::GetFolderPath('Programs'); Remove-Item -Force -ErrorAction SilentlyContinue ($sm + '\Claude Switch Pro.lnk'); " ^
  "Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run' -Name 'ClaudeSwitchPro' -ErrorAction SilentlyContinue; " ^
  "Remove-Item -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\ClaudeSwitchPro' -Recurse -Force -ErrorAction SilentlyContinue; " ^
  "$dir = Join-Path $env:LOCALAPPDATA 'Programs\ClaudeSwitchPro'; Remove-Item -Path $dir -Recurse -Force -ErrorAction SilentlyContinue"

echo Claude Switch Pro uninstalled cleanly.
echo User settings preserved at %LOCALAPPDATA%\ClaudeSwitchPro
pause
