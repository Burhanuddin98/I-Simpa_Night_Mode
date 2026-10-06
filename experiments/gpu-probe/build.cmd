@echo off
rem Builds box_tracer.exe with nvcc and the VS 2022 host compiler. Usage: build.cmd [out-dir] [gencode...]
rem Default out C:\tmp\nm-gpu-probe, -arch=native. "fat" builds for Turing (Zeph's RTX 2060, sm_75,
rem plus its PTX for anything newer) and Blackwell (Grace's RTX 5070, sm_120).
setlocal
set OUT=%1
if "%OUT%"=="" set OUT=C:\tmp\nm-gpu-probe
set ARCH=-arch=native
if /i "%2"=="fat" set ARCH=-gencode arch=compute_75,code=sm_75 -gencode arch=compute_75,code=compute_75 -gencode arch=compute_120,code=sm_120
if /i "%2"=="sm75" set ARCH=-gencode arch=compute_75,code=sm_75 -gencode arch=compute_75,code=compute_75
if not exist "%OUT%" mkdir "%OUT%"
set VSDEV=
for /d %%d in ("C:\Program Files\Microsoft Visual Studio\2022\*") do if exist "%%d\Common7\Tools\VsDevCmd.bat" set VSDEV=%%d\Common7\Tools\VsDevCmd.bat
if "%VSDEV%"=="" ( echo no VS 2022 VsDevCmd.bat & exit /b 2 )
call "%VSDEV%" -arch=amd64 -no_logo 2>nul
nvcc --version | findstr /c:"release"
nvcc -O3 -std=c++17 %ARCH% -Xcompiler "/openmp /O2" -lineinfo -o "%OUT%\box_tracer.exe" "%~dp0box_tracer.cu"
if errorlevel 1 ( echo nvcc failed & exit /b 1 )
dir "%OUT%\box_tracer.exe" | findstr box_tracer
