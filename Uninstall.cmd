@echo off
setlocal
echo Stopping AuraBridge...
taskkill /F /IM AuraBridge.exe >nul 2>&1
taskkill /F /IM claude-switch-pro.exe >nul 2>&1
timeout /t 1 /nobreak >nul

echo Removing installation directory, shortcuts, and registry entries...
powershell -NoProfile -ExecutionPolicy Bypass -Command ^
  "$d = [Environment]::GetFolderPath('Desktop'); Remove-Item -Force -ErrorAction SilentlyContinue ($d + '\AuraBridge.lnk'); Remove-Item -Force -ErrorAction SilentlyContinue ($d + '\Claude Switch Pro.lnk'); " ^
  "$sm = [Environment]::GetFolderPath('Programs'); Remove-Item -Force -ErrorAction SilentlyContinue ($sm + '\AuraBridge.lnk'); Remove-Item -Force -ErrorAction SilentlyContinue ($sm + '\Claude Switch Pro.lnk'); " ^
  "Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run' -Name 'AuraBridge' -ErrorAction SilentlyContinue; " ^
  "Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run' -Name 'ClaudeSwitchPro' -ErrorAction SilentlyContinue; " ^
  "Remove-Item -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\AuraBridge' -Recurse -Force -ErrorAction SilentlyContinue; " ^
  "Remove-Item -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\ClaudeSwitchPro' -Recurse -Force -ErrorAction SilentlyContinue; " ^
  "foreach ($p in @('Programs\AuraBridge','Programs\ClaudeSwitchPro')) { $dir = Join-Path $env:LOCALAPPDATA $p; Remove-Item -Path $dir -Recurse -Force -ErrorAction SilentlyContinue }"

echo AuraBridge uninstalled cleanly.
echo User settings preserved at %LOCALAPPDATA%\AuraBridge
pause
