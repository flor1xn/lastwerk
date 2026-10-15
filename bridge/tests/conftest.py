"""Gemeinsame Fixtures: Netzwerk gesperrt, Attrappen-Client, Fake-Uhr (SPEC 11.4)."""

from __future__ import annotations

import socket
from pathlib import Path

import pytest

from .fakes import FakeClock, FakeGarmin


class NetworkBlocked(RuntimeError):
    """Ein Test hat versucht, eine Netzwerkverbindung zu öffnen."""


@pytest.fixture(autouse=True)
def block_network(monkeypatch: pytest.MonkeyPatch) -> None:
    """Sperrt ``socket`` in jedem Test. Jede echte Verbindung wird zur Ausnahme."""

    def refuse(*_args: object, **_kwargs: object) -> None:
        raise NetworkBlocked("Netzwerkzugriff im Test gesperrt")

    monkeypatch.setattr(socket, "socket", refuse)
    monkeypatch.setattr(socket, "create_connection", refuse)
    monkeypatch.setattr(socket, "getaddrinfo", refuse)


@pytest.fixture
def clock() -> FakeClock:
    # Fester Startzeitpunkt: 2026-10-13T12:00:00Z.
    return FakeClock(start=1792065600.0)


@pytest.fixture
def fake_client() -> FakeGarmin:
    return FakeGarmin()


@pytest.fixture
def root(tmp_path: Path) -> Path:
    return tmp_path / "archive" / "garmin"


@pytest.fixture
def token_dir(tmp_path: Path) -> Path:
    """Token-Verzeichnis mit einer Platzhalterdatei (kein echtes Token)."""
    directory = tmp_path / "tokens"
    directory.mkdir()
    (directory / "garmin_tokens.json").write_text("{}", encoding="utf-8")
    return directory
