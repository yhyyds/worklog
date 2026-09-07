[CmdletBinding()]
param(
    [string]$OutputDirectory = ""
)

$ErrorActionPreference = "Stop"

function Get-Sha256Hex {
    param([Parameter(Mandatory = $true)][string]$Path)
    $stream = [IO.File]::OpenRead($Path)
    try {
        $sha = [Security.Cryptography.SHA256]::Create()
        try {
            $bytes = $sha.ComputeHash($stream)
            return ([BitConverter]::ToString($bytes)).Replace("-", "").ToLowerInvariant()
        } finally {
            $sha.Dispose()
        }
    } finally {
        $stream.Dispose()
    }
}

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$targetBundleRoot = Join-Path $repoRoot "src-tauri\target\release\bundle"
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

$artifactName = "Worklog_${version}_x64-no-webview2-setup.exe"

$artifactPath = Join-Path $OutputDirectory $artifactName
if (Test-Path -LiteralPath $artifactPath) { Remove-Item -LiteralPath $artifactPath -Force }
foreach ($name in @("SHA256SUMS.txt", "BUILD-INFO.txt")) {
    $path = Join-Path $OutputDirectory $name
    if (Test-Path -LiteralPath $path) { Remove-Item -LiteralPath $path -Force }
}

function Build-Installer {
    param([Parameter(Mandatory = $true)][string]$ConfigPath)

    $bundleDirectory = Join-Path $targetBundleRoot "nsis"
    New-Item -ItemType Directory -Force -Path $bundleDirectory | Out-Null
    Get-ChildItem -LiteralPath $bundleDirectory -File -ErrorAction SilentlyContinue | Remove-Item -Force

    $arguments = @("run", "tauri", "--", "build", "--bundles", "nsis", "--ci", "--config", $ConfigPath)

    Write-Host "Building no-WebView2 NSIS package..."
    & npm.cmd @arguments | Out-Host
    if ($LASTEXITCODE -ne 0) {
        throw "Tauri build failed for no-WebView2 NSIS with exit code $LASTEXITCODE"
    }

    $built = @(Get-ChildItem -LiteralPath $bundleDirectory -Filter "*-setup.exe" -File)
    if ($built.Count -ne 1) {
        throw "Expected exactly one no-WebView2 NSIS package, found $($built.Count)"
    }

    $destination = Join-Path $OutputDirectory $artifactName
    Copy-Item -LiteralPath $built[0].FullName -Destination $destination
    Get-Item -LiteralPath $destination
}

$lockHashBefore = Get-Sha256Hex -Path $cargoLock
Push-Location $repoRoot
try {
    $withoutWebViewNsis = Build-Installer -ConfigPath "src-tauri/tauri.no-webview2.conf.json"
} finally {
    Pop-Location
}
$lockHashAfter = Get-Sha256Hex -Path $cargoLock

if ($lockHashBefore -ne $lockHashAfter) { throw "Cargo.lock changed during the package build" }
$packages = @($withoutWebViewNsis)
$checksumLines = foreach ($installer in $packages) {
    $hash = Get-Sha256Hex -Path $installer.FullName
    "$hash  $($installer.Name)"
}
$checksumPath = Join-Path $OutputDirectory "SHA256SUMS.txt"
[IO.File]::WriteAllLines($checksumPath, $checksumLines, [Text.Encoding]::ASCII)

foreach ($installer in $packages) {
    $recorded = $checksumLines | Where-Object { $_ -like "*  $($installer.Name)" }
    $actual = Get-Sha256Hex -Path $installer.FullName
    if ($recorded -ne "$actual  $($installer.Name)") {
        throw "Checksum verification failed for $($installer.Name)"
    }
}

$commit = (& git rev-parse HEAD).Trim()
$buildInfo = @(
    "Worklog Windows build information"
    "Version: $version"
    "Commit: $commit"
    "BuiltAtUtc: $([DateTime]::UtcNow.ToString('o'))"
    "Runner: $env:RUNNER_OS $env:RUNNER_ARCH"
    ""
    "Authenticode note: publisher metadata is not a digital signature."
    "Unsigned packages may be blocked by Microsoft Defender SmartScreen or organization policy."
    "This package requires Microsoft Edge WebView2 Runtime to be installed on the target computer."
    "Use SHA256SUMS.txt for integrity checking and contact IT when organization policy blocks unsigned software."
    ""
)
$authenticodeCommand = Get-Command -Name Get-AuthenticodeSignature -ErrorAction SilentlyContinue
foreach ($installer in $packages) {
    if ($null -ne $authenticodeCommand) {
        $signature = Get-AuthenticodeSignature -LiteralPath $installer.FullName
        $signatureStatus = [string]$signature.Status
        $signer = if ($null -ne $signature.SignerCertificate) { $signature.SignerCertificate.Subject } else { "None" }
    } else {
        $signatureStatus = "Unavailable"
        $signer = "Unavailable"
    }
    $hash = Get-Sha256Hex -Path $installer.FullName
    $buildInfo += "File: $($installer.Name)"
    $buildInfo += "Size: $($installer.Length)"
    $buildInfo += "SHA256: $hash"
    $buildInfo += "AuthenticodeStatus: $signatureStatus"
    $buildInfo += "Signer: $signer"
    $buildInfo += ""
}
$buildInfoPath = Join-Path $OutputDirectory "BUILD-INFO.txt"
$buildInfo | Set-Content -LiteralPath $buildInfoPath -Encoding utf8

Write-Host "Windows package created:"
$packages | Select-Object Name, Length, FullName | Format-Table -AutoSize
Write-Host "Checksums: $checksumPath"
Write-Host "Build information: $buildInfoPath"
