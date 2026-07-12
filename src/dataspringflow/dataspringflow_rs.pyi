from __future__ import annotations
from typing import Any

class DatasetStatus:
    Healthy: DatasetStatus
    Broken: DatasetStatus
    BrokenDeps: DatasetStatus
    Unverified: DatasetStatus

class DataSetVerifyRes:
    status: DatasetStatus
    dep_status: list[DatasetStatus]

    def __init__(
        self, status: DatasetStatus, dep_status: list[DatasetStatus]
    ) -> None: ...

class MetaData:
    name: str
    tag: str
    hash: str
    path: str
    description_path: str
    script_path: str
    dependencies: list[str]
    merkle_tree_path: str

    def id(self) -> str: ...
    def __repr__(self) -> str: ...

class BackendAddr:
    @staticmethod
    def private(
        username: str | None = None,
    ) -> BackendAddr: ...
    @staticmethod
    def local_global() -> BackendAddr: ...
    @staticmethod
    def remote_global(server_url: str) -> BackendAddr: ...

class ScopedMetaData:
    @property
    def backend(self) -> BackendAddr: ...
    @property
    def metadata(self) -> MetaData: ...

class ScopedId:
    @property
    def backend(self) -> BackendAddr: ...
    @property
    def id(self) -> str: ...

class DSFDataset:
    @property
    def metadata(self) -> MetaData: ...
    @property
    def detailed_status(self) -> DataSetVerifyRes: ...
    def verify(
        self, _backend_auth: Any, _show_diff: bool = ...
    ) -> DataSetVerifyRes: ...
    def __repr__(self) -> str: ...

class DSFService:
    def __init__(self) -> None: ...
    def query_meta(self, id: str) -> MetaData: ...
    def register(
        self,
        name: str,
        tag: str,
        path: str,
        script_path: str,
        dependencies: list[str] | None = ...,
        description_path: str | None = ...,
        force_heal: bool = ...,
        yes: bool = ...,
    ) -> None: ...
    def update_merkle(self, id: str) -> None: ...
    def delete_metadata(self, id: str, force: bool = ...) -> None: ...
    def verify_deep(self, id: str, show_diff: bool = ...) -> DataSetVerifyRes: ...
    def verify_self(self, id: str, show_diff: bool = ...) -> DataSetVerifyRes: ...
    def list_all_metadata(self) -> list[MetaData]: ...
    def check_is_referenced(self, target_id: str) -> list[str]: ...
