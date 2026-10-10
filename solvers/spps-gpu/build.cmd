@echo off
rem Builds spps-gpu.exe with nvcc and the VS 2022 host compiler.
rem Usage: build.cmd [out-dir] [native|fat|sm75] [-D...]
rem   default out C:\tmp\nm-spps-gpu\bin, -arch=native; "fat" holds Turing (Zeph's RTX 2060, sm_75, plus its
rem   PTX for anything newer) and Blackwell (Grace's RTX 5070, sm_120).
rem Run it through cmd.exe, never from PowerShell 5.1's dev shell (it turns VsDevCmd's harmless vswhere
rem stderr into a fatal error). -fmad=false keeps the device's float arithmetic unfused, as the host's
rem SSE2 code is, so the CPU and GPU builds of the walk compute the same bits.
rem No -lineinfo: it writes this source folder's and the CUDA install's paths into the device code, so a
rem build from another folder had other code and failed solvers/manifest.json's code sha256 (nvcc has no
rem prefix-map option). For a profiling build only, set SPPS_GPU_LINEINFO=1; that exe is never the verified one.
setlocal
set LINEINFO=
if "%SPPS_GPU_LINEINFO%"=="1" set LINEINFO=-lineinfo
set OUT=%1
if "%OUT%"=="" set OUT=C:\tmp\nm-spps-gpu\bin
set ARCH=-arch=native
set XDEF=%~3
if /i "%2"=="fat" set ARCH=-gencode arch=compute_75,code=sm_75 -gencode arch=compute_75,code=compute_75 -gencode arch=compute_120,code=sm_120
if /i "%2"=="sm75" set ARCH=-gencode arch=compute_75,code=sm_75 -gencode arch=compute_75,code=compute_75
if not exist "%OUT%" mkdir "%OUT%"
set VSDEV=
for /d %%d in ("C:\Program Files\Microsoft Visual Studio\2022\*") do if exist "%%d\Common7\Tools\VsDevCmd.bat" set VSDEV=%%d\Common7\Tools\VsDevCmd.bat
if "%VSDEV%"=="" ( echo no VS 2022 VsDevCmd.bat & exit /b 2 )
call "%VSDEV%" -arch=amd64 -no_logo 2>nul
nvcc --version | findstr /c:"release"
set SRC=%~dp0src
set TP=%~dp0third_party\tinyxml2
nvcc -O3 -std=c++17 %ARCH% %XDEF% -fmad=false %LINEINFO% -Xcompiler "/openmp /O2 /EHsc /utf-8 /fp:precise /D_CRT_SECURE_NO_WARNINGS" -o "%OUT%\spps-gpu.exe" "%SRC%\main.cu" "%SRC%\model.cpp" "%SRC%\output.cpp" "%TP%\tinyxml2.cpp"
if errorlevel 1 ( echo nvcc failed & exit /b 1 )
dir "%OUT%\spps-gpu.exe" | findstr spps-gpu
