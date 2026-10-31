[CmdletBinding()]
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

function Get-GitOutput {
    param([Parameter(Mandatory = $true)][string[]]$Arguments)

    $Output = @(& git @Arguments)
    if ($LASTEXITCODE -ne 0) {
        throw "Falha no comando git $($Arguments -join ' ')."
    }
    return $Output
}

if (-not (Test-Path (Join-Path $RepoRoot ".git")) -or
    -not (Test-Path (Join-Path $RepoRoot "Cargo.toml"))) {
    throw "Execute na raiz do repositorio sega-emulator."
}

# Este atualizador opera somente na main. Nao troca branches nem descarta commits.
$Branch = ((Get-GitOutput -Arguments @("branch", "--show-current")) -join "").Trim()
if ($Branch -ne "main") {
    throw "A branch atual e '$Branch'. Este atualizador sincroniza somente main; preserve seu trabalho e mude de branch manualmente."
}

# Preserva arquivos locais nao rastreados em artifact/. Qualquer outra alteracao
# (staged, tracked ou untracked) bloqueia a sincronizacao para evitar perda de trabalho.
$Status = @(Get-GitOutput -Arguments @("status", "--porcelain=v1", "--untracked-files=all"))
$BlockingStatus = @()
$PreservedArtifacts = @()
foreach ($Entry in $Status) {
    if ($Entry.Length -lt 3) {
        $BlockingStatus += $Entry
        continue
    }

    $IndexStatus = $Entry.Substring(0, 2)
    $Path = $Entry.Substring(3).Trim('"') -replace '\\', '/'
    if ($IndexStatus -eq "??" -and ($Path -eq "artifact" -or $Path.StartsWith("artifact/", [System.StringComparison]::OrdinalIgnoreCase))) {
        $PreservedArtifacts += $Entry
    }
    else {
        $BlockingStatus += $Entry
    }
}

if ($PreservedArtifacts.Count -gt 0) {
    Write-Host "Arquivos nao rastreados em artifact/ serao preservados:" -ForegroundColor Yellow
    $PreservedArtifacts | ForEach-Object { Write-Host "  $_" -ForegroundColor Yellow }
}
if ($BlockingStatus.Count -gt 0) {
    $BlockingStatus | ForEach-Object { Write-Host $_ -ForegroundColor Yellow }
    throw "Ha alteracoes locais fora de artifact/. Nada foi sincronizado; preserve/commite seu trabalho antes de continuar."
}

$BeforeSha = ((Get-GitOutput -Arguments @("rev-parse", "HEAD")) -join "").Trim()
Invoke-Checked -File "git" -Arguments @("fetch", "origin", "main", "--prune") -Step "Consultar main remota"
$RemoteSha = ((Get-GitOutput -Arguments @("rev-parse", "origin/main")) -join "").Trim()

$Counts = ((Get-GitOutput -Arguments @("rev-list", "--left-right", "--count", "HEAD...origin/main")) -join "").Trim() -split '\s+'
if ($Counts.Count -ne 2) {
    throw "Falha ao comparar HEAD local com origin/main."
}
$Ahead = [int]$Counts[0]
$Behind = [int]$Counts[1]
if ($Ahead -gt 0) {
    throw "A main local tem $Ahead commit(s) que nao estao em origin/main. Nenhum reset/force-pull sera executado."
}
if ($Behind -gt 0) {
    Invoke-Checked -File "git" -Arguments @("merge", "--ff-only", "origin/main") -Step "Avancar main local por fast-forward"
}

$LocalSha = ((Get-GitOutput -Arguments @("rev-parse", "HEAD")) -join "").Trim()
if ($LocalSha -ne $RemoteSha) {
    throw "HEAD local nao corresponde a origin/main apos sincronizacao."
}
Write-Host "Repositorio sincronizado: $LocalSha" -ForegroundColor Green
if ($BeforeSha -eq $LocalSha) {
    Write-Host "Sem commits novos; builds incrementais serao reutilizados quando possivel."
}

# Instala o ambiente Python quando necessario e atualiza dependencias se algum
# manifesto/arquivo de dependencias mudou no intervalo sincronizado.
$NewVenv = -not (Test-Path $VenvPython)
if ($NewVenv) {
    Invoke-Checked -File "py" -Arguments @("-3.14", "-m", "venv", ".venv") -Step "Criar ambiente virtual Python 3.14"
}
if (-not (Test-Path $VenvPython)) {
    throw "Python 3.14 nao encontrado. Instale-o e tente novamente."
}

$Changed = @()
if ($BeforeSha -ne $LocalSha) {
    $Changed = @(Get-GitOutput -Arguments @("diff", "--name-only", $BeforeSha, $LocalSha))
}
$DependencyFiles = @(
    "pyproject.toml",
    "setup.py",
    "setup.cfg",
    "requirements.txt",
    "requirements-dev.txt",
    "requirements-qt.txt",
    "uv.lock",
    "poetry.lock",
    "Pipfile",
    "Pipfile.lock"
)
$DependenciesChanged = $false
foreach ($ChangedFile in $Changed) {
    if ($DependencyFiles -contains $ChangedFile -or
        $ChangedFile -match '(^|/)(requirements|constraints)[^/]*\.txt$') {
        $DependenciesChanged = $true
        break
    }
}
if ($NewVenv -or $DependenciesChanged) {
    Invoke-Checked -File $VenvPython -Arguments @("-m", "pip", "install", "-e", ".[dev,qt]") -Step "Instalar/atualizar dependencias Python do projeto"
}

if (-not (Test-Path $VenvMaturin)) {
    Invoke-Checked -File $VenvPython -Arguments @("-m", "pip", "install", "maturin>=1.9,<2") -Step "Instalar Maturin ausente"
}
if (-not (Test-Path $VenvMaturin)) {
    throw "Maturin nao foi instalado no ambiente virtual."
}

if ($Full) {
    Invoke-Checked -File "cargo" -Arguments @("fmt", "--all", "--", "--check") -Step "Rust fmt workspace"
    Invoke-Checked -File "cargo" -Arguments @("clippy", "--workspace", "--all-targets", "--locked", "--", "-D", "warnings") -Step "Rust Clippy workspace"
    Invoke-Checked -File "cargo" -Arguments @("test", "--workspace", "--locked") -Step "Testes Rust workspace"
}

# O build do workspace e executado tambem no modo rapido. O build Maturin abaixo
# e separado porque produz/instala a extensao Python PyO3.
Invoke-Checked -File "cargo" -Arguments @("build", "--workspace", "--release", "--locked") -Step "Build Release do workspace Rust"

if ($Full) {
    Invoke-Checked -File "cargo" -Arguments @("fmt", "--manifest-path", "native/z80_core/Cargo.toml", "--all", "--", "--check") -Step "Rust fmt PyO3"
    Invoke-Checked -File "cargo" -Arguments @("clippy", "--manifest-path", "native/z80_core/Cargo.toml", "--locked", "--all-targets", "--", "-D", "warnings") -Step "Rust Clippy PyO3"
    Invoke-Checked -File "cargo" -Arguments @("test", "--manifest-path", "native/z80_core/Cargo.toml", "--locked") -Step "Testes Rust PyO3"
}

Write-Host "`n==> Build Release da extensao PyO3" -ForegroundColor Cyan
Push-Location (Join-Path $RepoRoot "native\z80_core")
try {
    Invoke-Checked -File $VenvMaturin -Arguments @("develop", "--release", "--locked", "--features", "python-extension") -Step "Maturin Release"
}
finally {
    Pop-Location
}

Invoke-Checked -File $VenvPython -Arguments @("-c", "import sega_z80_core as m; assert hasattr(m, 'NativeSMSMachine'); assert hasattr(m, 'NativeSG1000Machine'); print('PyO3 SMS/SG-1000 OK')") -Step "Smoke test da extensao nativa"

if ($Full) {
    Invoke-Checked -File $VenvPython -Arguments @("-m", "compileall", "src") -Step "Python compileall"
    Invoke-Checked -File $VenvPython -Arguments @("-m", "pip", "check") -Step "Dependencias Python"
    # Executa pytest antes do lint para que uma falha de lint nao impeça a coleta
    # dos resultados dos testes funcionais.
    Invoke-Checked -File $VenvPython -Arguments @("-m", "pytest", "-q") -Step "Suite Python completa"
    Invoke-Checked -File $VenvPython -Arguments @("-m", "ruff", "check", ".") -Step "Ruff lint"
    Invoke-Checked -File $VenvPython -Arguments @("-m", "ruff", "format", "--check", ".") -Step "Ruff format"
}
else {
    Write-Host "Modo rapido: build Release do workspace, build Maturin e smoke test concluidos. Use -Full para a bateria completa." -ForegroundColor Yellow
}

if ($Launch -or -not [string]::IsNullOrWhiteSpace($RomPath)) {
    $AppArgs = @("-m", "sega_emulator")
    if (-not [string]::IsNullOrWhiteSpace($RomPath)) {
        if (-not (Test-Path $RomPath -PathType Leaf)) {
            throw "ROM nao encontrada: $RomPath"
        }
        $ResolvedRom = (Resolve-Path $RomPath).Path
        $AppArgs += $ResolvedRom
    }
    Invoke-Checked -File $VenvPython -Arguments $AppArgs -Step "Abrir frontend Qt"
}

$FinalStatus = @(Get-GitOutput -Arguments @("status", "--short", "--branch"))
Write-Host "`nEstado final do Git:" -ForegroundColor Cyan
$FinalStatus | ForEach-Object { Write-Host $_ }
Write-Host "`nCONCLUIDO — SHA $LocalSha" -ForegroundColor Green
