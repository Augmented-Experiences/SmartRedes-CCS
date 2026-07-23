/**
 * CCS Brand Assistant — Gepeto start script (daemon)
 */
module.exports = {
  daemon: true,
  run: [
    {
      method: "log",
      params: {
        html: "<div style='font-family:-apple-system,BlinkMacSystemFont,Segoe UI,Roboto,sans-serif;padding:20px 20px 0'><h3 style='margin:0;font-size:16px;color:#e2e8f0'>Iniciando CCS Brand Assistant</h3><p style='margin:4px 0 0;color:#94a3b8;font-size:12px'>Preparando servicios...</p></div>"
      }
    },

    // Asegurar que Ollama esté disponible (opcional)
    {
      when: "{{platform !== 'win32'}}",
      method: "shell.run",
      params: {
        message: "curl -s http://127.0.0.1:11434/api/tags > /dev/null 2>&1 && echo OLLAMA_ALREADY_RUNNING || (ollama serve > /dev/null 2>&1 & sleep 3 && echo OLLAMA_STARTED)"
      }
    },
    {
      when: "{{platform === 'win32'}}",
      method: "shell.run",
      params: {
        message: "curl -s http://127.0.0.1:11434/api/tags > nul 2>&1 && echo OLLAMA_ALREADY_RUNNING || (start /B ollama serve && ping -n 4 127.0.0.1 > nul && echo OLLAMA_STARTED)"
      }
    },

    // Lanzar servidor FastAPI
    {
      method: "shell.run",
      params: {
        venv: "venv",
        env: {
          PORT: "{{port}}",
          PYTHONIOENCODING: "utf-8",
          PYTHONUTF8: "1"
        },
        message: "python server/app.py",
        on: [{
          event: "/(http:\\/\\/[0-9.:]+)/",
          done: true
        }]
      }
    },

    // Guardar URL para el menú "Abrir UI"
    {
      method: "local.set",
      params: {
        url: "{{input.event[0]}}/ui/index.html"
      }
    },

    {
      method: "log",
      params: {
        html: "<div style='font-family:-apple-system,BlinkMacSystemFont,Segoe UI,Roboto,sans-serif;padding:12px 20px'><div style='background:#0f2d1a;border:1px solid #16a34a44;border-radius:10px;padding:16px;text-align:center'><span style='color:#22c55e;font-size:15px;font-weight:600'>&#10003; Aplicaci&oacute;n lista</span></div></div>"
      }
    },

    {
      method: "browser.open",
      params: {
        uri: "{{local.url}}"
      }
    }
  ]
}
