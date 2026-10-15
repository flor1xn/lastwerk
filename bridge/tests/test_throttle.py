"""Drosselung, Zeitlimit, Wiederholung und Sperre (SPEC 5.5 Regeln 4 und 8, SPEC 12 M4a)."""

from __future__ import annotations

import json
import random
import threading
from pathlib import Path

import pytest
from garminconnect import (
    GarminConnectAuthenticationError,
    GarminConnectConnectionError,
    GarminConnectNotFoundError,
    GarminConnectTooManyRequestsError,
)

from lastwerk_bridge.throttle import (
    AuthRequiredError,
    BlockedResponseError,
    CallTimeoutError,
    LockoutError,
    ThrottleConfig,
    ThrottledCaller,
    active_lockout,
    http_status_of,
)

from .fakes import FakeClock, FakeGarmin


def make_caller(
    client: FakeGarmin, tmp_path: Path, clock: FakeClock, **overrides
) -> ThrottledCaller:
    config = ThrottleConfig(**{**ThrottleConfig().__dict__, "jitter_s": 0.0, **overrides})
    return ThrottledCaller(
        client, state_dir=tmp_path / "state", config=config, clock=clock, rng=random.Random(0)
    )


def forbidden_403() -> GarminConnectConnectionError:
    return GarminConnectConnectionError("API call client error (403): Forbidden")


def test_min_interval_between_calls(tmp_path: Path, clock: FakeClock) -> None:
    client = FakeGarmin(default={"ok": True})
    caller = make_caller(client, tmp_path, clock, min_interval_s=1.5)
    t0 = clock.now()
    for _ in range(3):
        caller.call("get_user_profile")
    assert len(client.calls) == 3
    # Erster Aufruf sofort, danach je 1,5 s Abstand: 3,0 s gesamt.
    assert clock.now() - t0 == pytest.approx(3.0)
    assert clock.sleeps == [1.5, 1.5]


def test_jitter_adds_random_spacing(tmp_path: Path, clock: FakeClock) -> None:
    client = FakeGarmin(default={"ok": True})
    caller = make_caller(client, tmp_path, clock, min_interval_s=1.0, jitter_s=0.5)
    caller.call("get_user_profile")
    caller.call("get_user_profile")
    assert 1.0 <= clock.sleeps[0] <= 1.5


def test_hanging_call_is_cut_by_timeout(tmp_path: Path, clock: FakeClock) -> None:
    release = threading.Event()

    def hang(*_args, **_kwargs):
        release.wait(10.0)
        return {"late": True}

    client = FakeGarmin({"get_user_profile": hang})
    caller = make_caller(client, tmp_path, clock, call_timeout_s=0.2, retries=0)
    try:
        with pytest.raises(CallTimeoutError):
            caller.call("get_user_profile")
    finally:
        release.set()
    assert caller.stats.timeouts == 1
    assert caller.stats.errors == 1


def test_timeout_is_retried_then_fails(tmp_path: Path, clock: FakeClock) -> None:
    release = threading.Event()

    def hang(*_args, **_kwargs):
        release.wait(10.0)

    client = FakeGarmin({"get_user_profile": hang})
    caller = make_caller(client, tmp_path, clock, call_timeout_s=0.1, retries=1, backoff_s=2.0)
    try:
        with pytest.raises(CallTimeoutError):
            caller.call("get_user_profile")
    finally:
        release.set()
    assert caller.stats.timeouts == 2
    assert caller.stats.retries == 1
    assert 2.0 in clock.sleeps


def test_three_429_in_a_row_lock_out_and_persist_state(tmp_path: Path, clock: FakeClock) -> None:
    client = FakeGarmin({"get_sleep_data": GarminConnectTooManyRequestsError("429")})
    caller = make_caller(client, tmp_path, clock, block_pause_s=30.0, lockout_hold_s=3600.0)
    with pytest.raises(BlockedResponseError) as first:
        caller.call("get_sleep_data", "2026-10-06")
    assert first.value.consecutive == 1
    with pytest.raises(BlockedResponseError):
        caller.call("get_sleep_data", "2026-10-06")
    # Nach einem 429 wartet der Aufrufer die Sperrpause ab.
    assert max(clock.sleeps) >= 30.0
    with pytest.raises(LockoutError) as lock:
        caller.call("get_sleep_data", "2026-10-06")
    assert len(client.calls) == 3
    state_file = tmp_path / "state" / "lockout.json"
    assert state_file.is_file()
    state = json.loads(state_file.read_text(encoding="utf-8"))
    assert state["consecutive"] == 3
    assert state["http_status"] == 429
    assert state["last_method"] == "get_sleep_data"
    assert state["until_epoch"] == pytest.approx(clock.now() + 3600.0)
    assert lock.value.state == state
    assert active_lockout(tmp_path / "state", clock) is not None


def test_mixed_403_and_429_count_together(tmp_path: Path, clock: FakeClock) -> None:
    answers = iter([GarminConnectTooManyRequestsError("429"), forbidden_403(), forbidden_403()])

    def respond(*_args, **_kwargs):
        raise next(answers)

    client = FakeGarmin({"get_stats": respond})
    caller = make_caller(client, tmp_path, clock)
    with pytest.raises(BlockedResponseError):
        caller.call("get_stats", "2026-10-06")
    with pytest.raises(BlockedResponseError):
        caller.call("get_stats", "2026-10-06")
    with pytest.raises(LockoutError):
        caller.call("get_stats", "2026-10-06")
    assert caller.stats.blocked == 3


def test_success_resets_consecutive_counter(tmp_path: Path, clock: FakeClock) -> None:
    answers = iter(
        [
            GarminConnectTooManyRequestsError("429"),
            GarminConnectTooManyRequestsError("429"),
            {"ok": True},
            GarminConnectTooManyRequestsError("429"),
            GarminConnectTooManyRequestsError("429"),
        ]
    )

    def respond(*_args, **_kwargs):
        answer = next(answers)
        if isinstance(answer, BaseException):
            raise answer
        return answer

    client = FakeGarmin({"get_stats": respond})
    caller = make_caller(client, tmp_path, clock)
    for expected in (
        BlockedResponseError,
        BlockedResponseError,
        None,
        BlockedResponseError,
        BlockedResponseError,
    ):
        if expected is None:
            assert caller.call("get_stats", "d") == {"ok": True}
        else:
            with pytest.raises(expected):
                caller.call("get_stats", "d")
    assert not (tmp_path / "state" / "lockout.json").exists()


def test_following_run_aborts_immediately_while_locked(tmp_path: Path, clock: FakeClock) -> None:
    client = FakeGarmin({"get_stats": GarminConnectTooManyRequestsError("429")})
    caller = make_caller(client, tmp_path, clock, lockout_hold_s=3600.0)
    for _ in range(2):
        with pytest.raises(BlockedResponseError):
            caller.call("get_stats", "d")
    with pytest.raises(LockoutError):
        caller.call("get_stats", "d")
    calls_before = len(client.calls)

    # Zweiter Lauf: neuer Aufrufer, gleicher Zustand, Sperre noch jung.
    second = make_caller(FakeGarmin(default={"ok": True}), tmp_path, clock)
    with pytest.raises(LockoutError):
        second.call("get_user_profile")
    assert second.stats.calls == 0
    assert len(client.calls) == calls_before

    # Nach Ablauf der Frist geht es weiter.
    clock.advance(3601.0)
    assert active_lockout(tmp_path / "state", clock) is None
    assert second.call("get_user_profile") == {"ok": True}


def test_network_errors_are_retried_with_backoff(tmp_path: Path, clock: FakeClock) -> None:
    answers = iter(
        [OSError("reset"), GarminConnectConnectionError("server error (502)"), {"ok": 1}]
    )

    def respond(*_args, **_kwargs):
        answer = next(answers)
        if isinstance(answer, BaseException):
            raise answer
        return answer

    client = FakeGarmin({"get_devices": respond})
    caller = make_caller(client, tmp_path, clock, retries=2, backoff_s=2.0, min_interval_s=1.0)
    assert caller.call("get_devices") == {"ok": 1}
    assert caller.stats.retries == 2
    assert caller.stats.calls == 3
    # Backoff 2 s, dann 4 s, dazu der Mindestabstand vor jedem Folgeaufruf.
    assert 2.0 in clock.sleeps and 4.0 in clock.sleeps


def test_retries_are_bounded(tmp_path: Path, clock: FakeClock) -> None:
    client = FakeGarmin({"get_devices": OSError("down")})
    caller = make_caller(client, tmp_path, clock, retries=2)
    with pytest.raises(OSError):
        caller.call("get_devices")
    assert caller.stats.calls == 3
    assert caller.stats.errors == 1


def test_404_passes_through_and_is_not_retried(tmp_path: Path, clock: FakeClock) -> None:
    client = FakeGarmin({"get_activity": GarminConnectNotFoundError("client error (404)")})
    caller = make_caller(client, tmp_path, clock)
    with pytest.raises(GarminConnectNotFoundError):
        caller.call("get_activity", "1")
    assert caller.stats.calls == 1


def test_400_is_not_retried(tmp_path: Path, clock: FakeClock) -> None:
    client = FakeGarmin({"get_activity": GarminConnectConnectionError("client error (400): bad")})
    caller = make_caller(client, tmp_path, clock, retries=2)
    with pytest.raises(GarminConnectConnectionError):
        caller.call("get_activity", "1")
    assert caller.stats.calls == 1


def test_401_becomes_auth_required(tmp_path: Path, clock: FakeClock) -> None:
    client = FakeGarmin({"get_user_profile": GarminConnectAuthenticationError("401")})
    caller = make_caller(client, tmp_path, clock)
    with pytest.raises(AuthRequiredError):
        caller.call("get_user_profile")


def test_http_status_detection() -> None:
    assert http_status_of(GarminConnectTooManyRequestsError("x")) == 429
    assert http_status_of(forbidden_403()) == 403
    assert http_status_of(GarminConnectConnectionError("HTTP 503")) == 503
    assert http_status_of(OSError("reset")) is None

    class Resp:
        status_code = 403

    exc = GarminConnectConnectionError("whatever")
    exc.response = Resp()
    assert http_status_of(exc) == 403


def test_no_parallel_calls(tmp_path: Path, clock: FakeClock) -> None:
    """Zwei Threads rufen gleichzeitig, die Attrappe sieht nie zwei offene Aufrufe."""
    active = 0
    max_active = 0
    guard = threading.Lock()

    def respond(*_args, **_kwargs):
        nonlocal active, max_active
        with guard:
            active += 1
            max_active = max(max_active, active)
        threading.Event().wait(0.02)
        with guard:
            active -= 1
        return {}

    client = FakeGarmin({"get_user_profile": respond})
    caller = make_caller(client, tmp_path, clock, min_interval_s=0.0)
    threads = [threading.Thread(target=caller.call, args=("get_user_profile",)) for _ in range(4)]
    for t in threads:
        t.start()
    for t in threads:
        t.join(5.0)
    assert len(client.calls) == 4
    assert max_active == 1
