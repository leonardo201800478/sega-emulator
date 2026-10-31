param(
    [string]$Destination = (Split-Path -Parent $PSScriptRoot)
)

$ErrorActionPreference = "Stop"

$version = "3.4.18"
$archiveName = "SDL3-$version-win32-x64.zip"
$uri = "https://www.libsdl.org/release/$archiveName"
$expectedSha256 = "75c2c0fc74e7d1206aaedc22893a35887332f4f85f5c8b74fddc10d177985f2c"

$tempRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("sega-sdl3-" + [guid]::NewGuid())
$archivePath = Join-Path $tempRoot $archiveName
$extractPath = Join-Path $tempRoot "extract"

try {
    New-Item -ItemType Directory -Path $extractPath -Force | Out-Null
    Invoke-WebRequest -Uri $uri -OutFile $archivePath

    $actualSha256 = (Get-FileHash -Path $archivePath -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($actualSha256 -ne $expectedSha256) {
        throw "SHA-256 inválido para $archiveName. Esperado: $expectedSha256; recebido: $actualSha256"
    }

    Expand-Archive -Path $archivePath -DestinationPath $extractPath -Force
    $dll = Get-ChildItem -Path $extractPath -Filter "SDL3.dll" -File -Recurse |
        Select-Object -First 1
    if ($null -eq $dll) {
        throw "O pacote oficial não contém SDL3.dll."
    }

    New-Item -ItemType Directory -Path $Destination -Force | Out-Null
    $target = Join-Path $Destination "SDL3.dll"
    Copy-Item -Path $dll.FullName -Destination $target -Force
    Write-Host "SDL3 $version instalado em: $target"
    Write-Host "O backend Rust carrega SDL3.dll pelo diretório da aplicação ou por SEGA_EMULATOR_SDL3_DLL."
}
finally {
    if (Test-Path $tempRoot) {
        Remove-Item -Path $tempRoot -Recurse -Force
    }
}
