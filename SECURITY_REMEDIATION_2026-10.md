# Remediación de seguridad — 27 oct 2026

Informe de lo que se cambió en el código después del resumen Checkmarx del 14 oct 2026 (scan `8255521f-3893-4276-a3d1-b75ccf17fc89`, zip `SmartRedes-CCS-main`, 16 mil líneas). Ese PDF no trae query, archivo, línea ni data flow. Donde no hay ubicación, el hallazgo queda pendiente del export.

La aplicación sigue siendo de escritorio, un usuario, backend en loopback. Analizar un sitio público que el usuario escribe sigue habilitado.

## 1. Antes y después

| Tema | Antes | Después | Disposición |
|---|---|---|---|
| DOMPurify | 3.4.17 no existía. El archivo entregado era `app/lib/purify.min.js` 3.4.1, sin manifiesto npm del frontend. | 3.4.17 en `app/lib/purify.min.js`, `app/package.json` y `app/package-lock.json`. | Corregido en el producto. La interfaz cargó `DOMPurify.version === "3.4.17"` y `safeHTML('<b>ok</b><script>alert(1)</script>')` devolvió `<b>ok</b>`. |
| `transformers` 5.19.0 | No está en el repositorio. `requirements.txt` dice `transformers>=4.40.0`. `requirements.lock` fija `transformers==5.6.2`. | No se subió la versión. | Fuera del artefacto del instalador. El escáner resolvió el rango sin pin a la última publicación. |
| `accelerate` 1.15.0 | No está en el repositorio. `requirements.txt` dice `accelerate>=0.30.0`. `requirements.lock` fija `accelerate==1.13.0`. `desktop/src-tauri/Cargo.lock` tiene `portable-atomic` 1.15.0, que no es el paquete Python. | No se subió la versión. | Fuera del artefacto del instalador. Misma causa que transformers: rango sin pin. |

Búsqueda ejecutada de `5.19.0` y `accelerate==1.15` en el árbol: sin coincidencias. `pip freeze` del venv con el que se arma el sidecar no lista `transformers` ni `accelerate`. El instalador instala `requirements-desktop.txt`, que tampoco los incluye. `desktop/smartsuite.config.json` los excluye del empaquetado.

El código que sí puede cargar un modelo de imagen (`server/image_engine.py`) ahora solo acepta los tres identificadores del catálogo (`SimianLuo/LCM_Dreamshaper_v7`, `stabilityai/sd-turbo`, `stabilityai/sdxl-turbo`) y llama a `from_pretrained` con `trust_remote_code=False`. Una ruta, una URL o un archivo aportado por el usuario no llega a esa carga. Esos tres identificadores siguen descargándose a la caché local del motor opcional; ese motor no va en el instalador.

### DOMPurify: `IN_PLACE` y `setConfig`

Búsqueda en `app/index.html` y en `server/` de `IN_PLACE` y `setConfig(`: ninguna coincidencia. El único llamado propio es `DOMPurify.sanitize(html, { ALLOWED_TAGS, ALLOWED_ATTR, ADD_ATTR })` en `safeHTML`. No usa `IN_PLACE: true` ni `setConfig`. El archivo minificado de la librería contiene la implementación de esas opciones; no es un llamado de esta app. No se eliminó un modo que no se usa.

## 2. Archivos modificados

| Archivo | Por qué |
|---|---|
| `app/lib/purify.min.js` | Sustituye DOMPurify 3.4.1 por 3.4.17. |
| `app/package.json` | Manifiesto del frontend que antes no existía. Fija `dompurify` 3.4.17. |
| `app/package-lock.json` | Lockfile de esa dependencia. |
| `server/ssrf_guard.py` | Guardia nueva: solo http/https, resolución de todas las IP, rechazo de privadas, loopback, link-local, reservadas y metadata, máximo 5 redirecciones, 12 segundos y 2 MB. |
| `server/web_scraper.py` | El análisis de sitios públicos usa esa guardia en cada pedido y en cada redirección, incluida Jina y Playwright. |
| `server/path_safety.py` | Basename de imagen y confinamiento de la ruta resuelta. Rechaza `..`, absolutas, separadores y un symlink que sale de la carpeta. |
| `server/app.py` | Sirve y guarda imágenes solo dentro de `exports/images`. Importación confinada. Tope de 366 días y 8 canales. Log del modelo sin el texto completo. Host solo loopback. CORS incluye `[::1]`. |
| `server/log_safety.py` | Filtro central que quita CR y LF de todo mensaje de log. |
| `server/image_engine.py` | Catálogo cerrado y `trust_remote_code=False`. |
| `requirements.txt` | Comentario de que el lock fija 5.6.2 y 1.13.0 y de que el instalador no los lleva. No cambia versiones. |
| `tests/test_ssrf_path_2026.py` | Pruebas nuevas de SSRF, path traversal y del filtro de log. |
| `tests/test_security_remediation.py`, `tests/test_api.py`, `tests/test_fixes_v2.py`, `tests/test_fixes_v3.py`, `tests/test_interview_integration.py` | El cliente de prueba usa `Host: 127.0.0.1`, que es el único host que el backend acepta. |
| `.github/workflows/desktop-build.yml`, `.github/workflows/macos-dmg.yml` | `permissions: contents: read`. |

## 3. Tests ejecutados

Comando:

```
venv\Scripts\python.exe -m pytest tests\test_ssrf_path_2026.py tests\test_security_remediation.py::TestSSRFProtection tests\test_security_remediation.py::TestURLValidation -s -p no:cacheprovider --tb=line
```

Resultado: 23 ítems, todos en verde, código de salida 0. Cubren URL pública, localhost, 127.0.0.1, 192.168.x.x, 10.x.x.x, 169.254.169.254, redirección a 127.0.0.1, `file://`, `..%2f`, ruta absoluta, nombre con separadores y symlink que escapa. El symlink real no se pudo crear en este Windows (`WinError 1314`, falta el privilegio). La prueba del escape usa un `Path` que se comporta como symlink y apunta fuera de la carpeta; `confined_file` lo rechaza.

`SMARTREDES_ALLOW_PRIVATE_URLS=1` deja pasar 127.0.0.1. Sin la variable, lo privado se rechaza. La variable está documentada en `server/ssrf_guard.py` y queda desactivada por defecto.

La suite completa `pytest tests` no se puede dar por verde en esta máquina. Pytest revienta con `ValueError: I/O operation on closed file` al imprimir un fallo. Eso ya ocurría antes de este cambio. Ejecutado archivo por archivo:

| Archivo | Resultado |
|---|---|
| `tests/test_ssrf_path_2026.py` | 13 pasaron |
| `tests/test_api.py` | 43 pasaron |
| `tests/test_hardware_profile.py` | 7 pasaron |
| `tests/test_llm_resilience.py` | 19 pasaron |
| `tests/test_context_manager.py` | 38 pasaron |
| `tests/test_security_remediation.py` | 36 pasaron, 1 falló |
| `tests/test_fixes_v2.py` | el fallo visto en lote no se reprodujo al correr el test solo |
| `tests/test_fixes_v3.py` | la sesión de pytest murió al escribir el resultado |
| `tests/test_concurrency_and_integration.py` | los tres tests de auditoría pasaron al correrlos solos |
| `tests/test_interview_integration.py` | no se corrió entero |

El fallo reproducible es `TestDirectoryPermissions::test_data_dir_permissions`: en este Windows `os.chmod(..., 0o700)` deja el directorio en `0o777`. No lo toqué. No es una de las cinco altas.

La interfaz se abrió en `http://127.0.0.1:8766/index.html`. El dashboard pintó. DOMPurify reportó 3.4.17 y quitó un `<script>`. No se regeneró el instalador de Windows ni el DMG en esta pasada.

## 4. Categorías SAST de `server/app.py`

| Categoría | Estado |
|---|---|
| Path traversal (1 alta) | Corregida. Nombre local generado o basename de imagen; la ruta resuelta tiene que quedar en `DATA_DIR/exports/images`. `..`, `%2f`, absolutas, separadores y symlink que escapa se rechazan. Hay tests. |
| SSRF (1 alta, el flujo vive en `web_scraper.py` y en `validate_url_safe`) | Corregida como endurecimiento. Los sitios públicos siguen. Esquema distinto de http/https se rechaza. Cada IP resuelta se revisa, también después de cada redirección, con tope de 5. Timeout 12 s, cuerpo máximo 2 MB, content-type antes de leer el HTML. |
| Filtering Sensitive Logs | Corregida en el punto que escribía texto del modelo (`llm_output[:200]`). Ahora se registra solo la longitud. No hay logs de contraseñas, tokens de API ni del HTML completo del sitio. El resto de líneas sin ubicación de Checkmarx queda pendiente del export. |
| Log forging | Corregida. `LogSanitizerFilter` quita CR y LF de todo mensaje del logger raíz y de `css-brand-assistant`. Test incluido. |
| Unchecked input for loop condition (3 medias, sin línea en el PDF) | Corregida donde el límite sale de datos externos: campaña máximo 366 días y 8 canales; el JSON del modelo se recorre hasta 200 000 caracteres; una importación no acepta más de 400 publicaciones. Las otras dos medias, si no son estas, siguen pendientes del export. |
| El resto de las ~104 bajas de `app.py` | Pendiente del export. El PDF no da query ni línea. No inventé parches. |

## 5. Entorno del instalador

`desktop/scripts/build-backend.ps1` instala `requirements-desktop.txt` (FastAPI, uvicorn, requests, httpx, beautifulsoup4, pydantic, Pillow y dependencias directas). No instala torch, diffusers, transformers, accelerate ni playwright.

`pip freeze` del venv local usado para compilar el sidecar, el 27 oct 2026:

```
aiofiles==25.1.0
altgraph==0.17.5
annotated-doc==0.0.5
annotated-types==0.8.0
anyio==4.15.1
beautifulsoup4==4.15.0
certifi==2026.7.22
charset-normalizer==3.5.1
click==8.5.0
colorama==0.4.6
fastapi==0.141.1
h11==0.16.0
httpcore==1.0.9
httptools==0.8.0
httpx==0.28.1
idna==3.19
iniconfig==2.3.0
packaging==26.3
pefile==2024.8.26
pillow==12.3.0
pluggy==1.6.0
pydantic==2.13.5
pydantic_core==2.46.5
Pygments==2.21.0
pyinstaller==6.22.3
pyinstaller-hooks-contrib==2026.7
pytest==9.1.1
pytest-asyncio==1.4.0
python-dotenv==1.2.3
python-multipart==0.0.32
pywin32-ctypes==0.2.3
PyYAML==6.0.3
requests==2.34.2
setuptools==84.0.0
soupsieve==2.9.2
starlette==1.6.0
typing-inspection==0.4.4
typing_extensions==4.16.0
urllib3==2.8.0
uvicorn==0.53.0
watchfiles==1.2.0
websockets==17.1
```

Pytest está en ese venv porque es el entorno de desarrollo. No forma parte de `requirements-desktop.txt`. No hay SBOM CycloneDX generado.

## 6. Pendientes que necesitan el export de Checkmarx

Sin query, archivo, línea y data flow no se cierran estos puntos:

- Las bajas de SAST que el PDF solo cuenta dentro de `server/app.py` (109) y `server/web_scraper.py` (2), fuera de las categorías ya corregidas arriba.
- Cuáles de las 3 medias «Unchecked Input for Loop Condition» son las que no coinciden con el calendario, el parser JSON o la importación.
- Los CVE concretos de DOMPurify 3.4.1 que no dependan de `IN_PLACE`. La librería ya está en 3.4.17; falta el export para marcar cada CVE como resuelto o no aplicable.
- El único hallazgo IaC. En el repo solo hay `.github/workflows/desktop-build.yml` y `.github/workflows/macos-dmg.yml`. Se les puso `permissions: contents: read`. Las actions siguen en tag mayor (`@v4`, `@v5`, `@stable`), no en SHA. No se imprimen secretos. La regla exacta que disparó el hallazgo no está en el PDF.
- No hay token de sesión local. Agregarlo ahora rompería el frontend, que no lo envía. Propuesta, no implementada: un secreto por arranque del sidecar, enviado solo por la ventana Tauri, rechazado si falta. No cambia el producto hasta decidirlo.

`SMARTREDES_ALLOW_PRIVATE_URLS=1` existe para un caso interno explícito. Sin esa variable, una IP privada se rechaza siempre.
