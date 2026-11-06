[CmdletBinding()]
param([switch]$Release, [switch]$AllWorkspaces)
# Fast compile checks Rust code without running tests.
$ErrorActionPreference = "Stop"
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
. (Join-Path $PSScriptRoot "common.ps1")
Initialize-SegaMsvcEnvironment
$cargo = Get-Command cargo -ErrorAction SilentlyContinue
if ($null -eq $cargo) { throw "Cargo nao encontrado. Instale a toolchain definida em rust-toolchain.toml." }
$manifests = @((Join-Path $repoRoot "Cargo.toml"))
if ($AllWorkspaces) {
    $manifests += Join-Path $repoRoot "native/audio_output/Cargo.toml"
    $manifests += Join-Path $repoRoot "native/sega_app/Cargo.toml"
}
$mode = if ($Release) { "release" } else { "dev" }
foreach ($manifest in ($manifests | Select-Object -Unique)) {
    if (-not (Test-Path -LiteralPath $manifest -PathType Leaf)) { continue }
    $arguments = @("check", "--workspace", "--locked", "--manifest-path", $manifest)
    if ($Release) { $arguments += "--release" }
    Invoke-SegaChecked -File $cargo.Source -Arguments $arguments -Step "Cargo check ($mode): $manifest"
}
Write-Host "BUILD RAPIDO CONCLUIDO - $($manifests.Count) workspace(s), perfil $mode." -ForegroundColor Green
