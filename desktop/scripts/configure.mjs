#!/usr/bin/env node
// ============================================================
// configure.mjs — Genera los archivos variables del launcher Tauri
// a partir de desktop/smartsuite.config.json:
//   - src-tauri/tauri.conf.json   (nombre, id, iconos, ventana, MSI/NSIS)
//   - src-tauri/appconfig.json    (productName, dataDirName, modelos Ollama)
//   - src-tauri/Cargo.toml        (nombre del paquete/binario Rust)
//   - package.json                (nombre npm del proyecto desktop)
//   - src-tauri/capabilities/default.json
//   - ui/index.html               (pantalla de carga / splash)
//   - ui/ccce-theme.css + ui/accent.css
//
// Se ejecuta automáticamente antes de 'npm run build' / 'npm run dev'.
// ============================================================
import { readFileSync, writeFileSync, copyFileSync, mkdirSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const HERE = dirname(fileURLToPath(import.meta.url));
const DESKTOP = resolve(HERE, "..");
const cfgPath = resolve(DESKTOP, "smartsuite.config.json");
const cfg = JSON.parse(readFileSync(cfgPath, "utf8"));

function req(name) {
  if (!cfg[name]) throw new Error(`smartsuite.config.json: falta '${name}'`);
  return cfg[name];
}

function cargoPackageName() {
  if (cfg.cargoPackageName) return String(cfg.cargoPackageName).toLowerCase();
  const id = String(cfg.identifier || "");
  const last = id.split(".").pop() || req("productName");
  return last.toLowerCase().replace(/[^a-z0-9_]/g, "") || "smartsuite_app";
}

function brandHtml(productName) {
  const name = String(productName);
  if (/^Smart[A-Za-zÁÉÍÓÚÑáéíóúñ]/.test(name) && name.length > 5) {
    const rest = name.slice(5);
    return `Smart<span class="accent">${rest}</span>`;
  }
  return name.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}

function splashSubtitle() {
  if (cfg.splashSubtitle) return String(cfg.splashSubtitle);
  if (cfg.shortDescription) return `Preparando ${cfg.shortDescription.toLowerCase()}…`;
  return `Preparando ${req("productName")}…`;
}

const productName = req("productName");
const version = cfg.version || "1.0.0";
const cargoName = cargoPackageName();
const accent = cfg.accent || "#F4C10E";

// --- 1) tauri.conf.json ---
const tauriConf = {
  $schema: "https://schema.tauri.app/config/2",
  productName,
  version,
  identifier: req("identifier"),
  build: { frontendDist: "../ui" },
  app: {
    withGlobalTauri: true,
    windows: [
      {
        label: "main",
        title: (cfg.window && cfg.window.title) || productName,
        width: (cfg.window && cfg.window.width) || 1200,
        height: (cfg.window && cfg.window.height) || 800,
        minWidth: (cfg.window && cfg.window.minWidth) || 900,
        minHeight: (cfg.window && cfg.window.minHeight) || 600,
        resizable: true,
        center: true,
      },
    ],
    security: { csp: null },
  },
  bundle: {
    active: true,
    targets: "all",
    icon: [
      "icons/32x32.png",
      "icons/128x128.png",
      "icons/128x128@2x.png",
      "icons/icon.icns",
      "icons/icon.ico",
    ],
    externalBin: ["binaries/backend"],
    category: cfg.bundleCategory || "Productivity",
    shortDescription: cfg.shortDescription || productName,
    longDescription: cfg.longDescription || productName,
  },
  plugins: {},
};
writeFileSync(
  resolve(DESKTOP, "src-tauri/tauri.conf.json"),
  JSON.stringify(tauriConf, null, 2) + "\n"
);

// --- 2) appconfig.json (lo lee Rust vía include_str!) ---
const tiers = ((cfg.ollama && cfg.ollama.tiers) || [{ maxRamGb: 0, model: "llama3.2:3b" }]).map(
  (t) => ({ maxRamGb: Number(t.maxRamGb) || 0, model: String(t.model) })
);
const extraModels = ((cfg.ollama && cfg.ollama.extraModels) || []).map(String);
const appConfig = {
  productName,
  dataDirName: req("dataDirName"),
  ollamaTiers: tiers,
  extraModels,
};
writeFileSync(
  resolve(DESKTOP, "src-tauri/appconfig.json"),
  JSON.stringify(appConfig, null, 2) + "\n"
);

// --- 3) Cargo.toml (nombre del .exe y metadatos del crate) ---
const cargoDescription =
  cfg.cargoDescription || cfg.shortDescription || `${productName} — CCCE SmartSuite`;
const cargoToml = `[package]
name = "${cargoName}"
version = "${version}"
description = "${cargoDescription.replace(/"/g, '\\"')}"
authors = ["Cámara Colombiana de Comercio Electrónico (CCCE)"]
edition = "2021"
rust-version = "1.77"

[build-dependencies]
tauri-build = { version = "2", features = [] }

[dependencies]
tauri = { version = "2", features = [] }
tauri-plugin-shell = "2"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
sysinfo = "0.33"
ureq = { version = "2", default-features = false }

[[bin]]
name = "${cargoName}"
path = "src/main.rs"

[profile.release]
panic = "abort"
codegen-units = 1
lto = true
opt-level = "s"
strip = true
`;
writeFileSync(resolve(DESKTOP, "src-tauri/Cargo.toml"), cargoToml);

// --- 4) package.json (proyecto npm desktop) ---
const pkgPath = resolve(DESKTOP, "package.json");
const pkg = JSON.parse(readFileSync(pkgPath, "utf8"));
pkg.name = `${cargoName}-desktop`;
pkg.version = version;
pkg.description = `Instalador/app de escritorio nativo de ${productName} (Tauri) — CCCE`;
writeFileSync(pkgPath, JSON.stringify(pkg, null, 2) + "\n");

// --- 5) capabilities ---
const capabilities = {
  $schema: "../gen/schemas/desktop-schema.json",
  identifier: "default",
  description: `Permisos base para la ventana principal de ${productName}.`,
  windows: ["main"],
  permissions: ["core:default"],
};
writeFileSync(
  resolve(DESKTOP, "src-tauri/capabilities/default.json"),
  JSON.stringify(capabilities, null, 2) + "\n"
);

// --- 6) Pantalla de carga (splash) ---
mkdirSync(resolve(DESKTOP, "ui"), { recursive: true });
const splashTpl = readFileSync(resolve(DESKTOP, "ui/splash.template.html"), "utf8");
const splashHtml = splashTpl
  .replaceAll("__PRODUCT_NAME__", productName.replace(/&/g, "&amp;").replace(/</g, "&lt;"))
  .replaceAll("__BRAND_HTML__", brandHtml(productName))
  .replaceAll("__SPLASH_SUBTITLE__", splashSubtitle().replace(/&/g, "&amp;").replace(/</g, "&lt;"));
writeFileSync(resolve(DESKTOP, "ui/index.html"), splashHtml);

// --- 7) Marca CCCE + acento por-herramienta ---
copyFileSync(resolve(DESKTOP, "brand/ccce-theme.css"), resolve(DESKTOP, "ui/ccce-theme.css"));
writeFileSync(
  resolve(DESKTOP, "ui/accent.css"),
  `/* Generado por configure.mjs — acento por-herramienta */\n:root { --ccce-accent: ${accent}; }\n`
);

console.log(
  `configure.mjs: '${productName}' configurado (cargo ${cargoName}, accent ${accent}, dataDir ${cfg.dataDirName}).`
);
