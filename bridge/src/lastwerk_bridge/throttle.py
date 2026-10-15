"""Gedrosselter Aufrufer (SPEC 5.5, Abrufregeln 4, 7 und 8; SPEC 13 Gegenmaßnahmen).

Der einzige Weg zum Netzwerk. Er

- lehnt jede Methode ab, die nicht in der Allowlist steht,
- hält einen Mindestabstand mit Zufallsanteil zwischen Aufrufen,
- begrenzt die Dauer je Aufruf (hängende Aufrufe werden abgebrochen),
- wiederholt Netzfehler begrenzt mit Wartezeit,
- stoppt nach drei Antworten 429 oder 403 in Folge, schreibt den Zustand nach
  ``<state_dir>/lockout.json`` und sperrt folgende Läufe für eine Frist,
- erlaubt keinen Parallelabruf (ein Lock je Aufrufer).

Alle Zahlen sind vorläufige Faustregeln bis zur Messung in L1 (SPEC 14, O24).
Zeit und Schlaf sind über ein Clock-Objekt injizierbar, damit Tests ohne Wartezeit
laufen. Das Zeitlimit je Aufruf läuft über einen Hilfs-Thread in Echtzeit.
"""

from __future__ import annotations

import json
import os
import random
import re
import threading
import time
from dataclasses import asdict, dataclass
from datetime import UTC, date, datetime, tzinfo
from pathlib import Path
from typing import Any, Protocol

from .allowlist import ensure_allowed

LOCKOUT_FILE = "lockout.json"


class Clock(Protocol):
    def now(self) -> float:
        """Sekunden seit der Unix-Epoche."""

    def sleep(self, seconds: float) -> None: ...


def local_today(clock: Clock) -> date:
    """Der heutige Kalendertag des Nutzers (lokale Zeit), nicht der UTC-Tag.

    Plandaten, Tagesdaten und Ordnernamen sind lokale Kalendertage (SPEC 6.1); um 23:30 in
    UTC-5 ist es in UTC schon morgen. Zeitstempel in Protokollen und Manifest bleiben UTC.
    Die Zeitzone kommt aus ``clock.tz`` (nur Tests setzen sie), sonst aus dem System.
    """
    tz: tzinfo | None = getattr(clock, "tz", None)
    return datetime.fromtimestamp(clock.now(), tz=tz).date()


class SystemClock:
    def now(self) -> float:
        return time.time()

    def sleep(self, seconds: float) -> None:
        if seconds > 0:
            time.sleep(seconds)


@dataclass(frozen=True)
class ThrottleConfig:
    """Vorläufige Startwerte (Faustregel, O24). Nur bei Erfolg in L1 lockern."""

    # Mindestabstand zwischen zwei Aufrufen in Sekunden, plus Zufallsanteil.
    min_interval_s: float = 1.5
    jitter_s: float = 0.5
    # Zeitlimit je Aufruf. Die Bibliothek selbst setzt 15 s je HTTP-Anfrage, ein
    # Wrapper kann aber mehrere Anfragen bündeln, deshalb großzügiger.
    call_timeout_s: float = 30.0
    # Wiederholungen bei Netzfehlern und Zeitüberschreitung, mit Backoff.
    retries: int = 2
    backoff_s: float = 2.0
    # Pause nach einer Antwort 429 oder 403, bevor der nächste Aufruf startet.
    block_pause_s: float = 30.0
    # Nach so vielen 429 oder 403 in Folge wird der Lauf abgebrochen.
    lockout_threshold: int = 3
    # Solange bleibt die Sperre aktiv, folgende Läufe brechen sofort ab.
    lockout_hold_s: float = 6 * 3600.0


class BridgeError(Exception):
    """Basisklasse der Fehler des Zusatzprogramms."""


class LockoutError(BridgeError):
    """Abbruch wegen Sperre: drei 429 oder 403 in Folge, oder Sperre noch aktiv."""

    def __init__(self, message: str, *, until: datetime, state: dict[str, Any]) -> None:
        super().__init__(message)
        self.until = until
        self.state = state


class BlockedResponseError(BridgeError):
    """Eine einzelne Antwort 429 oder 403. Zählt für die Sperre."""

    def __init__(self, status: int, method: str, consecutive: int) -> None:
        super().__init__(f"{method}: HTTP {status} ({consecutive}. Mal in Folge)")
        self.status = status
        self.method = method
        self.consecutive = consecutive


class CallTimeoutError(BridgeError):
    """Der Aufruf hat das Zeitlimit überschritten."""


class CallBudgetExhausted(BridgeError):
    """Das Aufrufbudget des Laufs (``--max-calls``) ist verbraucht. Kein weiterer Aufruf."""


class AuthRequiredError(BridgeError):
    """Die Bibliothek meldet eine ungültige oder fehlende Anmeldung (401)."""


@dataclass
class CallStats:
    calls: int = 0
    ok: int = 0
    errors: int = 0
    blocked: int = 0
    timeouts: int = 0
    retries: int = 0

    def as_dict(self) -> dict[str, int]:
        return asdict(self)


# Nur Zahlen, die die Bibliothek als Status formuliert ("API Error 503", "HTTP 403",
# "client error (403)"). Eine bloße dreistellige Zahl reicht nicht: "port 443" in der Meldung
# eines Netzfehlers ist kein Status, und "column 404" in einem JSON-Fehler auch nicht.
_STATUS_RE = re.compile(
    r"(?:\bAPI Error|\b[Ee]rror|\bHTTP|\bstatus(?: code)?)[\s:=]*\(?([45]\d\d)\b"
)


def is_json_decode_error(exc: BaseException) -> bool:
    """Wahr bei jedem Fehler, der beim Lesen einer Antwort als JSON (oder Text) entsteht.

    ``requests`` und ``curl_cffi`` haben je eine eigene ``JSONDecodeError``-Klasse, die von
    ``json.JSONDecodeError`` und damit von ``ValueError`` erbt (``curl_cffi`` zusätzlich von
    ``OSError``). Dazu kommen ``simplejson`` und ``orjson``, die denselben Namen tragen, und
    ``UnicodeDecodeError`` beim Dekodieren des Rumpfes. Der Fehler entsteht erst, nachdem die
    Antwort da ist: bei einem schreibenden Aufruf hat der Server die Anfrage also verarbeitet.
    """
    if isinstance(exc, UnicodeDecodeError):
        return True
    return any(cls.__name__ == "JSONDecodeError" for cls in type(exc).__mro__)


def http_status_of(exc: BaseException) -> int | None:
    """Liest den HTTP-Status aus einer Ausnahme der Bibliothek.

    Reihenfolge: eigene Klasse für 429, dann ``response.status_code``, dann die
    erste dreistellige 4xx- oder 5xx-Zahl in der Meldung (die Bibliothek
    formuliert "client error (403)" oder "HTTP 403").
    """
    try:
        from garminconnect import GarminConnectTooManyRequestsError
    except ImportError:  # pragma: no cover
        GarminConnectTooManyRequestsError = ()  # type: ignore[assignment]
    if isinstance(exc, GarminConnectTooManyRequestsError):
        return 429
    response = getattr(exc, "response", None)
    code = getattr(response, "status_code", None)
    if isinstance(code, int):
        return code
    if is_json_decode_error(exc):
        # Die Meldung nennt Zeile, Spalte und Zeichen, nie einen Status.
        return None
    match = _STATUS_RE.search(str(exc))
    if match:
        return int(match.group(1))
    return None


def _iso(epoch: float) -> str:
    return datetime.fromtimestamp(epoch, tz=UTC).isoformat(timespec="seconds")


def lockout_path(state_dir: Path) -> Path:
    return Path(state_dir) / LOCKOUT_FILE


def read_lockout(state_dir: Path) -> dict[str, Any] | None:
    path = lockout_path(state_dir)
    if not path.is_file():
        return None
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return None
    return data if isinstance(data, dict) else None


def active_lockout(state_dir: Path, clock: Clock | None = None) -> dict[str, Any] | None:
    """Gibt den gespeicherten Sperrzustand zurück, solange die Frist läuft."""
    clock = clock or SystemClock()
    state = read_lockout(state_dir)
    if state is None:
        return None
    until = state.get("until_epoch")
    if isinstance(until, (int, float)) and until > clock.now():
        return state
    return None


def _write_json_atomic(path: Path, data: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_name(path.name + ".tmp")
    with open(tmp, "w", encoding="utf-8") as fh:
        json.dump(data, fh, indent=2, sort_keys=True)
        fh.flush()
        os.fsync(fh.fileno())
    os.replace(tmp, path)


class ThrottledCaller:
    """Ruft Methoden eines ``garminconnect.Garmin``-Objekts gedrosselt auf.

    ``client`` kann eine Attrappe sein. Es wird nur ``getattr(client, name)``
    verwendet, nach Prüfung gegen die Allowlist.
    """

    def __init__(
        self,
        client: Any,
        *,
        state_dir: Path,
        config: ThrottleConfig | None = None,
        clock: Clock | None = None,
        rng: random.Random | None = None,
        max_calls: int | None = None,
    ) -> None:
        self.client = client
        self.state_dir = Path(state_dir)
        self.config = config or ThrottleConfig()
        self.clock = clock or SystemClock()
        self.rng = rng or random.Random()
        self.stats = CallStats()
        # Obergrenze für Aufrufe an den Client in diesem Lauf, Wiederholungen
        # eingerechnet (SPEC 5.5, Regel 8). ``None`` heißt ohne Grenze.
        self.max_calls = max_calls
        self._lock = threading.Lock()
        self._last_call_at: float | None = None
        self._pause_until: float = 0.0
        self._consecutive_blocks = 0

    # Öffentliche Schnittstelle

    def call(self, method: str, /, *args: Any, **kwargs: Any) -> Any:
        self._ensure_method_allowed(method)
        self._guard_free_methods(method, args, kwargs)
        if not self._lock.acquire(blocking=False):
            # Kein Parallelabruf (SPEC 5.5, Regel 4). Ein zweiter Aufruf wartet.
            self._lock.acquire()
        try:
            return self._call_locked(method, args, kwargs)
        finally:
            self._lock.release()

    # Interna

    def _ensure_method_allowed(self, method: str) -> None:
        """Prüfung gegen die Allowlist. Der Schreibpfad (``push``) ersetzt sie durch seine."""
        ensure_allowed(method)

    def _guard_free_methods(
        self, method: str, args: tuple[Any, ...], kwargs: dict[str, Any]
    ) -> None:
        # Die freien Methoden senden nur GET (Bibliothek). Zusätzlich keine
        # Nutzdaten und keine GraphQL-Mutationen, als zweite Sicherung.
        if method in {"connectapi", "connectwebproxy", "download"}:
            for key in ("data", "json", "files"):
                if key in kwargs:
                    raise PermissionError(f"{method}: Parameter {key!r} ist nicht erlaubt")
            if str(kwargs.get("method", "GET")).upper() != "GET":
                raise PermissionError(f"{method}: nur GET ist erlaubt")
        if method == "query_garmin_graphql":
            text = json.dumps([args, kwargs], default=str).lower()
            if "mutation" in text:
                raise PermissionError("query_garmin_graphql: Mutationen sind nicht erlaubt")

    def _check_lockout(self, method: str) -> None:
        state = active_lockout(self.state_dir, self.clock)
        if state is not None:
            until = datetime.fromtimestamp(float(state["until_epoch"]), tz=UTC)
            raise LockoutError(
                f"Sperre aktiv bis {until.isoformat(timespec='seconds')} "
                f"(gespeichert in {lockout_path(self.state_dir)}), kein Aufruf von {method}",
                until=until,
                state=state,
            )

    def _wait_for_slot(self) -> None:
        now = self.clock.now()
        earliest = self._pause_until
        if self._last_call_at is not None:
            spacing = self.config.min_interval_s + self.rng.uniform(0.0, self.config.jitter_s)
            earliest = max(earliest, self._last_call_at + spacing)
        if earliest > now:
            self.clock.sleep(earliest - now)

    def _run_with_timeout(self, method: str, args: tuple[Any, ...], kwargs: dict[str, Any]) -> Any:
        func = getattr(self.client, method)
        result: dict[str, Any] = {}

        def target() -> None:
            try:
                result["value"] = func(*args, **kwargs)
            except BaseException as exc:  # noqa: BLE001, wird im Aufrufer neu geworfen
                result["error"] = exc

        # Daemon-Thread, damit ein hängender Aufruf das Programmende nicht blockiert.
        thread = threading.Thread(target=target, name=f"garmin-{method}", daemon=True)
        thread.start()
        thread.join(self.config.call_timeout_s)
        if thread.is_alive():
            raise CallTimeoutError(
                f"{method}: keine Antwort nach {self.config.call_timeout_s:.0f} s"
            )
        if "error" in result:
            raise result["error"]
        return result["value"]

    def _record_block(self, status: int, method: str) -> None:
        self.stats.blocked += 1
        self._consecutive_blocks += 1
        now = self.clock.now()
        if self._consecutive_blocks >= self.config.lockout_threshold:
            until = now + self.config.lockout_hold_s
            state = {
                "since": _iso(now),
                "since_epoch": now,
                "until": _iso(until),
                "until_epoch": until,
                "http_status": status,
                "consecutive": self._consecutive_blocks,
                "last_method": method,
                "calls_in_run": self.stats.calls,
            }
            _write_json_atomic(lockout_path(self.state_dir), state)
            raise LockoutError(
                f"Abbruch: {self._consecutive_blocks} Antworten 429 oder 403 in Folge "
                f"(zuletzt {status} bei {method}). Sperre bis {_iso(until)}, Zustand in "
                f"{lockout_path(self.state_dir)}. Kein Neuversuch, keine Umgehung (SPEC 5.5).",
                until=datetime.fromtimestamp(until, tz=UTC),
                state=state,
            )
        self._pause_until = now + self.config.block_pause_s
        raise BlockedResponseError(status, method, self._consecutive_blocks)

    def _call_locked(self, method: str, args: tuple[Any, ...], kwargs: dict[str, Any]) -> Any:
        from garminconnect import (
            GarminConnectAuthenticationError,
            GarminConnectNotFoundError,
        )

        self._check_lockout(method)
        attempt = 0
        while True:
            if self.max_calls is not None and self.stats.calls >= self.max_calls:
                raise CallBudgetExhausted(f"Aufrufbudget von {self.max_calls} erreicht")
            self._wait_for_slot()
            self.stats.calls += 1
            self._last_call_at = self.clock.now()
            try:
                value = self._run_with_timeout(method, args, kwargs)
            except CallTimeoutError:
                self.stats.timeouts += 1
                if not self._retry(attempt):
                    self.stats.errors += 1
                    raise
                attempt += 1
                continue
            except GarminConnectAuthenticationError as exc:
                self.stats.errors += 1
                raise AuthRequiredError(f"{method}: Anmeldung ungültig ({exc})") from exc
            except GarminConnectNotFoundError:
                # 404 ist eine gültige Antwort ("nicht verfügbar"), kein Netzfehler.
                self._consecutive_blocks = 0
                raise
            except Exception as exc:
                status = http_status_of(exc)
                if status in (429, 403):
                    self._record_block(status, method)
                if status is not None and 400 <= status < 500:
                    # Fehler der Anfrage, keine Wiederholung.
                    self.stats.errors += 1
                    raise
                # Netzfehler, 5xx, Zeitüberschreitung der Bibliothek: begrenzt wiederholen.
                if not self._retry(attempt):
                    self.stats.errors += 1
                    raise
                attempt += 1
                continue
            self._consecutive_blocks = 0
            self.stats.ok += 1
            return value

    def _retry(self, attempt: int) -> bool:
        if attempt >= self.config.retries:
            return False
        self.stats.retries += 1
        self.clock.sleep(self.config.backoff_s * (2**attempt))
        return True
