from __future__ import annotations

import uuid
from pathlib import Path

import pytest
from dataspringflow import DSFService, DatasetStatus


def _create_dummy_dataset(base_dir: Path, folder_name: str, file_content: str) -> Path:
    ds_path = base_dir / folder_name
    ds_path.mkdir(parents=True, exist_ok=True)
    (ds_path / "data.txt").write_text(file_content, encoding="utf-8")
    return ds_path


def _create_dummy_req_files(base_dir: Path, name: str) -> tuple[Path, Path]:
    script_path = base_dir / f"{name}_script.py"
    desc_path = base_dir / f"{name}_desc.md"
    script_path.write_text("# dummy script\n", encoding="utf-8")
    desc_path.write_text(f"# {name}\n", encoding="utf-8")
    return script_path, desc_path


@pytest.fixture
def service() -> DSFService:
    return DSFService()


@pytest.fixture
def sandbox(tmp_path: Path) -> Path:
    return tmp_path


def _register(
    service: DSFService,
    sandbox: Path,
    name: str,
    tag: str,
    dependencies: list[str] | None = None,
) -> str:
    ds_path = _create_dummy_dataset(sandbox, name, f"{name} content")
    script_path, desc_path = _create_dummy_req_files(sandbox, name)

    service.register(
        name=name,
        tag=tag,
        path=str(ds_path),
        script_path=str(script_path),
        dependencies=dependencies or [],
        description_path=str(desc_path),
        force_heal=False,
        yes=False,
    )
    return f"{name}@{tag}"


def test_check_is_referenced(service: DSFService, sandbox: Path) -> None:
    suffix = uuid.uuid4().hex[:8]

    base = _register(service, sandbox, f"py_base_{suffix}", "v1")
    d1 = _register(service, sandbox, f"py_d1_{suffix}", "v1", [base])
    d2 = _register(service, sandbox, f"py_d2_{suffix}", "v1", [base])

    refs = sorted(service.check_is_referenced(base))
    assert refs == sorted([d1, d2])


def test_list_all_metadata_contains_registered(
    service: DSFService, sandbox: Path
) -> None:
    suffix = uuid.uuid4().hex[:8]

    a = _register(service, sandbox, f"py_list_a_{suffix}", "v1")
    b = _register(service, sandbox, f"py_list_b_{suffix}", "v1")

    ids = sorted([m.id() for m in service.list_all_metadata()])
    assert a in ids
    assert b in ids


def test_update_merkle_success(service: DSFService, sandbox: Path) -> None:
    suffix = uuid.uuid4().hex[:8]

    ds_id = _register(service, sandbox, f"py_um_{suffix}", "v1")
    # 不抛异常即成功
    service.update_merkle(ds_id)


def test_verify_self_success(service: DSFService, sandbox: Path) -> None:
    suffix = uuid.uuid4().hex[:8]

    ds_id = _register(service, sandbox, f"py_vs_{suffix}", "v1")
    res = service.verify_self(ds_id, show_diff=False)

    assert res is not None
    assert res.status in (
        DatasetStatus.Healthy,
        DatasetStatus.Broken,
        DatasetStatus.BrokenDeps,
        DatasetStatus.Unverified,
    )


def test_verify_deep_success_with_dependency(
    service: DSFService, sandbox: Path
) -> None:
    suffix = uuid.uuid4().hex[:8]

    base = _register(service, sandbox, f"py_vd_base_{suffix}", "v1")
    derived = _register(service, sandbox, f"py_vd_derived_{suffix}", "v1", [base])

    res = service.verify_deep(derived, show_diff=False)

    assert res is not None
    assert res.status in (
        DatasetStatus.Healthy,
        DatasetStatus.Broken,
        DatasetStatus.BrokenDeps,
        DatasetStatus.Unverified,
    )
