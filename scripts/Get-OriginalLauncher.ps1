param([Parameter(Mandatory=$true)][string]$Root,[switch]$AcceptSdkTerms,[string]$ExistingToolchainRoot)
$ErrorActionPreference='Stop'
$ProgressPreference='SilentlyContinue'
[Net.ServicePointManager]::SecurityProtocol=[Net.SecurityProtocolType]::Tls12
$Root=[IO.Path]::GetFullPath($Root)
$work=Join-Path $Root 'data\original-builder'
$output=Join-Path $Root 'data\launchers'
New-Item -ItemType Directory -Force -Path $work,$output | Out-Null
function Progress([int]$n,[string]$message){@{completed=$n;message=$message}|ConvertTo-Json -Compress|Set-Content -LiteralPath (Join-Path $work 'progress.json') -Encoding UTF8}
function Download([string]$url,[string]$path,[string]$hash){
  if(-not $url.StartsWith('https://')){throw 'Only secure download sources are allowed.'}
  if(Test-Path -LiteralPath $path){if($hash-and(Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash-eq$hash){return}}
  $part=$path+'.part'
  Invoke-WebRequest -UseBasicParsing -Uri $url -OutFile $part -TimeoutSec 900
  if($hash-and(Get-FileHash -LiteralPath $part -Algorithm SHA256).Hash-ne$hash){throw 'A tool download did not match its published SHA-256. Nothing was installed on the TV.'}
  Move-Item -LiteralPath $part -Destination $path -Force
}
function ExpandSafe([string]$zip,[string]$destination){
  Add-Type -AssemblyName System.IO.Compression.FileSystem
  $destination=[IO.Path]::GetFullPath($destination).TrimEnd('\')
  New-Item -ItemType Directory -Force -Path $destination|Out-Null
  $archive=[IO.Compression.ZipFile]::OpenRead($zip)
  try{foreach($entry in $archive.Entries){$target=[IO.Path]::GetFullPath((Join-Path $destination $entry.FullName));if(-not$target.StartsWith($destination+'\',[StringComparison]::OrdinalIgnoreCase)){throw 'The source archive contains an invalid path.'};if($entry.FullName.EndsWith('/')){New-Item -ItemType Directory -Force -Path $target|Out-Null}else{New-Item -ItemType Directory -Force -Path (Split-Path -Parent $target)|Out-Null;[IO.Compression.ZipFileExtensions]::ExtractToFile($entry,$target,$true)}}}finally{$archive.Dispose()}
}
try{
  Progress 3 'Checking the official CleanYourTV release'
  $headers=@{'User-Agent'='TVOptimizer/0.1'}
  $release=Invoke-RestMethod -Headers $headers -Uri 'https://api.github.com/repos/Gathin23/cleanyourtv/releases/latest' -TimeoutSec 30
  if($release.draft-or$release.prerelease-or$release.tag_name-notmatch'^[A-Za-z0-9._-]{1,80}$'){throw 'The official project did not provide a supported stable release.'}
  $commit=Invoke-RestMethod -Headers $headers -Uri ('https://api.github.com/repos/Gathin23/cleanyourtv/commits/'+$release.tag_name) -TimeoutSec 30
  if($commit.sha-notmatch'^[0-9a-f]{40}$'){throw 'The release revision could not be verified.'}
  $revision=$commit.sha
  $sourceZip=Join-Path $work ($revision+'.zip')
  Progress 8 'Downloading the official launcher source'
  Download ('https://codeload.github.com/Gathin23/cleanyourtv/zip/'+$revision) $sourceZip ''
  $sources=Join-Path $work 'sources'
  $source=Join-Path $sources ('cleanyourtv-'+$revision+'\launcher')
  if(-not(Test-Path -LiteralPath $source)){ExpandSafe $sourceZip $sources}
  $buildFile=Join-Path $source 'app\build.gradle'
  if(-not(Test-Path -LiteralPath $buildFile)){throw 'This release changed its build layout. Keep SwiftOS or wait for a compatible tool update.'}
  $buildText=Get-Content -LiteralPath $buildFile -Raw
  if(-not$buildText.Contains("gradleProperty('launcherApplicationId')")){throw 'This release cannot use a separate launcher identity safely. SwiftOS has not been replaced.'}
  $sdkMatch=[regex]::Match($buildText,'compileSdk\s+(\d+)')
  if(-not$sdkMatch.Success){throw 'The required Android SDK could not be established.'}
  $compileSdk=[int]$sdkMatch.Groups[1].Value
  if($compileSdk-lt26-or$compileSdk-gt40){throw 'This release needs an unsupported Android build platform.'}
  $tools=Join-Path $work 'tools'
  New-Item -ItemType Directory -Force -Path $tools|Out-Null
  if($ExistingToolchainRoot){
    $existing=[IO.Path]::GetFullPath($ExistingToolchainRoot)
    $env:JAVA_HOME=(Get-ChildItem -LiteralPath (Join-Path $existing 'java') -Directory|Select-Object -First 1).FullName
    $env:ANDROID_HOME=Join-Path $existing 'android-sdk'
    $env:GRADLE_USER_HOME=Join-Path (Split-Path -Parent $existing) 'gradle-cache'
    $gradle=Join-Path $existing 'gradle-8.9\bin\gradle.bat'
  }else{
    $agreement=Join-Path $work 'sdk-terms-accepted.json'
    if(-not(Test-Path -LiteralPath $agreement)){
      if(-not$AcceptSdkTerms){throw 'Read and accept the Android SDK terms in the app before downloading build tools.'}
      @{acceptedUtc=[DateTime]::UtcNow.ToString('o');source='https://developer.android.com/studio#command-line-tools-only'}|ConvertTo-Json|Set-Content -LiteralPath $agreement -Encoding UTF8
    }
    Progress 15 'Preparing Java for the background build'
    $javaFolder=Join-Path $tools 'java'
    $java=Get-ChildItem -LiteralPath $javaFolder -Filter java.exe -Recurse -ErrorAction SilentlyContinue|Where-Object{$_.Directory.Name-eq'bin'}|Select-Object -First 1
    if(-not$java){
      $packages=Invoke-RestMethod -Uri 'https://api.adoptium.net/v3/assets/latest/21/hotspot?architecture=x64&image_type=jdk&os=windows&vendor=eclipse' -TimeoutSec 30
      $binary=$packages[0].binary.package
      if($binary.link-notlike'https://github.com/adoptium/temurin21-binaries/releases/download/*'-or$binary.checksum-notmatch'^[0-9a-f]{64}$'){throw 'The Java download metadata was not recognized.'}
      $javaZip=Join-Path $tools 'java.zip';Download $binary.link $javaZip $binary.checksum;ExpandSafe $javaZip $javaFolder
      $java=Get-ChildItem -LiteralPath $javaFolder -Filter java.exe -Recurse|Where-Object{$_.Directory.Name-eq'bin'}|Select-Object -First 1
    }
    $env:JAVA_HOME=$java.Directory.Parent.FullName
    Progress 30 'Preparing the Android build tools'
    $env:ANDROID_HOME=Join-Path $tools 'android-sdk'
    $sdkManager=Join-Path $env:ANDROID_HOME 'cmdline-tools\latest\bin\sdkmanager.bat'
    if(-not(Test-Path -LiteralPath $sdkManager)){
      $sdkZip=Join-Path $tools 'android-command-tools.zip'
      Download 'https://dl.google.com/android/repository/commandlinetools-win-15859902_latest.zip' $sdkZip '90ae805d20434428bffcb699c290860f19bb5f66a67e6b330067e3de801fb04a'
      $sdkStage=Join-Path $tools 'sdk-stage';ExpandSafe $sdkZip $sdkStage
      New-Item -ItemType Directory -Force -Path (Split-Path -Parent $sdkManager)|Out-Null
      Copy-Item -Path (Join-Path $sdkStage 'cmdline-tools\*') -Destination (Join-Path $env:ANDROID_HOME 'cmdline-tools\latest') -Recurse -Force
    }
    $env:PATH=(Join-Path $env:JAVA_HOME 'bin')+';'+$env:PATH
    1..100|ForEach-Object{'y'}|& $sdkManager --sdk_root=$env:ANDROID_HOME --licenses
    if($LASTEXITCODE-ne0){throw 'The Android build tools could not be prepared.'}
    & $sdkManager --sdk_root=$env:ANDROID_HOME ('platforms;android-'+$compileSdk) 'build-tools;34.0.0' 'platform-tools'
    if($LASTEXITCODE-ne0){throw 'The required Android platform could not be downloaded.'}
    $env:GRADLE_USER_HOME=Join-Path $tools 'gradle-cache'
    $gradle=Join-Path $source 'gradlew.bat'
  }
  $env:PATH=(Join-Path $env:JAVA_HOME 'bin')+';'+$env:PATH
  $env:ANDROID_USER_HOME=Join-Path $work 'android-user'
  New-Item -ItemType Directory -Force -Path $env:ANDROID_USER_HOME|Out-Null
  ('sdk.dir='+$env:ANDROID_HOME.Replace('\','/'))|Set-Content -LiteralPath (Join-Path $source 'local.properties') -Encoding ASCII
  Progress 55 'Building CleanYourTV; the TV is unchanged'
  Push-Location -LiteralPath $source
  try{& $gradle assembleRelease '-PlauncherApplicationId=com.gathin.tvhome.original' --no-daemon --console=plain --max-workers=2 '-Dorg.gradle.jvmargs=-Xmx1536m';if($LASTEXITCODE-ne0){throw 'The launcher build did not finish. The TV has not been changed.'}}finally{Pop-Location}
  Progress 88 'Checking the launcher package and signature'
  $apk=Join-Path $source 'app\build\outputs\apk\release\app-release.apk'
  $aapt=Join-Path $env:ANDROID_HOME 'build-tools\34.0.0\aapt.exe'
  $signer=Join-Path $env:ANDROID_HOME 'build-tools\34.0.0\apksigner.bat'
  $badging=(& $aapt dump badging $apk 2>&1)-join"`n";if($LASTEXITCODE-ne0){throw 'The APK metadata could not be inspected.'}
  if($badging-notmatch"package: name='com.gathin.tvhome.original' versionCode='(\d+)' versionName='([^']+)'" ){throw 'The package identity was unexpected. SwiftOS remains protected.'}
  $versionCode=[long]$Matches[1];$versionName=$Matches[2]
  if($badging-notmatch"sdkVersion:'(\d+)'" ){throw 'The Android requirements could not be inspected.'};$minSdk=[int]$Matches[1]
  $signature=(& $signer verify --verbose --print-certs $apk 2>&1)-join"`n";if($LASTEXITCODE-ne0-or$signature-notmatch'Signer #1 certificate SHA-256 digest: ([0-9a-fA-F]{64})'){throw 'The APK signing verification failed.'};$cert=$Matches[1].ToLowerInvariant()
  $destination=Join-Path $output 'original.apk';Copy-Item -LiteralPath $apk -Destination $destination -Force
  $manifest=@{schemaVersion=1;source='https://github.com/Gathin23/cleanyourtv';releaseTag=[string]$release.tag_name;revision=$revision;sourceSha256=(Get-FileHash -LiteralPath $sourceZip -Algorithm SHA256).Hash.ToLowerInvariant();applicationId='com.gathin.tvhome.original';versionCode=$versionCode;versionName=$versionName;minimumAndroidSdk=$minSdk;sha256=(Get-FileHash -LiteralPath $destination -Algorithm SHA256).Hash.ToLowerInvariant();signerSha256=$cert;builtAtUtc=[DateTime]::UtcNow.ToString('o');identityChange='Separate application ID only; launcher behavior comes from upstream.'}
  $manifest|ConvertTo-Json|Set-Content -LiteralPath (Join-Path $output 'original-manifest.json') -Encoding UTF8
  Progress 100 'CleanYourTV is ready; review its installation in the app'
}catch{Progress 0 ('Preparation stopped: '+$_.Exception.Message);throw}
