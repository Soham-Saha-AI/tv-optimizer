param()
$ErrorActionPreference = 'Stop'
$optimizerBuildRoot = Split-Path -Parent $PSScriptRoot
$trialToolRoot = (Resolve-Path -LiteralPath (Join-Path $optimizerBuildRoot '../cleanyourtv-trial')).Path
$customBuildSource = Join-Path $optimizerBuildRoot '.tools/custom-launcher-source'
$env:JAVA_HOME = (Get-ChildItem -LiteralPath (Join-Path $trialToolRoot 'toolchain/java') -Directory | Select-Object -First 1).FullName
$env:ANDROID_HOME = Join-Path $trialToolRoot 'toolchain/android-sdk'
# The existing debug certificate is read at build time only. It is never copied into the portable app.
$env:ANDROID_USER_HOME = Join-Path $trialToolRoot 'toolchain/android-user'
$env:GRADLE_USER_HOME = Join-Path $trialToolRoot 'gradle-cache'
('sdk.dir=' + ($env:ANDROID_HOME -replace '\\','/')) | Set-Content -LiteralPath (Join-Path $customBuildSource 'local.properties') -Encoding ascii
$customGradleTasks = @('testReleaseUnitTest',
  '--tests','com.gathin.tvhome.NumberedAppOrderDialogTest',
  '--tests','com.gathin.tvhome.OrderTest',
  '--tests','com.gathin.tvhome.DockSlotsTest',
  '--tests','com.gathin.tvhome.ThemeFilesTest',
  '--tests','com.gathin.tvhome.LibraryAppsTest',
  '--tests','com.gathin.tvhome.LibraryGridTest',
  '--tests','com.gathin.tvhome.RemoteKeysTest',
  '--tests','com.gathin.tvhome.HomeDoublePressTest',
  '--tests','com.gathin.tvhome.WeatherReadingTest',
  '--tests','com.gathin.tvhome.MotionTest',
  '--tests','com.gathin.tvhome.GlassDirectionTest',
  '--tests','com.gathin.tvhome.DeviceDefaultsTest',
  'assembleRelease','--offline','--no-daemon','--console=plain')
$buildEvidenceRoot = Join-Path $optimizerBuildRoot 'design/custom-launcher-22'
New-Item -ItemType Directory -Path $buildEvidenceRoot -Force | Out-Null
& (Join-Path $trialToolRoot 'toolchain/gradle-8.9/bin/gradle.bat') -p $customBuildSource @customGradleTasks 2>&1 | Tee-Object -FilePath (Join-Path $buildEvidenceRoot 'build.log')
if ($LASTEXITCODE -ne 0) { throw "Custom launcher build failed ($LASTEXITCODE)" }
$customApkPath = Join-Path $customBuildSource 'app/build/outputs/apk/release/app-release.apk'
$signatureTool = Get-ChildItem -LiteralPath (Join-Path $env:ANDROID_HOME 'build-tools') -Directory | Sort-Object Name -Descending | Select-Object -First 1
$signatureOutput = & (Join-Path $signatureTool.FullName 'apksigner.bat') verify --verbose --print-certs $customApkPath 2>&1
if ($LASTEXITCODE -ne 0) { throw 'The compatibility APK signature did not verify' }
$signatureOutput | Set-Content -LiteralPath (Join-Path $buildEvidenceRoot 'apk-signature.txt')
if (($signatureOutput -join "`n") -notmatch 'e0e52087919f09bb6bfe5d5fc7e986b643a2bf1f3d49b40e8827f87e3ac5f986') { throw 'The compatibility APK signer differs from code 21; a preference-preserving update is blocked' }
$customTests = Get-ChildItem -LiteralPath (Join-Path $customBuildSource 'app/build/test-results/testReleaseUnitTest') -Filter 'TEST-*.xml' | ForEach-Object {
  [xml]$testResult = Get-Content -LiteralPath $_.FullName
  [pscustomobject]@{Name=$testResult.testsuite.name;Tests=[int]$testResult.testsuite.tests;Failures=[int]$testResult.testsuite.failures;Errors=[int]$testResult.testsuite.errors}
}
$customTests | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $buildEvidenceRoot 'test-summary.json')
Get-FileHash -LiteralPath $customApkPath -Algorithm SHA256 | Format-List
$customTests | Measure-Object -Property Tests -Sum | Select-Object -ExpandProperty Sum
