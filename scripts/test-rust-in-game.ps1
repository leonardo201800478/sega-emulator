[CmdletBinding()]
param([string]$RomPath)

$ErrorActionPreference = "Stop"
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
. (Join-Path $PSScriptRoot "common.ps1")
$python = Get-SegaPython -RepositoryRoot $repoRoot
Initialize-SegaPythonRuntime -PythonExecutable $python
Initialize-SegaMsvcEnvironment

$maturinCheck = & $python -c "import importlib.util; print('yes' if importlib.util.find_spec('maturin') else 'no')"
if ($LASTEXITCODE -ne 0) { throw "Nao foi possivel verificar Maturin no ambiente Python." }
if ($maturinCheck.Trim() -ne "yes") {
    throw "Maturin nao esta instalado na .venv. Execute: .\.venv\Scripts\python.exe -m pip install maturin==1.16.0"
}

Push-Location (Join-Path $repoRoot "native\z80_core")
try {
    Invoke-SegaChecked -File $python -Arguments @(
        "-m", "maturin", "develop", "--release", "--locked", "--features", "python-extension"
    ) -Step "Build Release da extensao PyO3"
}
finally { Pop-Location }

Invoke-SegaChecked -File $python -Arguments @(
    "-c", "from sega_z80_core import NativeSG1000Machine, NativeSMSMachine; print('Extensao PyO3 SMS/SG-1000: import OK')"
) -Step "Smoke test da extensao nativa"

$appArgs = @("-m", "sega_emulator")
if (-not [string]::IsNullOrWhiteSpace($RomPath)) {
    if (-not (Test-Path -LiteralPath $RomPath -PathType Leaf)) { throw "ROM nao encontrada: $RomPath" }
    $appArgs += (Resolve-Path -LiteralPath $RomPath).Path
}
Invoke-SegaChecked -File $python -Arguments $appArgs -Step "Abrir frontend Qt"
