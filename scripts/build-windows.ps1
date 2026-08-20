param(
  [switch]$Release
)

$ErrorActionPreference = "Stop"
$repo = Split-Path -Parent $PSScriptRoot
$client = Join-Path $repo "privchat-client"
$profile = if ($Release) { "release" } else { "debug" }

Write-Host "[build] Windows client and mailbox ($profile)"
Push-Location $repo
try {
  cargo build --workspace $(if ($Release) { "--release" })
  $target = Join-Path $repo "target\$profile"
  Copy-Item (Join-Path $target "privchat-client.exe") (Join-Path $repo "client0\privchat-client.exe") -Force
  Copy-Item (Join-Path $target "privchat-client.exe") (Join-Path $repo "client1\privchat-client.exe") -Force
  Copy-Item (Join-Path $target "privchat-mailbox.exe") (Join-Path $repo "mailbox0\privchat-mailbox.exe") -Force
  Copy-Item (Join-Path $target "privchat-mailbox.exe") (Join-Path $repo "mailbox1\privchat-mailbox.exe") -Force
} finally {
  Pop-Location
}
