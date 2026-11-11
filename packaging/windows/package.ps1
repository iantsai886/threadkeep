# Builds the portable Windows package: dist\Threadkeep-<version>-windows-x64.zip
#   threadkeep.exe, ui\ (the built interface), install.ps1, uninstall.ps1, README.txt
# Needs the release build (cargo build --release) and the built interface (ui\dist).
# Unsigned. See docs\STATUS.md for what is and is not verified.
param([string]$Out = (Join-Path $PSScriptRoot '..\..\dist'))
$ErrorActionPreference = 'Stop'
$root = Resolve-Path (Join-Path $PSScriptRoot '..\..')
$exe = Join-Path $root 'target\release\threadkeep.exe'
$ui = Join-Path $root 'ui\dist'
if (-not (Test-Path $exe)) { throw "Missing $exe. Run: cargo build --release" }
if (-not (Test-Path (Join-Path $ui 'index.html'))) { throw "Missing the built interface at $ui. Run npm run build inside ui\ first." }
$version = (& $exe version).Trim()
$name = "Threadkeep-$version-windows-x64"
New-Item -ItemType Directory -Force $Out | Out-Null
$stage = Join-Path $Out $name
if (Test-Path $stage) { Remove-Item -Recurse -Force $stage }
New-Item -ItemType Directory $stage | Out-Null
Copy-Item $exe $stage
Copy-Item -Recurse $ui (Join-Path $stage 'ui')
Copy-Item (Join-Path $PSScriptRoot 'install.ps1'), (Join-Path $PSScriptRoot 'uninstall.ps1'), (Join-Path $PSScriptRoot 'README.txt') $stage
$zip = Join-Path $Out "$name.zip"
if (Test-Path $zip) { Remove-Item -Force $zip }
Compress-Archive -Path $stage -DestinationPath $zip
$hash = (Get-FileHash $zip -Algorithm SHA256).Hash.ToLower()
"$hash  $name.zip" | Set-Content -Encoding ascii "$zip.sha256"
Write-Output ("{0}  ({1:N1} MB)`nSHA-256 {2}" -f $zip, ((Get-Item $zip).Length / 1MB), $hash)
