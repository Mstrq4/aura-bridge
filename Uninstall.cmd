@echo off
chcp 65001 >nul
title إلغاء تثبيت Claude Switch Pro
echo =========================================================
echo  إلغاء تثبيت Claude Switch Pro
echo =========================================================
taskkill /F /IM claude-switch-pro.exe >nul 2>&1
powershell -NoProfile -Command "Remove-Item -Path \"$([Environment]::GetFolderPath('Desktop'))\Claude Switch Pro.lnk\" -Force -ErrorAction SilentlyContinue; Remove-Item -Path \"$([Environment]::GetFolderPath('Programs'))\Claude Switch Pro.lnk\" -Force -ErrorAction SilentlyContinue; Remove-Item -Path \"$env:LOCALAPPDATA\Programs\ClaudeSwitchPro\" -Recurse -Force -ErrorAction SilentlyContinue;"
echo تم إلغاء التثبيت وإزالة الاختصارات بنجاح.
pause
