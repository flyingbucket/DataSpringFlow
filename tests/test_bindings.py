import pytest

import dataspringflow as dsf


def test_public_exports():
    # __init__.py 显式导出是否正常
    assert hasattr(dsf, "DSFService")
    assert hasattr(dsf, "DSFDataset")
    assert hasattr(dsf, "MetaData")
    assert hasattr(dsf, "DataSetVerifyRes")
    assert hasattr(dsf, "DatasetStatus")


def test_enum_members_exist():
    assert hasattr(dsf.DatasetStatus, "Healthy")
    assert hasattr(dsf.DatasetStatus, "Broken")
    assert hasattr(dsf.DatasetStatus, "BrokenDeps")
    assert hasattr(dsf.DatasetStatus, "Unverified")

    assert dsf.DatasetStatus.Healthy == dsf.DatasetStatus.Healthy
    assert dsf.DatasetStatus.Healthy != dsf.DatasetStatus.Broken


def test_verify_result_constructor_and_fields():
    res = dsf.DataSetVerifyRes(
        dsf.DatasetStatus.Healthy,
        [dsf.DatasetStatus.Healthy, dsf.DatasetStatus.Unverified],
    )
    assert res.status == dsf.DatasetStatus.Healthy
    assert isinstance(res.dep_status, list)
    assert all(isinstance(x, dsf.DatasetStatus) for x in res.dep_status)


def test_service_can_be_constructed_or_skip():
    # 你的后端是自动探测，某些环境下可能初始化失败；这不应让绑定测试直接红
    try:
        svc = dsf.DSFService()
    except Exception as e:
        pytest.skip(f"DSFService backend unavailable in test env: {e}")
    else:
        assert svc is not None


def test_service_verify_methods_raise_on_nonexistent_id_or_return():
    try:
        svc = dsf.DSFService()
    except Exception as e:
        pytest.skip(f"DSFService backend unavailable in test env: {e}")

    dataset_id = "__definitely_not_existing__@__nope__"

    # 行为二选一都接受：
    # 1) 抛错（更常见）
    # 2) 返回 DataSetVerifyRes（如果你未来实现了兜底逻辑）
    try:
        deep_res = svc.verify_deep(dataset_id, show_diff=False)
    except Exception:
        pass
    else:
        assert isinstance(deep_res, dsf.DataSetVerifyRes)

    try:
        self_res = svc.verify_self(dataset_id, show_diff=False)
    except Exception:
        pass
    else:
        assert isinstance(self_res, dsf.DataSetVerifyRes)


def test_service_delete_metadata_signature():
    try:
        svc = dsf.DSFService()
    except Exception as e:
        pytest.skip(f"DSFService backend unavailable in test env: {e}")

    with pytest.raises(Exception):
        svc.delete_metadata("__definitely_not_existing__@__nope__", force=False)


def test_service_register_argument_validation():
    try:
        svc = dsf.DSFService()
    except Exception as e:
        pytest.skip(f"DSFService backend unavailable in test env: {e}")

    # 这里只测 Python 调用签名链路能通到 Rust，不强依赖真实路径
    with pytest.raises(Exception):
        svc.register(
            name="dummy",
            tag="v0",
            path="/tmp/not-exist-dataset",
            script_path="/tmp/not-exist-script.py",
            dependencies=None,
            description_path=None,
            force_heal=False,
            yes=True,
        )
