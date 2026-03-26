param(
    [ValidateSet('Debug', 'Release')]
    [string]$Profile = 'Debug',

    [string[]]$Targets = @('aarch64-linux-android'),

    [string]$TargetDirRoot = (Join-Path (Join-Path $PSScriptRoot '..') 'target\packaged-apks'),

    [string]$KeystorePath,

    [string]$KeystorePassword
)

$ErrorActionPreference = 'Stop'

$exampleRoot = Resolve-Path (Join-Path $PSScriptRoot '..')
$manifestPath = Join-Path $exampleRoot 'Cargo.toml'
$profileDir = if ($Profile -eq 'Release') { 'release' } else { 'debug' }
$apkName = 'dear-imgui-android-smoke.apk'

$installedTargets = @(& rustup target list --installed)
$missingTargets = @($Targets | Where-Object { $_ -notin $installedTargets })
if ($missingTargets.Count -gt 0) {
    throw "Missing Rust Android targets: $($missingTargets -join ', '). Install them with: rustup target add $($missingTargets -join ' ')"
}

$originalReleaseKeystore = $env:CARGO_APK_RELEASE_KEYSTORE
$originalReleaseKeystorePassword = $env:CARGO_APK_RELEASE_KEYSTORE_PASSWORD

try {
    if ($Profile -eq 'Release') {
        if ($KeystorePath) {
            $env:CARGO_APK_RELEASE_KEYSTORE = (Resolve-Path $KeystorePath)
        }
        if ($KeystorePassword) {
            $env:CARGO_APK_RELEASE_KEYSTORE_PASSWORD = $KeystorePassword
        }

        if (-not $env:CARGO_APK_RELEASE_KEYSTORE -or -not $env:CARGO_APK_RELEASE_KEYSTORE_PASSWORD) {
            throw 'Release builds require CARGO_APK_RELEASE_KEYSTORE and CARGO_APK_RELEASE_KEYSTORE_PASSWORD, either as parameters or pre-set environment variables.'
        }
    }

    foreach ($target in $Targets) {
        $targetDir = Join-Path $TargetDirRoot $target
        New-Item -ItemType Directory -Force -Path $targetDir | Out-Null

        $cargoArgs = @(
            'apk2',
            'build',
            '--manifest-path', $manifestPath,
            '--target', $target,
            '--target-dir', $targetDir
        )

        if ($Profile -eq 'Release') {
            $cargoArgs += '--release'
        }

        Write-Host "Building $Profile APK for $target ..."
        & cargo @cargoArgs
        if ($LASTEXITCODE -ne 0) {
            throw "cargo apk2 build failed for $target"
        }

        $apkPath = Join-Path $targetDir "$profileDir\apk\$apkName"
        if (Test-Path $apkPath) {
            Write-Host "APK ready: $apkPath"
        } else {
            throw "Expected APK was not produced for ${target}: $apkPath"
        }
    }
}
finally {
    if ($null -eq $originalReleaseKeystore) {
        Remove-Item Env:CARGO_APK_RELEASE_KEYSTORE -ErrorAction SilentlyContinue
    } else {
        $env:CARGO_APK_RELEASE_KEYSTORE = $originalReleaseKeystore
    }

    if ($null -eq $originalReleaseKeystorePassword) {
        Remove-Item Env:CARGO_APK_RELEASE_KEYSTORE_PASSWORD -ErrorAction SilentlyContinue
    } else {
        $env:CARGO_APK_RELEASE_KEYSTORE_PASSWORD = $originalReleaseKeystorePassword
    }
}
