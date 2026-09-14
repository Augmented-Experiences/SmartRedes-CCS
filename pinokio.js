/**
 * SmartRedes — Pinokio launcher (Gepeto v5.0 + one-click autostart)
 *
 * Flujo one-click (Pinokio 2.0+):
 *   1. Usuario abre la app → se ejecuta automáticamente el script con default: true
 *   2. Si no está instalado → install.js (instala todo y lanza start.js al final)
 *   3. Si está instalado → start.js (abre la UI automáticamente)
 *   4. Si ya corre → botón "Abrir UI" apunta directo a la interfaz
 */
module.exports = {
  version: "5.0",
  title: "SmartRedes",
  description: "Asistente de marca y redes con IA local para PYMEs — CCCE",
  icon: "icon.png",

  menu: async (kernel, info) => {
    let installed = info.exists("venv")
    let running = {
      install: info.running("install.js"),
      start: info.running("start.js"),
      stop: info.running("stop.js"),
      update: info.running("update.js"),
      reset: info.running("reset.js"),
      link: info.running("link.js"),
    }

    // ── Instalando ──────────────────────────────────────────────────────────
    if (running.install) {
      return [{
        default: true,
        icon: "fa-solid fa-plug",
        text: "Instalando...",
        href: "install.js",
      }]
    }

    // ── Corriendo ───────────────────────────────────────────────────────────
    if (installed && running.start) {
      let local = info.local("start.js")
      if (local && local.url) {
        return [{
          default: true,
          icon: "fa-solid fa-arrow-up-right-from-square",
          text: "Abrir UI",
          href: local.url,
        }, {
          icon: "fa-solid fa-terminal",
          text: "Terminal",
          href: "start.js",
        }, {
          icon: "fa-solid fa-stop",
          text: "Detener",
          href: "stop.js",
        }]
      }
      return [{
        default: true,
        icon: "fa-solid fa-terminal",
        text: "Iniciando...",
        href: "start.js",
      }, {
        icon: "fa-solid fa-stop",
        text: "Detener",
        href: "stop.js",
      }]
    }

    // ── Operaciones en curso ────────────────────────────────────────────────
    if (running.stop) {
      return [{
        default: true,
        icon: "fa-solid fa-stop",
        text: "Deteniendo...",
        href: "stop.js",
      }]
    }
    if (running.update) {
      return [{
        default: true,
        icon: "fa-solid fa-arrows-rotate",
        text: "Actualizando...",
        href: "update.js",
      }]
    }
    if (running.reset) {
      return [{
        default: true,
        icon: "fa-regular fa-circle-xmark",
        text: "Desinstalando...",
        href: "reset.js",
      }]
    }
    if (running.link) {
      return [{
        default: true,
        icon: "fa-solid fa-file-zipper",
        text: "Optimizando espacio...",
        href: "link.js",
      }]
    }

    // ── Instalado, detenido → autostart start.js ────────────────────────────
    if (installed) {
      return [{
        default: true,
        icon: "fa-solid fa-play",
        text: "Iniciar",
        href: "start.js",
      }, {
        icon: "fa-solid fa-arrows-rotate",
        text: "Actualizar",
        href: "update.js",
      }, {
        icon: "fa-solid fa-plug",
        text: "Reinstalar",
        href: "install.js",
      }, {
        icon: "fa-solid fa-file-zipper",
        text: "<div><strong>Ahorrar espacio</strong><div>Deduplica librerías redundantes</div></div>",
        href: "link.js",
      }, {
        icon: "fa-regular fa-circle-xmark",
        text: "<div><strong>Desinstalar</strong><div>Elimina el entorno virtual (conserva data/)</div></div>",
        href: "reset.js",
        confirm: "¿Desinstalar SmartRedes? Los datos en data/ se conservarán.",
      }]
    }

    // ── No instalado → autostart install.js (one-click) ─────────────────────
    return [{
      default: true,
      icon: "fa-solid fa-download",
      text: "Instalar",
      href: "install.js",
    }]
  },
}
