param(
    [Parameter(Position = 0)]
    [string]$RomPath
)

$ErrorActionPreference = "Stop"
$repoRoot = Split-Path -Parent $PSScriptRoot
Set-Location $repoRoot

$python = Join-Path $repoRoot ".venv\Scripts\python.exe"
if (-not (Test-Path $python)) {
    throw "Ambiente virtual não encontrado em .venv. Crie-o e instale as dependências conforme README.md."
}

Write-Host "==> Instalando Maturin no ambiente virtual"
& $python -m pip install maturin
if ($LASTEXITCODE -ne 0) { throw "Falha ao instalar Maturin." }

Write-Host "==> Compilando a extensão PyO3 do runtime SG-1000 existente (Release)"
Push-Location (Join-Path $repoRoot "native\z80_core")
try {
    & (Join-Path $repoRoot ".venv\Scripts\maturin.exe") develop --release --features python-extension
    if ($LASTEXITCODE -ne 0) { throw "Falha ao compilar/instalar a extensão Rust." }
}
finally {
    Pop-Location
}

Write-Host "==> Verificando importação da extensão nativa"
& $python -c "from sega_z80_core import NativeSG1000Machine; print('Extensão PyO3 NativeSG1000Machine: import OK')"
if ($LASTEXITCODE -ne 0) { throw "A extensão foi compilada, mas não pôde ser importada." }

if ([string]::IsNullOrWhiteSpace($RomPath)) {
    Write-Host "Build concluído. Abrindo frontend Qt; use Arquivo > Abrir ROM."
    & $python -m sega_emulator
}
else {
    $resolvedRom = (Resolve-Path $RomPath).Path
    Write-Host "Abrindo ROM para teste manual: $resolvedRom"
    & $python -m sega_emulator $resolvedRom
}

if ($LASTEXITCODE -ne 0) {
    throw "O frontend terminou com código de saída $LASTEXITCODE."
}
