# Prepares the Windows installer build: builds fw-ams-service as Tauri sidecar and fetches the
# PawnIO RyzenSMU module. Called by tauri.bundle-windows.conf.json (beforeBuildCommand).
$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot

cargo build --release -p fw-ams-service --manifest-path (Join-Path $Root "Cargo.toml")
if ($LASTEXITCODE -ne 0) { throw "cargo build fw-ams-service fehlgeschlagen" }

$Triple = (rustc -vV | Select-String "^host: ").ToString().Substring(6).Trim()
$Bin = Join-Path $Root "app/src-tauri/binaries"
New-Item -ItemType Directory -Force $Bin | Out-Null
Copy-Item (Join-Path $Root "target/release/fw-ams-service.exe") (Join-Path $Bin "fw-ams-service-$Triple.exe") -Force

if (-not (Test-Path (Join-Path $Root "app/src-tauri/resources/RyzenSMU.bin"))) {
    & (Join-Path $PSScriptRoot "fetch-pawnio-module.ps1")
}
