@echo off
rem Run the Lua content walkthrough tests. Extra args are forwarded to the
rem `content-test` CLI, e.g. --filter Man0l0.
setlocal EnableExtensions EnableDelayedExpansion

set "SCRIPT_DIR=%~dp0"
call "%SCRIPT_DIR%_lib.cmd"

if /i "%PROFILE%"=="release" (
    set "FLAG=--release"
    set "BIN=%REPO_ROOT%\target\release\content-test.exe"
) else (
    set "FLAG="
    set "BIN=%REPO_ROOT%\target\debug\content-test.exe"
)

call "%SCRIPT_DIR%check-env.cmd"
if errorlevel 1 exit /b 1

cd /d "%REPO_ROOT%"
echo ==^> Building content-test ^(%PROFILE%^)
cargo build -p content-test %FLAG%
if errorlevel 1 exit /b 1

echo ==^> Running content-test
if not exist "%BIN%" (
    1>&2 echo    X built binary not found at %BIN%
    exit /b 1
)
"%BIN%" %*
exit /b %errorlevel%
