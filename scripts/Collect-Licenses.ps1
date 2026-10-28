param([Parameter(Mandatory=$true)][string]$Destination)
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
New-Item -ItemType Directory -Path $Destination -Force | Out-Null
$components = [System.Collections.Generic.List[object]]::new()
$registry = Join-Path $repo '.tools\cargo\registry\src'
foreach ($container in Get-ChildItem -LiteralPath $registry -Directory -ErrorAction SilentlyContinue) {
  foreach ($crate in Get-ChildItem -LiteralPath $container.FullName -Directory) {
    $text = Get-Content -LiteralPath (Join-Path $crate.FullName 'Cargo.toml') -Raw
    $name = [regex]::Match($text,'(?m)^name\s*=\s*"([^"]+)"').Groups[1].Value
    $version = [regex]::Match($text,'(?m)^version\s*=\s*"([^"]+)"').Groups[1].Value
    $license = [regex]::Match($text,'(?m)^license\s*=\s*"([^"]+)"').Groups[1].Value
    $components.Add([pscustomobject]@{ecosystem='Cargo'; name=$name; version=$version; license=$license})
    $licenseFiles = @(Get-ChildItem -LiteralPath $crate.FullName -File | Where-Object { $_.Name -match '^(LICENSE|COPYING|NOTICE|COPYRIGHT)' })
    if ($licenseFiles.Count) {
      $dest = Join-Path $Destination "cargo\$($crate.Name)"
      New-Item -ItemType Directory -Path $dest -Force | Out-Null
      foreach ($file in $licenseFiles) { Copy-Item -LiteralPath $file.FullName -Destination $dest }
    }
  }
}
$nodeModules = Join-Path $repo 'node_modules'
$nodeRoots = @(Get-ChildItem -LiteralPath $nodeModules -Directory -ErrorAction SilentlyContinue | Where-Object { $_.Name -ne '.pnpm' })
foreach ($entry in $nodeRoots) {
  $packages = if ($entry.Name.StartsWith('@')) { @(Get-ChildItem -LiteralPath $entry.FullName -Directory) } else { @($entry) }
  foreach ($package in $packages) {
    $jsonPath = Join-Path $package.FullName 'package.json'
    if (-not (Test-Path -LiteralPath $jsonPath)) { continue }
    $json = Get-Content -LiteralPath $jsonPath -Raw | ConvertFrom-Json
    $components.Add([pscustomobject]@{ecosystem='npm'; name=$json.name; version=$json.version; license=$json.license})
    $files = @(Get-ChildItem -LiteralPath $package.FullName -File | Where-Object { $_.Name -match '^(LICENSE|COPYING|NOTICE|COPYRIGHT)' })
    if ($files.Count) {
      $dest = Join-Path $Destination ('npm\' + ($json.name -replace '[/@]','_'))
      New-Item -ItemType Directory -Path $dest -Force | Out-Null
      foreach ($file in $files) { Copy-Item -LiteralPath $file.FullName -Destination $dest }
    }
  }
}
$components | Sort-Object ecosystem,name | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $Destination 'components.json') -Encoding utf8
