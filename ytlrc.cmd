@echo off
rem Jalankan ytlrc (di WSL) dari Command Prompt / Windows Terminal.
rem Ubah PROJECT di bawah kalau folder project dipindahkan.
setlocal
set "PROJECT=/mnt/d/Project/ytlrc"
set "BIN=%PROJECT%/target/x86_64-unknown-linux-musl/release/ytlrc"
wsl.exe -e bash -lc "exec '%BIN%' \"$@\"" ytlrc %*
endlocal
