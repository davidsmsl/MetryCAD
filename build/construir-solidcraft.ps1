<#
.SYNOPSIS
  Compila SolidCraft (tangency-labs/solidcraft) en Windows y genera el instalador MSI,
  el zip portable y el .exe, con el español completado.

.USO
  1. Abre PowerShell COMO ADMINISTRADOR en esta carpeta.
  2. Ejecuta:  Set-ExecutionPolicy -Scope Process Bypass -Force; .\construir-solidcraft.ps1
  Resultado: .\solidcraft\dist\release\  (MSI, ZIP portable, .exe)

  Instala solo lo que falte (winget): Git, PowerShell 7, Rust (rustup), Visual Studio Build Tools
  (C++), .NET SDK 8 y WiX 5.0.2. La primera vez tarda (VS Build Tools y la compilación, 30-90 min).
#>
$ErrorActionPreference = 'Stop'
$Here = $PSScriptRoot
$Repo = Join-Path $Here 'solidcraft'

function Test-Cmd($n) { [bool](Get-Command $n -ErrorAction SilentlyContinue) }
function Refresh-Path {
  $env:Path = [Environment]::GetEnvironmentVariable('Path','Machine') + ';' + [Environment]::GetEnvironmentVariable('Path','User') + ';' + "$env:USERPROFILE\.cargo\bin;$env:USERPROFILE\.dotnet\tools"
}
function Winget-Install($id, $extra = @()) {
  Write-Output "==> winget install $id"
  winget install --id $id -e --silent --accept-package-agreements --accept-source-agreements @extra
  Refresh-Path
}

if (-not (Test-Cmd winget)) { throw 'Falta winget (App Installer de Microsoft Store).' }
Refresh-Path

if (-not (Test-Cmd git))   { Winget-Install 'Git.Git' }
if (-not (Test-Cmd pwsh))  { Winget-Install 'Microsoft.PowerShell' }
if (-not (Test-Cmd rustup)){ Winget-Install 'Rustlang.Rustup' }
# Hace falta el SDK de .NET (no solo el runtime): 'dotnet' puede existir sin SDK.
function Test-DotnetSdk { (Test-Cmd dotnet) -and ((dotnet --list-sdks 2>$null) | Where-Object { $_ -match '^[89]\.|^[1-9][0-9]\.' }) }
if (-not (Test-DotnetSdk)) { Winget-Install 'Microsoft.DotNet.SDK.8' }
if (-not (Test-DotnetSdk)) { throw 'No se encontro el SDK de .NET 8+. Instalalo (winget install Microsoft.DotNet.SDK.8), abre una PowerShell NUEVA como administrador y repite.' }

# Visual Studio Build Tools con el componente C++ (linker MSVC y SDK de Windows)
$vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
$hasVC = (Test-Path $vswhere) -and (& $vswhere -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath)
if (-not $hasVC) {
  Winget-Install 'Microsoft.VisualStudio.2022.BuildTools' @('--override', '--quiet --wait --norestart --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended')
}

# WiX v5 (el script de empaquetado lo exige)
if (-not (Test-Cmd wix)) {
  dotnet tool install --global wix --version 5.0.2
  if ($LASTEXITCODE -ne 0) { throw "No se pudo instalar WiX ($LASTEXITCODE)" }
  Refresh-Path
}
if (-not (Test-Cmd wix)) { throw 'WiX no quedo en el PATH. Abre una PowerShell NUEVA como administrador y repite.' }
wix extension add -g WixToolset.UI.wixext/5.0.2 WixToolset.Util.wixext/5.0.2
if ($LASTEXITCODE -ne 0) { throw "No se pudieron agregar las extensiones de WiX ($LASTEXITCODE)" }

# Código fuente: version fija (commit exacto sobre el que se hizo el parche de MetryCAD)
$Commit = 'bb08447cf20b9b7028c3a1f0a145703b2820bb39'
if (-not (Test-Path (Join-Path $Repo '.git'))) {
  git clone --depth 1 https://github.com/tangency-labs/solidcraft $Repo
  if ($LASTEXITCODE -ne 0) { throw 'No se pudo clonar el repositorio' }
}
# Deja el repo limpio (quita cambios locales de ejecuciones anteriores)
git -C $Repo reset --hard
git -C $Repo clean -fd -e target -e dist
git -C $Repo fetch --depth 1 origin $Commit
if ($LASTEXITCODE -ne 0) { throw "No se pudo descargar el commit $Commit" }
git -C $Repo checkout --detach FETCH_HEAD
if ($LASTEXITCODE -ne 0) { throw 'No se pudo cambiar al commit fijo' }

# Marca MetryCAD: temas (Azul/Blanco y Grafito/Naranja), logo e icono, instalador,
# boton "+" que abre pestaña nueva y herramientas 3D que activan Sketch 3D solas.
$patch = Join-Path $Here 'metrycad.patch'
git -C $Repo apply --whitespace=nowarn $patch
if ($LASTEXITCODE -ne 0) { throw 'metrycad.patch no aplico limpio. No se compila para no dejar el codigo a medias.' }

# Español completo (292 cadenas) y nombre MetryCAD en los textos visibles (solo en valores, no en claves)
Copy-Item (Join-Path $Here 'es.json') (Join-Path $Repo 'assets\locales\es.json') -Force
foreach ($f in 'en.json','es.json') {
  $path = Join-Path $Repo "assets\locales\$f"
  $lines = [IO.File]::ReadAllLines($path, [Text.UTF8Encoding]::new($false))
  for ($i = 0; $i -lt $lines.Length; $i++) {
    if ($lines[$i] -match '^(\s*"(?:[^"\\]|\\.)*":\s*")(.*)$') {
      $lines[$i] = $Matches[1] + ($Matches[2] -replace 'SolidCraft','MetryCAD')
    }
  }
  [IO.File]::WriteAllLines($path, $lines, [Text.UTF8Encoding]::new($false))
}

# rustup lee rust-toolchain.toml del repo (Rust 1.99.1 + wasm32) la primera vez
Push-Location $Repo
try {
  rustup show | Out-Null
  $log = Join-Path $Here 'registro-compilacion.txt'
  $prev = $ErrorActionPreference; $ErrorActionPreference = 'Continue'
  & pwsh -NoProfile -File packaging\windows\package.ps1 -Arch x64 2>&1 | Tee-Object -FilePath $log
  $code = $LASTEXITCODE
  $ErrorActionPreference = $prev
  Write-Output "Registro completo guardado en: $log"
  if ($code -ne 0) { throw "package.ps1 fallo ($code). Revisa $log" }
} finally { Pop-Location }

Write-Output ''
Write-Output "Listo. Archivos del instalador en: $Repo\dist\release"
Get-ChildItem (Join-Path $Repo 'dist\release') | Format-Table Name, Length -AutoSize
