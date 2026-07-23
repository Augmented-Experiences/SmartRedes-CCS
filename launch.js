/**
 * Punto de entrada one-click: instala si hace falta y lanza la app.
 * Equivalente a abrir la app en Pinokio (el menú autostart hace lo mismo).
 */
module.exports = {
  run: [
    {
      when: "{{!kernel.exists(cwd, 'venv')}}",
      method: "script.start",
      params: {
        uri: "install.js"
      }
    },
    {
      when: "{{kernel.exists(cwd, 'venv')}}",
      method: "script.start",
      params: {
        uri: "start.js"
      }
    }
  ]
}
