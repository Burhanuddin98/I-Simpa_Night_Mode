@echo off
rem The GPU probe on this machine: the CUDA arm, the SPPS arm (about 3 min on Grace's one thread; the
rem fixed seed keeps SPPS on one thread), then RESULT-<machine>.md beside this file. Double-click, or
rem   run-probe.cmd [particles] [steps]       (default 10000000 2000, 1 ms steps)
rem Needs: an NVIDIA driver that runs CUDA 13.2 programs (R580 or newer); Python 3 with no extra
rem packages. If box_tracer.exe says the driver is insufficient, update the driver, or rebuild with
rem build.cmd . sm75 in a VS 2022 + CUDA prompt.
setlocal
set HERE=%~dp0
set P=%1
if "%P%"=="" set P=10000000
set S=%2
if "%S%"=="" set S=2000
set OUT=%HERE%out-%COMPUTERNAME%
if not exist "%OUT%" mkdir "%OUT%"
echo [%TIME%] CUDA arm: %P% particles, %S% steps
"%HERE%box_tracer.exe" %P% %S% "%OUT%\gpu.csv" 100000 > "%OUT%\gpu.json"
if errorlevel 1 ( echo the CUDA arm failed & type "%OUT%\gpu.json" & pause & exit /b 1 )
type "%OUT%\gpu.json"
echo [%TIME%] SPPS arm: %P% particles, %S% steps (minutes)
set SIMPA_SOLVERS_DIR=%HERE%solvers
python "%HERE%spps_arm.py" %P% %S% "%OUT%\spps" --simpa "%HERE%simpa.exe" --solvers "%HERE%solvers" --fixture "%HERE%seats_box.simpa" > "%OUT%\spps.json"
if errorlevel 1 ( echo the SPPS arm failed & type "%OUT%\spps.json" & pause & exit /b 1 )
type "%OUT%\spps.json"
for /d %%r in ("%OUT%\spps\runs\*") do set RUN=%%r
python "%HERE%compare.py" "%OUT%\gpu.json" "%OUT%\gpu.csv" "%OUT%\spps.json" "%RUN%" --simpa "%HERE%simpa.exe" --machine %COMPUTERNAME% --out "%HERE%RESULT-%COMPUTERNAME%.md"
echo.
echo [%TIME%] done: %HERE%RESULT-%COMPUTERNAME%.md
pause
