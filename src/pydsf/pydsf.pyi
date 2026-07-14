from __future__ import annotations
from typing import Any, List, Optional

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
    def __init__(self) -> None:
        """Initializes the service by automatically detecting and building the backend."""
        ...

    def query_meta(
        self, id: str, target_backend: Optional[BackendAddr] = None
    ) -> List[ScopedMetaData]:
        """Query metadata for a specific dataset ID (e.g., "imagenet@v1.0")"""
        ...

    def register(
        self,
        name: str,
        tag: str,
        path: str,
        script_path: str,
        owner_nickname: Optional[str] = None,
        dependencies: Optional[List[str]] = None,
        description_path: Optional[str] = None,
        target_backend: Optional[BackendAddr] = None,
        force_heal: bool = False,
    ) -> None:
        """Register a new dataset with full options"""
        ...

    def update_merkle(
        self, id: str, target_backend: Optional[BackendAddr] = None
    ) -> None:
        """Update merkle tree hash for a dataset"""
        ...

    def delete_metadata(
        self,
        id: str,
        force: bool = False,
        target_backend: Optional[BackendAddr] = None,
    ) -> None:
        """Delete dataset metadata from the global database"""
        ...

    def verify_deep(
        self,
        id: str,
        show_diff: bool = False,
        target_backend: Optional[BackendAddr] = None,
    ) -> DataSetVerifyRes:
        """Perform deep verification (includes dependencies and DAG topological check)"""
        ...

    def verify_self(
        self,
        id: str,
        show_diff: bool = False,
        target_backend: Optional[BackendAddr] = None,
    ) -> DataSetVerifyRes:
        """Perform single verification (checks only the target dataset, ignoring dependencies)"""
        ...

    def list_all_metadata(self) -> List[ScopedMetaData]:
        """List all metadata registered on this machine"""
        ...

    def check_is_referenced(self, target_id: str) -> List[ScopedId]:
        """List all datasets that depend on <target_id>"""
        ...
