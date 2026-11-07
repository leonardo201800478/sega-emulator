param(
    [string]$Destination = (Split-Path -Parent $PSScriptRoot)
)

$ErrorActionPreference = "Stop"

$version = "3.4.20"
$archiveName = "SDL3-$version-win32-x64.zip"
$uri = "https://github.com/libsdl-org/SDL/releases/download/release-$version/$archiveName"
$expectedSha256 = "368bed8b24e44ad9ea372990814f2343fcfb9e468abb7f8f4a3bc146c7304752"

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
