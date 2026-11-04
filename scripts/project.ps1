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
$manager = Join-Path $repoRoot "tools\project_manager.py"
if (-not (Test-Path $manager)) { throw "Project manager not found: $manager" }
$python = Get-Command python -ErrorAction SilentlyContinue
if (-not $python) { throw "Python was not found on PATH. Activate .venv first." }
$arguments = @($manager, $Command)
if ($Command -eq "locks" -and $RefreshOffline) { $arguments += "--refresh-offline" }
if ($Command -eq "validate" -and $RustOnly) { $arguments += "--rust-only" }
if ($Command -eq "validate" -and $Json) { $arguments += @("--json", $Json) }
Push-Location $repoRoot
try {
    & $python.Source @arguments
    exit $LASTEXITCODE
}
finally { Pop-Location }
