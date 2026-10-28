param([switch]$CheckOnly, [switch]$Test, [switch]$Release, [switch]$FetchOnly, [switch]$SwiftOSV3)
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
$toolRoot = Join-Path $repo '.tools'
$env:RUSTUP_HOME = Join-Path $toolRoot 'rustup'
$env:CARGO_HOME = Join-Path $toolRoot 'cargo'
$cargo = Join-Path $env:CARGO_HOME 'bin\cargo.exe'
if (-not (Test-Path -LiteralPath $cargo)) { throw 'Run scripts/Prepare-Toolchain.ps1 first.' }
$env:PATH = "$(Join-Path $env:CARGO_HOME 'bin');$env:PATH"
$manifest = Join-Path $repo 'src-tauri\Cargo.toml'
if ($SwiftOSV3 -and $FetchOnly) { throw '-SwiftOSV3 cannot be combined with -FetchOnly.' }
if ($SwiftOSV3 -and $Release) {
  $candidate = Join-Path $repo 'launchers\candidates\SwiftOS-3.0.1-24.apk'
  if (-not (Test-Path -LiteralPath $candidate -PathType Leaf)) { throw 'SwiftOS v3 candidate APK is missing. Build and review it first.' }
  $variantSource = Get-Content -LiteralPath (Join-Path $repo 'src-tauri\src\launcher_variant.rs') -Raw
  $match = [regex]::Match($variantSource, '(?s)#\[cfg\(feature = "swiftos-v3-candidate"\)\]\s*pub const SWIFTOS_HASH: &str = "([^"]+)";')
  if (-not $match.Success -or $match.Groups[1].Value -notmatch '^[0-9a-f]{64}$') { throw 'The reviewed SwiftOS v3 SHA-256 is not hard-coded in launcher_variant.rs; candidate build is disabled.' }
  $actual = (Get-FileHash -LiteralPath $candidate -Algorithm SHA256).Hash.ToLowerInvariant()
  if ($actual -ne $match.Groups[1].Value) { throw 'SwiftOS v3 candidate APK SHA-256 does not match the reviewed hash compiled into this build.' }
}
if ($FetchOnly) {
  & $cargo fetch --manifest-path $manifest
  if ($LASTEXITCODE -ne 0) { throw 'Cargo dependency fetch failed.' }
  return
}
$msvc = Get-ChildItem -LiteralPath (Join-Path $toolRoot 'msvc\VC\Tools\MSVC') -Directory | Sort-Object Name -Descending | Select-Object -First 1
$sdkRoot = Join-Path $toolRoot 'msvc\Windows Kits\10'
$sdk = Get-ChildItem -LiteralPath (Join-Path $sdkRoot 'Lib') -Directory | Sort-Object Name -Descending | Select-Object -First 1
if (-not $msvc -or -not $sdk) { throw 'Run scripts/Prepare-Toolchain.ps1 -NativeOnly first.' }
$msvcBin = Join-Path $msvc.FullName 'bin\Hostx64\x64'
$sdkBin = Join-Path $sdkRoot "bin\$($sdk.Name)\x64"
$env:PATH = "$msvcBin;$sdkBin;$env:PATH"
$env:INCLUDE = @((Join-Path $msvc.FullName 'include'),(Join-Path $sdkRoot "Include\$($sdk.Name)\ucrt"),(Join-Path $sdkRoot "Include\$($sdk.Name)\shared"),(Join-Path $sdkRoot "Include\$($sdk.Name)\um"),(Join-Path $sdkRoot "Include\$($sdk.Name)\winrt")) -join ';'
$env:LIB = @((Join-Path $msvc.FullName 'lib\x64'),(Join-Path $sdkRoot "Lib\$($sdk.Name)\ucrt\x64"),(Join-Path $sdkRoot "Lib\$($sdk.Name)\um\x64")) -join ';'
$env:VCToolsInstallDir = "$($msvc.FullName)\"
$env:WindowsSdkDir = "$sdkRoot\"
$env:WindowsSDKVersion = "$($sdk.Name)\"
$env:VSCMD_ARG_HOST_ARCH = 'x64'
$env:VSCMD_ARG_TGT_ARCH = 'x64'
# Static CRT makes the distributed executable independent of VC runtime installers.
$env:RUSTFLAGS = '-C target-feature=+crt-static'
Write-Host "Using portable Rust, MSVC $($msvc.Name), Windows SDK $($sdk.Name)."
Push-Location $repo
try {
  if ($Release) {
    $variant = if ($SwiftOSV3) { 'swiftos-v3-candidate' } else { 'default' }
    $receiptPath = Join-Path $toolRoot 'last-build-receipt.json'
    $launcherApk = if ($SwiftOSV3) { Join-Path $repo 'launchers\candidates\SwiftOS-3.0.1-24.apk' } else { Join-Path $repo 'launchers\liquid-glass.apk' }
    $variantSource = Get-Content -LiteralPath (Join-Path $repo 'src-tauri\src\launcher_variant.rs') -Raw
    $hashPattern = if ($SwiftOSV3) { '(?s)#\[cfg\(feature = "swiftos-v3-candidate"\)\]\s*pub const SWIFTOS_HASH: &str = "([^"]+)";' } else { '(?s)#\[cfg\(not\(feature = "swiftos-v3-candidate"\)\)\]\s*pub const SWIFTOS_HASH: &str = "([^"]+)";' }
    $versionPattern = if ($SwiftOSV3) { '(?s)#\[cfg\(feature = "swiftos-v3-candidate"\)\]\s*pub const SWIFTOS_VERSION_CODE: u64 = (\d+);' } else { '(?s)#\[cfg\(not\(feature = "swiftos-v3-candidate"\)\)\]\s*pub const SWIFTOS_VERSION_CODE: u64 = (\d+);' }
    $approvedHash = [regex]::Match($variantSource, $hashPattern)
    $approvedVersion = [regex]::Match($variantSource, $versionPattern)
    if (-not $approvedHash.Success -or $approvedHash.Groups[1].Value -notmatch '^[0-9a-f]{64}$' -or -not $approvedVersion.Success) { throw 'The selected SwiftOS launcher hash/version is not hard-coded in launcher_variant.rs.' }
    $launcherHash = (Get-FileHash -LiteralPath $launcherApk -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($launcherHash -ne $approvedHash.Groups[1].Value) { throw 'SwiftOS APK SHA-256 does not match the selected hard-coded build hash.' }
    $launcherVersionCode = [long]$approvedVersion.Groups[1].Value
    [ordered]@{status='building';variant=$variant;sha256=$null;launcher_sha256=$launcherHash;launcher_version_code=$launcherVersionCode} | ConvertTo-Json | Set-Content -LiteralPath $receiptPath -Encoding utf8
    $pnpm = (Join-Path $env:USERPROFILE '.cache\codex-runtimes\codex-primary-runtime\dependencies\bin\fallback\pnpm.cmd')
    if (-not (Test-Path -LiteralPath $pnpm)) { $pnpm = (Get-Command pnpm -ErrorAction Stop).Source }
    & $pnpm run build
    if ($LASTEXITCODE -ne 0) { throw 'Frontend build failed.' }
    $features = if ($SwiftOSV3) { 'custom-protocol,swiftos-v3-candidate' } else { 'custom-protocol' }
    & $cargo build --manifest-path $manifest --release --features $features
    if ($LASTEXITCODE -eq 0) {
      $executable = Join-Path $repo 'src-tauri\target\release\tv-optimizer.exe'
      if (-not (Test-Path -LiteralPath $executable -PathType Leaf)) { throw 'Release executable is missing after a successful Cargo build.' }
      $receipt = [ordered]@{status='success';variant=$variant;executable='src-tauri/target/release/tv-optimizer.exe';sha256=(Get-FileHash -LiteralPath $executable -Algorithm SHA256).Hash.ToLowerInvariant();launcher_sha256=$launcherHash;launcher_version_code=$launcherVersionCode;builtUtc=[DateTime]::UtcNow.ToString('o')}
      $receipt | ConvertTo-Json | Set-Content -LiteralPath $receiptPath -Encoding utf8
    }
  } elseif ($Test) {
    # Native unit tests live in the library; the Windows GUI entry point has none.
    if ($SwiftOSV3) { & $cargo test --manifest-path $manifest --lib --features swiftos-v3-candidate }
    else { & $cargo test --manifest-path $manifest --lib }
  } else {
    if ($SwiftOSV3) { & $cargo check --manifest-path $manifest --features swiftos-v3-candidate }
    else { & $cargo check --manifest-path $manifest }
  }
  if ($LASTEXITCODE -ne 0) { throw 'Native build or check failed.' }
} finally { Pop-Location }
