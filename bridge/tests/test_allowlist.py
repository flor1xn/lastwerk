"""Allowlist gegen die installierte Bibliothek (SPEC 5.5 Regel 7, SPEC 12 M4a)."""

from __future__ import annotations

import inspect

import pytest

from lastwerk_bridge.allowlist import (
    ALLOWLIST,
    BLOCKLIST,
    FORBIDDEN_ALLOWLIST_PATTERNS,
    MethodNotAllowedError,
    ensure_allowed,
)
from lastwerk_bridge.registry import REGISTRY
from lastwerk_bridge.throttle import ThrottledCaller

from .fakes import FakeGarmin


def public_methods() -> set[str]:
    """Alle öffentlichen Methoden von ``garminconnect.Garmin``.

    ``inspect.isfunction`` lässt die verschachtelten Enum-Klassen
    (``ActivityDownloadFormat``, ``ActivityUploadFormat``) aus, sie sind keine Methoden.
    """
    from garminconnect import Garmin

    return {
        name
        for name, member in inspect.getmembers(Garmin)
        if not name.startswith("_") and inspect.isfunction(member)
    }


def test_every_public_method_is_classified_exactly_once() -> None:
    methods = public_methods()
    assert methods, "keine Methoden gefunden"
    unclassified = methods - ALLOWLIST - BLOCKLIST
    assert not unclassified, f"neue Methoden einstufen: {sorted(unclassified)}"
    stale = (ALLOWLIST | BLOCKLIST) - methods
    assert not stale, f"Einträge ohne Methode in der Bibliothek: {sorted(stale)}"


def test_lists_are_disjoint() -> None:
    assert not (ALLOWLIST & BLOCKLIST)


@pytest.mark.parametrize("pattern", FORBIDDEN_ALLOWLIST_PATTERNS)
def test_allowlist_contains_no_forbidden_pattern(pattern: str) -> None:
    offenders = sorted(name for name in ALLOWLIST if pattern in name)
    assert not offenders, f"{pattern!r} in Allowlist: {offenders}"


def test_menstrual_and_pregnancy_methods_are_blocked() -> None:
    methods = public_methods()
    sensitive = {m for m in methods if "menstrual" in m or "pregnancy" in m}
    assert sensitive, "die Bibliothek hat solche Methoden, der Test erwartet sie"
    assert sensitive <= BLOCKLIST
    assert not (sensitive & ALLOWLIST)


def test_write_and_login_methods_are_blocked() -> None:
    for name in ("login", "logout", "resume_login", "upload_activity", "delete_activity"):
        assert name in BLOCKLIST


def test_registry_uses_only_allowed_methods() -> None:
    for entry in REGISTRY:
        assert entry.method in ALLOWLIST, entry.name


def test_ensure_allowed_rejects_blocked_and_unknown() -> None:
    ensure_allowed("get_user_profile")
    with pytest.raises(MethodNotAllowedError):
        ensure_allowed("delete_activity")
    with pytest.raises(MethodNotAllowedError):
        ensure_allowed("get_menstrual_data_for_date")
    with pytest.raises(MethodNotAllowedError):
        ensure_allowed("some_future_method")


def test_caller_never_touches_client_for_blocked_methods(tmp_path, clock) -> None:
    client = FakeGarmin(default={"x": 1})
    caller = ThrottledCaller(client, state_dir=tmp_path, clock=clock)
    for name in ("delete_activity", "get_pregnancy_summary", "login", "unknown_method"):
        with pytest.raises(MethodNotAllowedError):
            caller.call(name)
    assert client.calls == []
    assert caller.stats.calls == 0


def test_caller_guards_free_methods(tmp_path, clock) -> None:
    client = FakeGarmin(default={"x": 1})
    caller = ThrottledCaller(client, state_dir=tmp_path, clock=clock)
    with pytest.raises(PermissionError):
        caller.call("connectapi", "/some/path", method="POST")
    with pytest.raises(PermissionError):
        caller.call("connectapi", "/some/path", json={"a": 1})
    with pytest.raises(PermissionError):
        caller.call("query_garmin_graphql", {"query": "mutation { x }"})
    assert client.calls == []
    assert caller.call("connectapi", "/some/path") == {"x": 1}
