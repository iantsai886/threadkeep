# Removes the Threadkeep program. Your notes and Threadkeep's settings are kept.
# First removes Threadkeep from every AI app it was connected to, so no app is left pointing at a
# program that no longer exists (each app's previous settings file is backed up first).
param([string]$Prefix = (Join-Path $env:LOCALAPPDATA 'Programs\Threadkeep'))
$ErrorActionPreference = 'Stop'
$exe = Join-Path $Prefix 'threadkeep.exe'
if (Test-Path $exe) {
  try {
    $apps = & $exe connections | ConvertFrom-Json
    foreach ($c in $apps) { if ($c.client -and $c.client -ne 'custom') { & $exe disconnect $c.id | Out-Null; Write-Output "Disconnected $($c.name)" } }
  } catch { Write-Warning "Could not list connections: $($_.Exception.Message). Remove the threadkeep entry from your AI apps' settings by hand if present." }
}
$user = [Environment]::GetEnvironmentVariable('Path', 'User')
if ($user) {
  $kept = @($user -split ';' | Where-Object { $_ -and $_ -ne $Prefix })
  [Environment]::SetEnvironmentVariable('Path', ($kept -join ';'), 'User')
}
if (Test-Path $Prefix) { Remove-Item -Recurse -Force $Prefix }
Write-Output 'Threadkeep was removed. Your notes and settings were not touched.'
Write-Output 'Settings and approval queue live in %APPDATA%\Threadkeep; delete that folder yourself to remove them.'
