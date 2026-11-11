[CmdletBinding()]
param(
    [switch]$Release,
    [switch]$AllWorkspaces
)

# Verifica rapidamente todos os workspaces ativos e a aplicacao desktop.
$ErrorActionPreference = "Stop"
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
. (Join-Path $PSScriptRoot "common.ps1")

$cargo = Get-Command cargo -ErrorAction SilentlyContinue
if ($null -eq $cargo) {
    throw "Cargo nao encontrado. Instale a toolchain definida em rust-toolchain.toml."
}

$env:CARGO_TARGET_DIR = Join-Path $repoRoot "target"
Initialize-SegaMsvcEnvironment

$manifests = @(
    (Join-Path $repoRoot "Cargo.toml"),
    (Join-Path $repoRoot "native\sega_app\Cargo.toml")
)

# Inclui crates standalone adicionais somente quando existirem no checkout.
if ($AllWorkspaces) {
    $optionalManifests = @(
        (Join-Path $repoRoot "native\audio_output\Cargo.toml")
    )
    $manifests += @($optionalManifests | Where-Object { Test-Path -LiteralPath $_ -PathType Leaf })
}

$mode = if ($Release) { "release" } else { "dev" }
foreach ($manifest in ($manifests | Select-Object -Unique)) {
    if (-not (Test-Path -LiteralPath $manifest -PathType Leaf)) {
        throw "Manifest esperado nao encontrado: $manifest"
    }

    $arguments = @("check", "--workspace", "--locked", "--manifest-path", $manifest)
    if ($Release) { $arguments += "--release" }
    Invoke-SegaChecked -File $cargo.Source -Arguments $arguments -Step "Cargo check ($mode): $manifest"
}

Write-Host "CHECK RAPIDO CONCLUIDO - $(@($manifests | Select-Object -Unique).Count) workspace(s), perfil $mode." -ForegroundColor Green
