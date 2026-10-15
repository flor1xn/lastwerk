"""Kommandozeile ``python -m lastwerk_bridge`` (SPEC 5.5 Befehle, SPEC 12 M4a).

Exit-Codes: 0 ok, 1 Fehler, 2 Nutzungs- oder Voraussetzungsfehler (zum Beispiel
keine Anmeldung, fehlender Archivordner, ungültiger Plan), 3 Abbruch wegen Sperre (drei
Antworten 429 oder 403 in Folge oder Sperre noch aktiv).

``backfill`` und ``sync`` melden zusätzlich: 1 bei Fehlern einzelner Abrufe (der
Lauf lief durch, die Fehler sind im Manifest und werden beim nächsten Lauf
wiederholt), 3 auch nach dem Abbruch durch drei 429 oder 403 in Folge.
"""

from __future__ import annotations

import argparse
import json
import sys
from collections.abc import Callable
from datetime import date, datetime
from pathlib import Path
from typing import Any, TextIO

from . import __version__, auth, normalize, push, reconcile, runner, usb
from .archive import (
    DEFAULT_REFRESH_DAYS,
    DEVICE_IMPORT_REPORT_NAME,
    NORMALIZE_REPORT_NAME,
    RECONCILE_REPORT_NAME,
    Archive,
)
from .coverage import build_coverage, format_coverage, write_coverage
from .probe import run_probe
from .throttle import (
    AuthRequiredError,
    BridgeError,
    Clock,
    LockoutError,
    SystemClock,
    ThrottleConfig,
    ThrottledCaller,
    active_lockout,
    local_today,
)

EXIT_OK = 0
EXIT_ERROR = 1
EXIT_USAGE = 2
EXIT_LOCKOUT = 3

ClientFactory = Callable[[Path], Any]


def _add_common_options(parser: argparse.ArgumentParser, *, suppress: bool) -> None:
    """Gemeinsame Optionen vor oder nach dem Unterbefehl (SPEC schreibt `backfill --root R`).

    In den Unterbefehlen mit SUPPRESS, damit ein fehlender Wert den globalen nicht
    überschreibt.
    """
    default: Any = argparse.SUPPRESS if suppress else None
    parser.add_argument(
        "--root",
        type=Path,
        default=default,
        help="Archivordner (enthält manifest.jsonl), im Normalfall local/archive/garmin",
    )
    parser.add_argument(
        "--token-dir",
        type=Path,
        default=default,
        help=f"Token-Verzeichnis, Standard {auth.DEFAULT_TOKEN_DIR} (nie im Projekt)",
    )
    parser.add_argument(
        "--min-interval",
        type=float,
        default=default,
        help="Mindestabstand zwischen Aufrufen in Sekunden (vorläufiger Standard 1,5, O24)",
    )
    parser.add_argument(
        "--call-timeout",
        type=float,
        default=default,
        help="Zeitlimit je Aufruf in Sekunden (Standard 30)",
    )


def _parse_date(text: str) -> date:
    try:
        return datetime.strptime(text, "%Y-%m-%d").date()
    except ValueError as exc:
        raise argparse.ArgumentTypeError(f"kein Datum (JJJJ-MM-TT): {text}") from exc


def _positive_int(text: str) -> int:
    try:
        value = int(text)
    except ValueError as exc:
        raise argparse.ArgumentTypeError(f"keine ganze Zahl: {text}") from exc
    if value < 1:
        raise argparse.ArgumentTypeError(f"muss mindestens 1 sein: {text}")
    return value


def _non_negative_int(text: str) -> int:
    try:
        value = int(text)
    except ValueError as exc:
        raise argparse.ArgumentTypeError(f"keine ganze Zahl: {text}") from exc
    if value < 0:
        raise argparse.ArgumentTypeError(f"darf nicht negativ sein: {text}")
    return value


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="lastwerk-bridge",
        description=(
            "Lesender Garmin-Abruf in ein lokales Archiv. Inoffizieller Zugang, "
            "siehe bridge/README.md. Exit-Codes: 0 ok, 1 Fehler, 2 Voraussetzung, 3 Sperre."
        ),
    )
    parser.add_argument("--version", action="version", version=f"lastwerk-bridge {__version__}")
    _add_common_options(parser, suppress=False)
    common = argparse.ArgumentParser(add_help=False)
    _add_common_options(common, suppress=True)

    sub = parser.add_subparsers(dest="command", required=True)

    p_auth = sub.add_parser(
        "auth", parents=[common], help="einmalige Anmeldung mit MFA, oder --check"
    )
    p_auth.add_argument(
        "--check",
        action="store_true",
        help="prüft das gespeicherte Token mit genau einem Aufruf (get_user_profile)",
    )

    p_probe = sub.add_parser(
        "probe", parents=[common], help="Sonde: je Datenart ein Aufruf, Bericht ohne Werte"
    )
    p_probe.add_argument(
        "--out", type=Path, required=True, help="Ausgabeordner, zum Beispiel local/probe"
    )
    p_probe.add_argument(
        "--today",
        type=lambda s: datetime.strptime(s, "%Y-%m-%d").date(),
        default=None,
        help="Bezugsdatum (Standard: heute, lokales Datum)",
    )

    p_backfill = sub.add_parser(
        "backfill",
        parents=[common],
        help="rückwirkender Abruf von neu nach alt, gedrosselt und fortsetzbar",
    )
    p_backfill.add_argument(
        "--until",
        type=_parse_date,
        default=None,
        help="älteste Grenze (JJJJ-MM-TT), weiter zurück wird nicht abgerufen",
    )
    p_backfill.add_argument(
        "--max-calls", type=_positive_int, default=None, help="höchstens so viele Aufrufe je Lauf"
    )
    p_backfill.add_argument(
        "--dry-run",
        action="store_true",
        help="zeigt die geplanten Aufrufe je Endpunkt, sendet nichts, braucht keine Anmeldung",
    )

    p_sync = sub.add_parser(
        "sync", parents=[common], help="nur Neues seit dem letzten Lauf, letzte Tage neu"
    )
    p_sync.add_argument(
        "--refresh-days",
        type=_non_negative_int,
        default=DEFAULT_REFRESH_DAYS,
        help=f"so viele letzte Tage immer neu holen (Standard {DEFAULT_REFRESH_DAYS}, Faustregel)",
    )
    p_sync.add_argument(
        "--max-calls", type=_positive_int, default=None, help="höchstens so viele Aufrufe je Lauf"
    )

    sub.add_parser(
        "coverage",
        parents=[common],
        help="Abdeckungsbericht aus dem Archiv, schreibt coverage.json, sendet nichts",
    )

    p_usb = sub.add_parser(
        "import-device",
        parents=[common],
        help="optional: Geräteordner (Activity, Monitor, Sleep) unverändert ins Archiv kopieren",
    )
    p_usb.add_argument(
        "ordner",
        type=Path,
        help="Datenträger oder Ordner des Geräts (enthält Activity, Monitor, Sleep, "
        "auch unter GARMIN/)",
    )
    p_usb.add_argument(
        "--device",
        default=None,
        help="Name unter device_usb/ (Standard: Name des Ordners)",
    )

    p_rec = sub.add_parser(
        "reconcile",
        parents=[common],
        help="Abgleich: Aktivitäten nur in der API, nur auf dem Gerät, in beiden (ohne Werte)",
    )
    p_rec.add_argument(
        "--name-pattern",
        default=reconcile.DEFAULT_NAME_PATTERN,
        help="regulärer Ausdruck für Dateinamen auf dem Gerät mit den Gruppen year, month, "
        "day, hour, minute, second oder activity_id (Standard: Zeitstempel oder Kennung)",
    )
    p_rec.add_argument(
        "--tolerance",
        type=_non_negative_int,
        default=reconcile.DEFAULT_TOLERANCE_S,
        help=f"Toleranz in Sekunden gegenüber startTimeGMT (Standard "
        f"{reconcile.DEFAULT_TOLERANCE_S}, Konvention)",
    )
    p_rec.add_argument(
        "--name-offset-minutes",
        type=int,
        default=0,
        help="Versatz der Namenszeit gegenüber UTC in Minuten (Standard 0: Name gilt als UTC)",
    )

    p_norm = sub.add_parser(
        "normalize",
        parents=[common],
        help="leitet exchange/out aus dem Archiv ab (FIT und Erholungstage), ohne Garmin-Aufruf",
    )
    p_norm.add_argument(
        "--out",
        type=Path,
        required=True,
        help="Austauschordner exchange/out (activities/<id>.fit, recovery/JJJJ-MM-TT.json)",
    )
    p_norm.add_argument(
        "--allow-sensitive",
        action="append",
        default=[],
        metavar="KATEGORIE",
        help="gibt eine Kategorie aus sensitive/ frei (Standard: keine). Wiederholbar. "
        f"Kategorien: {', '.join(normalize.sensitive_categories())}. In M4d wird daraus noch "
        "nichts abgeleitet",
    )

    p_push = sub.add_parser(
        "push",
        parents=[common],
        help="Workouts aus plan.json nach Garmin Connect: Trockenlauf (Standard) oder --yes",
        description=(
            "Berechnet je Einheit der plan.json die Nutzlast für ein Garmin-Connect-Workout. "
            "Ohne --yes ist das ein Trockenlauf: die Nutzlasten gehen als JSON auf die "
            "Standardausgabe, es wird nichts gesendet. Mit --yes werden die gewählten "
            "Einheiten angelegt und im Kalender geplant (schreibender Zugriff auf das Konto)."
        ),
    )
    p_push.add_argument("--plan", type=Path, required=True, help="plan.json (exchange/in)")
    mode = p_push.add_mutually_exclusive_group()
    mode.add_argument(
        "--dry-run",
        action="store_true",
        help="Trockenlauf (Standard): sendet nichts, braucht keine Anmeldung",
    )
    mode.add_argument(
        "--yes",
        action="store_true",
        help="echter Versand: legt Workouts an und plant sie. Braucht --out und --max-workouts",
    )
    p_push.add_argument(
        "--out",
        type=Path,
        default=None,
        help="Ordner exchange/out: schreibt push-dry-run.json (Trockenlauf) oder "
        "push-report.json (--yes)",
    )
    p_push.add_argument(
        "--templates",
        type=Path,
        default=None,
        help="Vorlagenkatalog (Standard: catalog/templates.json im Repository)",
    )
    p_push.add_argument(
        "--locales",
        type=Path,
        default=None,
        help="Ordner mit de.json und en.json (Standard: locales neben dem Katalog)",
    )
    p_push.add_argument(
        "--from",
        dest="from_date",
        type=_parse_date,
        default=None,
        help="nur Einheiten ab diesem Datum (Standard: im Trockenlauf alle, mit --yes heute)",
    )
    p_push.add_argument(
        "--to",
        dest="to_date",
        type=_parse_date,
        default=None,
        help="nur Einheiten bis zu diesem Datum",
    )
    p_push.add_argument(
        "--max-workouts",
        type=_positive_int,
        default=None,
        help="höchstens so viele Einheiten (die ersten in Planreihenfolge). Pflicht mit --yes",
    )
    return parser


def _throttle_config(args: argparse.Namespace) -> ThrottleConfig:
    base = ThrottleConfig()
    overrides: dict[str, float] = {}
    if args.min_interval is not None:
        overrides["min_interval_s"] = args.min_interval
    if args.call_timeout is not None:
        overrides["call_timeout_s"] = args.call_timeout
    return ThrottleConfig(**{**base.__dict__, **overrides})


def _state_dir(args: argparse.Namespace) -> Path:
    """Sperrzustand liegt beim Archiv. Ohne --root im Token-Verzeichnis (Benutzerprofil)."""
    if args.root is not None:
        return Path(args.root) / "state"
    return auth.token_dir_path(args.token_dir) / "state"


def _require_tokens(args: argparse.Namespace, err: TextIO) -> bool:
    if auth.has_tokens(args.token_dir):
        return True
    print(
        f"Keine Anmeldung gefunden ({auth.token_file(args.token_dir)}). "
        "Zuerst `lastwerk-bridge auth` ausführen. Es wurde keine Anfrage gesendet.",
        file=err,
    )
    return False


def _require_no_lockout(state_dir: Path, clock: Clock, err: TextIO) -> bool:
    state = active_lockout(state_dir, clock)
    if state is None:
        return True
    print(
        f"Sperre aktiv bis {state.get('until')} (Zustand in {state_dir / 'lockout.json'}). "
        "Kein Aufruf. Später erneut versuchen, keine Umgehung.",
        file=err,
    )
    return False


def cmd_auth(
    args: argparse.Namespace,
    *,
    client_factory: ClientFactory,
    clock: Clock,
    out: TextIO,
    err: TextIO,
) -> int:
    state_dir = _state_dir(args)
    if not args.check:
        if not _require_no_lockout(state_dir, clock, err):
            return EXIT_LOCKOUT
        auth.interactive_login(args.token_dir)
        print(
            f"Anmeldung abgeschlossen. Token liegen in {auth.token_dir_path(args.token_dir)}.",
            file=out,
        )
        return EXIT_OK
    if not _require_tokens(args, err):
        return EXIT_USAGE
    if not _require_no_lockout(state_dir, clock, err):
        return EXIT_LOCKOUT
    client = client_factory(auth.token_dir_path(args.token_dir))
    caller = ThrottledCaller(
        client, state_dir=state_dir, config=_throttle_config(args), clock=clock
    )
    caller.call("get_user_profile")
    print("Anmeldung gültig (genau ein Aufruf: get_user_profile).", file=out)
    return EXIT_OK


def cmd_probe(
    args: argparse.Namespace,
    *,
    client_factory: ClientFactory,
    clock: Clock,
    out: TextIO,
    err: TextIO,
) -> int:
    if not _require_tokens(args, err):
        return EXIT_USAGE
    state_dir = _state_dir(args)
    if not _require_no_lockout(state_dir, clock, err):
        return EXIT_LOCKOUT
    client = client_factory(auth.token_dir_path(args.token_dir))
    caller = ThrottledCaller(
        client, state_dir=state_dir, config=_throttle_config(args), clock=clock
    )
    today = args.today or local_today(clock)
    out_dir = Path(args.out)
    out_dir.mkdir(parents=True, exist_ok=True)
    try:
        report = run_probe(caller, out_dir, today=today, lib_version=auth.library_version())
    finally:
        stats = caller.stats
        print(
            f"Aufrufe: {stats.calls}, ok: {stats.ok}, Fehler: {stats.errors}, "
            f"gesperrt: {stats.blocked}, Zeitlimit: {stats.timeouts}",
            file=out,
        )
    counts: dict[str, int] = {}
    for entry in report["entries"]:
        counts[entry["status"]] = counts.get(entry["status"], 0) + 1
    print(f"Bericht: {out_dir / 'probe-report.json'}; Status je Datenart: {counts}", file=out)
    print(
        "Die Rohantworten unter responses/ enthalten Gesundheitsdaten. Nicht weitergeben.", file=err
    )
    return EXIT_OK


def _exit_code_of(summary: runner.RunSummary) -> int:
    if summary.stop_reason == "lockout":
        return EXIT_LOCKOUT
    if summary.stop_reason == "auth":
        return EXIT_USAGE
    return EXIT_ERROR if summary.errors else EXIT_OK


def _run_archive_command(
    args: argparse.Namespace,
    *,
    client_factory: ClientFactory,
    clock: Clock,
    out: TextIO,
    err: TextIO,
) -> int:
    """Gemeinsamer Ablauf von ``backfill`` und ``sync`` (SPEC 5.5, Abrufregeln 4 und 8)."""
    if args.root is None:
        print(f"{args.command}: --root fehlt. Es wurde keine Anfrage gesendet.", file=err)
        return EXIT_USAGE
    archive = Archive(Path(args.root), lib_version=auth.library_version(), clock=clock)
    until = getattr(args, "until", None)
    refresh_days = getattr(args, "refresh_days", DEFAULT_REFRESH_DAYS)
    if args.command == "sync" and not archive.entries():
        print(
            f"sync: kein Archiv unter {args.root} (manifest.jsonl fehlt oder ist leer). "
            "Zuerst `backfill` ausführen. Es wurde keine Anfrage gesendet.",
            file=err,
        )
        return EXIT_USAGE
    if getattr(args, "dry_run", False):
        plan = runner.plan_run(archive, args.command, until=until, refresh_days=refresh_days)
        for line in runner.format_plan(plan, max_calls=args.max_calls):
            print(line, file=out)
        return EXIT_OK
    if not _require_tokens(args, err):
        return EXIT_USAGE
    if not _require_no_lockout(archive.state_dir, clock, err):
        return EXIT_LOCKOUT
    client = client_factory(auth.token_dir_path(args.token_dir))
    caller = ThrottledCaller(
        client,
        state_dir=archive.state_dir,
        config=_throttle_config(args),
        clock=clock,
        max_calls=args.max_calls,
    )
    archive.copy_registry()
    summary = runner.run(archive, caller, args.command, until=until, refresh_days=refresh_days)
    for line in runner.format_summary(summary, max_calls=args.max_calls):
        print(line, file=out)
    if summary.stop_message and summary.stop_reason in ("lockout", "auth"):
        print(summary.stop_message, file=err)
    return _exit_code_of(summary)


def cmd_coverage(
    args: argparse.Namespace, *, clock: Clock, out: TextIO, err: TextIO, **_: Any
) -> int:
    if args.root is None or not Path(args.root).is_dir():
        print(
            f"coverage: Archivordner nicht gefunden ({args.root}). Keine Anfrage gesendet.",
            file=err,
        )
        return EXIT_USAGE
    archive = Archive(Path(args.root), lib_version=auth.library_version(), clock=clock)
    report = build_coverage(archive)
    write_coverage(archive, report)
    for line in format_coverage(report):
        print(line, file=out)
    print(f"Bericht: {archive.root / 'coverage.json'}", file=out)
    return EXIT_OK


def _archive_for_files(args: argparse.Namespace, clock: Clock) -> Archive:
    """Archiv für Befehle ohne Garmin-Zugang. Die Quelle der Einträge ist dieses Programm."""
    return Archive(Path(args.root), lib_version=usb.lib_version(), clock=clock)


def cmd_import_device(
    args: argparse.Namespace, *, clock: Clock, out: TextIO, err: TextIO, **_: Any
) -> int:
    if args.root is None:
        print("import-device: --root fehlt. Es wurde nichts kopiert.", file=err)
        return EXIT_USAGE
    archive = _archive_for_files(args, clock)
    try:
        report = usb.import_device(Path(args.ordner), archive, device=args.device)
    except usb.DeviceImportError as exc:
        print(f"import-device: {exc} Es wurde nichts kopiert.", file=err)
        return EXIT_USAGE
    for line in usb.format_report(report):
        print(line, file=out)
    print(f"Bericht: {archive.root / DEVICE_IMPORT_REPORT_NAME}", file=out)
    return EXIT_OK


def cmd_reconcile(
    args: argparse.Namespace, *, clock: Clock, out: TextIO, err: TextIO, **_: Any
) -> int:
    if args.root is None or not Path(args.root).is_dir():
        print(f"reconcile: Archivordner nicht gefunden ({args.root}).", file=err)
        return EXIT_USAGE
    archive = _archive_for_files(args, clock)
    config = reconcile.MatchConfig(args.name_pattern, args.tolerance, args.name_offset_minutes)
    try:
        result = reconcile.run_reconcile(archive, config)
    except reconcile.ReconcileError as exc:
        print(f"reconcile: {exc}", file=err)
        return EXIT_USAGE
    report = reconcile.build_report(archive, result, config)
    reconcile.write_report(archive, report)
    for line in reconcile.format_report(report):
        print(line, file=out)
    print(f"Bericht: {archive.root / RECONCILE_REPORT_NAME}", file=out)
    return EXIT_OK


def cmd_normalize(
    args: argparse.Namespace, *, clock: Clock, out: TextIO, err: TextIO, **_: Any
) -> int:
    if args.root is None or not Path(args.root).is_dir():
        print(
            f"normalize: Archivordner nicht gefunden ({args.root}). Nichts geschrieben.", file=err
        )
        return EXIT_USAGE
    archive = _archive_for_files(args, clock)
    try:
        report = normalize.run_normalize(
            archive, Path(args.out), allow_sensitive=tuple(args.allow_sensitive)
        )
    except normalize.NormalizeError as exc:
        print(f"normalize: {exc} Nichts geschrieben.", file=err)
        return EXIT_USAGE
    for line in normalize.format_report(report):
        print(line, file=out)
    print(f"Bericht: {archive.root / NORMALIZE_REPORT_NAME}", file=out)
    return EXIT_ERROR if report.problems else EXIT_OK


def cmd_push(
    args: argparse.Namespace,
    *,
    client_factory: ClientFactory,
    clock: Clock,
    out: TextIO,
    err: TextIO,
) -> int:
    """``push``: Trockenlauf ohne Client, oder mit ``--yes`` der Schreibpfad (SPEC 12 M6)."""
    today = local_today(clock)
    from_date = args.from_date
    if args.yes and from_date is None:
        from_date = today  # vergangene Termine werden nicht in den Kalender gesetzt
    if args.yes and (args.out is None or args.max_workouts is None):
        print(
            "push --yes: --out (Ordner exchange/out) und --max-workouts N sind Pflicht, damit "
            "Protokoll und Umfang ausdrücklich feststehen. Es wurde nichts gesendet.",
            file=err,
        )
        return EXIT_USAGE
    try:
        plan = push.load_plan(Path(args.plan))
        templates_path = Path(args.templates) if args.templates else push.default_templates_path()
        templates = push.load_templates(templates_path)
        locales = Path(args.locales) if args.locales else push.default_locales_dir(templates_path)
        texts = push.load_locale(locales, plan["locale"])
        selection = push.select_workouts(
            plan,
            templates,
            texts,
            from_date=from_date,
            to_date=args.to_date,
            max_workouts=args.max_workouts,
        )
    except push.PushInputError as exc:
        print(f"push: {exc}. Es wurde nichts gesendet.", file=err)
        return EXIT_USAGE

    if not args.yes:
        document = push.dry_run_document(
            plan,
            selection,
            from_date=from_date,
            to_date=args.to_date,
            max_workouts=args.max_workouts,
            today=today,
        )
        print(json.dumps(document, indent=2, ensure_ascii=False), file=out)
        if args.out is not None:
            target = Path(args.out) / push.DRY_RUN_REPORT_NAME
            push.write_json(target, document)
            print(f"Trockenlauf geschrieben: {target}", file=err)
        print(
            f"Trockenlauf: {len(selection.workouts)} Einheiten, nichts gesendet. "
            "Mit --yes (und --out, --max-workouts) würde gesendet.",
            file=err,
        )
        return EXIT_OK

    if not selection.workouts:
        print("push: keine Einheit gewählt. Es wurde nichts gesendet.", file=err)
        return EXIT_OK
    report_path = Path(args.out) / push.PUSH_REPORT_NAME
    try:
        push.load_report(report_path)  # ein unbrauchbares Protokoll stoppt vor jeder Anfrage
    except push.PushInputError as exc:
        print(f"push: {exc}", file=err)
        return EXIT_USAGE
    if not _require_tokens(args, err):
        return EXIT_USAGE
    state_dir = _state_dir(args)
    if not _require_no_lockout(state_dir, clock, err):
        return EXIT_LOCKOUT
    print(
        "Achtung: schreibender Zugriff auf das Garmin-Konto über einen inoffiziellen Zugang "
        "(SPEC 14 O18). Struktur der Nutzlast ist bis zum ersten Versand eine Annahme.",
        file=err,
    )
    base = ThrottleConfig(min_interval_s=3.0, retries=0)
    overrides: dict[str, float] = {}
    if args.min_interval is not None:
        overrides["min_interval_s"] = args.min_interval
    if args.call_timeout is not None:
        overrides["call_timeout_s"] = args.call_timeout
    client = client_factory(auth.token_dir_path(args.token_dir))
    caller = push.WorkoutWriteCaller(
        client,
        state_dir=state_dir,
        config=ThrottleConfig(**{**base.__dict__, **overrides}),
        clock=clock,
        # Je Einheit höchstens zwei Aufrufe (anlegen, planen), keine Wiederholung.
        max_calls=2 * len(selection.workouts),
    )
    result = push.send_workouts(
        selection,
        caller,
        report_path,
        plan_id=plan["plan_id"],
        lib_version=auth.library_version(),
        clock=clock,
        line=lambda text: print(text, file=out),
    )
    for text in push.format_send_summary(result, requested=len(selection.workouts)):
        print(text, file=out)
    print(f"Protokoll: {report_path}", file=out)
    if result.stop_reason == "lockout":
        return EXIT_LOCKOUT
    if result.stop_reason == "auth":
        return EXIT_USAGE
    needs_attention = result.stop_reason == "error" or result.errors or result.blocked_unknown
    return EXIT_ERROR if needs_attention else EXIT_OK


COMMANDS: dict[str, Callable[..., int]] = {
    "auth": cmd_auth,
    "probe": cmd_probe,
    "backfill": _run_archive_command,
    "sync": _run_archive_command,
    "coverage": cmd_coverage,
    "import-device": cmd_import_device,
    "reconcile": cmd_reconcile,
    "normalize": cmd_normalize,
    "push": cmd_push,
}


def main(
    argv: list[str] | None = None,
    *,
    client_factory: ClientFactory | None = None,
    clock: Clock | None = None,
    stdout: TextIO | None = None,
    stderr: TextIO | None = None,
) -> int:
    out = stdout or sys.stdout
    err = stderr or sys.stderr
    parser = build_parser()
    try:
        args = parser.parse_args(argv)
    except SystemExit as exc:
        # argparse beendet mit 0 (--help, --version) oder 2 (Nutzungsfehler).
        return int(exc.code or 0)
    handler = COMMANDS[args.command]
    try:
        return handler(
            args,
            client_factory=client_factory or auth.login_with_tokens,
            clock=clock or SystemClock(),
            out=out,
            err=err,
        )
    except LockoutError as exc:
        print(f"Abbruch wegen Sperre: {exc}", file=err)
        return EXIT_LOCKOUT
    except AuthRequiredError as exc:
        print(f"Anmeldung ungültig: {exc}. `lastwerk-bridge auth` erneut ausführen.", file=err)
        return EXIT_USAGE
    except BridgeError as exc:
        print(f"Fehler: {exc}", file=err)
        return EXIT_ERROR
    except KeyboardInterrupt:
        print("Abgebrochen.", file=err)
        return EXIT_ERROR
    except Exception as exc:  # noqa: BLE001, letzte Stufe: Meldung statt Stacktrace
        print(f"Fehler ({type(exc).__name__}): {exc}", file=err)
        return EXIT_ERROR
