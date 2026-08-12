"""Pruebas de regresión del cambio de nombre a SmartRedes.

Estas pruebas validan que las superficies visibles y técnicas del plugin usan
SmartRedes, sin modificar los activos ni la paleta corporativa de la CCS.
"""
from __future__ import annotations

import hashlib
import importlib
import json
import re
import sys
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[1]
APP_DIR = ROOT / "app"
SERVER_DIR = ROOT / "server"

# El repositorio conserva temporalmente esta URL histórica hasta que se cambie
# su nombre remoto. No es un nombre mostrado de producto dentro del plugin.
HISTORICAL_REPOSITORY_URL = "https://github.com/vtomasv/ccs-brand-assistant"

# Se construyen en tiempo de ejecución para que la propia prueba no introduzca
# coincidencias literales que busca en los archivos versionados.
LEGACY_PRODUCT_TERMS = (
    "CCS " + "Brand " + "Assistant",
    "CSS " + "Brand " + "Assistant",
    "Brand " + "Assistant",
)

CORPORATE_ASSET_HASHES = {
    "icon.png": "d36c5ae2750ad9e7961692b2e1850a1f2326bc2dcd5f1af1b4fbc9845871d4de",
    "app/logo-ccs.svg": "a5f41aebfe00751cd38351ba69f943a90960255effb9944885cd7d207e38898f",
}

CORPORATE_CSS_TOKENS = {
    "--ccs-azul-oscuro": "#0D3DA6",
    "--ccs-verde": "#3DAE2B",
    "--ccs-azul": "#0D56CA",
    "--ccs-celeste": "#3A6DDE",
    "--ccs-verde-brillante": "#0DD700",
}

TEXT_SUFFIXES = {
    ".css",
    ".html",
    ".js",
    ".json",
    ".md",
    ".ps1",
    ".py",
    ".sh",
    ".txt",
}


def _tracked_text_files() -> list[Path]:
    """Devuelve archivos de texto versionados, excluyendo artefactos de Git."""
    return [
        path
        for path in ROOT.rglob("*")
        if path.is_file()
        and ".git" not in path.parts
        and "__pycache__" not in path.parts
        and path.suffix.lower() in TEXT_SUFFIXES
    ]


def _read_without_historical_url(path: Path) -> str:
    """Elimina la única URL técnica permitida antes de revisar nombres visibles."""
    return path.read_text(encoding="utf-8").replace(HISTORICAL_REPOSITORY_URL, "")


def _sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


class TestSmartRedesPresentation:
    """Comprueba las superficies de nombre que ve o consume el usuario."""

    def test_pinokio_menu_uses_smartredes(self):
        content = (ROOT / "pinokio.js").read_text(encoding="utf-8")
        assert 'title: "SmartRedes"' in content
        assert "SmartRedes" in content

    @pytest.mark.parametrize("filename", ["install.json", "start.json", "stop.json", "reset.json"])
    def test_lifecycle_scripts_are_valid_and_named_smartredes(self, filename: str):
        path = ROOT / filename
        parsed = json.loads(path.read_text(encoding="utf-8"))
        assert isinstance(parsed["run"], list)
        assert "SmartRedes" in path.read_text(encoding="utf-8")

    def test_frontend_title_and_sidebar_identify_smartredes(self):
        content = (APP_DIR / "index.html").read_text(encoding="utf-8")
        assert "<title>SmartRedes — Cámara de Comercio de Santiago</title>" in content
        assert '<div class="sidebar-logo-text">SmartRedes</div>' in content
        assert "smartredes_export.json" in content

    def test_backend_metadata_uses_smartredes(self):
        if str(SERVER_DIR) not in sys.path:
            sys.path.insert(0, str(SERVER_DIR))
        app_module = importlib.import_module("app")
        assert app_module.app.title == "SmartRedes"
        assert app_module.app.openapi()["info"]["title"] == "SmartRedes"
        source = (SERVER_DIR / "app.py").read_text(encoding="utf-8")
        assert '"plugin_name": "smartredes"' in source
        assert "smartredes_export_" in source

    def test_no_legacy_product_name_remains_in_versioned_text(self):
        legacy_matches: list[str] = []
        for path in _tracked_text_files():
            content = _read_without_historical_url(path)
            for term in LEGACY_PRODUCT_TERMS:
                if term in content:
                    legacy_matches.append(f"{path.relative_to(ROOT)}: {term}")
        assert not legacy_matches, "Referencias heredadas encontradas:\n" + "\n".join(legacy_matches)


class TestCorporateIdentityPreservation:
    """Asegura que el cambio de nombre no altera la identidad visual de la CCS."""

    def test_corporate_assets_keep_their_verified_hashes(self):
        for relative_path, expected_hash in CORPORATE_ASSET_HASHES.items():
            path = ROOT / relative_path
            assert path.is_file(), f"Falta el activo corporativo {relative_path}"
            assert _sha256(path) == expected_hash, f"El activo corporativo cambió: {relative_path}"

    def test_corporate_palette_and_logo_reference_are_preserved(self):
        content = (APP_DIR / "index.html").read_text(encoding="utf-8")
        for token, color in CORPORATE_CSS_TOKENS.items():
            assert re.search(rf"{re.escape(token)}:\s*{re.escape(color)};", content)
        assert 'src="logo-ccs.svg"' in content
        assert 'alt="Cámara de Comercio de Santiago"' in content

    def test_install_and_start_keep_the_existing_visual_style(self):
        install = (ROOT / "install.json").read_text(encoding="utf-8")
        start = (ROOT / "start.json").read_text(encoding="utf-8")
        for content in (install, start):
            assert "linear-gradient(135deg,#3DAE2B,#2d8a1f)" in content
            assert ">S</div>" in content


class TestSmartRedesStorageMigration:
    """Verifica el identificador actual y la compatibilidad con instalaciones previas."""

    def test_new_environment_variable_has_priority_and_legacy_fallback(self):
        source = (SERVER_DIR / "app.py").read_text(encoding="utf-8")
        new_variable = 'os.environ.get("SMARTREDES_DATA_DIR")'
        legacy_variable = 'os.environ.get("CCS_DATA_DIR")'
        assert new_variable in source
        assert legacy_variable in source
        assert source.index(new_variable) < source.index(legacy_variable)

    def test_tests_are_configured_with_the_new_environment_variable(self):
        test_sources = "\n".join(
            path.read_text(encoding="utf-8")
            for path in (ROOT / "tests").glob("test_*.py")
        )
        assert "SMARTREDES_DATA_DIR" in test_sources

    def test_platform_validation_scripts_follow_current_pinokio_pattern(self):
        for filename in ("test_mac.sh", "test_windows.ps1"):
            content = (ROOT / "tests" / filename).read_text(encoding="utf-8")
            assert "SmartRedes" in content
            assert "local.set" in content
            assert "local.url" in content
            if filename == "test_mac.sh":
                assert "kernel.memory.local" in content
                assert "! grep -q 'self.session.url'" in content
            else:
                assert "kernel\\.memory\\.local" in content
                assert '-not ($content -match "self\\.session\\.url")' in content
