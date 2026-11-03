"""SSRF, path traversal y sanitizado de logs de la remediación de octubre 2026."""
import logging
import os
import sys
from pathlib import Path
from unittest.mock import MagicMock, patch

import pytest

sys.path.insert(0, str(Path(__file__).parent.parent / "server"))


def _public_dns():
    return [(2, 1, 6, "", ("93.184.216.34", 0))]


class TestSsrfGuard:
    def setup_method(self):
        os.environ.pop("SMARTREDES_ALLOW_PRIVATE_URLS", None)

    def test_public_url_passes(self):
        from ssrf_guard import assert_public_http_url
        with patch("socket.getaddrinfo", return_value=_public_dns()):
            assert assert_public_http_url("https://example.com/marca") == "https://example.com/marca"

    def test_localhost_rejected(self):
        from ssrf_guard import SsrfBlocked, assert_public_http_url
        with pytest.raises(SsrfBlocked):
            assert_public_http_url("http://localhost/admin")

    def test_loopback_ip_rejected(self):
        from ssrf_guard import SsrfBlocked, assert_public_http_url
        with pytest.raises(SsrfBlocked):
            assert_public_http_url("http://127.0.0.1/")

    def test_rfc1918_rejected(self):
        from ssrf_guard import SsrfBlocked, assert_public_http_url
        with pytest.raises(SsrfBlocked):
            assert_public_http_url("http://192.168.1.20/panel")
        with pytest.raises(SsrfBlocked):
            assert_public_http_url("http://10.1.2.3/panel")

    def test_metadata_rejected(self):
        from ssrf_guard import SsrfBlocked, assert_public_http_url
        with pytest.raises(SsrfBlocked):
            assert_public_http_url("http://169.254.169.254/latest/meta-data")

    def test_file_scheme_rejected(self):
        from ssrf_guard import SsrfBlocked, assert_public_http_url
        with pytest.raises(SsrfBlocked):
            assert_public_http_url("file:///etc/passwd")

    def test_redirect_to_private_ip_rejected(self):
        from ssrf_guard import SsrfBlocked, safe_fetch

        first = MagicMock()
        first.status_code = 302
        first.headers = {"Location": "http://127.0.0.1/secret"}
        first.is_redirect = True
        first.is_permanent_redirect = False
        first.close = MagicMock()

        session = MagicMock()
        session.request.return_value = first
        session.close = MagicMock()
        with patch("socket.getaddrinfo", return_value=_public_dns()):
            with patch("ssrf_guard.requests.Session", return_value=session):
                with pytest.raises(SsrfBlocked):
                    safe_fetch("https://example.com/start", allowed_types={"text/html"})

    def test_private_allowed_only_with_env(self):
        from ssrf_guard import assert_public_http_url
        os.environ["SMARTREDES_ALLOW_PRIVATE_URLS"] = "1"
        try:
            assert assert_public_http_url("http://127.0.0.1/ollama").startswith("http://127.0.0.1")
        finally:
            os.environ.pop("SMARTREDES_ALLOW_PRIVATE_URLS", None)


class TestImagePathConfinement:
    def test_public_name_stays_inside(self, tmp_path):
        from path_safety import confined_file
        path = confined_file(tmp_path, "pub_1.png", image=True)
        assert path.parent == tmp_path.resolve()
        assert path.name == "pub_1.png"

    def test_dotdot_percent_rejected(self, tmp_path):
        from path_safety import PathRejected, confined_file
        with pytest.raises(PathRejected):
            confined_file(tmp_path, "..%2fetc/passwd", image=True)
        with pytest.raises(PathRejected):
            confined_file(tmp_path, "..%2fetc%2fpasswd.png", image=True)

    def test_absolute_and_separators_rejected(self, tmp_path):
        from path_safety import PathRejected, confined_file
        with pytest.raises(PathRejected):
            confined_file(tmp_path, "/etc/passwd.png", image=True)
        with pytest.raises(PathRejected):
            confined_file(tmp_path, "carpeta/foto.png", image=True)
        with pytest.raises(PathRejected):
            confined_file(tmp_path, r"C:\Windows\foto.png", image=True)

    def test_symlink_escape_is_rejected_when_the_link_points_outside(self, tmp_path, monkeypatch):
        from path_safety import PathRejected, confined_file
        outside = tmp_path.parent / ("fuera-" + tmp_path.name)
        outside.mkdir(exist_ok=True)
        secret = outside / "secreto.png"
        secret.write_bytes(b"no")
        real_is_symlink = Path.is_symlink
        real_resolve = Path.resolve

        def is_symlink(self):
            if self.name == "foto.png":
                return True
            return real_is_symlink(self)

        def resolve(self, strict=False):
            if self.name == "foto.png":
                return secret
            return real_resolve(self, strict)

        monkeypatch.setattr(Path, "is_symlink", is_symlink)
        monkeypatch.setattr(Path, "resolve", resolve)
        with pytest.raises(PathRejected):
            confined_file(tmp_path, "foto.png", image=True)


class TestLogSanitizer:
    def test_strips_crlf(self):
        from log_safety import LogSanitizerFilter
        record = logging.LogRecord("t", logging.INFO, __file__, 1, "linea\r\ninyectada", (), None)
        assert LogSanitizerFilter().filter(record) is True
        assert "\n" not in record.msg
        assert "\r" not in record.msg


class TestLoopBounds:
    """CWE-606: el id no entra en un glob y el recorrido de disco tiene tope."""

    def _write_campaign(self, root: Path, brand_id: str, camp_id: str):
        folder = root / "campaigns" / f"{brand_id}_{camp_id}"
        folder.mkdir(parents=True)
        (folder / "campaign.json").write_text(
            '{"brand_id": "%s", "id": "%s", "created_at": "2026-01-01"}' % (brand_id, camp_id),
            encoding="utf-8",
        )

    def test_brand_id_glob_does_not_list_other_brands(self, tmp_path, monkeypatch):
        import app
        from fastapi import HTTPException
        monkeypatch.setattr(app, "DATA_DIR", tmp_path)
        self._write_campaign(tmp_path, "brand1", "campA")
        self._write_campaign(tmp_path, "brand2", "campB")
        listed = app.list_campaigns("brand1")
        assert [c["id"] for c in listed["campaigns"]] == ["campA"]
        for bad in ("*", "brand*", "..", "a/b", "brand1/../brand2", ""):
            with pytest.raises(HTTPException) as exc:
                app.list_campaigns(bad)
            assert exc.value.status_code == 400

    def test_stats_and_images_stop_at_the_cap(self, tmp_path, monkeypatch):
        import app
        monkeypatch.setattr(app, "DATA_DIR", tmp_path)
        monkeypatch.setattr(app, "_MAX_DIR_ENTRIES", 2)
        for i in range(4):
            self._write_campaign(tmp_path, "brand1", f"camp{i}")
        stats = app.get_stats()
        assert stats["campaigns"] == 2

        img_dir = tmp_path / "exports" / "images"
        img_dir.mkdir(parents=True)
        for i in range(4):
            (img_dir / f"foto{i}.png").write_bytes(b"\x89PNG")
        listed = app.list_generated_images()
        assert listed["count"] == 2

    def test_comfy_prompt_id_is_not_placed_in_the_url(self, monkeypatch):
        import app
        history_urls = []

        def get(url, **kwargs):
            if str(url).endswith("/system_stats"):
                ok = MagicMock()
                ok.status_code = 200
                return ok
            history_urls.append(str(url))
            missing = MagicMock()
            missing.status_code = 404
            return missing

        def post(url, **kwargs):
            queued = MagicMock()
            queued.raise_for_status = MagicMock()
            queued.json = MagicMock(return_value={"prompt_id": "../../admin"})
            return queued

        monkeypatch.setattr(app.requests, "get", get)
        monkeypatch.setattr(app.requests, "post", post)
        assert app._try_comfyui("un prompt", {"comfyui_url": "http://127.0.0.1:8188", "timeout": 1}) is None
        assert history_urls == []
