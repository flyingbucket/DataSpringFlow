from __future__ import annotations
from typing import TYPE_CHECKING, Optional, Protocol, Iterable
from pathlib import Path

if TYPE_CHECKING:
    from .core.metadata import Metadata


class HashSnapshot(Protocol):
    def items(self) -> Iterable[tuple[Path, str]]: ...


class MetadataLoader(Protocol):
    def load(self, name: str, tag: str) -> Metadata: ...


class HashDictLoader(Protocol):
    def load(self, name: str, tag: str) -> HashSnapshot: ...


class AtomicWriter(Protocol):
    def save(
        self, metadata: Metadata, hash_snapshot: Optional[HashSnapshot] = None
    ) -> bool:
        """
        原子保存：
        - 写入 metadata 和 hash_dict
        - 成功返回 True
        - 如果任何步骤失败，应回滚已写入数据
        """
        ...

    def rollback(self, metadata: Metadata) -> None:
        """
        回滚已写入的数据
        """
        ...


class RegistryFactory(Protocol):
    def create_metadata_loader(self) -> MetadataLoader: ...
    def create_hash_loader(self) -> HashDictLoader: ...
    def create_atomic_writer(self) -> AtomicWriter: ...
