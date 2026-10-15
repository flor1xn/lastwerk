"""Manifest-Einträge und ihre Prüfung gegen ``schema/archive-manifest.schema.json``.

Die Prüfung ist eine eigene, kleine Umsetzung der in den Schemas verwendeten
Schlüsselwörter (``type``, ``const``, ``enum``, ``required``, ``additionalProperties``,
``properties``, ``items``, ``minItems``, ``maxItems``, ``maxLength``, ``minimum``,
``maximum``, ``pattern``, ``format`` für ``date``, ``date-time`` und ``uuid``, ``$ref``
innerhalb von ``$defs``, ``if``/``then``/``else``). Begründung: keine zusätzliche
Abhängigkeit, das Schema bleibt trotzdem die einzige Quelle, weil die Datei zur Laufzeit
gelesen wird. Die Schlüsselwörter ``SUPPORTED_KEYWORDS`` sind vollständig; ``unsupported_keywords``
findet Schlüsselwörter, die ein Schema nutzt und die hier nicht geprüft würden (Test), damit
eine Schemaänderung nicht unbemerkt ungeprüft bleibt.
"""

from __future__ import annotations

import json
import re
from dataclasses import asdict, dataclass
from datetime import datetime
from pathlib import Path
from typing import Any

SCHEMA_FILENAME = "archive-manifest.schema.json"

KINDS = ("account", "activity", "daily", "range", "device_usb")
STATUSES = ("ok", "empty", "not_available", "error")


class ManifestValidationError(ValueError):
    """Ein Manifest-Eintrag verletzt das Schema."""


def default_schema_path() -> Path:
    """``schema/archive-manifest.schema.json`` im Repository (bridge/ liegt daneben)."""
    return Path(__file__).resolve().parents[3] / "schema" / SCHEMA_FILENAME


def load_schema(path: Path | None = None) -> dict[str, Any]:
    path = path or default_schema_path()
    with open(path, encoding="utf-8") as fh:
        schema = json.load(fh)
    if not isinstance(schema, dict):
        raise ManifestValidationError(f"Schema ist kein Objekt: {path}")
    return schema


@dataclass(frozen=True)
class ManifestEntry:
    kind: str
    endpoint: str
    status: str
    fetched_at: str
    lib_version: str
    params: dict[str, Any] | None = None
    http_status: int | None = None
    path: str | None = None
    bytes: int | None = None
    sha256: str | None = None
    error_class: str | None = None

    def to_dict(self) -> dict[str, Any]:
        return {key: value for key, value in asdict(self).items() if value is not None}

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> ManifestEntry:
        return cls(**{key: data.get(key) for key in cls.__dataclass_fields__})

    @property
    def key(self) -> str:
        """Schlüssel der Fortsetzung: Art, Endpunkt und Parameter, kanonisch."""
        return make_key(self.kind, self.endpoint, self.params)


def make_key(kind: str, endpoint: str, params: dict[str, Any] | None) -> str:
    canonical = json.dumps(params or {}, sort_keys=True, separators=(",", ":"), default=str)
    return f"{kind}|{endpoint}|{canonical}"


_TYPE_CHECKS: dict[str, Any] = {
    "object": lambda v: isinstance(v, dict),
    "string": lambda v: isinstance(v, str),
    "integer": lambda v: isinstance(v, int) and not isinstance(v, bool),
    "number": lambda v: isinstance(v, (int, float)) and not isinstance(v, bool),
    "boolean": lambda v: isinstance(v, bool),
    "array": lambda v: isinstance(v, list),
    "null": lambda v: v is None,
}


_DATE_RE = re.compile(r"^\d{4}-\d{2}-\d{2}$")
_DATE_TIME_RE = re.compile(
    r"^\d{4}-\d{2}-\d{2}[Tt ]\d{2}:\d{2}:\d{2}(\.\d+)?([Zz]|[+-]\d{2}:\d{2})$"
)
_UUID_RE = re.compile(
    r"^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$"
)

# Schlüsselwörter, die ``validate_value`` auswertet, und Schlüssel ohne Prüfwirkung.
SUPPORTED_KEYWORDS: frozenset[str] = frozenset(
    {
        "type",
        "const",
        "enum",
        "required",
        "additionalProperties",
        "properties",
        "items",
        "minItems",
        "maxItems",
        "maxLength",
        "minimum",
        "maximum",
        "pattern",
        "format",
        "$ref",
        "if",
        "then",
        "else",
    }
)
ANNOTATION_KEYWORDS: frozenset[str] = frozenset({"$schema", "$id", "$defs", "title", "description"})
SUPPORTED_FORMATS: frozenset[str] = frozenset({"date", "date-time", "uuid"})


def unsupported_keywords(schema: Any, where: str = "$") -> list[str]:
    """Schlüsselwörter und Formate in ``schema``, die ``validate_value`` nicht prüft."""
    found: list[str] = []
    if isinstance(schema, dict):
        for key, sub in schema.items():
            if key not in SUPPORTED_KEYWORDS | ANNOTATION_KEYWORDS:
                # ``properties`` und ``$defs`` enthalten Namen, keine Schlüsselwörter.
                found.append(f"{where}.{key}")
            if key == "format" and sub not in SUPPORTED_FORMATS:
                found.append(f"{where}.format={sub}")
            if key in ("properties", "$defs") and isinstance(sub, dict):
                for name, child in sub.items():
                    found.extend(unsupported_keywords(child, f"{where}.{key}.{name}"))
            elif key in ("items", "if", "then", "else"):
                found.extend(unsupported_keywords(sub, f"{where}.{key}"))
    return found


def _json_equal(a: Any, b: Any) -> bool:
    """Gleichheit wie in JSON: ``true`` ist nicht ``1`` (Python setzt beides gleich)."""
    if isinstance(a, bool) != isinstance(b, bool):
        return False
    return bool(a == b)


def _resolve_ref(ref: str, root: dict[str, Any], where: str) -> dict[str, Any]:
    """Löst ``#/$defs/name`` im selben Schema auf. Andere Verweise sind nicht erlaubt."""
    prefix = "#/$defs/"
    if not ref.startswith(prefix):
        raise ManifestValidationError(f"{where}: nicht unterstützter Verweis {ref!r}")
    target = root.get("$defs", {}).get(ref[len(prefix) :])
    if not isinstance(target, dict):
        raise ManifestValidationError(f"{where}: Verweis {ref!r} nicht gefunden")
    return target


def _check_format(value: Any, fmt: str, where: str) -> None:
    if fmt == "uuid" and isinstance(value, str):
        if not _UUID_RE.match(value):
            raise ManifestValidationError(f"{where}: keine uuid: {value!r}")
    elif fmt == "date-time" and isinstance(value, str):
        try:
            if not _DATE_TIME_RE.match(value):
                raise ValueError(value)
            datetime.fromisoformat(value.replace("Z", "+00:00"))
        except ValueError as exc:
            raise ManifestValidationError(f"{where}: kein date-time: {value!r}") from exc
    elif fmt == "date" and isinstance(value, str):
        try:
            if not _DATE_RE.match(value):
                raise ValueError(value)
            datetime.strptime(value, "%Y-%m-%d")
        except ValueError as exc:
            raise ManifestValidationError(f"{where}: kein date: {value!r}") from exc


def validate_value(
    value: Any,
    schema: dict[str, Any],
    where: str = "$",
    root: dict[str, Any] | None = None,
) -> None:
    """Prüft ``value`` gegen die unterstützte Teilmenge von JSON Schema.

    ``root`` ist das Wurzelschema für ``$ref`` (Standard: ``schema`` selbst).
    """
    root = schema if root is None else root
    if "$ref" in schema:
        validate_value(value, _resolve_ref(schema["$ref"], root, where), where, root)
    if "const" in schema and not _json_equal(value, schema["const"]):
        raise ManifestValidationError(f"{where}: erwartet {schema['const']!r}, ist {value!r}")
    if "enum" in schema and not any(_json_equal(value, item) for item in schema["enum"]):
        raise ManifestValidationError(f"{where}: {value!r} nicht in {schema['enum']}")
    expected = schema.get("type")
    if expected is not None:
        types = expected if isinstance(expected, list) else [expected]
        if not any(_TYPE_CHECKS[t](value) for t in types):
            raise ManifestValidationError(f"{where}: Typ {expected} erwartet, ist {value!r}")
    if isinstance(value, str):
        if "maxLength" in schema and len(value) > schema["maxLength"]:
            raise ManifestValidationError(f"{where}: länger als {schema['maxLength']}")
        if "pattern" in schema and not re.search(schema["pattern"], value):
            raise ManifestValidationError(f"{where}: passt nicht zu {schema['pattern']}")
        if "format" in schema:
            _check_format(value, schema["format"], where)
    if isinstance(value, (int, float)) and not isinstance(value, bool):
        if "minimum" in schema and value < schema["minimum"]:
            raise ManifestValidationError(f"{where}: kleiner als {schema['minimum']}")
        if "maximum" in schema and value > schema["maximum"]:
            raise ManifestValidationError(f"{where}: größer als {schema['maximum']}")
    if isinstance(value, list):
        if "minItems" in schema and len(value) < schema["minItems"]:
            raise ManifestValidationError(f"{where}: weniger als {schema['minItems']} Einträge")
        if "maxItems" in schema and len(value) > schema["maxItems"]:
            raise ManifestValidationError(f"{where}: mehr als {schema['maxItems']} Einträge")
        if isinstance(schema.get("items"), dict):
            for index, item in enumerate(value):
                validate_value(item, schema["items"], f"{where}[{index}]", root)
    if isinstance(value, dict):
        properties = schema.get("properties", {})
        for name in schema.get("required", []):
            if name not in value:
                raise ManifestValidationError(f"{where}: Pflichtfeld {name} fehlt")
        if schema.get("additionalProperties") is False:
            extra = sorted(set(value) - set(properties))
            if extra:
                raise ManifestValidationError(f"{where}: unbekannte Felder {extra}")
        for name, sub in properties.items():
            if name in value:
                validate_value(value[name], sub, f"{where}.{name}", root)
    if "if" in schema:
        try:
            validate_value(value, schema["if"], where, root)
            condition = True
        except ManifestValidationError:
            condition = False
        branch = schema.get("then" if condition else "else")
        if isinstance(branch, dict):
            validate_value(value, branch, where, root)


def validate_entry(data: dict[str, Any], schema: dict[str, Any] | None = None) -> None:
    validate_value(data, schema or load_schema())
