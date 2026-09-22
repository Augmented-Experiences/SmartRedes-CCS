param(
  [switch]$Installer  # If set, runs 'npm run build' in desktop/ (MSI + setup.exe)
)

# ============================================================
# build-backend.ps1 — Empaqueta el backend FastAPI de SmartRedes
# en un binario único (sidecar de Tauri) con PyInstaller (Windows)
# y lo coloca en desktop/src-tauri/binaries con el sufijo del target.
#
# Requiere Python 3.10–3.12 (recomendado 3.12). Con 3.13/3.14 las
# dependencias pinneadas (numpy 1.26.4) no tienen wheels y pip intentaría
# compilarlas desde el código fuente, lo que falla en Windows.
# ============================================================
$ErrorActionPreference = "Stop"

$Here = Split-Path -Parent $MyInvocation.MyCommand.Path
$Desktop = Split-Path -Parent $Here
$Root = Split-Path -Parent $Desktop
Set-Location $Root

# --- App identity (process names to stop before build) ---
$ProductName = "SmartRedes"
$CargoExeName = "smartredes"
try {
  $cfgPath = Join-Path $Desktop "smartsuite.config.json"
  if (Test-Path $cfgPath) {
    $cfg = Get-Content $cfgPath -Raw | ConvertFrom-Json
    if ($cfg.productName) { $ProductName = [string]$cfg.productName }
    if ($cfg.identifier) {
      $CargoExeName = ($cfg.identifier -split '\.')[-1].ToLower()
    }
  }
} catch {
  Write-Host "==> (aviso) no se leyo smartsuite.config.json; usando $ProductName" -ForegroundColor Yellow
}

function Write-BuildLockHelp {
  Write-Host ""
  Write-Host "ARCHIVO EN USO (build bloqueado por proceso anterior)" -ForegroundColor Red
  Write-Host "  Mensajes tipicos de Windows / PyInstaller / cargo:" -ForegroundColor Yellow
  Write-Host "    - The process cannot access the file because it is being used by another process" -ForegroundColor DarkYellow
  Write-Host "    - Acceso denegado / cannot access ... backend.exe" -ForegroundColor DarkYellow
  Write-Host "    - error: failed to remove file ... $CargoExeName.exe" -ForegroundColor DarkYellow
  Write-Host ""
  Write-Host "  Cierre la app de escritorio y sidecars, luego vuelva a ejecutar este script:" -ForegroundColor Cyan
  Write-Host "    Stop-Process -Name '$ProductName','$CargoExeName','backend' -Force -ErrorAction SilentlyContinue" -ForegroundColor Cyan
  Write-Host ""
}

function Stop-SuiteDesktopProcesses {
  param([switch]$Quiet)
  $names = @(
    $ProductName,
    $CargoExeName,
    "backend"
  ) | Select-Object -Unique

  $stopped = 0
  foreach ($n in $names) {
    if (-not $n) { continue }
    Get-Process -Name $n -ErrorAction SilentlyContinue | ForEach-Object {
      if (-not $Quiet) {
        Write-Host "==> Cerrando proceso: $($_.Name) (PID $($_.Id))" -ForegroundColor Yellow
      }
      Stop-Process -Id $_.Id -Force -ErrorAction SilentlyContinue
      $stopped++
    }
  }
  if ($stopped -gt 0) {
    Start-Sleep -Seconds 1
  }
  return $stopped
}

function Remove-FileWithRetry {
  param(
    [Parameter(Mandatory)][string]$Path,
    [int]$Retries = 4
  )
  if (-not (Test-Path $Path)) { return $true }
  for ($i = 1; $i -le $Retries; $i++) {
    try {
      Remove-Item -LiteralPath $Path -Force -ErrorAction Stop
      return $true
    } catch {
      Write-Host "==> No se pudo borrar $Path (intento $i/$Retries): $($_.Exception.Message)" -ForegroundColor Yellow
      Stop-SuiteDesktopProcesses -Quiet | Out-Null
      Start-Sleep -Seconds 2
    }
  }
  return $false
}

function Copy-SidecarExecutable {
  param(
    [Parameter(Mandatory)][string]$Source,
    [Parameter(Mandatory)][string]$Destination
  )
  $destDir = Split-Path -Parent $Destination
  New-Item -ItemType Directory -Force -Path $destDir | Out-Null

  Stop-SuiteDesktopProcesses | Out-Null
  Remove-FileWithRetry -Path $Destination | Out-Null

  for ($i = 1; $i -le 5; $i++) {
    try {
      Copy-Item -LiteralPath $Source -Destination $Destination -Force -ErrorAction Stop
      Write-Host "==> Sidecar copiado a $Destination"
      return $true
    } catch {
      Write-Host "==> Copy-Item bloqueado (intento $i/5): $($_.Exception.Message)" -ForegroundColor Yellow
      Stop-SuiteDesktopProcesses | Out-Null
      Start-Sleep -Seconds 2
    }
  }

  $stamp = Get-Date -Format "yyyyMMdd-HHmmss"
  $alt = [System.IO.Path]::Combine(
    $destDir,
    ([System.IO.Path]::GetFileNameWithoutExtension($Destination) + "-build-$stamp.exe")
  )
  try {
    Copy-Item -LiteralPath $Source -Destination $alt -Force -ErrorAction Stop
    Write-Host ""
    Write-Host "AVISO: no se pudo sobrescribir (archivo en uso):" -ForegroundColor Red
    Write-Host "  $Destination" -ForegroundColor Red
    Write-Host "Sidecar nuevo guardado en:" -ForegroundColor Yellow
    Write-Host "  $alt" -ForegroundColor Yellow
    Write-BuildLockHelp
    return $false
  } catch {
    Write-BuildLockHelp
    throw "No se pudo copiar el sidecar: $($_.Exception.Message)"
  }
}

function Prepare-TauriReleaseDir {
  $releaseDir = Join-Path $Desktop "src-tauri\target\release"
  $releaseExe = Join-Path $releaseDir "$CargoExeName.exe"
  if (Test-Path $releaseExe) {
    if (-not (Remove-FileWithRetry -Path $releaseExe)) {
      Write-Host "==> (aviso) $releaseExe sigue en uso; cargo puede fallar al enlazar" -ForegroundColor Yellow
      Write-BuildLockHelp
    }
  }
}

Write-Host "==> Preparando build (cerrar apps que bloqueen backend.exe / $CargoExeName.exe)"
$null = Stop-SuiteDesktopProcesses

# --- Verificar Rust (se usa para el target triple y luego para 'npm run build') ---
if (-not (Get-Command rustc -ErrorAction SilentlyContinue)) {
    Write-Host ""
    Write-Host "ERROR: no se encontró 'rustc' (Rust) en el PATH." -ForegroundColor Red
    Write-Host "       Rust es necesario para nombrar el sidecar y para compilar la app Tauri." -ForegroundColor Yellow
    Write-Host ""
    Write-Host "  Solución:" -ForegroundColor Cyan
    Write-Host "    winget install -e --id Rustlang.Rustup" -ForegroundColor Cyan
    Write-Host "    # cierra y reabre PowerShell (para refrescar el PATH), luego:" -ForegroundColor Cyan
    Write-Host "    rustup default stable-msvc" -ForegroundColor Cyan
    throw "Rust no está instalado o no está en el PATH."
}

# --- Seleccionar intérprete de Python ---
$Py = $null
if (Test-Path "venv\Scripts\python.exe") {
    $Py = "venv\Scripts\python.exe"
} elseif (Get-Command py -ErrorAction SilentlyContinue) {
    try { & py -3.12 --version *> $null; if ($LASTEXITCODE -eq 0) { $Py = "py -3.12" } } catch {}
    if (-not $Py) { $Py = "py" }
} elseif (Get-Command python -ErrorAction SilentlyContinue) {
    $Py = "python"
} else {
    throw "No se encontró Python. Instala Python 3.12 (https://www.python.org/downloads/)."
}

# --- Validar versión soportada (3.10–3.12) ---
$verRaw = & cmd /c "$Py -c ""import sys;print('%d.%d'%sys.version_info[:2])"""
$parts = $verRaw.Trim().Split('.')
$major = [int]$parts[0]; $minor = [int]$parts[1]
if ($major -ne 3 -or $minor -lt 10 -or $minor -gt 12) {
    Write-Host ""
    Write-Host "ERROR: Python $verRaw no es compatible para empaquetar el backend." -ForegroundColor Red
    Write-Host "       Las dependencias pinneadas (numpy 1.26.4) solo tienen wheels" -ForegroundColor Yellow
    Write-Host "       para Python 3.10-3.12. Con 3.13/3.14 pip compila desde fuente y falla." -ForegroundColor Yellow
    Write-Host ""
    Write-Host "  Solución:" -ForegroundColor Cyan
    Write-Host "    winget install -e --id Python.Python.3.12" -ForegroundColor Cyan
    Write-Host "    Remove-Item -Recurse -Force venv" -ForegroundColor Cyan
    Write-Host "    py -3.12 -m venv venv" -ForegroundColor Cyan
    Write-Host "    venv\Scripts\python -m pip install -r requirements.txt pyinstaller" -ForegroundColor Cyan
    Write-Host "    powershell -ExecutionPolicy Bypass -File desktop\scripts\build-backend.ps1" -ForegroundColor Cyan
    throw "Versión de Python no soportada: $verRaw (usa 3.12)."
}
Write-Host "==> Usando Python $verRaw ($Py)"

$Req = "requirements.txt"
if (Test-Path "requirements-desktop.txt") { $Req = "requirements-desktop.txt" }
Write-Host "==> Instalando dependencias de build desde $Req (+ PyInstaller)"
& cmd /c "$Py -m pip install --quiet -r $Req pyinstaller"

$DistStaging = Join-Path $env:TEMP ("ccs-backend-dist-" + [Guid]::NewGuid().ToString("N").Substring(0, 10))
$WorkStaging = Join-Path $env:TEMP ("ccs-backend-work-" + [Guid]::NewGuid().ToString("N").Substring(0, 10))
New-Item -ItemType Directory -Force -Path $DistStaging | Out-Null
New-Item -ItemType Directory -Force -Path $WorkStaging | Out-Null

Write-Host "==> Empaquetando backend con PyInstaller (dist en TEMP: $DistStaging)"
Stop-SuiteDesktopProcesses | Out-Null
& cmd /c "$Py -m PyInstaller --clean --noconfirm --distpath `"$DistStaging`" --workpath `"$WorkStaging`" desktop/backend/backend.spec"
if ($LASTEXITCODE -ne 0) {
  Write-BuildLockHelp
  throw "PyInstaller fallo (codigo $LASTEXITCODE). Si el log menciona 'being used by another process', cierre $ProductName.exe y backend.exe."
}

$BuiltExe = Join-Path $DistStaging "backend.exe"
if (-not (Test-Path $BuiltExe)) {
  throw "PyInstaller no genero $BuiltExe"
}

$Triple = ((rustc -vV | Select-String "host: ") -replace "host: ", "").Trim()
$SidecarDest = Join-Path $Root "desktop/src-tauri/binaries/backend-$Triple.exe"
$null = Copy-SidecarExecutable -Source $BuiltExe -Destination $SidecarDest

Write-Host "==> Sidecar listo: desktop/src-tauri/binaries/backend-$Triple.exe"

Write-Host "==> Configurando splash / Tauri (configure.mjs)"
Set-Location $Desktop
& node scripts/configure.mjs
if ($LASTEXITCODE -ne 0) {
  throw "configure.mjs fallo (codigo $LASTEXITCODE)"
}
Set-Location $Root

Write-Host ""
if ($Installer) {
  Write-Host "==> Building Tauri installer (MSI + NSIS setup.exe)..." -ForegroundColor Cyan
  Prepare-TauriReleaseDir
  Stop-SuiteDesktopProcesses | Out-Null
  Set-Location $Desktop
  if (-not (Test-Path "node_modules")) {
    Write-Host "==> npm install (desktop)"
    npm install
  }
  if (-not (Test-Path "src-tauri/icons/icon.ico")) {
    Write-Host "==> Generating icons (npm run icon) - first time only"
    npm run icon
  }
  npm run build
  if ($LASTEXITCODE -ne 0) {
    Write-BuildLockHelp
    throw "npm run build / cargo fallo. Cierre $ProductName.exe si el error es 'failed to remove' o 'access denied' en target\release."
  }
  Write-Host ""
  Write-Host "Installers:" -ForegroundColor Green
  Write-Host "  $Desktop\src-tauri\target\release\bundle\msi\*.msi"
  Write-Host "  $Desktop\src-tauri\target\release\bundle\nsis\*-setup.exe"
} else {
  Write-Host "Siguiente paso (.msi / *-setup.exe):" -ForegroundColor Yellow
  Write-Host "  powershell -ExecutionPolicy Bypass -File desktop\scripts\build-backend.ps1 -Installer"
}
