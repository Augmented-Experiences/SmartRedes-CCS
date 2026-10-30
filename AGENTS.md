# AGENTS.md

## Cursor Cloud specific instructions

CCS Brand Assistant is a single-product repo: a local FastAPI backend (`server/app.py`)
that also serves a vanilla-JS SPA frontend (`app/index.html`). The shipped product
is the desktop installer under `desktop/`. Pinokio/Gepeto launchers are not part
of the release. There is no database — all state is JSON under `data/`
(gitignored). Desktop deps are Node under `desktop/`; the UI pins DOMPurify in `app/package.json`.

### Environment
- Python deps for the installer are `requirements-desktop.txt`. Use `venv/bin/python`.
- Do not add torch, diffusers, transformers, accelerate, playwright, or Pillow:
  they are not in the release and SCA resolves unpinned ranges to versions we do not ship.

### Running the app (dev) `PORT=7860 venv/bin/python server/app.py` (binds `127.0.0.1:7860` only).
- UI: `http://127.0.0.1:7860/ui/index.html` (root `/` 307-redirects there).
- Health check: `curl http://127.0.0.1:7860/api/health`.
- Do NOT run A1111 on the default port — its default (`7860`) collides with this app.

### Data directory (gotcha)
- `data/` is gitignored, so a truly fresh checkout has no data dirs. The app creates
  most subdirs on demand, but agent/prompt defaults come from `defaults/`. If `data/`
  is missing, re-create the data dirs the app expects:
  ```
  venv/bin/python -c "import os;[os.makedirs(d,exist_ok=True) for d in ['data/agents','data/prompts/system','data/prompts/skills','data/sessions','data/exports','data/brands','data/campaigns','data/audit','data/images']]"
  venv/bin/python -c "import os,shutil;dst='data/agents/agents.json';shutil.copy2('defaults/agents.json',dst) if not os.path.exists(dst) else None"
  venv/bin/python -c "import os,shutil,glob;[shutil.copy2(f,'data/prompts/system/') for f in glob.glob('defaults/prompts/*.md') if not os.path.exists('data/prompts/system/'+os.path.basename(f))]"
  ```

### Ollama (LLM) — optional
- Ollama is NOT installed here. The app runs and boots fine without it (health shows
  `ollama_available:false`; a "No se pudo verificar modelos" warning on startup is
  expected and harmless).
- Flows that need the LLM (website analysis, AI interview, ADN generation, campaign
  generation) require a local Ollama at `http://localhost:11434` with a model pulled
  (e.g. `ollama pull llama3.2:3b`). Non-LLM flows (create/list brands, serving the UI)
  work without it.

### Tests
- Run: `venv/bin/python -m pytest tests/ --asyncio-mode=auto`
- `pytest` and `pytest-asyncio` are required and are installed by the update script.
  The repo has no pytest config, so `--asyncio-mode=auto` MUST be passed on the CLI or
  the ~10 `async def` tests error out with "async def functions are not natively
  supported". Tests mock Ollama, so they do not need it running.
- There is no separate linter configured (no flake8/ruff/eslint config in the repo).
