"""Ein Registereintrag, ein Parametersatz, ein Abruf in das Archiv.

Verbindet Register, gedrosselten Aufrufer und Archiv. ``backfill`` und ``sync``
(T12) bauen auf ``fetch_one`` auf. Hier liegt die Fortsetzungslogik: Vorhandene
Schlüssel werden nicht erneut geholt, leere Antworten negativ vermerkt, die letzten
N Tage immer neu abgefragt (SPEC 5.5).
"""

from __future__ import annotations

from dataclasses import dataclass
from datetime import date
from typing import Any

from .archive import DEFAULT_REFRESH_DAYS, Archive, encode_payload, is_empty
from .manifest import ManifestEntry
from .registry import RegistryEntry
from .throttle import (
    AuthRequiredError,
    BlockedResponseError,
    CallBudgetExhausted,
    CallTimeoutError,
    LockoutError,
    ThrottledCaller,
    http_status_of,
)


@dataclass
class FetchOutcome:
    skipped: bool
    entry: ManifestEntry | None = None


def call_args(
    entry: RegistryEntry, params: dict[str, Any], client: Any
) -> tuple[tuple[Any, ...], dict[str, Any]]:
    """Positionsargumente nach Granularität, feste Schlüsselwortargumente aus dem Register."""
    if entry.granularity == "once":
        args: tuple[Any, ...] = tuple(params[p] for p in entry.params)
    elif entry.granularity == "day":
        args = (str(params["date"]),)
    elif entry.granularity == "range":
        args = (str(params["start"]), str(params["end"]))
    elif entry.granularity == "page":
        args = (int(params["start"]), int(params["limit"]))
    else:
        args = (params["activity_id"],)
    kwargs: dict[str, Any] = {}
    for key, value in entry.kwargs.items():
        if key == "dl_fmt" and isinstance(value, str):
            # Enum der Bibliothek, bei einer Attrappe bleibt die Zeichenkette.
            formats = getattr(client, "ActivityDownloadFormat", None)
            kwargs[key] = getattr(formats, value, value)
        else:
            kwargs[key] = value
    return args, kwargs


def profile_number(profile: Any) -> Any:
    """Profilnummer aus der Antwort von ``get_user_profile`` (für ``get_gear``).

    Feldnamen sind bis L1 eine Annahme: ``userProfileNumber``, sonst ``id``.
    """
    if isinstance(profile, dict):
        return profile.get("userProfileNumber", profile.get("id"))
    return None


def refresh_day(entry: RegistryEntry, params: dict[str, Any]) -> date | None:
    """Datum, nach dem die Neuabfrage der letzten N Tage entschieden wird."""
    raw = None
    if entry.granularity == "day":
        raw = params.get("date")
    elif entry.granularity == "range":
        raw = params.get("end")
    if raw is None:
        return None
    return raw if isinstance(raw, date) else date.fromisoformat(str(raw))


def fetch_one(
    archive: Archive,
    caller: ThrottledCaller,
    entry: RegistryEntry,
    params: dict[str, Any],
    *,
    latest: dict[str, ManifestEntry] | None = None,
    refresh_days: int = DEFAULT_REFRESH_DAYS,
    today: date | None = None,
    force: bool = False,
) -> FetchOutcome:
    """Holt einen Schlüssel, falls nötig, und vermerkt das Ergebnis im Manifest.

    ``force`` überspringt die Prüfung auf vorhandene Einträge: der Aufrufer (Plan
    von ``backfill`` oder ``sync``) hat bereits entschieden, dass dieser Schlüssel
    geholt wird. ``LockoutError``, ``AuthRequiredError`` und ``CallBudgetExhausted``
    werden durchgereicht, damit der Lauf endet. Alle anderen Fehler ergeben einen
    Eintrag ``error`` oder ``not_available``.
    """
    kind = entry.manifest_kind
    params = {k: (v.isoformat() if isinstance(v, date) else v) for k, v in params.items()}
    if not force and not archive.needs_fetch(
        kind,
        entry.name,
        params,
        latest=latest,
        day=refresh_day(entry, params),
        refresh_days=refresh_days,
        today=today,
    ):
        return FetchOutcome(skipped=True)

    args, kwargs = call_args(entry, params, caller.client)
    try:
        value = caller.call(entry.method, *args, **kwargs)
    except (LockoutError, AuthRequiredError, CallBudgetExhausted):
        raise
    except BlockedResponseError as exc:
        return FetchOutcome(
            skipped=False,
            entry=archive.note(
                kind, entry.name, params, "error", http_status=exc.status, error_class="Blocked"
            ),
        )
    except CallTimeoutError:
        return FetchOutcome(
            skipped=False,
            entry=archive.note(kind, entry.name, params, "error", error_class="CallTimeout"),
        )
    except Exception as exc:  # noqa: BLE001, alle Bibliotheksfehler landen im Manifest
        status = http_status_of(exc)
        if status == 404:
            return FetchOutcome(
                skipped=False,
                entry=archive.note(kind, entry.name, params, "not_available", http_status=404),
            )
        return FetchOutcome(
            skipped=False,
            entry=archive.note(
                kind,
                entry.name,
                params,
                "error",
                http_status=status,
                error_class=type(exc).__name__,
            ),
        )

    if is_empty(value):
        return FetchOutcome(skipped=False, entry=archive.note(kind, entry.name, params, "empty"))
    payload = encode_payload(value)
    stored = archive.store(kind, entry.name, params, payload, archive.layout_path(entry, params))
    return FetchOutcome(skipped=False, entry=stored)
