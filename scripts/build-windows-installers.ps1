[CmdletBinding()]
param(
    [string]$OutputDirectory = ""
)

$ErrorActionPreference = "Stop"
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

$artifactNames = @{
    "no-webview2-nsis" = "Worklog_${version}_x64-no-webview2-setup.exe"
    "with-webview2-nsis" = "Worklog_${version}_x64-with-webview2-setup.exe"
    "no-webview2-msi" = "Worklog_${version}_x64-no-webview2.msi"
}

foreach ($name in $artifactNames.Values) {
    $path = Join-Path $OutputDirectory $name
    if (Test-Path -LiteralPath $path) { Remove-Item -LiteralPath $path -Force }
}
foreach ($name in @("SHA256SUMS.txt", "BUILD-INFO.txt")) {
    $path = Join-Path $OutputDirectory $name
    if (Test-Path -LiteralPath $path) { Remove-Item -LiteralPath $path -Force }
}

function Build-Installer {
    param(
        [Parameter(Mandatory = $true)][string]$Variant,
        [Parameter(Mandatory = $true)][ValidateSet("nsis", "msi")][string]$Bundle,
        [string]$ConfigPath = ""
    )

    $bundleDirectory = Join-Path $targetBundleRoot $Bundle
    New-Item -ItemType Directory -Force -Path $bundleDirectory | Out-Null
    Get-ChildItem -LiteralPath $bundleDirectory -File -ErrorAction SilentlyContinue | Remove-Item -Force

    $arguments = @("run", "tauri", "--", "build", "--bundles", $Bundle, "--ci")
    if (-not [string]::IsNullOrWhiteSpace($ConfigPath)) {
        $arguments += @("--config", $ConfigPath)
    }

    Write-Host "Building $Variant $Bundle package..."
    & npm.cmd @arguments | Out-Host
    if ($LASTEXITCODE -ne 0) {
        throw "Tauri build failed for $Variant $Bundle with exit code $LASTEXITCODE"
    }

    $filter = if ($Bundle -eq "nsis") { "*-setup.exe" } else { "*.msi" }
    $built = @(Get-ChildItem -LiteralPath $bundleDirectory -Filter $filter -File)
    if ($built.Count -ne 1) {
        throw "Expected exactly one $Bundle package for $Variant, found $($built.Count)"
    }

    $key = "$Variant-$Bundle"
    $destination = Join-Path $OutputDirectory $artifactNames[$key]
    Copy-Item -LiteralPath $built[0].FullName -Destination $destination
    Get-Item -LiteralPath $destination
}

$lockHashBefore = (Get-FileHash -LiteralPath $cargoLock -Algorithm SHA256).Hash
Push-Location $repoRoot
try {
    $withoutWebViewNsis = Build-Installer -Variant "no-webview2" -Bundle "nsis" -ConfigPath "src-tauri/tauri.no-webview2.conf.json"
    $withWebViewNsis = Build-Installer -Variant "with-webview2" -Bundle "nsis"
    $withoutWebViewMsi = Build-Installer -Variant "no-webview2" -Bundle "msi" -ConfigPath "src-tauri/tauri.no-webview2.conf.json"
} finally {
    Pop-Location
}
$lockHashAfter = (Get-FileHash -LiteralPath $cargoLock -Algorithm SHA256).Hash

if ($lockHashBefore -ne $lockHashAfter) { throw "Cargo.lock changed during the package build" }
if ($withWebViewNsis.Length -le $withoutWebViewNsis.Length) {
    throw "The offline WebView2 installer should be larger than the no-WebView2 installer"
}

$packages = @($withoutWebViewNsis, $withWebViewNsis, $withoutWebViewMsi)
$checksumLines = foreach ($installer in $packages) {
    $hash = (Get-FileHash -LiteralPath $installer.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
    "$hash  $($installer.Name)"
}
$checksumPath = Join-Path $OutputDirectory "SHA256SUMS.txt"
[IO.File]::WriteAllLines($checksumPath, $checksumLines, [Text.Encoding]::ASCII)

foreach ($installer in $packages) {
    $recorded = $checksumLines | Where-Object { $_ -like "*  $($installer.Name)" }
    $actual = (Get-FileHash -LiteralPath $installer.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
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
    "Use SHA256SUMS.txt for integrity checking and submit the MSI to IT for approved deployment."
    ""
)
foreach ($installer in $packages) {
    $signature = Get-AuthenticodeSignature -LiteralPath $installer.FullName
    $signer = if ($null -ne $signature.SignerCertificate) { $signature.SignerCertificate.Subject } else { "None" }
    $hash = (Get-FileHash -LiteralPath $installer.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
    $buildInfo += "File: $($installer.Name)"
    $buildInfo += "Size: $($installer.Length)"
    $buildInfo += "SHA256: $hash"
    $buildInfo += "AuthenticodeStatus: $($signature.Status)"
    $buildInfo += "Signer: $signer"
    $buildInfo += ""
}
$buildInfoPath = Join-Path $OutputDirectory "BUILD-INFO.txt"
$buildInfo | Set-Content -LiteralPath $buildInfoPath -Encoding utf8

Write-Host "Windows packages created:"
$packages | Select-Object Name, Length, FullName | Format-Table -AutoSize
Write-Host "Checksums: $checksumPath"
Write-Host "Build information: $buildInfoPath"