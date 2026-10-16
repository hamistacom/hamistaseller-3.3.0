@echo off
rem Builds the three editions and puts them in .\bin with their final names.
rem Needs only: Rust (https://rustup.rs) and Visual Studio Build Tools (C++ workload).
setlocal
cd /d "%~dp0src-tauri" || exit /b 1
if not exist "..\bin" mkdir "..\bin"

echo === customer edition ===
cargo build --release --features custom-protocol || exit /b 1
copy /y target\release\hamista-seller.exe ..\bin\HamistaSeller.exe >nul

echo === master edition ===
cargo build --release --features custom-protocol,master || exit /b 1
copy /y target\release\hamista-seller.exe ..\bin\HamistaSeller-Master.exe >nul

echo === admin panel ===
cargo build --release --features custom-protocol,issuer || exit /b 1
copy /y target\release\hamista-seller.exe ..\bin\HamistaAdmin.exe >nul

echo.
echo Done: %~dp0bin
endlocal
