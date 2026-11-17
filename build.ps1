# Native Windows build helper.
# Requires: Rust (MSVC toolchain) + Visual Studio Build Tools with the
# "Desktop development with C++" workload. See README for details.
$ErrorActionPreference = "Stop"
Set-Location $PSScriptRoot

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    throw "cargo not found. Install Rust from https://rustup.rs and reopen the terminal."
}

cargo build --release
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

Copy-Item "target\release\ytlrc.exe" ".\ytlrc.exe" -Force
Write-Host ""
Write-Host "Done:"
Write-Host "  target\release\ytlrc.exe"
Write-Host "  .\ytlrc.exe  (copy)"
