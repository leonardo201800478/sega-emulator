[CmdletBinding()]
param(
    [Parameter(Position = 0)]
    [ValidateSet("audit", "locks", "validate")]
    [string]$Command = "audit",
    [switch]$RefreshOffline,
    [string]$Json
)

$ErrorActionPreference = "Stop"
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$workspaces = @(
    [pscustomobject]@{ Name = "root"; Manifest = "Cargo.toml"; Lockfile = "Cargo.lock" },
    [pscustomobject]@{ Name = "sega-app"; Manifest = "native/sega_app/Cargo.toml"; Lockfile = "native/sega_app/Cargo.lock" }
)
$results = @()
$cargo = Get-Command cargo -ErrorAction SilentlyContinue
$env:CARGO_TARGET_DIR = Join-Path $repoRoot "target"

function Invoke-ProjectCargo {
    param([string]$Name, [string[]]$CargoArguments)
    if ($null -eq $cargo) {
        return [pscustomobject]@{ Name = $Name; Status = "BLOCKED"; ExitCode = $null; Output = "Cargo nao encontrado no PATH." }
    }
    Write-Host "==> $Name" -ForegroundColor Cyan
    Push-Location $repoRoot
    try {
        $output = & $cargo.Source @CargoArguments 2>&1
        $code = $LASTEXITCODE
    }
    finally { Pop-Location }
    $text = ($output | Out-String).Trim()
    $status = if ($code -eq 0) { "PASS" } else { "FAIL" }
    if ($status -ne "PASS" -and $text) { Write-Host $text }
    return [pscustomobject]@{ Name = $Name; Status = $status; ExitCode = $code; Output = $text }
}

if ($Command -eq "audit") {
    foreach ($tool in @("git", "rustc", "cargo")) {
        $commandInfo = Get-Command $tool -ErrorAction SilentlyContinue
        if ($null -eq $commandInfo) {
            $results += [pscustomobject]@{ Name = "tool $tool"; Status = "BLOCKED"; ExitCode = $null; Output = "Executavel nao encontrado no PATH." }
        }
        else {
            $version = & $commandInfo.Source --version 2>&1
            $code = $LASTEXITCODE
            $results += [pscustomobject]@{ Name = "tool $tool"; Status = $(if ($code -eq 0) { "PASS" } else { "FAIL" }); ExitCode = $code; Output = ($version | Out-String).Trim() }
        }
    }

    $knownManifestPaths = @()
    foreach ($workspace in $workspaces) {
        $manifestPath = Join-Path $repoRoot $workspace.Manifest
        $lockPath = Join-Path $repoRoot $workspace.Lockfile
        if (-not (Test-Path -LiteralPath $manifestPath -PathType Leaf)) {
            $results += [pscustomobject]@{ Name = "manifest $($workspace.Manifest)"; Status = "FAIL"; ExitCode = $null; Output = "Manifest esperado nao encontrado." }
            continue
        }

        $knownManifestPaths += [System.IO.Path]::GetFullPath($manifestPath)
        $metadata = Invoke-ProjectCargo -Name "metadata $($workspace.Name)" -CargoArguments @("metadata", "--locked", "--no-deps", "--format-version", "1", "--manifest-path", $manifestPath)
        $results += $metadata

        $results += [pscustomobject]@{
            Name = "lockfile $($workspace.Lockfile)"
            Status = $(if (Test-Path -LiteralPath $lockPath -PathType Leaf) { "PASS" } else { "FAIL" })
            ExitCode = $null
            Output = $(if (Test-Path -LiteralPath $lockPath -PathType Leaf) { "presente" } else { "ausente" })
        }
    }

    $allManifests = Get-ChildItem -Path $repoRoot -Filter "Cargo.toml" -File -Recurse | Where-Object { $_.FullName -notmatch "[\/](target|\.git)[\/]" }
    foreach ($manifest in $allManifests) {
        $fullPath = [System.IO.Path]::GetFullPath($manifest.FullName)
        if ($knownManifestPaths -contains $fullPath) { continue }
        $relativePath = $manifest.FullName.Substring($repoRoot.Length).TrimStart([char[]]@([char]92, [char]47))
        $results += [pscustomobject]@{ Name = "unassigned manifest $relativePath"; Status = "NOT RUN"; ExitCode = $null; Output = "Crate fora dos workspaces explicitamente validados." }
    }
}
elseif ($Command -eq "locks") {
    foreach ($workspace in $workspaces) {
        $manifestPath = Join-Path $repoRoot $workspace.Manifest
        if (-not (Test-Path -LiteralPath $manifestPath -PathType Leaf)) { continue }
        if ($RefreshOffline) {
            $results += Invoke-ProjectCargo -Name "refresh lockfile offline $($workspace.Name)" -CargoArguments @("update", "--offline", "--manifest-path", $manifestPath)
        }
        $results += Invoke-ProjectCargo -Name "locked metadata $($workspace.Name)" -CargoArguments @("metadata", "--locked", "--no-deps", "--format-version", "1", "--manifest-path", $manifestPath)
    }
}
else {
    foreach ($workspace in $workspaces) {
        $manifestPath = Join-Path $repoRoot $workspace.Manifest
        if (-not (Test-Path -LiteralPath $manifestPath -PathType Leaf)) { continue }
        $results += Invoke-ProjectCargo -Name "format $($workspace.Name)" -CargoArguments @("fmt", "--all", "--manifest-path", $manifestPath, "--", "--check")
        $results += Invoke-ProjectCargo -Name "clippy $($workspace.Name)" -CargoArguments @("clippy", "--workspace", "--all-targets", "--locked", "--manifest-path", $manifestPath, "--", "-D", "warnings")
        $results += Invoke-ProjectCargo -Name "tests $($workspace.Name)" -CargoArguments @("test", "--workspace", "--locked", "--manifest-path", $manifestPath)
    }
}

foreach ($result in $results) { Write-Host ("[{0,-7}] {1}" -f $result.Status, $result.Name) }
$counts = @{}
foreach ($status in @("PASS", "FAIL", "BLOCKED", "NOT RUN")) { $counts[$status] = @($results | Where-Object Status -eq $status).Count }
Write-Host ("Summary: PASS={0} | FAIL={1} | BLOCKED={2} | NOT RUN={3}" -f $counts["PASS"], $counts["FAIL"], $counts["BLOCKED"], $counts["NOT RUN"])
if ($Json) {
    $jsonPath = Join-Path $repoRoot $Json
    $parent = Split-Path -Parent $jsonPath
    if ($parent -and -not (Test-Path $parent)) { New-Item -ItemType Directory -Path $parent -Force | Out-Null }
    [pscustomobject]@{ Command = $Command; Results = $results; Summary = $counts } | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $jsonPath -Encoding utf8
}
if (@($results | Where-Object Status -eq "FAIL").Count -gt 0) { exit 1 }
if ($Command -ne "audit" -and @($results | Where-Object Status -eq "BLOCKED").Count -gt 0) { exit 1 }
