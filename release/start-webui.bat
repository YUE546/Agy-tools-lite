@echo off
setlocal
cd /d "%~dp0"

rem ============ Optional config (edit below if needed) ============
rem Web UI login password. Leave empty to auto-generate a random password
rem (printed in the console log on first run).
if not defined WEB_PASSWORD set "WEB_PASSWORD="
rem Service port (WebUI + /api)
if not defined PORT set "PORT=8045"
rem Bind to 127.0.0.1 only (set to 0 to allow LAN access)
if not defined ABV_BIND_LOCAL_ONLY set "ABV_BIND_LOCAL_ONLY=1"
rem =================================================================

if not exist "antigravity-tools.exe" (
    echo [ERROR] antigravity-tools.exe not found next to this script.
    pause
    exit /b 1
)

if exist "dist\index.html" (
    set "ABV_DIST_PATH=%~dp0dist"
) else (
    echo [WARN] dist\ not found - Web UI assets unavailable, /api still works.
)

echo ==============================================
echo  Antigravity Tools Lite - Web UI service
echo  URL : http://localhost:%PORT%
echo        ^(browser opens automatically^)
echo  Data: %USERPROFILE%\.antigravity_tools
echo  Stop: press Ctrl+C in this window or use the tray icon
echo ==============================================

antigravity-tools.exe --headless --open
pause
