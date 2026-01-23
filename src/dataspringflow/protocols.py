from typing import Protocol, Iterable
from pathlib import Path
from dataspringflow.core.metadata import Metadata


class HashSnapshot(Protocol):
    def items(self) -> Iterable[tuple[Path, str]]: ...


class MetadataLoader(Protocol):
    def load(self, name: str, tag: str) -> Metadata: ...


class MetadataWriter(Protocol):
    def save(self, name: str, tag: str) -> int: ...


class HashDictLoader(Protocol):
    def load(self, name: str, tag: str) -> HashSnapshot: ...


class HashDictWriter(Protocol):
    def save(self, name: str, tag: str) -> int: ...


class RegistryFactory(Protocol):
    def create_metadata_loader(self) -> MetadataLoader: ...
    def create_metadata_writer(self) -> MetadataWriter: ...
    def create_hash_loader(self) -> HashDictLoader: ...
    def create_hash_writer(self) -> HashDictWriter: ...
