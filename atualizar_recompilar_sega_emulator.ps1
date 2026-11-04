
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

# ---------------------------------------------------------------------------
# FUNCOES AUXILIARES
# ---------------------------------------------------------------------------

function Invoke-Checked {
    param(
        [Parameter(Mandatory = $true)]
        [string]$File,

        [Parameter(Mandatory = $true)]
        [string[]]$Arguments,

        [Parameter(Mandatory = $true)]
        [string]$Step
    )

    Write-Host "`n==> $Step" -ForegroundColor Cyan

    & $File @Arguments

    if ($LASTEXITCODE -ne 0) {
        throw "Falha em '$Step' (exit code $LASTEXITCODE)."
    }
}

function Get-GitOutput {
    param(
        [Parameter(Mandatory = $true)]
        [string[]]$Arguments
    )

    $Output = @(& git -C $RepoRoot @Arguments)

    if ($LASTEXITCODE -ne 0) {
        throw "Falha no comando git $($Arguments -join ' ')."
    }

    return $Output
}

function Test-GitAncestor {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Ancestor,

        [Parameter(Mandatory = $true)]
        [string]$Commit
    )

    & git -C $RepoRoot merge-base --is-ancestor $Ancestor $Commit

    $ExitCode = $LASTEXITCODE

    if ($ExitCode -eq 0) {
        return $true
    }

    if ($ExitCode -eq 1) {
        return $false
    }

    throw "Falha ao verificar ancestralidade Git entre '$Ancestor' e '$Commit'."
}

function Set-DependencyMarker {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Path,

        [Parameter(Mandatory = $true)]
        [string]$Sha
    )

    $Directory = Split-Path -Parent $Path

    New-Item -ItemType Directory -Path $Directory -Force |
        Out-Null

    $TemporaryPath = "$Path.$([guid]::NewGuid().ToString('N')).tmp"

    try {
        [System.IO.File]::WriteAllText(
            $TemporaryPath,
            $Sha,
            [System.Text.Encoding]::ASCII
        )

        Move-Item -LiteralPath $TemporaryPath -Destination $Path -Force
    }
    finally {
        if (Test-Path -LiteralPath $TemporaryPath) {
            Remove-Item -LiteralPath $TemporaryPath -Force
        }
    }
}

# ---------------------------------------------------------------------------
# VALIDACAO DO REPOSITORIO
# ---------------------------------------------------------------------------

if (
    -not (Test-Path (Join-Path $RepoRoot ".git")) -or
    -not (Test-Path (Join-Path $RepoRoot "Cargo.toml"))
) {
    throw "Execute na raiz do repositorio sega-emulator."
}

# ---------------------------------------------------------------------------
# IDENTIFICACAO DA BRANCH
# ---------------------------------------------------------------------------

$Branch = (
    Get-GitOutput -Arguments @("branch", "--show-current")
) -join ""

$Branch = $Branch.Trim()

if ([string]::IsNullOrWhiteSpace($Branch)) {
    throw "Nao foi possivel identificar a branch atual. HEAD destacado nao e permitido."
}

Write-Host "Branch de trabalho: $Branch" -ForegroundColor Cyan

# ---------------------------------------------------------------------------
# PROTECAO DAS ALTERACOES LOCAIS
# ---------------------------------------------------------------------------

$Status = @(
    Get-GitOutput -Arguments @(
        "status",
        "--porcelain=v1",
        "--untracked-files=all"
    )
)

$BlockingStatus = @()
$PreservedLocalFiles = @()

foreach ($Entry in $Status) {

    if ($Entry.Length -lt 3) {
        $BlockingStatus += $Entry
        continue
    }

    $IndexStatus = $Entry.Substring(0, 2)

    $Path = $Entry.Substring(3).Trim('"') -replace '\\', '/'

    $IsArtifact =
        $IndexStatus -eq "??" -and
        (
            $Path -eq "artifact" -or
            $Path.StartsWith(
                "artifact/",
                [System.StringComparison]::OrdinalIgnoreCase
            )
        )

    $IsCurrentUpdater =
        $IndexStatus -eq " M" -and
        $Path -eq "atualizar_recompilar_sega_emulator.ps1"

    if ($IsArtifact -or $IsCurrentUpdater) {
        $PreservedLocalFiles += $Entry
    }
    else {
        $BlockingStatus += $Entry
    }
}

if ($PreservedLocalFiles.Count -gt 0) {

    Write-Host "`nArquivos locais permitidos serao preservados:" `
        -ForegroundColor Yellow

    $PreservedLocalFiles |
        ForEach-Object {
            Write-Host "  $_" -ForegroundColor Yellow
        }
}

if ($BlockingStatus.Count -gt 0) {

    Write-Host "`nAlteracoes locais bloqueantes:" `
        -ForegroundColor Red

    $BlockingStatus |
        ForEach-Object {
            Write-Host "  $_" -ForegroundColor Yellow
        }

    throw "Ha alteracoes locais bloqueantes fora das excecoes permitidas. Nenhuma sincronizacao foi executada."
}

# ---------------------------------------------------------------------------
# SINCRONIZACAO DAS REFERENCIAS REMOTAS
# ---------------------------------------------------------------------------

Invoke-Checked `
    -File "git" `
    -Arguments @("fetch", "origin", "main", "--prune") `
    -Step "Consultar main remota"

$RemoteSha = (
    Get-GitOutput -Arguments @("rev-parse", "origin/main")
) -join ""

$RemoteSha = $RemoteSha.Trim()

$LocalSha = (
    Get-GitOutput -Arguments @("rev-parse", "HEAD")
) -join ""

$LocalSha = $LocalSha.Trim()

# A branch pode possuir commits proprios.
# A main remota precisa ser ancestral, mas nao sera incorporada automaticamente.
if (-not (Test-GitAncestor -Ancestor "origin/main" -Commit "HEAD")) {

    throw "A branch atual nao contem origin/main como ancestral. Atualize sua branch manualmente. Nenhum merge, rebase ou reset foi executado."
}

Write-Host "`nBranch validada: $Branch" -ForegroundColor Green
Write-Host "HEAD de trabalho: $LocalSha" -ForegroundColor Green
Write-Host "Base remota main: $RemoteSha" -ForegroundColor Green

# ---------------------------------------------------------------------------
# DETECCAO DE ALTERACOES NAS DEPENDENCIAS PYTHON
# ---------------------------------------------------------------------------

# O marcador e armazenado dentro do diretorio Git.
# Ele nao e versionado e sobrevive a trocas de branch.

$MarkerRelativePath = (
    Get-GitOutput -Arguments @(
        "rev-parse",
        "--git-path",
        "sega-emulator-dependency-sync"
    )
) -join ""

$MarkerRelativePath = $MarkerRelativePath.Trim()

if ([System.IO.Path]::IsPathRooted($MarkerRelativePath)) {
    $MarkerPath = $MarkerRelativePath
}
else {
    $MarkerPath = Join-Path $RepoRoot $MarkerRelativePath
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
$MarkerSha = $null

if (Test-Path -LiteralPath $MarkerPath -PathType Leaf) {

    $MarkerSha = (
        Get-Content -LiteralPath $MarkerPath -Raw
    ).Trim()
}

if ([string]::IsNullOrWhiteSpace($MarkerSha)) {

    Write-Host "Marcador de dependencias ausente. Instalacao conservadora." `
        -ForegroundColor Yellow

    $DependenciesChanged = $true
}
else {

    # Verifica se o commit registrado ainda existe.
    & git -C $RepoRoot cat-file -e "$MarkerSha^{commit}" 2>$null

    $MarkerExists = ($LASTEXITCODE -eq 0)

    if (-not $MarkerExists) {

        Write-Host "Marcador aponta para commit inexistente. Instalacao conservadora." `
            -ForegroundColor Yellow

        $DependenciesChanged = $true
    }
    else {

        # Verifica se o marcador pertence a historia atual.
        $MarkerIsAncestor = Test-GitAncestor `
            -Ancestor $MarkerSha `
            -Commit "HEAD"

        if (-not $MarkerIsAncestor) {

            Write-Host "Historico divergiu do marcador. Instalacao conservadora." `
                -ForegroundColor Yellow

            $DependenciesChanged = $true
        }
        elseif ($MarkerSha -ne $LocalSha) {

            $ChangedFiles = @(
                Get-GitOutput -Arguments @(
                    "diff",
                    "--name-only",
                    "--no-renames",
                    $MarkerSha,
                    "HEAD"
                )
            )

            foreach ($ChangedFile in $ChangedFiles) {

                if (
                    $DependencyFiles -contains $ChangedFile -or
                    $ChangedFile -match '(^|/)(requirements|constraints)[^/]*\.txt$'
                ) {
                    $DependenciesChanged = $true
                    break
                }
            }
        }
    }
}

# ---------------------------------------------------------------------------
# AMBIENTE PYTHON
# ---------------------------------------------------------------------------

$NewVenv = -not (Test-Path -LiteralPath $VenvPython -PathType Leaf)

if ($NewVenv) {

    Invoke-Checked `
        -File "py" `
        -Arguments @("-3.15", "-m", "venv", ".venv") `
        -Step "Criar ambiente virtual Python 3.15.2"
}

if (-not (Test-Path -LiteralPath $VenvPython -PathType Leaf)) {
    throw "Python 3.15.2 nao encontrado. Instale-o e tente novamente."
}

$VenvPythonVersion = (& $VenvPython --version 2>&1 | Out-String).Trim()

if ($VenvPythonVersion -notmatch '^Python 3\.15\.') {
    throw "O ambiente .venv usa '$VenvPythonVersion'. Preserve-o como backup e recrie .venv com 'py -3.15 -m venv .venv' antes de continuar."
}

if ($NewVenv -or $DependenciesChanged) {

    Invoke-Checked `
        -File $VenvPython `
        -Arguments @(
            "-m",
            "pip",
            "install",
            "-e",
            ".[dev,qt]"
        ) `
        -Step "Instalar/atualizar dependencias Python do projeto"
}
else {

    Write-Host "Dependencias Python sem alteracoes detectadas." `
        -ForegroundColor Green
}

# O marcador somente e atualizado depois da instalacao necessaria.
Set-DependencyMarker -Path $MarkerPath -Sha $LocalSha

Write-Host "Marcador de dependencias atualizado: $LocalSha" `
    -ForegroundColor Green

# ---------------------------------------------------------------------------
# MATURIN
# ---------------------------------------------------------------------------

if (-not (Test-Path -LiteralPath $VenvMaturin -PathType Leaf)) {

    Invoke-Checked `
        -File $VenvPython `
        -Arguments @(
            "-m",
            "pip",
            "install",
            "maturin==1.16.0"
        ) `
        -Step "Instalar Maturin ausente"
}

if (-not (Test-Path -LiteralPath $VenvMaturin -PathType Leaf)) {
    throw "Maturin nao foi instalado no ambiente virtual."
}

# ---------------------------------------------------------------------------
# VALIDACAO RUST - WORKSPACE PRINCIPAL
# ---------------------------------------------------------------------------

if ($Full) {

    Invoke-Checked `
        -File "cargo" `
        -Arguments @(
            "fmt",
            "--all",
            "--",
            "--check"
        ) `
        -Step "Rust fmt workspace"

    Invoke-Checked `
        -File "cargo" `
        -Arguments @(
            "clippy",
            "--workspace",
            "--all-targets",
            "--locked",
            "--",
            "-D",
            "warnings"
        ) `
        -Step "Rust Clippy workspace"

    Invoke-Checked `
        -File "cargo" `
        -Arguments @(
            "test",
            "--workspace",
            "--locked"
        ) `
        -Step "Testes Rust workspace"
}

# ---------------------------------------------------------------------------
# BUILD RELEASE DO WORKSPACE
# ---------------------------------------------------------------------------

Invoke-Checked `
    -File "cargo" `
    -Arguments @(
        "build",
        "--workspace",
        "--release",
        "--locked"
    ) `
    -Step "Build Release do workspace Rust"

# ---------------------------------------------------------------------------
# VALIDACAO RUST - PY O3
# ---------------------------------------------------------------------------

if ($Full) {

    Invoke-Checked `
        -File "cargo" `
        -Arguments @(
            "fmt",
            "--manifest-path",
            "native/z80_core/Cargo.toml",
            "--all",
            "--",
            "--check"
        ) `
        -Step "Rust fmt PyO3"

    Invoke-Checked `
        -File "cargo" `
        -Arguments @(
            "clippy",
            "--manifest-path",
            "native/z80_core/Cargo.toml",
            "--locked",
            "--all-targets",
            "--",
            "-D",
            "warnings"
        ) `
        -Step "Rust Clippy PyO3"

    Invoke-Checked `
        -File "cargo" `
        -Arguments @(
            "test",
            "--manifest-path",
            "native/z80_core/Cargo.toml",
            "--locked"
        ) `
        -Step "Testes Rust PyO3"
}

# ---------------------------------------------------------------------------
# BUILD RELEASE DA EXTENSAO NATIVA
# ---------------------------------------------------------------------------

Write-Host "`n==> Build Release da extensao PyO3" `
    -ForegroundColor Cyan

Push-Location (Join-Path $RepoRoot "native\z80_core")

try {

    Invoke-Checked `
        -File $VenvMaturin `
        -Arguments @(
            "develop",
            "--release",
            "--locked",
            "--features",
            "python-extension"
        ) `
        -Step "Maturin Release"
}
finally {

    Pop-Location
}

# ---------------------------------------------------------------------------
# SMOKE TEST DA EXTENSAO
# ---------------------------------------------------------------------------

Invoke-Checked `
    -File $VenvPython `
    -Arguments @(
        "-c",
        "import sega_z80_core as m; assert hasattr(m, 'NativeSMSMachine'); assert hasattr(m, 'NativeSG1000Machine'); print('PyO3 SMS/SG-1000 OK')"
    ) `
    -Step "Smoke test da extensao nativa"

# ---------------------------------------------------------------------------
# VALIDACAO PYTHON
# ---------------------------------------------------------------------------

if ($Full) {

    Invoke-Checked `
        -File $VenvPython `
        -Arguments @(
            "-m",
            "compileall",
            "src"
        ) `
        -Step "Python compileall"

    Invoke-Checked `
        -File $VenvPython `
        -Arguments @(
            "-m",
            "pip",
            "check"
        ) `
        -Step "Dependencias Python"

    # Executa pytest antes do lint para coletar os resultados funcionais.
    Invoke-Checked `
        -File $VenvPython `
        -Arguments @(
            "-m",
            "pytest",
            "-q"
        ) `
        -Step "Suite Python completa"

    Invoke-Checked `
        -File $VenvPython `
        -Arguments @(
            "-m",
            "ruff",
            "check",
            "."
        ) `
        -Step "Ruff lint"

    Invoke-Checked `
        -File $VenvPython `
        -Arguments @(
            "-m",
            "ruff",
            "format",
            "--check",
            "."
        ) `
        -Step "Ruff format"
}
else {

    Write-Host "`nModo rapido: build Release do workspace, build Maturin e smoke test concluidos. Use -Full para a bateria completa." `
        -ForegroundColor Yellow
}

# ---------------------------------------------------------------------------
# EXECUCAO OPCIONAL DO FRONTEND
# ---------------------------------------------------------------------------

if (
    $Launch -or
    -not [string]::IsNullOrWhiteSpace($RomPath)
) {

    $AppArgs = @("-m", "sega_emulator")

    if (-not [string]::IsNullOrWhiteSpace($RomPath)) {

        if (-not (Test-Path -LiteralPath $RomPath -PathType Leaf)) {
            throw "ROM nao encontrada: $RomPath"
        }

        $ResolvedRom = (
            Resolve-Path -LiteralPath $RomPath
        ).Path

        $AppArgs += $ResolvedRom
    }

    Invoke-Checked `
        -File $VenvPython `
        -Arguments $AppArgs `
        -Step "Abrir frontend Qt"
}

# ---------------------------------------------------------------------------
# ESTADO FINAL
# ---------------------------------------------------------------------------

$FinalStatus = @(
    Get-GitOutput -Arguments @(
        "status",
        "--short",
        "--branch"
    )
)

Write-Host "`nEstado final do Git:" -ForegroundColor Cyan

$FinalStatus |
    ForEach-Object {
        Write-Host $_
    }

Write-Host "`nCONCLUIDO — SHA $LocalSha" -ForegroundColor Green
