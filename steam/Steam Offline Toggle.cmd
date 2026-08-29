@echo off
setlocal
title Steam Offline Toggle

set "RULE=Steam offline"
set "STEAM=C:\Program Files (x86)\Steam\steam.exe"

net session >nul 2>&1
if errorlevel 1 (
    powershell -NoProfile -Command "Start-Process -FilePath '%~f0' -Verb RunAs"
    exit /b
)

netsh advfirewall firewall show rule name="%RULE%" >nul 2>&1
if errorlevel 1 (set "MODE=BLOCK") else (set "MODE=UNBLOCK")

echo Shutting Steam down...
if exist "%STEAM%" start "" /wait "%STEAM%" -shutdown
timeout /t 5 /nobreak >nul

if "%MODE%"=="BLOCK" (
    netsh advfirewall firewall add rule name="%RULE%" dir=out action=block program="%STEAM%" enable=yes >nul
    echo.
    echo   BLOCKED. Steam cannot reach its servers.
    echo   Start Steam, accept "Start in Offline Mode", then launch your game.
) else (
    netsh advfirewall firewall delete rule name="%RULE%" >nul
    echo.
    echo   UNBLOCKED. Steam is back to normal.
    echo   In Steam: menu ^> Go Online, to leave offline mode.
)

echo.
start "" "%STEAM%"
timeout /t 6 /nobreak >nul
