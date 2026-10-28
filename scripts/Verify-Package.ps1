param([string]$Root = $PSScriptRoot)
$ErrorActionPreference = 'Stop'
$rootPath = [System.IO.Path]::GetFullPath($Root).TrimEnd('\')
$manifest = Get-Content -LiteralPath (Join-Path $rootPath 'integrity-manifest.json') -Raw | ConvertFrom-Json
foreach ($file in $manifest.files) {
  $path = [System.IO.Path]::GetFullPath((Join-Path $rootPath $file.path))
  if (-not $path.StartsWith($rootPath + '\',[System.StringComparison]::OrdinalIgnoreCase)) { throw 'Manifest path escapes the portable folder.' }
  if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "Missing file: $($file.path)" }
  if ((Get-Item -LiteralPath $path).Length -ne $file.bytes) { throw "Size mismatch: $($file.path)" }
  if ((Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant() -ne $file.sha256) { throw "Integrity failure: $($file.path)" }
}
Write-Host "Verified $($manifest.files.Count) packaged files."
