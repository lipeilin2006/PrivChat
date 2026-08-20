param(
  [string]$Device = "emulator-5554"
)

$ErrorActionPreference = "Stop"
$package = "com.keydown.privchat_client"
$activity = "$package/.MainActivity"

adb devices | Select-String $Device | Out-Null
if ($LASTEXITCODE -ne 0) { throw "Android device is not online: $Device" }

function Assert-AppForeground([string]$step) {
  $state = adb shell dumpsys activity activities
  if ($state -notmatch [regex]::Escape($activity)) {
    throw "App left foreground during: $step"
  }
  Write-Host "[android-nav] $step: ok"
}

adb shell am force-stop $package
adb shell monkey -p $package 1 | Out-Null
Start-Sleep -Seconds 2
Assert-AppForeground "launch"

# The script verifies that system back never crashes the activity. Page-level
# assertions remain manual because the password gate and contacts are data-dependent.
1..3 | ForEach-Object {
  adb shell input keyevent 4
  Start-Sleep -Milliseconds 500
  Assert-AppForeground "back $_"
}

Write-Host "[android-nav] completed"
