param(
    [Parameter(Position = 0)]
    [string]$RomPath
)

$ErrorActionPreference = "Stop"
$ExpectedMainSha = "8cf488d18774ad3ecb3e14c08d21e2a7b5f75f39"
$RepoRoot = (Get-Location).Path

function Invoke-Checked {
    param(
        [Parameter(Mandatory = $true)][string]$File,
        [Parameter(Mandatory = $true)][string[]]$Arguments,
        [Parameter(Mandatory = $true)][string]$Step
    )
    Write-Host "`n==> $Step" -ForegroundColor Cyan
    & $File @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "Falha em '$Step' (exit code $LASTEXITCODE). Execucao interrompida."
    }
}

if (-not (Test-Path (Join-Path $RepoRoot ".git")) -or
    -not (Test-Path (Join-Path $RepoRoot "Cargo.toml"))) {
    throw "Execute este script com o diretorio atual na raiz do repositorio sega-emulator."
}

$Branch = (& git branch --show-current).Trim()
if ($Branch -ne "main") {
    throw "A branch atual e '$Branch'. Mude manualmente para main apos preservar seu trabalho; o script nao troca branches."
}

$StatusLines = @(& git status --porcelain --untracked-files=all)

if ($LASTEXITCODE -ne 0) {
    throw "Nao foi possivel verificar o estado do Git."
}

$Dirty = @(
    $StatusLines | Where-Object {
        $_ -notmatch '^\?\? "?atualizar_recompilar_sega_emulator\.ps1"?$'
    }
)

if ($Dirty.Count -gt 0) {
    Write-Host "Alteracoes locais detectadas:" -ForegroundColor Yellow
    $Dirty | ForEach-Object { Write-Host $_ -ForegroundColor Yellow }

    throw "A arvore de trabalho possui alteracoes locais. Preserve-as antes de continuar."
}

Invoke-Checked -File "git" -Arguments @("fetch", "origin", "main") -Step "Atualizar referencias remotas"
$RemoteSha = (& git rev-parse origin/main).Trim()
if ($RemoteSha -ne $ExpectedMainSha) {
    throw "origin/main mudou. SHA esperado: $ExpectedMainSha; SHA remoto encontrado: $RemoteSha. Pare e revalide o novo HEAD antes de atualizar."
}

Invoke-Checked -File "git" -Arguments @("pull", "--ff-only", "origin", "main") -Step "Git pull fast-forward da main"
$LocalSha = (& git rev-parse HEAD).Trim()
if ($LocalSha -ne $ExpectedMainSha) {
    throw "HEAD local ($LocalSha) nao corresponde ao SHA validado ($ExpectedMainSha)."
}
Write-Host "HEAD validado: $LocalSha" -ForegroundColor Green

$Python = Join-Path $RepoRoot ".venv\Scripts\python.exe"
if (-not (Test-Path $Python)) {
    Invoke-Checked -File "py" -Arguments @("-3.14", "-m", "venv", ".venv") -Step "Criar ambiente virtual Python 3.14"
}
if (-not (Test-Path $Python)) {
    throw "Python 3.14 nao foi encontrado ou o ambiente virtual nao foi criado."
}

Invoke-Checked -File $Python -Arguments @("-m", "pip", "install", "--upgrade", "pip") -Step "Atualizar pip"
Invoke-Checked -File $Python -Arguments @("-m", "pip", "install", "-e", ".[dev,qt]", "maturin>=1.9,<2") -Step "Instalar dependencias de desenvolvimento e Maturin"

Invoke-Checked -File "cargo" -Arguments @("fmt", "--all", "--", "--check") -Step "Rust fmt do workspace"
Invoke-Checked -File "cargo" -Arguments @("clippy", "--workspace", "--all-targets", "--locked", "--", "-D", "warnings") -Step "Clippy do workspace"
Invoke-Checked -File "cargo" -Arguments @("test", "--workspace", "--locked") -Step "Testes Rust do workspace (inclui sega-sms-core)"
Invoke-Checked -File "cargo" -Arguments @("fmt", "--manifest-path", "native/z80_core/Cargo.toml", "--all", "--", "--check") -Step "Rust fmt da extensao PyO3"
Invoke-Checked -File "cargo" -Arguments @("clippy", "--manifest-path", "native/z80_core/Cargo.toml", "--all-targets", "--", "-D", "warnings") -Step "Clippy da extensao PyO3"
Invoke-Checked -File "cargo" -Arguments @("test", "--manifest-path", "native/z80_core/Cargo.toml") -Step "Testes Rust da extensao PyO3"

Write-Host "`n==> Compilar e instalar runtime Rust/PyO3 em Release" -ForegroundColor Cyan
$Maturin = Join-Path $RepoRoot ".venv\Scripts\maturin.exe"
if (-not (Test-Path $Maturin)) {
    throw "Executavel Maturin nao encontrado em .venv\Scripts."
}
Push-Location (Join-Path $RepoRoot "native\z80_core")
try {
    & $Maturin develop --release --features python-extension
    if ($LASTEXITCODE -ne 0) {
        throw "Falha na compilacao/instalacao Release via Maturin (exit code $LASTEXITCODE)."
    }
}
finally {
    Pop-Location
}

Invoke-Checked -File $Python -Arguments @("-c", "from sega_z80_core import NativeSG1000Machine; print('Extensao PyO3 importada com sucesso')") -Step "Verificar importacao do runtime nativo"

Invoke-Checked -File $Python -Arguments @("-m", "ruff", "check", ".") -Step "Ruff lint"
Invoke-Checked -File $Python -Arguments @("-m", "ruff", "format", "--check", ".") -Step "Ruff format"
Invoke-Checked -File $Python -Arguments @("-m", "pytest", "-q") -Step "Suite Python"

Write-Host "`nVALIDACAO AUTOMATIZADA CONCLUIDA. SHA: $LocalSha" -ForegroundColor Green
Write-Host "A proxima etapa e reproduzir a cena do Aladdin e comparar o resultado visual com Ares/hardware."
if ([string]::IsNullOrWhiteSpace($RomPath)) {
    Write-Host "Abrindo frontend Qt. Carregue a ROM local pelo menu Arquivo > Abrir ROM."
    & $Python -m sega_emulator
}
else {
    $ResolvedRom = (Resolve-Path $RomPath).Path
    Write-Host "Abrindo ROM local para verificacao: $ResolvedRom"
    & $Python -m sega_emulator $ResolvedRom
}
if ($LASTEXITCODE -ne 0) {
    throw "O frontend terminou com codigo de saida $LASTEXITCODE."
}
