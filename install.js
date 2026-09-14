/**
 * CCS Brand Assistant — Gepeto install script
 *
 * Instala Ollama (opcional), dependencias Python (FastAPI, Torch, Diffusers,
 * Playwright) e inicializa la estructura de datos del plugin.
 */
module.exports = {
  run: [
    // ── Banner ──────────────────────────────────────────────────────────────
    {
      method: "log",
      params: {
        html: "<div style='font-family:-apple-system,BlinkMacSystemFont,Segoe UI,Roboto,sans-serif;padding:24px 20px 0'><div style='display:flex;align-items:center;gap:14px;margin-bottom:16px'><div style='width:48px;height:48px;border-radius:12px;background:linear-gradient(135deg,#0FB5A6,#0a8f83);display:flex;align-items:center;justify-content:center;font-size:24px;color:#fff;flex-shrink:0'>S</div><div><h2 style='margin:0;font-size:20px;color:#e2e8f0'>SmartRedes</h2><p style='margin:4px 0 0;color:#94a3b8;font-size:13px'>ADN de marca y campa&ntilde;as con IA local (CCCE)</p></div></div><div style='background:#1e293b;border-radius:8px;padding:14px 16px;margin-bottom:8px'><div style='display:flex;justify-content:space-between;margin-bottom:8px'><span style='color:#94a3b8;font-size:12px'>Plataforma</span><span style='color:#e2e8f0;font-size:12px;font-weight:600'>{{platform}}</span></div><div style='display:flex;justify-content:space-between;margin-bottom:8px'><span style='color:#94a3b8;font-size:12px'>RAM disponible</span><span style='color:#e2e8f0;font-size:12px;font-weight:600'>{{ram}} GB</span></div><div style='display:flex;justify-content:space-between'><span style='color:#94a3b8;font-size:12px'>Modelo de IA</span><span style='color:#3DAE2B;font-size:12px;font-weight:600'>{{ram < 6 ? 'llama3.2:1b (ligero)' : ram < 12 ? 'llama3.2:3b (est&aacute;ndar)' : 'llama3.1:8b (avanzado)'}}</span></div></div></div>"
      }
    },

    // ── 1. Ollama ───────────────────────────────────────────────────────────
    {
      method: "log",
      params: {
        html: "<div style='font-family:-apple-system,BlinkMacSystemFont,Segoe UI,Roboto,sans-serif;padding:16px 20px 0'><div style='background:#0f172a;border:1px solid #1e293b;border-radius:10px;padding:16px 18px'><div style='display:flex;align-items:center;gap:10px;margin-bottom:12px'><div style='width:28px;height:28px;border-radius:50%;background:#3b82f6;display:flex;align-items:center;justify-content:center;font-size:13px;color:#fff;font-weight:700;flex-shrink:0'>1</div><span style='color:#e2e8f0;font-size:14px;font-weight:600'>Verificando Ollama</span></div><p style='color:#94a3b8;font-size:12px;margin:0'>Buscando Ollama en el sistema...</p></div></div>"
      }
    },
    {
      when: "{{platform !== 'win32'}}",
      method: "shell.run",
      params: {
        message: "which ollama > /dev/null 2>&1 && echo OLLAMA_FOUND || (echo INSTALLING_OLLAMA && curl -fsSL https://ollama.com/install.sh | sh && echo OLLAMA_INSTALLED)"
      }
    },
    {
      when: "{{platform === 'win32'}}",
      method: "shell.run",
      params: {
        message: "where ollama > nul 2>&1 && echo OLLAMA_FOUND || (echo INSTALLING_OLLAMA && curl -L -o OllamaSetup.exe https://ollama.com/download/OllamaSetup.exe && start /wait OllamaSetup.exe /S && echo OLLAMA_INSTALLED)"
      }
    },

    // ── 2. Iniciar Ollama ─────────────────────────────────────────────────────
    {
      when: "{{platform !== 'win32'}}",
      method: "shell.run",
      params: {
        message: "curl -s http://127.0.0.1:11434/api/tags > /dev/null 2>&1 && echo OLLAMA_ALREADY_RUNNING || (echo STARTING_OLLAMA_SERVICE && ollama serve > /dev/null 2>&1 & sleep 5 && (curl -s http://127.0.0.1:11434/api/tags > /dev/null 2>&1 && echo OLLAMA_READY || echo OLLAMA_DEFERRED))"
      }
    },
    {
      when: "{{platform === 'win32'}}",
      method: "shell.run",
      params: {
        message: "curl -s http://127.0.0.1:11434/api/tags > nul 2>&1 && echo OLLAMA_ALREADY_RUNNING || (echo STARTING_OLLAMA_SERVICE && start /B ollama serve && ping -n 6 127.0.0.1 > nul && (curl -s http://127.0.0.1:11434/api/tags > nul 2>&1 && echo OLLAMA_READY || echo OLLAMA_DEFERRED))"
      }
    },

    // ── 3. Descargar modelos ──────────────────────────────────────────────────
    {
      when: "{{platform !== 'win32'}}",
      method: "shell.run",
      params: {
        message: "curl -s http://127.0.0.1:11434/api/tags > /dev/null 2>&1 && (ollama pull llama3.1:8b && echo MODEL_8B_DOWNLOADED && {{ram < 6 ? 'ollama pull llama3.2:1b' : ram < 12 ? 'ollama pull llama3.2:3b' : 'echo SKIP_EXTRA'}} && echo MODELS_DOWNLOADED || echo MODEL_DOWNLOAD_DEFERRED) || echo OLLAMA_NOT_RUNNING_SKIP_PULL"
      }
    },
    {
      when: "{{platform === 'win32'}}",
      method: "shell.run",
      params: {
        message: "curl -s http://127.0.0.1:11434/api/tags > nul 2>&1 && (ollama pull llama3.1:8b && echo MODEL_8B_DOWNLOADED && {{ram < 6 ? 'ollama pull llama3.2:1b' : ram < 12 ? 'ollama pull llama3.2:3b' : 'echo SKIP_EXTRA'}} && echo MODELS_DOWNLOADED || echo MODEL_DOWNLOAD_DEFERRED) || echo OLLAMA_NOT_RUNNING_SKIP_PULL"
      }
    },

    // ── 4. Dependencias Python (patrón Gepeto) ───────────────────────────────
    {
      method: "log",
      params: {
        html: "<div style='font-family:-apple-system,BlinkMacSystemFont,Segoe UI,Roboto,sans-serif;padding:16px 20px 0'><div style='background:#0f172a;border:1px solid #1e293b;border-radius:10px;padding:16px 18px'><div style='display:flex;align-items:center;gap:10px;margin-bottom:12px'><div style='width:28px;height:28px;border-radius:50%;background:#06b6d4;display:flex;align-items:center;justify-content:center;font-size:13px;color:#fff;font-weight:700;flex-shrink:0'>4</div><span style='color:#e2e8f0;font-size:14px;font-weight:600'>Instalando dependencias Python</span></div><p style='color:#94a3b8;font-size:12px;margin:0'>Entorno virtual + FastAPI + Playwright (scraper)...</p></div></div>"
      }
    },
    {
      method: "shell.run",
      params: {
        venv: "venv",
        message: [
          "pip install --upgrade pip",
          "pip install -r requirements-core.txt"
        ]
      }
    },
    {
      method: "shell.run",
      params: {
        venv: "venv",
        message: [
          "pip install playwright",
          "playwright install chromium"
        ]
      }
    },
    {
      method: "shell.run",
      params: {
        venv: "venv",
        message: "python -c \"import requests, fastapi, uvicorn, pydantic, PIL; print('VERIFY_OK')\""
      }
    },

    // ── 5. Inicializar datos ──────────────────────────────────────────────────
    {
      method: "log",
      params: {
        html: "<div style='font-family:-apple-system,BlinkMacSystemFont,Segoe UI,Roboto,sans-serif;padding:16px 20px 0'><div style='background:#0f172a;border:1px solid #1e293b;border-radius:10px;padding:16px 18px'><div style='display:flex;align-items:center;gap:10px;margin-bottom:12px'><div style='width:28px;height:28px;border-radius:50%;background:#ec4899;display:flex;align-items:center;justify-content:center;font-size:13px;color:#fff;font-weight:700;flex-shrink:0'>5</div><span style='color:#e2e8f0;font-size:14px;font-weight:600'>Inicializando datos</span></div><p style='color:#94a3b8;font-size:12px;margin:0'>Creando directorios y copiando agentes del sistema...</p></div></div>"
      }
    },
    {
      method: "shell.run",
      params: {
        venv: "venv",
        message: [
          "python -c \"import os; [os.makedirs(d, exist_ok=True) for d in ['data/agents','data/prompts/system','data/sessions','data/exports','data/brands','data/campaigns','data/audit','data/images']]; print('DIRS_OK')\"",
          "python -c \"import os, shutil; src='defaults/agents.json'; dst='data/agents/agents.json'; shutil.copy2(src, dst) if not os.path.exists(dst) else None; print('AGENTS_OK')\"",
          "python -c \"import os, shutil, glob; [shutil.copy2(f, 'data/prompts/system/') for f in glob.glob('defaults/prompts/*.md') if not os.path.exists('data/prompts/system/' + os.path.basename(f))]; print('PROMPTS_OK')\"",
          "python -c \"import os, shutil, glob; os.makedirs('data/prompts/skills', exist_ok=True); [shutil.copy2(f, 'data/prompts/skills/') for f in glob.glob('defaults/prompts/skills/*.md') if not os.path.exists('data/prompts/skills/' + os.path.basename(f))]; print('SKILLS_OK')\""
        ]
      }
    },
    {
      method: "fs.write",
      params: {
        path: "data/config.json",
        text: "{{JSON.stringify({version:'1.0.0',installedAt:new Date().toISOString(),platform:platform,ram:ram,default_model:'llama3.1:8b'},null,2)}}"
      }
    },

    // ── Completado ────────────────────────────────────────────────────────────
    {
      method: "notify",
      params: {
        html: "Instalación completada. Iniciando SmartRedes..."
      }
    },

    // One-click: lanzar la app automáticamente al terminar de instalar
    {
      method: "script.start",
      params: {
        uri: "start.js"
      }
    }
  ]
}
