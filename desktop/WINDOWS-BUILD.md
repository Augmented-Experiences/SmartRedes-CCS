# Generar los instaladores de Windows localmente (SmartSuite CCS)

Guía paso a paso para compilar en **Windows** el instalador `.msi`/`.exe` de cada herramienta (SmartCaja, SmartRedes, SmartGastos) con el kit Tauri. "Nuestro propio Pinokio": un instalador nativo por app, con marca de Cámara de Comercio de Santiago y descarga de modelos LLM con progreso en el arranque.

---

## 0) Prerrequisitos (instalar una sola vez)

En **PowerShell**:

```powershell
winget install -e --id Python.Python.3.12
winget install -e --id Rustlang.Rustup
winget install -e --id OpenJS.NodeJS.LTS
winget install -e --id Microsoft.VisualStudio.2022.BuildTools
```

- En **VS Build Tools** marca **"Desktop development with C++"** (linker MSVC que Rust necesita).
- **Cierra y reabre PowerShell** (para refrescar el PATH) y fija el toolchain:
  ```powershell
  rustup default stable-msvc
  ```
- WebView2 ya viene en Windows 10/11. Ollama **no** es necesario para compilar (la app lo prepara al ejecutarse).
- Usa **Python 3.12** exactamente (no 3.13/3.14; numpy 1.26.4 no tiene wheels para esas versiones).

---

## Notas comunes
- El instalador queda en `desktop\src-tauri\target\release\bundle\` → `msi\*.msi` y `nsis\*-setup.exe`.
- El backend se empaqueta con PyInstaller usando `requirements-desktop.txt` (subconjunto liviano). Las capacidades pesadas opcionales **no** se empaquetan:
  - **SmartRedes**: generación de imágenes por IA — eliminada (no va en ningún caso).
  - **SmartGastos**: OCR neuronal — se hace con un **modelo de visión de Ollama (`moondream`)** que se descarga con barra de progreso en el primer arranque, igual que el modelo del LLM.
- La primera ejecución descarga Ollama portable (si hace falta) y `llama3.1:8b` con progreso en el splash; no requiere reiniciar ni el instalador MSI.

---

## 1) SmartCaja

```powershell
git clone https://github.com/Augmented-Experiences/ccs-cashflow-assistant.git SmartCaja
cd SmartCaja
py -3.12 -m venv venv
powershell -ExecutionPolicy Bypass -File desktop\scripts\build-backend.ps1
cd desktop
npm install
npm run icon      # una sola vez
npm run build
```
> Nota: usa la rama del instalador (`cursor/smartcaja-tauri-installer-2bcd`) si quieres exactamente el estado con todo el kit más reciente.

## 2) SmartRedes (repo `SmartRedes-CCS`)

Compila desde la rama `kit`, que contiene el splash con logo CCS, subtítulo de marca/redes y acento `#0FB5A6`:

```powershell
git clone -b kit https://github.com/Augmented-Experiences/SmartRedes-CCS.git SmartRedes
cd SmartRedes
# Debe existir la plantilla del splash:
if (-not (Test-Path desktop\ui\splash.template.html)) { throw "Falta splash.template.html — usa la rama kit" }
py -3.12 -m venv venv
powershell -ExecutionPolicy Bypass -File desktop\scripts\build-backend.ps1
cd desktop
npm install
npm run icon
npm run build
# npm run build ejecuta configure.mjs antes de tauri build; verifica el splash generado:
Select-String -Path ui\index.html -Pattern 'splash-logo','marca y redes','splash-kit: configure.mjs'
Select-String -Path ui\accent.css -Pattern '0FB5A6'
```

Si ya aplicaste `smartredes_full.patch` en un clone antiguo, mejor clona de nuevo y usa la rama `kit`.

## 3) SmartGastos (repo `pyme-ledger-ai.pinokio`)

```powershell
git clone https://github.com/Augmented-Experiences/pyme-ledger-ai.pinokio.git SmartGastos
cd SmartGastos
git apply smartgastos_full.patch
py -3.12 -m venv venv
powershell -ExecutionPolicy Bypass -File desktop\scripts\build-backend.ps1
cd desktop
npm install
npm run icon
npm run build
```

---

## Aplicar los cambios sin el `.patch` (manual)
Si prefieres no usar el `.patch`, en cada repo (SmartRedes/SmartGastos):
1. Copia la carpeta `desktop/` del kit (del repo SmartCaja) a la raíz del repo.
2. Copia `desktop/examples/<tool>.config.json` como `desktop/smartsuite.config.json`.
3. Usa `app/logo-ccs.svg` como el logo del instalador (`npm run icon`).
4. Aplica el parche de `server/app.py` (`smartredes_app_py.patch` / `smartgastos_app_py.patch`): `BASE_DIR` frozen-aware, `DATA_DIR` por env, y `import sys`.
5. Crea `requirements-desktop.txt` (subconjunto liviano sin torch/easyocr/diffusers).
6. Aplica el brand kit a la UI (`app/index.html`): variables `--ccs-*`, logo `logo-ccs.svg`, nombre de la herramienta.

---

## Problemas frecuentes
- **`numpy ... Preparing metadata ... error`**: usas Python 3.13/3.14. Usa 3.12 (`py -3.12 -m venv venv`).
- **`rustc no reconocido`**: instala Rust (rustup) y reabre la terminal; `rustup default stable-msvc`.
- **La app abre pero la IA dice "desconectado"**: no instales el MSI de Ollama. En el splash la app descarga el zip portable a `%APPDATA%\SmartRedes\ollama\`, arranca `serve` y hace pull de `llama3.1:8b` **sin reiniciar**. Si 11434 ya tenía un Ollama de sistema sano, se reutiliza y no se mata al salir.
