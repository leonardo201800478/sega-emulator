[CmdletBinding()]
param([switch]$Release, [switch]$AllWorkspaces)
# Fast compile checks Rust compilation without running tests.
$ErrorActionPreference = "Stop"
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
. (Join-Path $PSScriptRoot "common.ps1")
Initialize-SegaMsvcEnvironment
$python = Get-SegaPython -RepositoryRoot $repoRoot
Initialize-SegaPythonRuntime -PythonExecutable $python
$cargo = Get-Command cargo -ErrorAction SilentlyContinue
if ($null -eq $cargo) { throw "Cargo nao encontrado. Instale a toolchain definida em rust-toolchain.toml." }
$manifests = @((Join-Path $repoRoot "Cargo.toml"))
if ($AllWorkspaces) {
    $env:SEGA_REPOSITORY_ROOT = $repoRoot
    $discovery = & $python -c "import os,sys; sys.path.insert(0,os.environ['SEGA_REPOSITORY_ROOT']); from tools.project_manager import discover_workspaces; ws,errors,unassigned=discover_workspaces(); print('|'.join(w.manifest for w in ws if not w.is_root)); raise SystemExit(1 if errors else 0)"
    if ($LASTEXITCODE -ne 0) { throw "Falha ao descobrir workspaces Cargo ativos." }
    foreach ($manifest in ($discovery -split "\|" | Where-Object { $_.Trim() })) { $manifests += Join-Path $repoRoot $manifest }
}
$mode = if ($Release) { "release" } else { "dev" }
foreach ($manifest in ($manifests | Select-Object -Unique)) {
    $arguments = @("check", "--workspace", "--locked", "--manifest-path", $manifest)
    if ($Release) { $arguments += "--release" }
    Invoke-SegaChecked -File $cargo.Source -Arguments $arguments -Step "Cargo check ($mode): $manifest"
}
Write-Host ""
Write-Host "BUILD RAPIDO CONCLUIDO - $($manifests.Count) workspace(s), perfil $mode." -ForegroundColor Green
