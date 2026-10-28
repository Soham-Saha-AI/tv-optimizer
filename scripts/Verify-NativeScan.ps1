param([string]$Serial='192.0.2.10:5555')
$ErrorActionPreference='Stop'
$repo=Split-Path -Parent $PSScriptRoot
$workspace=Split-Path -Parent $repo
$qa=Join-Path $repo '.tools\native-qa'
New-Item -ItemType Directory -Force -Path $qa,(Join-Path $qa 'tools'),(Join-Path $qa 'launchers') | Out-Null
Copy-Item -LiteralPath (Join-Path $repo 'src-tauri\target\release\tv-optimizer.exe') -Destination (Join-Path $qa 'TV Optimizer.exe') -Force
foreach($name in @('adb.exe','AdbWinApi.dll','AdbWinUsbApi.dll','libwinpthread-1.dll')){Copy-Item -LiteralPath (Join-Path $workspace "platform-tools\$name") -Destination (Join-Path $qa "tools\$name") -Force}
foreach($name in @('liquid-glass.apk','original.apk','original.sha256')){Copy-Item -LiteralPath (Join-Path $repo "launchers\$name") -Destination (Join-Path $qa "launchers\$name") -Force}
if($Serial.Contains(':')) {
  & (Join-Path $qa 'tools\adb.exe') connect $Serial
  if($LASTEXITCODE -ne 0){throw 'Could not connect to the TV for the read-only scan.'}
}
$p=Start-Process -FilePath (Join-Path $qa 'TV Optimizer.exe') -ArgumentList @('--scan-only',$Serial) -Wait -PassThru -WindowStyle Hidden
if($p.ExitCode -ne 0){throw "Native scan exited $($p.ExitCode). Check startup-error.txt."}
$result=Get-Content -LiteralPath (Join-Path $qa 'data\native-scan.json') -Raw | ConvertFrom-Json
if($result.error){throw $result.error}
[pscustomobject]@{model=$result.snapshot.identity.model;api=$result.snapshot.identity.api;profile=$result.snapshot.profile;packages=$result.snapshot.packages.Count;home=$result.snapshot.home_component;unknownStates=@($result.snapshot.packages|Where-Object {$null-eq$_.enabled}).Count;inspections=$result.snapshot.inspections;recommendations=$result.recommendations.Count;report=(Join-Path $qa 'data\native-scan.json')} | ConvertTo-Json -Depth 5
