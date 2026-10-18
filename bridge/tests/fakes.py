"""Attrappen: Garmin-Client ohne Netzwerk und eine Uhr ohne Wartezeit.

Alle Antworten sind synthetisch. Keine echten Werte, keine echten Feldlisten.
"""

from __future__ import annotations

from collections.abc import Callable
from datetime import tzinfo
from enum import Enum
from pathlib import Path
from typing import Any

import pytest


def symlink_or_skip(link: Path, target: Path) -> None:
    """Legt einen Symlink an oder überspringt den Test, wenn das System es nicht erlaubt.

    Windows verlangt dafür Adminrechte oder den Entwicklermodus (WinError 1314). Die CI testet
    bridge/ unter Ubuntu, dort laufen diese Tests immer; lokal unter Windows blockierte der
    Fehler sonst jeden Lauf von scripts/check-all.sh (T46).
    """
    try:
        link.symlink_to(target)
    except OSError as err:
        pytest.skip(f"Symlinks hier nicht erlaubt: {err.__class__.__name__}")


class FakeClock:
    """Zeit und Schlaf ohne zu warten. ``sleep`` rückt die Uhr vor."""

    def __init__(self, start: float = 1_000_000.0, tz: tzinfo | None = None) -> None:
        self.t = start
        self.sleeps: list[float] = []
        # Zeitzone für den lokalen Kalendertag (``local_today``). None: Zeitzone des Systems.
        self.tz = tz

    def now(self) -> float:
        return self.t

    def sleep(self, seconds: float) -> None:
        self.sleeps.append(seconds)
        self.t += max(seconds, 0.0)

    def advance(self, seconds: float) -> None:
        self.t += seconds


class FakeGarmin:
    """Liefert für jeden Methodennamen eine aufzeichnende Funktion.

    ``responses`` bildet Methodennamen auf einen Wert, eine Ausnahme (wird
    geworfen) oder eine Funktion ``f(*args, **kwargs)`` ab. Unbekannte Methoden
    liefern ``default``.
    """

    class ActivityDownloadFormat(Enum):
        ORIGINAL = 1
        TCX = 2
        GPX = 3
        KML = 4
        CSV = 5

    def __init__(
        self,
        responses: dict[str, Any] | None = None,
        default: Any = None,
    ) -> None:
        self.responses: dict[str, Any] = dict(responses or {})
        self.default = default
        self.calls: list[tuple[str, tuple[Any, ...], dict[str, Any]]] = []

    def set(self, name: str, handler: Any) -> None:
        self.responses[name] = handler

    def names_called(self) -> list[str]:
        return [name for name, _args, _kwargs in self.calls]

    def __getattr__(self, name: str) -> Callable[..., Any]:
        if name.startswith("_"):
            raise AttributeError(name)

        def method(*args: Any, **kwargs: Any) -> Any:
            self.calls.append((name, args, kwargs))
            handler = self.responses.get(name, self.default)
            if isinstance(handler, BaseException):
                raise handler
            if callable(handler):
                return handler(*args, **kwargs)
            return handler

        return method
