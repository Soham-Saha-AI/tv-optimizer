param()
$ErrorActionPreference = 'Stop'
$v3OptimizerRoot = Split-Path -Parent $PSScriptRoot
$v3TrialRoot = (Resolve-Path -LiteralPath (Join-Path $v3OptimizerRoot '../cleanyourtv-trial')).Path
$v3Source = Join-Path $v3OptimizerRoot '.tools/swiftos-v3-source'
$v3Evidence = Join-Path $v3OptimizerRoot 'design/swiftos-v3-wallpaper-build24'
$v3CandidateDir = Join-Path $v3OptimizerRoot 'launchers/candidates'
$env:JAVA_HOME = (Get-ChildItem -LiteralPath (Join-Path $v3TrialRoot 'toolchain/java') -Directory | Select-Object -First 1).FullName
$env:ANDROID_HOME = Join-Path $v3TrialRoot 'toolchain/android-sdk'
# Existing signer is used in place. Never read, copy, export or package the private key.
$env:ANDROID_USER_HOME = Join-Path $v3TrialRoot 'toolchain/android-user'
$env:GRADLE_USER_HOME = Join-Path $v3TrialRoot 'gradle-cache'
('sdk.dir=' + ($env:ANDROID_HOME -replace '\\','/')) | Set-Content -LiteralPath (Join-Path $v3Source 'local.properties') -Encoding ascii
New-Item -ItemType Directory -Path $v3Evidence,$v3CandidateDir -Force | Out-Null
$v3Baseline = Join-Path $v3OptimizerRoot 'launchers/liquid-glass.apk'
$v3BaselineHash = (Get-FileHash -LiteralPath $v3Baseline -Algorithm SHA256).Hash
if ($v3BaselineHash -ne 'FF6F0C1F93C086E803532F40B7B2165E1C820CD62E588DA233FCDF96A4123C06') { throw 'Build22 baseline changed; stop before building v3' }
$v3Tasks = @('testReleaseUnitTest','lintRelease','assembleRelease','--offline','--no-daemon','--console=plain')
& (Join-Path $v3TrialRoot 'toolchain/gradle-8.9/bin/gradle.bat') -p $v3Source @v3Tasks 2>&1 | Tee-Object -FilePath (Join-Path $v3Evidence 'build.log')
if ($LASTEXITCODE -ne 0) { throw "SwiftOS v3 build/checks failed ($LASTEXITCODE)" }
$v3Apk = Join-Path $v3Source 'app/build/outputs/apk/release/app-release.apk'
$v3Tools = Get-ChildItem -LiteralPath (Join-Path $env:ANDROID_HOME 'build-tools') -Directory | Sort-Object Name -Descending | Select-Object -First 1
$v3Signature = & (Join-Path $v3Tools.FullName 'apksigner.bat') verify --verbose --print-certs $v3Apk 2>&1
if ($LASTEXITCODE -ne 0) { throw 'Candidate APK signature did not verify' }
$v3Signature | Set-Content -LiteralPath (Join-Path $v3Evidence 'apk-signature.txt')
if (($v3Signature -join "`n") -notmatch 'e0e52087919f09bb6bfe5d5fc7e986b643a2bf1f3d49b40e8827f87e3ac5f986') { throw 'Signer continuity failed; update is blocked' }
$v3Metadata = & (Join-Path $v3Tools.FullName 'aapt.exe') dump badging $v3Apk 2>&1
if ($LASTEXITCODE -ne 0) { throw 'Cannot inspect candidate metadata' }
$v3Metadata | Set-Content -LiteralPath (Join-Path $v3Evidence 'apk-metadata.txt')
if (($v3Metadata -join "`n") -notmatch "package: name='com.gathin.tvhome' versionCode='24' versionName='3.0.1-swiftos'") { throw 'Unexpected candidate identity/version' }
$v3Tests = @(Get-ChildItem -LiteralPath (Join-Path $v3Source 'app/build/test-results/testReleaseUnitTest') -Filter 'TEST-*.xml' | ForEach-Object {
    [xml]$v3Result = Get-Content -LiteralPath $_.FullName
    [pscustomobject]@{Name=$v3Result.testsuite.name; Tests=[int]$v3Result.testsuite.tests; Failures=[int]$v3Result.testsuite.failures; Errors=[int]$v3Result.testsuite.errors}
})
$v3Tests | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $v3Evidence 'test-summary.json')
if (($v3Tests | Measure-Object -Property Failures -Sum).Sum -ne 0 -or ($v3Tests | Measure-Object -Property Errors -Sum).Sum -ne 0) { throw 'Unit test report contains failures' }
Copy-Item -LiteralPath (Join-Path $v3Source 'app/build/reports/lint-results-release.html') -Destination (Join-Path $v3Evidence 'lint-results-release.html')
Copy-Item -LiteralPath (Join-Path $v3Source 'app/build/reports/lint-results-release.xml') -Destination (Join-Path $v3Evidence 'lint-results-release.xml')
Add-Type -AssemblyName System.IO.Compression.FileSystem
$v3Zip = [IO.Compression.ZipFile]::OpenRead($v3Apk)
try {
    $v3Entries = @($v3Zip.Entries | ForEach-Object FullName)
    if ($v3Entries | Where-Object { $_ -match '(?i)\.(jks|keystore|pem|p12|pfx)$' }) { throw 'Signing/private-key material is forbidden in APK' }
    if ('assets/licenses/Inter-OFL.txt' -notin $v3Entries) { throw 'Bundled font licence missing' }
    # Ship only optimized PNG bytes verified against both desktop RGBA and API28 BitmapFactory ARGB pixels.
    $v3PixelProofPath = Join-Path $v3OptimizerRoot 'design/swiftos-v3-optimization/optimized-png-proof.json'
    $v3PixelProof = Get-Content -Raw -LiteralPath $v3PixelProofPath | ConvertFrom-Json
    if ($v3PixelProof.baseline_apk_sha256 -ne '0c2238bdd010e99dcc05c87d6039124d5f4c63f4e9c501f869980a5e42a6fbef' -or
        -not $v3PixelProof.all_rgba_exact -or @($v3PixelProof.entries).Count -ne 78 -or
        $v3PixelProof.android_api -ne 28 -or $v3PixelProof.android_exact_comparisons -ne 156) { throw 'Optimized PNG proof is incomplete or has the wrong baseline' }
    $v3AndroidPixelLog = Join-Path $v3OptimizerRoot 'design/swiftos-v3-optimization/android-pixel-proof/device-png-proof.txt'
    if ((Get-FileHash -LiteralPath $v3AndroidPixelLog -Algorithm SHA256).Hash -ne $v3PixelProof.android_proof_sha256 -or
        (Get-Content -Raw -LiteralPath $v3AndroidPixelLog) -notmatch 'PIXEL_PROOF PASS assets=78 comparisons=156 failures=0') { throw 'Native Android PNG pixel proof changed or did not pass' }
    if (@($v3Entries | Where-Object { $_ -match '^assets/.*\.webp$' }).Count -ne 0) { throw 'Experimental WebP artwork must not ship' }
    if (@($v3Entries | Where-Object { $_ -match '^assets/.*\.png$' }).Count -ne 78) { throw 'Unexpected bundled PNG artwork count' }
    if (@($v3PixelProof.unchanged_theme_jpgs).Count -ne 4 -or
        @($v3Entries | Where-Object { $_ -match '^assets/themes/.*\.(png|jpg)$' }).Count -ne 5) { throw 'All five bundled theme images must remain present' }
    $v3PixelHasher = [Security.Cryptography.SHA256]::Create()
    try {
        foreach ($v3Pixel in $v3PixelProof.entries) {
            if (-not $v3Pixel.rgba_exact -or $v3Pixel.decoded_rgba_sha256 -ne $v3Pixel.roundtrip_rgba_sha256 -or
                $v3Pixel.native_straight_argb_sha256 -notmatch '^[a-f0-9]{64}$' -or
                $v3Pixel.native_premultiplied_argb_sha256 -notmatch '^[a-f0-9]{64}$') { throw "Artwork pixel proof missing: $($v3Pixel.source)" }
            $v3Original = Join-Path $v3OptimizerRoot (Join-Path 'design/swiftos-v3-optimization/original-png' $v3Pixel.source)
            if ((Get-FileHash -LiteralPath $v3Original -Algorithm SHA256).Hash -ne $v3Pixel.source_sha256) { throw "Artwork source proof changed: $($v3Pixel.source)" }
            $v3EncodedEntry = $v3Zip.GetEntry($v3Pixel.source)
            if ($null -eq $v3EncodedEntry) { throw "Optimized PNG missing: $($v3Pixel.source)" }
            $v3EncodedStream = $v3EncodedEntry.Open()
            try { $v3EncodedHash = [BitConverter]::ToString($v3PixelHasher.ComputeHash($v3EncodedStream)).Replace('-', '').ToLowerInvariant() }
            finally { $v3EncodedStream.Dispose() }
            if ($v3EncodedHash -ne $v3Pixel.png_sha256) { throw "Native-proven PNG encoding changed: $($v3Pixel.source)" }
        }
        foreach ($v3Theme in $v3PixelProof.unchanged_theme_jpgs) {
            $v3ThemeEntry = $v3Zip.GetEntry($v3Theme.source)
            if ($null -eq $v3ThemeEntry) { throw "Bundled theme missing: $($v3Theme.source)" }
            $v3ThemeStream = $v3ThemeEntry.Open()
            try { $v3ThemeHash = [BitConverter]::ToString($v3PixelHasher.ComputeHash($v3ThemeStream)).Replace('-', '').ToLowerInvariant() }
            finally { $v3ThemeStream.Dispose() }
            if ($v3ThemeHash -ne $v3Theme.sha256) { throw "Bundled JPEG theme changed: $($v3Theme.source)" }
        }
    } finally { $v3PixelHasher.Dispose() }

} finally { $v3Zip.Dispose() }
$v3Candidate = Join-Path $v3CandidateDir 'SwiftOS-3.0.1-24.apk'
Copy-Item -LiteralPath $v3Apk -Destination $v3Candidate -Force
if ((Get-FileHash -LiteralPath $v3Baseline -Algorithm SHA256).Hash -ne $v3BaselineHash) { throw 'Build22 changed during v3 build' }
$v3Report = [pscustomobject]@{
    Package='com.gathin.tvhome'; VersionCode=24; VersionName='3.0.1-swiftos'; BasedOn='SwiftOS 2.7 build22, selective Library reveal fix; no full upstream CleanYourTV v3 rebase';
    Apk=$v3Candidate; SHA256=(Get-FileHash -LiteralPath $v3Candidate -Algorithm SHA256).Hash; Bytes=(Get-Item -LiteralPath $v3Candidate).Length;
    Tests=($v3Tests | Measure-Object -Property Tests -Sum).Sum; SignerSHA256='e0e52087919f09bb6bfe5d5fc7e986b643a2bf1f3d49b40e8827f87e3ac5f986';
    R8=$true; LintCompleted=$true; DeviceValidated=$false; BaselineSHA256=$v3BaselineHash;
    ArtworkEncoding='Lossless optimized PNG'; ArtworkNativePixelComparisons=156; ArtworkProofSHA256=(Get-FileHash -LiteralPath $v3PixelProofPath -Algorithm SHA256).Hash
}
$v3Report | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $v3Evidence 'candidate.json')
$v3Report | Format-List
