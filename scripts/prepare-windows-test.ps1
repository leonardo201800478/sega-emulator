param(
    [Parameter(Position = 0)]
    [string]$RomPath
)

$ErrorActionPreference = "Stop"
$repoRoot = Split-Path -Parent $PSScriptRoot
Set-Location $repoRoot

$venvPython = Join-Path $repoRoot ".venv\Scripts\python.exe"
if (-not (Test-Path $venvPython)) {
    Write-Host "==> Criando ambiente virtual com Python 3.14"
    py -3.14 -m venv .venv
    if ($LASTEXITCODE -ne 0) {
        throw "Não foi possível criar .venv. Instale Python 3.14 x64 e habilite o launcher py."
    }
}

Write-Host "==> Instalando dependências da interface Qt"
& $venvPython -m pip install --upgrade pip
if ($LASTEXITCODE -ne 0) { throw "Falha ao atualizar pip." }

& $venvPython -m pip install -e ".[dev,qt]"
if ($LASTEXITCODE -ne 0) { throw "Falha ao instalar as dependências do Sega Emulator." }

Write-Host "==> Instalando Maturin"
& $venvPython -m pip install "maturin>=1.9,<2"
if ($LASTEXITCODE -ne 0) { throw "Falha ao instalar Maturin." }

Write-Host "==> Compilando o runtime Rust/PyO3 em Release"
Push-Location (Join-Path $repoRoot "native\z80_core")
try {
    & (Join-Path $repoRoot ".venv\Scripts\maturin.exe") develop --release --features python-extension
    if ($LASTEXITCODE -ne 0) {
        throw "Falha na compilação. Verifique Rust stable e Visual Studio Build Tools (C++ MSVC)."
    }
}
finally {
    Pop-Location
}

Write-Host "==> Verificando o runtime nativo"
& $venvPython -c "from sega_z80_core import NativeSG1000Machine; print('Runtime SG-1000 Rust/PyO3: OK')"
if ($LASTEXITCODE -ne 0) { throw "A extensão nativa não pôde ser importada." }

if ([string]::IsNullOrWhiteSpace($RomPath)) {
    Write-Host "==> Abrindo frontend Qt. Use Arquivo > Abrir ROM."
    & $venvPython -m sega_emulator
}
else {
    $resolvedRom = (Resolve-Path $RomPath).Path
    Write-Host "==> Abrindo ROM local: $resolvedRom"
    & $venvPython -m sega_emulator $resolvedRom
}

if ($LASTEXITCODE -ne 0) {
    throw "O frontend terminou com código de saída $LASTEXITCODE."
}
