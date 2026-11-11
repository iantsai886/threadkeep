# Installs Threadkeep for the current user. No administrator rights needed.
#   .\install.ps1                  copies the program to %LOCALAPPDATA%\Programs\Threadkeep
#   .\install.ps1 -AddToPath       also lets you type "threadkeep" in a new terminal
#   .\install.ps1 -Prefix <folder> install somewhere else
# Your notes and Threadkeep's settings are never touched by install or uninstall.
param([string]$Prefix = (Join-Path $env:LOCALAPPDATA 'Programs\Threadkeep'), [switch]$AddToPath)
$ErrorActionPreference = 'Stop'
$here = $PSScriptRoot
if (-not (Test-Path (Join-Path $here 'threadkeep.exe'))) { throw 'Run this from the unzipped Threadkeep folder (threadkeep.exe is missing).' }
New-Item -ItemType Directory -Force $Prefix | Out-Null
Copy-Item (Join-Path $here 'threadkeep.exe') $Prefix -Force
if (Test-Path (Join-Path $Prefix 'ui')) { Remove-Item -Recurse -Force (Join-Path $Prefix 'ui') }
Copy-Item -Recurse (Join-Path $here 'ui') (Join-Path $Prefix 'ui')
Copy-Item (Join-Path $here 'uninstall.ps1'), (Join-Path $here 'README.txt') $Prefix -Force
if ($AddToPath) {
  $user = [Environment]::GetEnvironmentVariable('Path', 'User')
  $parts = @($user -split ';' | Where-Object { $_ })
  if ($parts -notcontains $Prefix) { [Environment]::SetEnvironmentVariable('Path', (($parts + $Prefix) -join ';'), 'User') }
  Write-Output 'Added to your PATH. Open a new terminal to use "threadkeep".'
}
Write-Output "Installed to $Prefix"
Write-Output "Next:  & '$Prefix\threadkeep.exe' app"
