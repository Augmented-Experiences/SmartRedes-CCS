# SmartRedes Desktop Kit (Cámara de Comercio de Santiago) — App/Instalador de escritorio (Tauri)

Kit **reutilizable** para empaquetar SmartRedes como instalador nativo para **Windows**, **macOS** y **Linux**, sin necesidad de Pinokio. Empaqueta la interfaz web + el backend FastAPI en una app de escritorio con identidad de la Cámara de Comercio de Santiago.

Este mismo `desktop/` sirve para las 3 herramientas: solo cambia `smartsuite.config.json` (nombre, id, acento, modelos) — el resto es común. `scripts/configure.mjs` genera los archivos variables antes de compilar.

## Arquitectura

```
Ventana Tauri (webview nativo)
        │  al iniciar muestra ui/index.html (pantalla de carga)
        ▼
Rust (src-tauri/src/main.rs) — genérico para todas las herramientas
  1. Elige un puerto libre.
  2. Lanza el backend empaquetado como "sidecar" (backend), pasándole
     PORT y DATA_DIR por variables de entorno.
  3. Prepara Ollama (API HTTP, sin MSI): usa un Ollama de sistema ya
     sano en 11434, o arranca un binario portable en la carpeta de datos
     (%APPDATA%/SmartRedes/ollama o XDG), descargando zip/tgz oficial si
     hace falta. El splash muestra descarga/extracción/arranque/pull de
     llama3.1:8b y continúa **sin reiniciar**. Al cerrar, detiene el
     sidecar y solo el Ollama que esta app arrancó.
        │
        ▼
Backend FastAPI (server/app.py) empaquetado con PyInstaller
  - Sirve la UI (/ui) y la API (/api), igual que en Pinokio.
  - Recursos (app/, defaults/) desde el bundle; datos del usuario en una
    carpeta escribible por-usuario (DATA_DIR por entorno).
```

Ventajas: el usuario descarga **un instalador** y ejecuta la app; no necesita conocer Pinokio ni la terminal.

## Configuración por herramienta (`smartsuite.config.json`)

Todo lo específico de cada app vive en `desktop/smartsuite.config.json`:

| Campo | Uso |
|---|---|
| `productName`, `identifier`, `version` | Nombre, id de bundle y versión. |
| `dataDirName` | Nombre de la carpeta de datos por-usuario. |
| `accent` | Color de acento de la herramienta (`#0FB5A6` para SmartRedes). |
| `window` | Título y tamaño de ventana. |
| `ollama.tiers` | Modelo (chat/LLM) a descargar según la RAM (`maxRamGb: 0` = sin límite / último). |
| `ollama.extraModels` | Modelos adicionales a descargar con progreso (p. ej. un modelo de **visión para OCR neuronal** como `moondream`). Se descargan igual que el LLM, con barra de progreso en la pantalla de carga — así capacidades pesadas (OCR) no requieren empaquetar torch/easyocr. |

`scripts/configure.mjs` (se ejecuta solo con `npm run build`/`npm run dev`) genera desde ese config: `src-tauri/tauri.conf.json` (nombre de instaladores `.msi`/`-setup.exe` en Windows), `src-tauri/Cargo.toml`, `src-tauri/appconfig.json` (que Rust lee), `package.json`, `ui/index.html` (splash con logo `splashLogo` de CCS), y los CSS de marca (`ui/ccs-theme.css`, `ui/accent.css`). Edita `ui/splash.template.html` para cambios estructurales del splash; el título, subtítulo y logo salen de `productName`, `splashSubtitle` y `splashLogo`.

## Brand kit CCS (estilo compartido)

`desktop/brand/ccs-theme.css` contiene la paleta compartida de CCS y usa variables `--ccs-*`. El logo de CCS vive en `app/logo-ccs.svg` y es la fuente tanto del splash como de los iconos del instalador.

## Requisitos de build

- **Python 3.10–3.12 (recomendado 3.12)** + este repo (`requirements.txt`) — para empaquetar el backend. **No uses 3.13/3.14**: `numpy 1.26.4` (pinneado) no publica wheels para esas versiones y pip intentaría compilarlo desde fuente (falla en Windows). Los scripts de build validan la versión y avisan.
- Rust (stable) + Cargo.
- Node 18+ (para la CLI de Tauri).
- Linux: `libwebkit2gtk-4.1-dev`, `librsvg2-dev`, `libgtk-3-dev`, `libayatana-appindicator3-dev`, `patchelf`, `build-essential` (ver CI).
- **Ollama** no usa el MSI/instalador de sistema: si no hay un Ollama
  sano en 11434, la app descarga el zip/tgz oficial portable a la carpeta
  de datos del producto, lo arranca y hace pull de `llama3.1:8b` en el splash.

## Build local

```bash
# 1) Empaquetar el backend (genera el sidecar 'backend' en src-tauri/binaries/)
bash desktop/scripts/build-backend.sh      # Windows: desktop/scripts/build-backend.ps1

# 2) Generar iconos CCS (una sola vez; requiere red la primera vez)
cd desktop && npm install && npm run icon   # usa ../app/logo-ccs.svg y crea src-tauri/icons/

# 3) Construir el instalador (configure.mjs corre antes de tauri build)
npm run build
```

Los instaladores quedan en `desktop/src-tauri/target/release/bundle/` (`.AppImage`/`.deb`/`.rpm` en Linux, `.dmg` en macOS, `.msi`/`.exe` en Windows).

## Aplicar el kit a otra herramienta (SmartRedes, SmartGastos)

Como los repos son separados, se copia el kit a cada uno:

1. Copia a la raíz del repo destino la carpeta `desktop/` y conserva el logo oficial en `app/logo-ccs.svg`.
2. Sustituye `desktop/smartsuite.config.json` por el de la herramienta (hay ejemplos listos en `desktop/examples/smartredes.config.json` y `desktop/examples/smartgastos.config.json`).
3. **Parche de `server/app.py` (2–3 líneas, retrocompatible con Pinokio):**
   - `BASE_DIR` debe apuntar al bundle cuando la app está empaquetada (para servir `app/` y `defaults/`):
     ```python
     # antes:
     BASE_DIR = Path(__file__).parent.parent.resolve()
     # después:
     BASE_DIR = Path(sys._MEIPASS) if getattr(sys, "frozen", False) else Path(__file__).parent.parent.resolve()
     ```
   - `DATA_DIR` debe respetar la variable de entorno (el kit la fija a una carpeta escribible por-usuario):
     ```python
     DATA_DIR = Path(os.environ.get("DATA_DIR", str(BASE_DIR / "data")))
     ```
   - Asegúrate de que `import sys` esté presente.

   Ya hay parches listos en los artefactos del agente: `smartredes_app_py.patch` y `smartgastos_app_py.patch` (aplícalos con `git apply`). Es retrocompatible: Pinokio no define `DATA_DIR` ni `frozen`, así que su comportamiento no cambia.
4. Genera iconos (`npm run icon`) y construye (`bash scripts/build-backend.sh` + `npm run build`).

El backend se lanza con `PORT` y `DATA_DIR` por entorno, así que sirve para las 3 apps (SmartCaja/SmartRedes usan `--port`/`PORT`; SmartGastos usa `PORT`, default 8000 — irrelevante porque el kit fija el puerto).

Para aplicar el estilo CCS a la UI, enlaza `brand/ccs-theme.css` en `app/index.html` y usa las variables `--ccs-*` con el acento de la herramienta.

## Build multiplataforma (recomendado)

Los instaladores de cada SO deben construirse en su propio SO. Usa el workflow de GitHub Actions incluido: `.github/workflows/desktop-build.yml` (ejecútalo con *workflow_dispatch* o al publicar un tag `v*`). Genera los artefactos para Windows/macOS/Linux y los sube como *artifacts*.

## Troubleshooting

- **`Preparing metadata (pyproject.toml) ... error` al instalar numpy (Windows):** tu Python es demasiado nuevo (3.13/3.14) y `numpy 1.26.4` no tiene wheel; pip intenta compilar desde fuente. Solución: usa Python 3.12.
  ```powershell
  winget install -e --id Python.Python.3.12
  Remove-Item -Recurse -Force venv
  py -3.12 -m venv venv
  venv\Scripts\python -m pip install -r requirements.txt pyinstaller
  powershell -ExecutionPolicy Bypass -File desktop\scripts\build-backend.ps1
  ```

- **`rustc : The term 'rustc' is not recognized...` (Windows) o `rustc: command not found`:** Rust no está instalado o no está en el PATH. Instálalo y reabre la terminal:
  ```powershell
  winget install -e --id Rustlang.Rustup
  # reabre PowerShell y luego:
  rustup default stable-msvc
  ```
  Rust es necesario tanto para nombrar el sidecar como para `npm run build`.

## Cómo funciona con Ollama y los modelos

Igual que la versión Pinokio: Ollama y los modelos viven en la máquina del usuario. Al iniciar, la app (best-effort) arranca `ollama serve` y descarga el modelo según la RAM (`<6 GB` → `llama3.2:1b`, `6–12 GB` → `llama3.2:3b`, `>12 GB` → `llama3.1:8b`). Si Ollama no está instalado, la app abre igual y la UI indica que el motor de IA está desconectado con instrucciones. Requiere internet solo la primera vez; luego funciona 100% local.
