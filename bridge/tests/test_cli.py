"""Kommandozeile: Exit-Codes, keine Netzwerkanfrage ohne Anmeldung (SPEC 12 M4a)."""

from __future__ import annotations

import io
import json
import subprocess
import sys
from pathlib import Path

from garminconnect import GarminConnectAuthenticationError, GarminConnectTooManyRequestsError

from lastwerk_bridge.cli import EXIT_ERROR, EXIT_LOCKOUT, EXIT_OK, EXIT_USAGE, main

from .fakes import FakeClock, FakeGarmin


class Factory:
    def __init__(self, client: FakeGarmin | None = None) -> None:
        self.client = client or FakeGarmin(default={"synthetic": True})
        self.calls: list[Path] = []

    def __call__(self, token_dir: Path) -> FakeGarmin:
        self.calls.append(token_dir)
        return self.client


def run(argv: list[str], factory: Factory, clock: FakeClock) -> tuple[int, str, str]:
    out, err = io.StringIO(), io.StringIO()
    code = main(argv, client_factory=factory, clock=clock, stdout=out, stderr=err)
    return code, out.getvalue(), err.getvalue()


def test_auth_check_without_tokens_exits_2_and_sends_nothing(tmp_path: Path, clock) -> None:
    factory = Factory()
    code, _out, err = run(
        ["auth", "--check", "--token-dir", str(tmp_path / "none")], factory, clock
    )
    assert code == EXIT_USAGE
    assert "Keine Anmeldung" in err
    assert factory.calls == []
    assert factory.client.calls == []


def test_auth_check_with_tokens_makes_exactly_one_call(token_dir: Path, root: Path, clock) -> None:
    factory = Factory()
    code, out, _err = run(
        ["auth", "--check", "--token-dir", str(token_dir), "--root", str(root)], factory, clock
    )
    assert code == EXIT_OK
    assert factory.calls == [token_dir]
    assert factory.client.names_called() == ["get_user_profile"]
    assert "gültig" in out
    # Keine Werte aus der Antwort in der Ausgabe.
    assert "synthetic" not in out


def test_auth_check_with_rejected_token_exits_2(token_dir: Path, clock) -> None:
    factory = Factory(FakeGarmin({"get_user_profile": GarminConnectAuthenticationError("401")}))
    code, _out, err = run(["auth", "--check", "--token-dir", str(token_dir)], factory, clock)
    assert code == EXIT_USAGE
    assert "Anmeldung ungültig" in err


def test_auth_check_with_429_exits_1(token_dir: Path, clock) -> None:
    factory = Factory(FakeGarmin({"get_user_profile": GarminConnectTooManyRequestsError("429")}))
    code, _out, err = run(["auth", "--check", "--token-dir", str(token_dir)], factory, clock)
    assert code == EXIT_ERROR
    assert "429" in err


def test_active_lockout_exits_3_without_any_call(token_dir: Path, root: Path, clock) -> None:
    state_dir = root / "state"
    state_dir.mkdir(parents=True)
    (state_dir / "lockout.json").write_text(
        json.dumps({"until": "later", "until_epoch": clock.now() + 600}), encoding="utf-8"
    )
    factory = Factory()
    code, _out, err = run(
        ["auth", "--check", "--token-dir", str(token_dir), "--root", str(root)], factory, clock
    )
    assert code == EXIT_LOCKOUT
    assert "Sperre aktiv" in err
    assert factory.calls == []


def test_three_429_during_probe_exit_3_and_persist(
    token_dir: Path, root: Path, tmp_path, clock
) -> None:
    factory = Factory(FakeGarmin(default=GarminConnectTooManyRequestsError("429")))
    code, _out, err = run(
        [
            "probe",
            "--out",
            str(tmp_path / "probe"),
            "--token-dir",
            str(token_dir),
            "--root",
            str(root),
            "--today",
            "2026-10-13",
        ],
        factory,
        clock,
    )
    assert code == EXIT_LOCKOUT
    assert "Abbruch wegen Sperre" in err
    assert (root / "state" / "lockout.json").is_file()
    assert len(factory.client.calls) == 3


def test_global_options_before_subcommand(token_dir: Path, root: Path, clock) -> None:
    factory = Factory()
    code, _out, _err = run(
        ["--token-dir", str(token_dir), "--root", str(root), "auth", "--check"], factory, clock
    )
    assert code == EXIT_OK


def test_probe_writes_report(token_dir: Path, root: Path, tmp_path: Path, clock) -> None:
    factory = Factory(FakeGarmin(default={"synthetic": 1}))
    out_dir = tmp_path / "probe"
    code, out, _err = run(
        [
            "probe",
            "--out",
            str(out_dir),
            "--token-dir",
            str(token_dir),
            "--root",
            str(root),
            "--today",
            "2026-10-13",
            "--min-interval",
            "0.2",
        ],
        factory,
        clock,
    )
    assert code == EXIT_OK
    assert (out_dir / "probe-report.json").is_file()
    assert "Bericht" in out
    # Nichts im Archiv, die Sonde schreibt nur in --out.
    assert not root.exists() or not (root / "manifest.jsonl").exists()


def test_probe_default_reference_day_is_the_local_day(
    token_dir: Path, root: Path, tmp_path: Path
) -> None:
    from datetime import UTC, datetime, timedelta, timezone

    # 2026-10-15 03:00 UTC ist in UTC-5 noch der 14. Oktober. Tageswerte der Sonde liegen drei
    # Tage davor (Standard der Sonde), also am 11. Oktober, nicht am 12.
    clock = FakeClock(
        start=datetime(2026, 10, 15, 3, 0, tzinfo=UTC).timestamp(),
        tz=timezone(timedelta(hours=-5)),
    )
    factory = Factory(FakeGarmin(default={"synthetic": 1}))
    out_dir = tmp_path / "probe"
    code, _out, _err = run(
        ["probe", "--out", str(out_dir), "--token-dir", str(token_dir), "--root", str(root)],
        factory,
        clock,
    )
    assert code == EXIT_OK
    report = json.loads((out_dir / "probe-report.json").read_text(encoding="utf-8"))
    dates = {
        e["params_used"]["date"]
        for e in report["entries"]
        if isinstance(e.get("params_used"), dict) and "date" in e["params_used"]
    }
    assert dates == {"2026-10-11"}


def test_no_command_is_usage_error(clock) -> None:
    code, _out, _err = run([], Factory(), clock)
    assert code == EXIT_USAGE


def test_module_entry_point_without_tokens(tmp_path: Path) -> None:
    """``python -m lastwerk_bridge auth --check`` ohne Token: Exit 2, keine Anfrage.

    Läuft als Unterprozess, deshalb wirkt die Socket-Sperre dort nicht. Der Pfad
    endet aber vor jedem Client, ein Fehlschlag wäre hier als Stacktrace sichtbar.
    """
    result = subprocess.run(
        [
            sys.executable,
            "-m",
            "lastwerk_bridge",
            "auth",
            "--check",
            "--token-dir",
            str(tmp_path / "none"),
        ],
        capture_output=True,
        text=True,
        timeout=60,
        check=False,
    )
    assert result.returncode == EXIT_USAGE, result.stderr
    assert "Keine Anmeldung" in result.stderr
