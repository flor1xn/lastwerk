"""Anmeldung und Token-Prüfung (SPEC 5.5 Regel 6, SPEC 13).

Token liegen im Benutzerprofil (``~/.garminconnect/garmin_tokens.json``), nie im
Projektordner. Zugangsdaten stehen in keiner Datei. Dieses Modul ist der einzige
Ort, der ``login`` der Bibliothek aufruft. Der gedrosselte Aufrufer kennt ``login``
nur als gesperrte Methode.

Die interaktive Anmeldung (``auth`` ohne ``--check``) wird erst in L1 gegen das
echte Konto ausgeführt und ist hier nicht getestet.
"""

from __future__ import annotations

import getpass
from collections.abc import Callable
from importlib import metadata
from pathlib import Path
from typing import Any

TOKEN_FILE = "garmin_tokens.json"
DEFAULT_TOKEN_DIR = "~/.garminconnect"


def library_version() -> str:
    try:
        return metadata.version("garminconnect")
    except metadata.PackageNotFoundError:  # pragma: no cover
        return "unknown"


def token_dir_path(token_dir: str | Path | None) -> Path:
    return Path(token_dir or DEFAULT_TOKEN_DIR).expanduser()


def token_file(token_dir: str | Path | None) -> Path:
    return token_dir_path(token_dir) / TOKEN_FILE


def has_tokens(token_dir: str | Path | None) -> bool:
    """Wahr, wenn eine frühere Anmeldung Token hinterlassen hat. Keine Netzwerkanfrage."""
    return token_file(token_dir).is_file()


def login_with_tokens(token_dir: str | Path | None) -> Any:
    """Erzeugt den Bibliotheks-Client aus gespeicherten Token.

    Die Bibliothek erneuert ein bald ablaufendes Token dabei selbst (eine
    Anfrage an den Token-Endpunkt), eine Anmeldung mit Zugangsdaten findet nicht
    statt, weil keine übergeben werden.
    """
    from garminconnect import Garmin

    client = Garmin()
    client.login(str(token_dir_path(token_dir)))
    return client


def interactive_login(
    token_dir: str | Path | None,
    *,
    input_fn: Callable[[str], str] = input,
    password_fn: Callable[[str], str] = getpass.getpass,
) -> Any:
    """Einmalige Anmeldung mit MFA. Legt die Token im Token-Verzeichnis ab."""
    from garminconnect import Garmin

    email = input_fn("Garmin-E-Mail: ").strip()
    password = password_fn("Garmin-Passwort: ")
    client = Garmin(email, password, prompt_mfa=lambda: input_fn("MFA-Code: ").strip())
    directory = token_dir_path(token_dir)
    directory.mkdir(mode=0o700, parents=True, exist_ok=True)
    client.login(str(directory))
    return client
