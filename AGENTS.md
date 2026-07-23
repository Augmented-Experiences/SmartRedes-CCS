# AGENTS.md

## Cursor Cloud specific instructions

CCS Brand Assistant is a single-product repo: a local FastAPI backend (`server/app.py`)
that also serves a vanilla-JS SPA frontend (`app/index.html`). It is packaged as a
Pinokio plugin (`pinokio.js`, `install.js`, `start.js`), but Pinokio is NOT needed
to develop or run it here. There is no database — all state is JSON under `data/`
(gitignored). There is no Node/`package.json`.

### Environment
- Python deps live in a local venv at `venv/` (created by the update script via
  `scripts/setup_venv.sh`). Use `venv/bin/python` / `venv/bin/pip` directly.
- System package `python3.12-venv` is required to create the venv; it is installed once
  during environment setup (not part of the update script).
- Optional heavy deps (torch CPU, diffusers/transformers, playwright + chromium) are
  installed by `scripts/setup_venv.sh` and are failure-tolerant — the app degrades
  gracefully without them.

### Pinokio / Gepeto launcher
- Pinokio scripts are **Gepeto v5.0** format: `install.js`, `start.js`, `stop.js`, `reset.js`, `update.js`, `torch.js`, `link.js`, `pinokio.js`, `pinokio.json`.
- Legacy `.json` scripts (`install.json`, etc.) were replaced by `.js` equivalents.
- Default port is `7860`; `start.js` sets `local.url` to `{server}/ui/index.html` for the "Abrir UI" menu button.
- `torch.js` is adapted from Gepeto (uses `pip`, not `uv pip`) for broader compatibility.
- Manual dev setup without Pinokio still uses `scripts/setup_venv.sh`.

### Running the app (dev) `PORT=7860 venv/bin/python server/app.py` (binds `127.0.0.1:7860` only).
- UI: `http://127.0.0.1:7860/ui/index.html` (root `/` 307-redirects there).
- Health check: `curl http://127.0.0.1:7860/api/health`.
- Do NOT run A1111 on the default port — its default (`7860`) collides with this app.

### Data directory (gotcha)
- `data/` is gitignored, so a truly fresh checkout has no data dirs. The app creates
  most subdirs on demand, but agent/prompt defaults come from `defaults/`. If `data/`
  is missing, re-create it with the same steps `install.js` uses:
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
