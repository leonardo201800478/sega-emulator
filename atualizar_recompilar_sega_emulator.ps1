param(
    [string]$RomPath,
    [switch]$Full,
    [switch]$Launch
)

$ErrorActionPreference = "Stop"
$RepoRoot = (Get-Location).Path
$VenvPython = Join-Path $RepoRoot ".venv\Scripts\python.exe"
$VenvMaturin = Join-Path $RepoRoot ".venv\Scripts\maturin.exe"

function Invoke-Checked {
    param(
        [Parameter(Mandatory = $true)][string]$File,
        [Parameter(Mandatory = $true)][string[]]$Arguments,
        [Parameter(Mandatory = $true)][string]$Step
    )
    Write-Host "`n==> $Step" -ForegroundColor Cyan
    & $File @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "Falha em '$Step' (exit code $LASTEXITCODE)."
    }
}

if (-not (Test-Path (Join-Path $RepoRoot ".git")) -or
    -not (Test-Path (Join-Path $RepoRoot "Cargo.toml"))) {
    throw "Execute na raiz do repositorio sega-emulator."
}

$Branch = (& git branch --show-current).Trim()
if ($LASTEXITCODE -ne 0 -or $Branch -ne "main") {
    throw "A branch atual e '$Branch'. Este atualizador sincroniza somente main; preserve seu trabalho e mude de branch manualmente."
}

$Status = @(& git status --porcelain --untracked-files=all)
if ($LASTEXITCODE -ne 0) { throw "Nao foi possivel verificar o Git." }
if ($Status.Count -gt 0) {
    $Status | ForEach-Object { Write-Host $_ -ForegroundColor Yellow }
    throw "Arvore de trabalho nao esta limpa. Nada foi alterado; preserve/commite seu trabalho antes do pull."
}

$BeforeSha = (& git rev-parse HEAD).Trim()
Invoke-Checked -File "git" -Arguments @("fetch", "origin", "main", "--prune") -Step "Consultar main remota"
$RemoteSha = (& git rev-parse origin/main).Trim()
if ($LASTEXITCODE -ne 0) { throw "Nao foi possivel resolver origin/main." }

$Counts = (& git rev-list --left-right --count "HEAD...origin/main").Trim() -split '\s+'
if ($LASTEXITCODE -ne 0 -or $Counts.Count -ne 2) { throw "Falha ao comparar HEAD local com origin/main." }
$Ahead = [int]$Counts[0]
$Behind = [int]$Counts[1]
if ($Ahead -gt 0) {
    throw "A main local tem $Ahead commit(s) que nao estao em origin/main. Nenhum reset/force-pull sera executado."
}
if ($Behind -gt 0) {
    Invoke-Checked -File "git" -Arguments @("merge", "--ff-only", "origin/main") -Step "Avancar main local por fast-forward"
}
$LocalSha = (& git rev-parse HEAD).Trim()
if ($LocalSha -ne $RemoteSha) { throw "HEAD local nao corresponde a origin/main apos sincronizacao." }
Write-Host "Repositorio sincronizado: $LocalSha" -ForegroundColor Green
if ($BeforeSha -eq $LocalSha) { Write-Host "Sem commits novos; build incremental sera reutilizado quando possivel." }

if (-not (Test-Path $VenvPython)) {
    Invoke-Checked -File "py" -Arguments @("-3.14", "-m", "venv", ".venv") -Step "Criar ambiente virtual Python 3.14"
}
if (-not (Test-Path $VenvPython)) { throw "Python 3.14 nao encontrado. Instale-o e tente novamente." }

if (-not (Test-Path $VenvMaturin)) {
    Invoke-Checked -File $VenvPython -Arguments @("-m", "pip", "install", "maturin>=1.9,<2") -Step "Instalar Maturin ausente"
}
if (-not (Test-Path $VenvMaturin)) { throw "Maturin nao foi instalado no ambiente virtual." }

# Reinstala dependencias somente quando o manifesto Python mudou.
$Changed = @()
if ($BeforeSha -ne $LocalSha) {
    $Changed = @(& git diff --name-only $BeforeSha $LocalSha)
    if ($LASTEXITCODE -ne 0) { throw "Falha ao identificar arquivos alterados no pull." }
}
if ($Changed -contains "pyproject.toml") {
    Invoke-Checked -File $VenvPython -Arguments @("-m", "pip", "install", "-e", ".[dev,qt]") -Step "Atualizar dependencias Python alteradas"
}

if ($Full) {
    Invoke-Checked -File "cargo" -Arguments @("fmt", "--all", "--", "--check") -Step "Rust fmt workspace"
    Invoke-Checked -File "cargo" -Arguments @("clippy", "--workspace", "--all-targets", "--locked", "--", "-D", "warnings") -Step "Rust Clippy workspace"
    Invoke-Checked -File "cargo" -Arguments @("test", "--workspace", "--locked") -Step "Testes Rust workspace"
    Invoke-Checked -File "cargo" -Arguments @("fmt", "--manifest-path", "native/z80_core/Cargo.toml", "--all", "--", "--check") -Step "Rust fmt PyO3"
    Invoke-Checked -File "cargo" -Arguments @("clippy", "--manifest-path", "native/z80_core/Cargo.toml", "--locked", "--all-targets", "--", "-D", "warnings") -Step "Rust Clippy PyO3"
    Invoke-Checked -File "cargo" -Arguments @("test", "--manifest-path", "native/z80_core/Cargo.toml", "--locked") -Step "Testes Rust PyO3"
}

Write-Host "`n==> Build incremental Release + instalacao PyO3" -ForegroundColor Cyan
Push-Location (Join-Path $RepoRoot "native\z80_core")
try {
    Invoke-Checked -File $VenvMaturin -Arguments @("develop", "--release", "--locked", "--features", "python-extension") -Step "Maturin Release"
}
finally { Pop-Location }

Invoke-Checked -File $VenvPython -Arguments @("-c", "import sega_z80_core as m; assert hasattr(m, 'NativeSMSMachine'); assert hasattr(m, 'NativeSG1000Machine'); print('PyO3 SMS/SG-1000 OK')") -Step "Smoke test da extensao nativa"

if ($Full) {
    Invoke-Checked -File $VenvPython -Arguments @("-m", "compileall", "src") -Step "Python compileall"
    Invoke-Checked -File $VenvPython -Arguments @("-m", "pip", "check") -Step "Dependencias Python"
    Invoke-Checked -File $VenvPython -Arguments @("-m", "ruff", "check", ".") -Step "Ruff lint"
    Invoke-Checked -File $VenvPython -Arguments @("-m", "ruff", "format", "--check", ".") -Step "Ruff format"
    Invoke-Checked -File $VenvPython -Arguments @("-m", "pytest", "-q") -Step "Suite Python completa"
}
else {
    Write-Host "Modo rapido: build Release e smoke test concluidos. Use -Full para a bateria completa." -ForegroundColor Yellow
}

if ($Launch -or -not [string]::IsNullOrWhiteSpace($RomPath)) {
    $AppArgs = @("-m", "sega_emulator")
    if (-not [string]::IsNullOrWhiteSpace($RomPath)) {
        $ResolvedRom = (Resolve-Path $RomPath).Path
        $AppArgs += $ResolvedRom
    }
    Invoke-Checked -File $VenvPython -Arguments $AppArgs -Step "Abrir frontend Qt"
}

Write-Host "`nCONCLUIDO — SHA $LocalSha" -ForegroundColor Green
