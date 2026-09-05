[CmdletBinding()]
param(
    [string]$OutputDirectory = ""
)

$ErrorActionPreference = "Stop"
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$bundleDirectory = Join-Path $repoRoot "src-tauri\target\release\bundle\nsis"
$cargoLock = Join-Path $repoRoot "src-tauri\Cargo.lock"
$package = Get-Content -Raw -LiteralPath (Join-Path $repoRoot "package.json") | ConvertFrom-Json
$version = $package.version

if ([string]::IsNullOrWhiteSpace($OutputDirectory)) {
    $OutputDirectory = Join-Path $repoRoot "artifacts\windows"
} elseif (-not [IO.Path]::IsPathRooted($OutputDirectory)) {
    $OutputDirectory = Join-Path $repoRoot $OutputDirectory
}
$OutputDirectory = [IO.Path]::GetFullPath($OutputDirectory)
New-Item -ItemType Directory -Force -Path $OutputDirectory | Out-Null

$artifactNames = @{
    "no-webview2" = "Worklog_${version}_x64-no-webview2-setup.exe"
    "with-webview2" = "Worklog_${version}_x64-with-webview2-setup.exe"
}

foreach ($name in $artifactNames.Values) {
    $path = Join-Path $OutputDirectory $name
    if (Test-Path -LiteralPath $path) {
        Remove-Item -LiteralPath $path -Force
    }
}
$checksumPath = Join-Path $OutputDirectory "SHA256SUMS.txt"
if (Test-Path -LiteralPath $checksumPath) {
    Remove-Item -LiteralPath $checksumPath -Force
}

function Build-Installer {
    param(
        [Parameter(Mandatory = $true)][string]$Variant,
        [string]$ConfigPath = ""
    )

    New-Item -ItemType Directory -Force -Path $bundleDirectory | Out-Null
    Get-ChildItem -LiteralPath $bundleDirectory -Filter "*-setup.exe" -File -ErrorAction SilentlyContinue |
        Remove-Item -Force

    $arguments = @("run", "tauri", "--", "build", "--bundles", "nsis", "--ci")
    if (-not [string]::IsNullOrWhiteSpace($ConfigPath)) {
        $arguments += @("--config", $ConfigPath)
    }

    Write-Host "Building $Variant installer..."
    & npm.cmd @arguments | Out-Host
    if ($LASTEXITCODE -ne 0) {
        throw "Tauri build failed for $Variant with exit code $LASTEXITCODE"
    }

    $built = @(Get-ChildItem -LiteralPath $bundleDirectory -Filter "*-setup.exe" -File)
    if ($built.Count -ne 1) {
        throw "Expected exactly one NSIS installer for $Variant, found $($built.Count)"
    }

    $destination = Join-Path $OutputDirectory $artifactNames[$Variant]
    Copy-Item -LiteralPath $built[0].FullName -Destination $destination
    Get-Item -LiteralPath $destination
}

$lockHashBefore = (Get-FileHash -LiteralPath $cargoLock -Algorithm SHA256).Hash
Push-Location $repoRoot
try {
    $withoutWebView = Build-Installer -Variant "no-webview2" -ConfigPath "src-tauri/tauri.no-webview2.conf.json"
    $withWebView = Build-Installer -Variant "with-webview2"
} finally {
    Pop-Location
}
$lockHashAfter = (Get-FileHash -LiteralPath $cargoLock -Algorithm SHA256).Hash

if ($lockHashBefore -ne $lockHashAfter) {
    throw "Cargo.lock changed during the installer build"
}
if ($withWebView.Length -le $withoutWebView.Length) {
    throw "The offline WebView2 installer should be larger than the no-WebView2 installer"
}

$installers = @($withoutWebView, $withWebView)
$checksumLines = foreach ($installer in $installers) {
    $hash = (Get-FileHash -LiteralPath $installer.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
    "$hash  $($installer.Name)"
}
[IO.File]::WriteAllLines($checksumPath, $checksumLines, [Text.Encoding]::ASCII)

foreach ($installer in $installers) {
    $recorded = $checksumLines | Where-Object { $_ -like "*  $($installer.Name)" }
    $actual = (Get-FileHash -LiteralPath $installer.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($recorded -ne "$actual  $($installer.Name)") {
        throw "Checksum verification failed for $($installer.Name)"
    }
}

Write-Host "Windows installers created:"
$installers | Select-Object Name, Length, FullName | Format-Table -AutoSize
Write-Host "Checksums: $checksumPath"
