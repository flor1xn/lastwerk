"""Synthetische Geräteordner für die Tests von import-device und reconcile.

Es liegen keine ``*.fit``-Dateien im Index (SPEC 1.5, 11.4). Die Dateien entstehen
zur Laufzeit in einem temporären Verzeichnis. Der Inhalt ist ein minimaler FIT-Kopf
(14 Byte, Kennung ``.FIT``, Kopf-CRC), wenige Datenbytes und die Datei-CRC. Die
Bridge parst FIT nicht, sie prüft nur den Kopf. Die Bytes sind aus einem Startwert
abgeleitet und erfunden.
"""

from __future__ import annotations

import struct
from pathlib import Path

# CRC-16 des FIT-SDK (nibbleweise Tabelle).
_CRC_TABLE = (
    0x0000, 0xCC01, 0xD801, 0x1400, 0xF001, 0x3C00, 0x2800, 0xE401,
    0xA001, 0x6C00, 0x7800, 0xB401, 0x5000, 0x9C01, 0x8801, 0x4400,
)  # fmt: skip


def fit_crc(data: bytes) -> int:
    crc = 0
    for byte in data:
        tmp = _CRC_TABLE[crc & 0xF]
        crc = (crc >> 4) & 0x0FFF
        crc = crc ^ tmp ^ _CRC_TABLE[byte & 0xF]
        tmp = _CRC_TABLE[crc & 0xF]
        crc = (crc >> 4) & 0x0FFF
        crc = crc ^ tmp ^ _CRC_TABLE[(byte >> 4) & 0xF]
    return crc


def fake_fit(seed: str | int, *, body_size: int = 24) -> bytes:
    """Minimale FIT-Datei: Kopf (14 Byte) + ``body_size`` Datenbytes + Datei-CRC."""
    stream = (str(seed).encode() * body_size)[:body_size]
    body = bytes((b * 7 + i) % 256 for i, b in enumerate(stream))
    head = struct.pack("<BBHI4s", 14, 0x20, 2100, len(body), b".FIT")
    head += struct.pack("<H", fit_crc(head))
    return head + body + struct.pack("<H", fit_crc(head + body))


def write(path: Path, data: bytes) -> Path:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(data)
    return path


def make_device(
    base: Path,
    *,
    activity: tuple[str, ...] = (),
    monitor: tuple[str, ...] = (),
    sleep: tuple[str, ...] = (),
    vendor_dir: str | None = None,
) -> Path:
    """Legt ``Activity``, ``Monitor`` und ``Sleep`` an, wahlweise unter ``GARMIN``.

    Die Namen sind Dateinamen, der Inhalt entsteht aus dem Namen.
    """
    root = base / "DEVICE"
    parent = root / vendor_dir if vendor_dir else root
    for folder, names in (("Activity", activity), ("Monitor", monitor), ("Sleep", sleep)):
        for name in names:
            write(parent / folder / name, fake_fit(f"{folder}/{name}"))
    parent.mkdir(parents=True, exist_ok=True)
    return root
