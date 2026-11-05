# Shared Windows build-environment helpers for repository scripts.
function Get-SegaPython {
    [CmdletBinding()]
    param([Parameter(Mandatory = $true)][string]$RepositoryRoot)
    $venvPython = Join-Path $RepositoryRoot ".venv\Scripts\python.exe"
    if (Test-Path -LiteralPath $venvPython -PathType Leaf) { return $venvPython }
    $pythonCommand = Get-Command python -ErrorAction SilentlyContinue
    if ($null -eq $pythonCommand) { throw "Python nao encontrado. Crie .venv ou instale Python 3.15 e adicione-o ao PATH." }
    return $pythonCommand.Source
}
function Initialize-SegaPythonRuntime {
    [CmdletBinding()]
    param([Parameter(Mandatory = $true)][string]$PythonExecutable)
    $pythonVersion = (& $PythonExecutable -c "import sys; print('.'.join(map(str, sys.version_info[:3])))").Trim()
    if ($LASTEXITCODE -ne 0 -or $pythonVersion -ne "3.15.2") {
        throw "O projeto exige CPython 3.15.2; encontrado '$pythonVersion' em '$PythonExecutable'. Recrie .venv com Python 3.15.2."
    }
    $basePrefix = (& $PythonExecutable -c "import sys; print(sys.base_prefix)").Trim()
    if ($LASTEXITCODE -ne 0 -or [string]::IsNullOrWhiteSpace($basePrefix)) { throw "Nao foi possivel determinar sys.base_prefix de '$PythonExecutable'." }
    $pathEntries = @($env:PATH -split [System.IO.Path]::PathSeparator)
    $alreadyPresent = $pathEntries | Where-Object { $_.TrimEnd('\') -ieq $basePrefix.TrimEnd('\') }
    if (-not $alreadyPresent) { $env:PATH = "$basePrefix$([System.IO.Path]::PathSeparator)$env:PATH" }
}
function Initialize-SegaMsvcEnvironment {
    [CmdletBinding()]
    param()
    if (Get-Command link.exe -ErrorAction SilentlyContinue) { return }
    $programFilesX86 = ${env:ProgramFiles(x86)}
    if ([string]::IsNullOrWhiteSpace($programFilesX86)) { $programFilesX86 = $env:ProgramFiles }
    $vswhere = Join-Path $programFilesX86 "Microsoft Visual Studio\Installer\vswhere.exe"
    if (-not (Test-Path -LiteralPath $vswhere -PathType Leaf)) { throw "MSVC linker nao encontrado e vswhere.exe indisponivel. Instale Visual Studio Build Tools com MSVC C++ x64/x86." }
    $vsPath = & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
    if ($LASTEXITCODE -ne 0 -or [string]::IsNullOrWhiteSpace($vsPath)) { throw "Nenhuma instalacao do Visual Studio com toolset MSVC C++ foi encontrada." }
    $devShell = Join-Path $vsPath.Trim() "Common7\Tools\Launch-VsDevShell.ps1"
    if (-not (Test-Path -LiteralPath $devShell -PathType Leaf)) { throw "Script de inicializacao MSVC nao encontrado: $devShell" }
    . $devShell -Arch amd64 -HostArch amd64
    if (-not (Get-Command link.exe -ErrorAction SilentlyContinue)) { throw "O Developer Shell foi inicializado, mas link.exe continua indisponivel." }
}
function Invoke-SegaChecked {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)][string]$File,
        [Parameter(Mandatory = $true)][string[]]$Arguments,
        [Parameter(Mandatory = $true)][string]$Step
    )
    Write-Host ""
    Write-Host "==> $Step" -ForegroundColor Cyan
    & $File @Arguments
    if ($LASTEXITCODE -ne 0) { throw "Falha em '$Step' (exit code $LASTEXITCODE)." }
}
