[CmdletBinding()]
param(
    [switch]$Full,
    [switch]$Launch
)

$ErrorActionPreference = "Stop"
$repoRoot = (Resolve-Path $PSScriptRoot).Path
. (Join-Path $repoRoot "scripts\common.ps1")

function Get-GitOutput {
    param([Parameter(Mandatory = $true)][string[]]$Arguments)
    $output = @(& git -C $repoRoot @Arguments)
    if ($LASTEXITCODE -ne 0) {
        throw "Falha no comando git $($Arguments -join ' ')."
    }
    return ($output -join "").Trim()
}

if (-not (Test-Path -LiteralPath (Join-Path $repoRoot ".git"))) {
    throw "Execute este script na raiz do repositorio sega-emulator."
}

$branchName = Get-GitOutput -Arguments @("branch", "--show-current")
if ([string]::IsNullOrWhiteSpace($branchName)) {
    throw "HEAD destacado nao e permitido para atualizar/compilar."
}

$status = @(Get-GitOutput -Arguments @("status", "--porcelain=v1", "--untracked-files=all"))
if (-not [string]::IsNullOrWhiteSpace(($status -join ""))) {
    throw "Existem alteracoes locais. Preserve ou commit as alteracoes antes de atualizar."
}

Invoke-SegaChecked -File "git" -Arguments @("fetch", "origin", "main", "--prune") -Step "Atualizar referencias remotas"
$remoteMain = Get-GitOutput -Arguments @("rev-parse", "origin/main")
$head = Get-GitOutput -Arguments @("rev-parse", "HEAD")
& git -C $repoRoot merge-base --is-ancestor origin/main HEAD
if ($LASTEXITCODE -ne 0) {
    throw "A branch atual nao contem origin/main. Atualize-a por fast-forward/rebase controlado antes do build; nenhum reset ou merge automatico foi executado."
}

Write-Host "Branch: $branchName | HEAD: $head | origin/main: $remoteMain" -ForegroundColor Cyan

$cargo = Get-Command cargo -ErrorAction SilentlyContinue
if ($null -eq $cargo) {
    throw "Cargo nao encontrado. Instale a toolchain definida em rust-toolchain.toml."
}
$env:CARGO_TARGET_DIR = Join-Path $repoRoot "target"
Initialize-SegaMsvcEnvironment

$rootManifest = Join-Path $repoRoot "Cargo.toml"
$appManifest = Join-Path $repoRoot "native\sega_app\Cargo.toml"
if (-not (Test-Path -LiteralPath $appManifest -PathType Leaf)) {
    throw "Manifest da aplicacao nativa nao encontrado: $appManifest"
}

if ($Full) {
    Invoke-SegaChecked -File $cargo.Source -Arguments @("fmt", "--all", "--manifest-path", $rootManifest, "--", "--check") -Step "rustfmt workspace principal"
    Invoke-SegaChecked -File $cargo.Source -Arguments @("fmt", "--all", "--manifest-path", $appManifest, "--", "--check") -Step "rustfmt sega-app"
    Invoke-SegaChecked -File $cargo.Source -Arguments @("clippy", "--workspace", "--all-targets", "--locked", "--manifest-path", $rootManifest, "--", "-D", "warnings") -Step "Clippy workspace principal"
    Invoke-SegaChecked -File $cargo.Source -Arguments @("clippy", "--workspace", "--all-targets", "--locked", "--manifest-path", $appManifest, "--", "-D", "warnings") -Step "Clippy sega-app"
    Invoke-SegaChecked -File $cargo.Source -Arguments @("test", "--workspace", "--locked", "--manifest-path", $rootManifest) -Step "Testes workspace principal"
    Invoke-SegaChecked -File $cargo.Source -Arguments @("test", "--workspace", "--locked", "--manifest-path", $appManifest) -Step "Testes sega-app"
}

Invoke-SegaChecked -File $cargo.Source -Arguments @("build", "--workspace", "--release", "--locked", "--manifest-path", $rootManifest) -Step "Build Release workspace principal"
Invoke-SegaChecked -File $cargo.Source -Arguments @("build", "--release", "--locked", "--manifest-path", $appManifest, "--package", "sega-app", "--bin", "sega-app") -Step "Build Release aplicacao sega-app"

$appBinary = Join-Path $env:CARGO_TARGET_DIR "release\sega-app.exe"
if (-not (Test-Path -LiteralPath $appBinary -PathType Leaf)) {
    throw "Executavel da aplicacao nao foi gerado: $appBinary"
}

Write-Host "BUILD RUST CONCLUIDO: $appBinary" -ForegroundColor Green
Write-Host "Observacao: sega-app e atualmente apenas o shell egui; a integracao completa de SMS/gameplay ainda nao esta concluida." -ForegroundColor Yellow

if ($Launch) {
    Start-Process -FilePath $appBinary -WorkingDirectory $repoRoot
}
