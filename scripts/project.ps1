[CmdletBinding()]
param(
    [Parameter(Position = 0)]
    [ValidateSet("audit", "locks", "validate")]
    [string]$Command = "audit",
    [switch]$RefreshOffline,
    [switch]$RustOnly,
    [string]$Json
)

$ErrorActionPreference = "Stop"
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
. (Join-Path $PSScriptRoot "common.ps1")
$manager = Join-Path $repoRoot "tools\project_manager.py"
if (-not (Test-Path -LiteralPath $manager -PathType Leaf)) { throw "Project manager not found: $manager" }

$python = Get-SegaPython -RepositoryRoot $repoRoot
Initialize-SegaPythonRuntime -PythonExecutable $python
$arguments = @($manager, $Command)
if ($Command -eq "locks" -and $RefreshOffline) { $arguments += "--refresh-offline" }
if ($Command -eq "validate" -and $RustOnly) { $arguments += "--rust-only" }
if ($Command -eq "validate" -and $Json) { $arguments += @("--json", $Json) }

Push-Location $repoRoot
try {
    & $python @arguments
    $exitCode = $LASTEXITCODE
}
finally { Pop-Location }
exit $exitCode
