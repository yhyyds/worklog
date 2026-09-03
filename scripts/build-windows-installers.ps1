param(
  [string]$OutputDirectory = "artifacts"
)

$ErrorActionPreference = "Stop"
$repositoryRoot = Split-Path -Parent $PSScriptRoot
Set-Location $repositoryRoot

$version = (Get-Content "package.json" -Raw | ConvertFrom-Json).version
$bundleDirectory = Join-Path $repositoryRoot "src-tauri/target/release/bundle/nsis"
$outputPath = if ([System.IO.Path]::IsPathRooted($OutputDirectory)) {
  $OutputDirectory
} else {
  Join-Path $repositoryRoot $OutputDirectory
}
New-Item -ItemType Directory -Force -Path $outputPath | Out-Null

function Copy-LatestInstaller {
  param([string]$DestinationName)

  $installer = Get-ChildItem -Path (Join-Path $bundleDirectory "*-setup.exe") |
    Sort-Object LastWriteTimeUtc -Descending |
    Select-Object -First 1
  if (-not $installer) {
    throw "NSIS installer not found in $bundleDirectory"
  }
  Copy-Item -Force $installer.FullName (Join-Path $outputPath $DestinationName)
}

& npm run bundle:windows:no-webview
if ($LASTEXITCODE -ne 0) {
  throw "Windows installer build without embedded WebView2 failed"
}
Copy-LatestInstaller "Worklog-$version-windows-x64-no-webview2-setup.exe"

& npm run bundle:windows:with-webview
if ($LASTEXITCODE -ne 0) {
  throw "Windows installer build with offline WebView2 failed"
}
Copy-LatestInstaller "Worklog-$version-windows-x64-with-webview2-setup.exe"

$installers = @(Get-ChildItem -Path (Join-Path $outputPath "*.exe") | Sort-Object Name)
if ($installers.Count -ne 2) {
  throw "Expected exactly two Windows installers, found $($installers.Count)"
}
$installers
