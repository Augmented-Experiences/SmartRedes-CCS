/**
 * CCS Brand Assistant — Pinokio launcher (Gepeto v5.0)
 *
 * Menú dinámico según estado del plugin:
 *   - No instalado: Instalar
 *   - Instalado y corriendo: Abrir UI + Terminal + Detener
 *   - Instalado y detenido: Iniciar + Actualizar + Desinstalar
 */
module.exports = {
  version: "5.0",
  title: "CCS Brand Assistant",
  description: "Plataforma de ADN de marca y campañas digitales con IA local para PYMEs — Cámara de Comercio de Santiago",
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

    if (running.install) {
      return [{
        default: true,
        icon: "fa-solid fa-plug",
        text: "Instalando...",
        href: "install.js",
      }]
    }

    if (installed) {
      if (running.start) {
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
          text: "Terminal",
          href: "start.js",
        }, {
          icon: "fa-solid fa-stop",
          text: "Detener",
          href: "stop.js",
        }]
      }

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
        confirm: "¿Desinstalar CCS Brand Assistant? Los datos en data/ se conservarán.",
      }]
    }

    return [{
      default: true,
      icon: "fa-solid fa-download",
      text: "Instalar",
      href: "install.js",
    }]
  },
}
