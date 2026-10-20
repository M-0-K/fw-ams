# Downloads the signed RyzenSMU module for PawnIO (https://github.com/namazso/PawnIO.Modules,
# LGPL-2.1) into app/src-tauri/resources. The release is pinned and checked by SHA-256.
# PawnIO only loads modules signed by its author, so the file cannot be built locally.
$ErrorActionPreference = "Stop"

$Version = "0.2.11"
$Sha256 = "43608cb89bc84247fef1368a139013f7d043e17db6d6c8dfc9b46bf0905a81f4"
$Url = "https://github.com/namazso/PawnIO.Modules/releases/download/$Version/release_$($Version -replace '\.', '_').zip"

$Root = Split-Path -Parent $PSScriptRoot
$Target = Join-Path $Root "app/src-tauri/resources"
$Work = Join-Path ([System.IO.Path]::GetTempPath()) "fw-ams-pawnio-$Version"

New-Item -ItemType Directory -Force $Target, $Work | Out-Null
$Zip = Join-Path $Work "release.zip"
Write-Host "Lade PawnIO.Modules $Version ..."
# Plain .NET instead of Invoke-WebRequest/Get-FileHash: Windows PowerShell started from
# PowerShell 7 (e.g. on GitHub runners) may fail to load those cmdlets' module
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
$Client = New-Object System.Net.WebClient
try { $Client.DownloadFile($Url, $Zip) } finally { $Client.Dispose() }

$Sha = [System.Security.Cryptography.SHA256]::Create()
$Stream = [System.IO.File]::OpenRead($Zip)
try {
    $Hash = -join ($Sha.ComputeHash($Stream) | ForEach-Object { $_.ToString("x2") })
} finally {
    $Stream.Dispose()
    $Sha.Dispose()
}
if ($Hash -ne $Sha256) {
    throw "SHA-256 stimmt nicht: $Hash (erwartet $Sha256)"
}

Add-Type -AssemblyName System.IO.Compression.FileSystem
$Archive = [System.IO.Compression.ZipFile]::OpenRead($Zip)
try {
    foreach ($Name in @("RyzenSMU.bin", "COPYING")) {
        $Entry = $Archive.Entries | Where-Object { $_.FullName -eq $Name }
        if (-not $Entry) { throw "$Name fehlt im Archiv" }
        $Out = if ($Name -eq "COPYING") { "PawnIO.Modules-COPYING.txt" } else { $Name }
        [System.IO.Compression.ZipFileExtensions]::ExtractToFile($Entry, (Join-Path $Target $Out), $true)
    }
} finally {
    $Archive.Dispose()
}
Remove-Item -Recurse -Force $Work
Write-Host "RyzenSMU.bin liegt in $Target"
