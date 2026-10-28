param([Parameter(Mandatory=$true)][string]$Root)
$ErrorActionPreference='Stop'
$package=[IO.Path]::GetFullPath($Root)
$executable=Join-Path $package 'TV Optimizer.exe'
if(-not(Test-Path -LiteralPath $executable)){throw 'Choose a complete staged portable folder.'}
& (Join-Path $package 'Verify-Package.ps1') -Root $package
$process=Start-Process -FilePath $executable -WorkingDirectory $package -WindowStyle Hidden -PassThru
try {
  $runtimeProcesses=@()
  for($attempt=0;$attempt-lt30;$attempt++) {
    Start-Sleep -Milliseconds 500
    $process.Refresh()
    if($process.HasExited){throw "Portable app closed unexpectedly: exit $($process.ExitCode)."}
    $runtimeProcesses=@(Get-Process -Name msedgewebview2 -ErrorAction SilentlyContinue | Where-Object { $_.Path -and $_.Path.StartsWith((Join-Path $package 'runtime\WebView2\'),[StringComparison]::OrdinalIgnoreCase) })
    if($process.MainWindowHandle-ne0-and$runtimeProcesses.Count-ge2){break}
  }
  if($process.MainWindowHandle-eq0){throw 'The app did not create a Windows window.'}
  if($runtimeProcesses.Count-lt2){throw 'The bundled WebView2 browser and renderer were not observed.'}
  if(-not(Test-Path -LiteralPath (Join-Path $package 'data\history.sqlite3'))){throw 'Portable history was not created beside the app.'}
  if(-not(Test-Path -LiteralPath (Join-Path $package 'data\webview'))){throw 'WebView data was not stored beside the app.'}
  [pscustomobject]@{result='passed';window=$process.MainWindowTitle;nativeBytes=(Get-Item -LiteralPath $executable).Length;workingSetMB=[math]::Round($process.WorkingSet64/1MB,1);bundledWebViewProcesses=$runtimeProcesses.Count;portableData=$true;limitations='Process and bundle smoke test on this PC. A clean Windows VM and physical remote/playback checks remain separate.'}|ConvertTo-Json
} finally {
  if(-not$process.HasExited){[void]$process.CloseMainWindow();if(-not$process.WaitForExit(5000)){Stop-Process -Id $process.Id -ErrorAction SilentlyContinue}}
}
