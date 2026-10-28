param([switch]$DownloadOnly, [switch]$NativeOnly)
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
$toolRoot = Join-Path $repo '.tools'
New-Item -ItemType Directory -Force -Path $toolRoot | Out-Null
$sources = @{
  'rustup-init.exe' = 'https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-msvc/rustup-init.exe'
  'portable-msvc.py' = 'https://gist.githubusercontent.com/mmozeiko/7f3162ec2988e81e56d5c4e22cde9977/raw/portable-msvc.py'
  'webview2-page.html' = 'https://developer.microsoft.com/en-us/microsoft-edge/webview2/'
}
foreach ($entry in $sources.GetEnumerator()) {
  $destination = Join-Path $toolRoot $entry.Key
  if (-not (Test-Path -LiteralPath $destination)) {
    Write-Host "Downloading $($entry.Key)..."
    Invoke-WebRequest -Uri $entry.Value -OutFile $destination
  }
}
if ($DownloadOnly) { return }
if ($NativeOnly) {
  $python = (Join-Path $env:USERPROFILE '.cache\codex-runtimes\codex-primary-runtime\dependencies\python\python.exe')
  if (-not (Test-Path -LiteralPath $python)) { $python = (Get-Command python -ErrorAction Stop).Source }
  Push-Location $toolRoot
  try {
    & $python -u '.\portable-msvc.py' --vs 2022 --host x64 --target x64 --accept-license
    if ($LASTEXITCODE -ne 0) { throw 'Portable Microsoft build tools extraction failed.' }
  } finally { Pop-Location }
  return
}
$env:RUSTUP_HOME = Join-Path $toolRoot 'rustup'
$env:CARGO_HOME = Join-Path $toolRoot 'cargo'
if (-not (Test-Path -LiteralPath (Join-Path $env:CARGO_HOME 'bin\cargo.exe'))) {
  & (Join-Path $toolRoot 'rustup-init.exe') -y --no-modify-path --profile minimal --default-toolchain stable-x86_64-pc-windows-msvc
  if ($LASTEXITCODE -ne 0) { throw 'Rust toolchain bootstrap failed.' }
}
Write-Host "Rust installed in $env:CARGO_HOME"
$runtimeParent = Join-Path $toolRoot 'webview2'
$runtimeExe = Get-ChildItem -LiteralPath $runtimeParent -Filter 'msedgewebview2.exe' -Recurse -ErrorAction SilentlyContinue | Select-Object -First 1
if (-not $runtimeExe) {
  $metadata = (Get-Content -LiteralPath (Join-Path $toolRoot 'webview2-page.html') -Raw).Replace('\u002F','/')
  $runtimeUrl = [regex]::Match($metadata,'https://msedge\.sf\.dl\.delivery\.mp\.microsoft\.com/filestreamingservice/files/[a-z0-9-]+/Microsoft\.WebView2\.FixedVersionRuntime\.[0-9.]+\.x64\.cab').Value
  if (-not $runtimeUrl) { throw 'Official metadata did not contain a fixed x64 WebView2 runtime.' }
  $runtimeCab = Join-Path $toolRoot ([System.IO.Path]::GetFileName($runtimeUrl))
  if (-not (Test-Path -LiteralPath $runtimeCab)) {
    Write-Host "Downloading official fixed WebView2 runtime: $runtimeUrl"
    Invoke-WebRequest -Uri $runtimeUrl -OutFile $runtimeCab
  }
  New-Item -ItemType Directory -Force -Path $runtimeParent | Out-Null
  & expand.exe $runtimeCab '-F:*' $runtimeParent | Out-Null
  if ($LASTEXITCODE -ne 0) { throw 'WebView2 extraction failed.' }
  $runtimeExe = Get-ChildItem -LiteralPath $runtimeParent -Filter 'msedgewebview2.exe' -Recurse | Select-Object -First 1
  if (-not $runtimeExe) { throw 'Extracted runtime does not contain msedgewebview2.exe.' }
  [pscustomobject]@{ source=$runtimeUrl; cabSha256=(Get-FileHash -LiteralPath $runtimeCab -Algorithm SHA256).Hash.ToLowerInvariant(); runtime=$runtimeExe.Directory.Name } | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $toolRoot 'webview2-source.json') -Encoding utf8
}
Write-Host "Fixed WebView2 available at $($runtimeExe.DirectoryName)"
