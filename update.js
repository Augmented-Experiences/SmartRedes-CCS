/**
 * Actualiza el repositorio y refresca dependencias Python.
 */
module.exports = {
  run: [
    {
      method: "log",
      params: {
        html: "<div style='font-family:-apple-system,BlinkMacSystemFont,Segoe UI,Roboto,sans-serif;padding:16px 20px'><span style='color:#e2e8f0;font-size:14px;font-weight:600'>Actualizando SmartRedes...</span></div>"
      }
    },
    {
      method: "shell.run",
      params: {
        message: "git pull"
      }
    },
    {
      method: "shell.run",
      params: {
        venv: "venv",
        message: [
          "pip install --upgrade pip",
          "pip install -r requirements-core.txt",
          "pip install playwright diffusers transformers accelerate safetensors"
        ]
      }
    },
    {
      method: "notify",
      params: {
        html: "SmartRedes actualizado correctamente."
      }
    }
  ]
}
