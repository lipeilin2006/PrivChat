param(
  [string]$Device = "emulator-5554",
  [switch]$ClearData
)

$ErrorActionPreference = "Stop"
$repo = Split-Path -Parent $PSScriptRoot
$apk = Join-Path $repo "privchat-client\src-tauri\gen\android\app\build\outputs\apk\universal\debug\app-universal-debug.apk"
$package = "com.keydown.privchat_client"

adb devices | Select-String $Device | Out-Null
if ($LASTEXITCODE -ne 0) { throw "Android device is not online: $Device" }
if (-not (Test-Path $apk)) { throw "APK not found: $apk" }

if ($ClearData) {
  adb uninstall $package | Out-Host
  adb install $apk | Out-Host
} else {
  adb install -r $apk | Out-Host
  if ($LASTEXITCODE -ne 0) {
    adb uninstall $package | Out-Host
    adb install $apk | Out-Host
  }
}
adb shell am force-stop $package
adb shell monkey -p $package 1 | Out-Host
