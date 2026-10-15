"""Register: Datei und Modul stimmen überein, Klassen nach SPEC 5.5."""

from __future__ import annotations

from lastwerk_bridge.registry import REGISTRY, registry_yaml_path, to_yaml


def test_registry_yaml_matches_module() -> None:
    path = registry_yaml_path()
    assert path.is_file(), path
    assert path.read_text(encoding="utf-8") == to_yaml(), (
        "bridge/registry.yaml neu erzeugen: "
        'uv run python -c "from lastwerk_bridge.registry import to_yaml; '
        "open('registry.yaml','w',encoding='utf-8').write(to_yaml())\""
    )


def test_names_are_unique_and_carry_probe_status() -> None:
    names = [e.name for e in REGISTRY]
    assert len(names) == len(set(names))
    for entry in REGISTRY:
        assert entry.status in ("ok", "empty", "error")
        if entry.granularity != "range":
            assert entry.max_span_days is None


def test_sensitive_classes_have_categories() -> None:
    s3 = [e for e in REGISTRY if e.sensitivity == "S3"]
    assert {e.category for e in s3} == {
        "weight",
        "blood_pressure",
        "hydration",
        "nutrition",
        "lifestyle",
    }
    for entry in REGISTRY:
        if entry.sensitivity != "S3":
            assert entry.category is None
        assert "menstrual" not in entry.method and "pregnancy" not in entry.method


def test_core_data_kinds_are_present() -> None:
    names = {e.name for e in REGISTRY}
    for required in ("profile", "activities_list", "summary", "original", "sleep", "hrv", "rhr"):
        assert required in names
