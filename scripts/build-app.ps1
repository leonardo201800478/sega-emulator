[CmdletBinding()]
param(
    [switch]$Release,
    [switch]$Launch
)

$ErrorActionPreference = "Stop"
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$appManifest = Join-Path $repoRoot "native\sega_app\Cargo.toml"
$buildConfiguration = if ($Release) { "release" } else { "debug" }

. (Join-Path $PSScriptRoot "common.ps1")

if (-not (Test-Path -LiteralPath $appManifest -PathType Leaf)) {
    throw "Manifest da aplicacao nao encontrado: $appManifest"
}

$cargo = Get-Command cargo -ErrorAction SilentlyContinue
if ($null -eq $cargo) {
    throw "Cargo nao encontrado. Instale a toolchain definida em rust-toolchain.toml."
}

# Centraliza os artefatos, inclusive os crates que possuem workspace proprio.
$env:CARGO_TARGET_DIR = Join-Path $repoRoot "target"
Initialize-SegaMsvcEnvironment

$arguments = @(
    "build",
    "--manifest-path", $appManifest,
    "--package", "sega-app",
    "--bin", "sega-app",
    "--locked"
)
if ($Release) { $arguments += "--release" }

Invoke-SegaChecked -File $cargo.Source -Arguments $arguments -Step "Build da aplicacao sega-app ($buildConfiguration)"

$binary = Join-Path $env:CARGO_TARGET_DIR (Join-Path $buildConfiguration "sega-app.exe")
if (-not (Test-Path -LiteralPath $binary -PathType Leaf)) {
    throw "Build terminou sem produzir o executavel esperado: $binary"
}

Write-Host "Aplicacao compilada: $binary" -ForegroundColor Green

if ($Launch) {
    Start-Process -FilePath $binary -WorkingDirectory $repoRoot
}
