param([switch]$DownloadOnly)
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
$upstreamRoot = Join-Path $repo '.tools\upstream'
New-Item -ItemType Directory -Force -Path $upstreamRoot | Out-Null
$archive = Join-Path $upstreamRoot 'cleanyourtv-v2.7.0.zip'
if (-not (Test-Path -LiteralPath $archive)) {
  Invoke-WebRequest -Uri 'https://github.com/Gathin23/cleanyourtv/archive/refs/tags/v2.7.0.zip' -OutFile $archive
}
$source = Join-Path $upstreamRoot 'cleanyourtv-2.7.0\launcher'
if (-not (Test-Path -LiteralPath $source)) { Expand-Archive -LiteralPath $archive -DestinationPath $upstreamRoot }
if ($DownloadOnly) { Write-Host "Original source: $source"; return }
$trial = Join-Path (Split-Path -Parent $repo) 'cleanyourtv-trial'
$env:JAVA_HOME = (Get-ChildItem -LiteralPath (Join-Path $trial 'toolchain\java') -Directory | Select-Object -First 1).FullName
$env:ANDROID_HOME = Join-Path $trial 'toolchain\android-sdk'
$env:ANDROID_USER_HOME = Join-Path $trial 'toolchain\android-user'
$env:GRADLE_USER_HOME = Join-Path $trial 'gradle-cache'
('sdk.dir=' + ($env:ANDROID_HOME -replace '\\','/')) | Set-Content -LiteralPath (Join-Path $source 'local.properties') -Encoding ascii
Push-Location $source
try {
  & (Join-Path $trial 'toolchain\gradle-8.9\bin\gradle.bat') assembleRelease '-PlauncherApplicationId=com.gathin.tvhome.original' --no-daemon --console=plain
  if ($LASTEXITCODE -ne 0) { throw 'Original launcher compilation failed.' }
} finally { Pop-Location }
$launcherDirectory = Join-Path $repo 'launchers'
New-Item -ItemType Directory -Force -Path $launcherDirectory | Out-Null
Copy-Item -LiteralPath (Join-Path $source 'app\build\outputs\apk\release\app-release.apk') -Destination (Join-Path $launcherDirectory 'original.apk')
$hash = (Get-FileHash -LiteralPath (Join-Path $launcherDirectory 'original.apk') -Algorithm SHA256).Hash.ToLowerInvariant()
[pscustomobject]@{source='https://github.com/Gathin23/cleanyourtv/releases/tag/v2.7.0'; sourceArchiveSha256=(Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant(); applicationId='com.gathin.tvhome.original'; versionCode=8; versionName='2.7'; minimumAndroidSdk=26; targetAndroidSdk=34; sha256=$hash; modification='Only launcherApplicationId Gradle property supplied; upstream sources unchanged.'; signerSha256='e0e52087919f09bb6bfe5d5fc7e986b643a2bf1f3d49b40e8827f87e3ac5f986'} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $launcherDirectory 'original-manifest.json') -Encoding utf8
Write-Host "Original launcher SHA256: $hash"
