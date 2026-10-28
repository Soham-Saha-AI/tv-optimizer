param([switch]$NoZip, [string]$Executable, [switch]$SwiftOSV3Candidate)
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
$workspace = Split-Path -Parent $repo
if (-not $Executable) { $Executable = Join-Path $repo 'src-tauri\target\release\tv-optimizer.exe' }
if (-not (Test-Path -LiteralPath $Executable)) { throw 'Release executable missing. Run scripts/Build-App.ps1 -Release.' }
$expectedVariant = if ($SwiftOSV3Candidate) { 'swiftos-v3-candidate' } else { 'default' }
$receiptPath = Join-Path $repo '.tools\last-build-receipt.json'
if (-not (Test-Path -LiteralPath $receiptPath -PathType Leaf)) { throw 'A successful Build-App.ps1 -Release receipt is required before packaging.' }
$buildReceipt = Get-Content -LiteralPath $receiptPath -Raw | ConvertFrom-Json
if ($buildReceipt.status -ne 'success' -or $buildReceipt.variant -ne $expectedVariant) { throw "The last successful GUI build is not the requested '$expectedVariant' variant. Run Build-App.ps1 -Release with the matching mode." }
$executableHash = (Get-FileHash -LiteralPath $Executable -Algorithm SHA256).Hash.ToLowerInvariant()
if ($executableHash -ne $buildReceipt.sha256) { throw 'The selected executable does not match the SHA-256 in the successful GUI build receipt.' }
$expectedVersionCode = if ($SwiftOSV3Candidate) { 24 } else { 22 }
if ($buildReceipt.launcher_version_code -ne $expectedVersionCode) { throw "The successful GUI build receipt does not target SwiftOS version code $expectedVersionCode." }
$version = (Get-Content -LiteralPath (Join-Path $repo 'src-tauri\tauri.conf.json') -Raw | ConvertFrom-Json).version
$packageRootName = 'TVOptimizer'
$zipLabel = "TV-Optimizer-$version-Windows-x64"
$swiftosApk = Join-Path $repo 'launchers\liquid-glass.apk'
$candidateEvidence = $null
if ($SwiftOSV3Candidate) {
  $packageRootName = 'TVOptimizer-SwiftOS-v3'
  $zipLabel = "TV-Optimizer-$version-SwiftOS-v3-Windows-x64"
  $swiftosApk = Join-Path $repo 'launchers\candidates\SwiftOS-3.0.1-24.apk'
  $evidencePath = Join-Path $repo 'design\swiftos-v3-wallpaper-build24\candidate.json'
  if (-not (Test-Path -LiteralPath $evidencePath -PathType Leaf)) { throw 'SwiftOS v3 build evidence is missing.' }
  $candidateEvidence = Get-Content -LiteralPath $evidencePath -Raw | ConvertFrom-Json
  if ($candidateEvidence.Package -ne 'com.gathin.tvhome' -or $candidateEvidence.VersionCode -ne 24 -or $candidateEvidence.VersionName -ne '3.0.1-swiftos') { throw 'SwiftOS v3 build evidence has an unexpected package identity or version.' }
  if (-not (Test-Path -LiteralPath $swiftosApk -PathType Leaf)) { throw 'SwiftOS v3 candidate APK is missing.' }
  $variantSource = Get-Content -LiteralPath (Join-Path $repo 'src-tauri\src\launcher_variant.rs') -Raw
  $match = [regex]::Match($variantSource, '(?s)#\[cfg\(feature = "swiftos-v3-candidate"\)\]\s*pub const SWIFTOS_HASH: &str = "([^"]+)";')
  if (-not $match.Success -or $match.Groups[1].Value -notmatch '^[0-9a-f]{64}$') { throw 'The reviewed SwiftOS v3 SHA-256 is not hard-coded in launcher_variant.rs; candidate packaging is disabled.' }
  $actual = (Get-FileHash -LiteralPath $swiftosApk -Algorithm SHA256).Hash.ToLowerInvariant()
  if ($actual -ne $match.Groups[1].Value -or $candidateEvidence.SHA256.ToLowerInvariant() -ne $match.Groups[1].Value) { throw 'SwiftOS v3 candidate APK SHA-256 does not match both build evidence and the reviewed hash compiled into the GUI.' }
  if ($buildReceipt.launcher_sha256 -ne $actual) { throw 'The candidate APK SHA-256 does not match the launcher hash embedded in the successful GUI build receipt.' }
  if (-not $candidateEvidence.SignerSHA256 -or $candidateEvidence.DeviceValidated -ne $false) { throw 'Candidate evidence must include its signer and clearly state that device validation is pending.' }
}
$runtimeExe = Get-ChildItem -LiteralPath (Join-Path $repo '.tools\webview2') -Recurse -Filter msedgewebview2.exe | Select-Object -First 1
if (-not $runtimeExe) { throw 'Fixed WebView2 runtime missing. Run scripts/Prepare-Toolchain.ps1.' }
foreach ($name in @('legacy-liquid-glass-21.apk')) {
  if (-not (Test-Path -LiteralPath (Join-Path $repo "launchers\$name"))) { throw "Missing launcher: $name" }
}
if (-not (Test-Path -LiteralPath $swiftosApk -PathType Leaf)) { throw "Missing SwiftOS launcher: $swiftosApk" }
if (-not $SwiftOSV3Candidate) {
  $baselineHash = (Get-FileHash -LiteralPath $swiftosApk -Algorithm SHA256).Hash.ToLowerInvariant()
  if ($buildReceipt.launcher_sha256 -ne $baselineHash) { throw 'The baseline APK SHA-256 does not match the launcher hash embedded in the successful GUI build receipt.' }
}
$packageApkName = if ($SwiftOSV3Candidate) { 'swiftos-v3.apk' } else { 'liquid-glass.apk' }
$staging = Join-Path $repo ('.tools\package-staging\' + (Get-Date -Format 'yyyyMMdd-HHmmss-fff'))
$package = Join-Path $staging $packageRootName
foreach ($directory in @($package,"$package\tools","$package\runtime\WebView2","$package\launchers","$package\profiles","$package\licenses","$package\help","$package\data")) {
  New-Item -ItemType Directory -Path $directory -Force | Out-Null
}
Copy-Item -LiteralPath $Executable -Destination (Join-Path $package 'TV Optimizer.exe')
foreach ($name in @('adb.exe','AdbWinApi.dll','AdbWinUsbApi.dll','libwinpthread-1.dll','source.properties')) {
  Copy-Item -LiteralPath (Join-Path $workspace "platform-tools\$name") -Destination (Join-Path $package "tools\$name")
}
Copy-Item -LiteralPath (Join-Path $workspace 'platform-tools\NOTICE.txt') -Destination (Join-Path $package 'licenses\Android-platform-tools-NOTICE.txt')
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'Get-OriginalLauncher.ps1') -Destination (Join-Path $package 'tools\Get-OriginalLauncher.ps1')
Copy-Item -Path (Join-Path $runtimeExe.DirectoryName '*') -Destination (Join-Path $package 'runtime\WebView2') -Recurse
Copy-Item -Path (Join-Path $repo 'profiles\*') -Destination (Join-Path $package 'profiles') -Recurse
Copy-Item -LiteralPath $swiftosApk -Destination (Join-Path $package "launchers\$packageApkName")
Copy-Item -LiteralPath (Join-Path $repo 'launchers\legacy-liquid-glass-21.apk') -Destination (Join-Path $package 'launchers')
if ($SwiftOSV3Candidate) {
  foreach ($retained in @(@{file='liquid-glass.apk';hash='ff6f0c1f93c086e803532f40b7b2165e1c820cd62e588da233fcdf96a4123c06'},@{file='swiftos-v3-tested-23.apk';hash='aee129904809bef86f258d8873bbba5ee7297986b61c2e0572cb9786ec7c4aec'})) {
    if ((Get-FileHash -LiteralPath (Join-Path $repo ('launchers\' + $retained.file)) -Algorithm SHA256).Hash.ToLowerInvariant() -ne $retained.hash) { throw "The retained $($retained.file) is not the unchanged reviewed APK." }
  }
  Copy-Item -LiteralPath (Join-Path $repo 'launchers\liquid-glass.apk') -Destination (Join-Path $package 'launchers\liquid-glass.apk')
  Copy-Item -LiteralPath (Join-Path $repo 'launchers\swiftos-v3-tested-23.apk') -Destination (Join-Path $package 'launchers\swiftos-v3-tested-23.apk')
}
if ($SwiftOSV3Candidate) {
  $candidateManifest = [ordered]@{name='SwiftOS';internalName='SS OS';applicationId=$candidateEvidence.Package;versionName=$candidateEvidence.VersionName;versionCode=$candidateEvidence.VersionCode;minimumAndroidSdk=26;targetAndroidSdk=34;sha256=$actual;signerSha256=$candidateEvidence.SignerSHA256;baseProject='https://github.com/Gathin23/cleanyourtv';candidate=$true;deviceValidated=$false;buildEvidence='Candidate build checks passed; television validation remains pending.'}
  $candidateManifest | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $package 'launchers\swiftos-v3-manifest.json') -Encoding utf8
} else {
  foreach ($metadata in Get-ChildItem -LiteralPath (Join-Path $repo 'launchers') -Filter 'swiftos*.json' -File -ErrorAction SilentlyContinue) { Copy-Item -LiteralPath $metadata.FullName -Destination (Join-Path $package 'launchers') }
}
$originalLicense = Join-Path $repo '.tools\upstream\cleanyourtv-2.7.0\LICENSE'
if (Test-Path -LiteralPath $originalLicense) { Copy-Item -LiteralPath $originalLicense -Destination (Join-Path $package 'licenses\CleanYourTV-LICENSE.txt') }
foreach ($license in Get-ChildItem -LiteralPath (Join-Path $repo 'launchers') -Filter '*license*' -File -ErrorAction SilentlyContinue) {
  Copy-Item -LiteralPath $license.FullName -Destination (Join-Path $package "licenses\$($license.Name)")
}
foreach ($name in @('README.md','LICENSE')) {
  $source = Join-Path $repo $name
  if (Test-Path -LiteralPath $source) { Copy-Item -LiteralPath $source -Destination $package }
}
foreach ($doc in Get-ChildItem -LiteralPath (Join-Path $repo 'docs') -File -ErrorAction SilentlyContinue | Where-Object { $SwiftOSV3Candidate -or $_.Name -ne 'SWIFTOS-V3-CANDIDATE.md' }) {
  Copy-Item -LiteralPath $doc.FullName -Destination (Join-Path $package 'help')
}
Copy-Item -LiteralPath (Join-Path $repo '.tools\webview2-source.json') -Destination (Join-Path $package 'licenses\WebView2-source.json')
$notice = @'
TV Optimizer is distributed as a portable folder. Keep every file together.

Bundled components
- Android SDK Platform-Tools 37.0.1, distributed by Google. The complete bundled
  Android-platform-tools-NOTICE.txt contains the applicable component notices.
- Microsoft Edge WebView2 Fixed Version Runtime. Copyright Microsoft Corporation.
  Runtime files include their Microsoft third-party notices and license materials.
  Redistribution terms: https://developer.microsoft.com/en-us/microsoft-edge/webview2/
- SwiftOS (internal SS OS), derived from CleanYourTV.
  Original project: https://github.com/Gathin23/cleanyourtv
  The original upstream launcher is not bundled; its latest release is checked online.
  The legacy Liquid Glass code21 APK is recovery-only; the active SwiftOS build is __SWIFTOS_BUILD__.
- Rust, Tauri, React and transitive components: see generated component inventory
  and bundled license texts under licenses/third-party.

No compiler, source checkout, signing private key, personal backup, ADB trust key,
or app data is included in this distribution.
'@
$notice = $notice.Replace('__SWIFTOS_BUILD__', $(if ($SwiftOSV3Candidate) { '24 local-wallpaper update (Mi TV 4A Pro only); tested build23 and build22 are retained separately' } else { '22' }))
$notice | Set-Content -LiteralPath (Join-Path $package 'licenses\THIRD-PARTY-NOTICES.txt') -Encoding utf8
& (Join-Path $PSScriptRoot 'Collect-Licenses.ps1') -Destination (Join-Path $package 'licenses\third-party')
'This folder stores only this portable copy''s history, backups and exports. Keep it with the application.' | Set-Content -LiteralPath (Join-Path $package 'data\README.txt') -Encoding utf8
$forbidden = Get-ChildItem -LiteralPath $package -Recurse -File | Where-Object { $_.Name -match '\.(jks|keystore|pem|pfx|p12)$' -or $_.Name -match '^adbkey' }
if ($forbidden) { throw 'A private key was found in the staging package. Packaging stopped.' }
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'Verify-Package.ps1') -Destination (Join-Path $package 'Verify-Package.ps1')
$records = @(Get-ChildItem -LiteralPath $package -Recurse -File | Sort-Object FullName | ForEach-Object {
  [pscustomobject]@{path=$_.FullName.Substring($package.Length+1).Replace('\','/'); bytes=$_.Length; sha256=(Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant()}
})
$manifest = [ordered]@{schemaVersion=1; application='TV Optimizer'; version=$version; architecture='windows-x64'; createdUtc=[DateTime]::UtcNow.ToString('o'); files=$records}
$manifest | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $package 'integrity-manifest.json') -Encoding utf8
& (Join-Path $package 'Verify-Package.ps1') -Root $package
$zip = $null
if (-not $NoZip) {
  $releases = Join-Path $repo 'releases'
  New-Item -ItemType Directory -Path $releases -Force | Out-Null
  $zip = Join-Path $releases "$zipLabel.zip"
  Add-Type -AssemblyName System.IO.Compression.FileSystem
  if (Test-Path -LiteralPath $zip) { throw "ZIP already exists: $zip. Preserve or move the earlier release before packaging again." }
  [System.IO.Compression.ZipFile]::CreateFromDirectory($staging,$zip,[System.IO.Compression.CompressionLevel]::SmallestSize,$false)
}
[pscustomobject]@{package=$package; zip=$zip; files=$records.Count; unpackedBytes=($records | Measure-Object -Property bytes -Sum).Sum} | ConvertTo-Json
