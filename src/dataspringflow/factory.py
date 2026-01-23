from typing import Any
from dataspringflow.protocols import (
    RegistryFactory,
    MetadataLoader,
    MetadataWriter,
    HashDictLoader,
    HashDictWriter,
)
from dataspringflow.core.metadata import Metadata
from pathlib import Path


# 空实现的 Loader/Writer 占位
class DummyMetadataLoader(MetadataLoader):
    def load(self, name: str, tag: str) -> Metadata:
        raise NotImplementedError("Dummy loader: backend not implemented yet")


class DummyMetadataWriter(MetadataWriter):
    def save(self, metadata: Metadata) -> int:
        raise NotImplementedError("Dummy writer: backend not implemented yet")


class DummyHashLoader(HashDictLoader):
    def load(self, name: str, tag: str) -> dict[Path, str]:
        raise NotImplementedError("Dummy hash loader: backend not implemented yet")


class DummyHashWriter(HashDictWriter):
    def save(self, name: str, tag: str) -> int:
        raise NotImplementedError("Dummy hash writer: backend not implemented yet")


# 空实现工厂
class DummyRegistryFactory(RegistryFactory):
    def __init__(self, config: dict[str, Any]) -> None:
        self.config = config

    def create_metadata_loader(self) -> MetadataLoader:
        return DummyMetadataLoader()

    def create_metadata_writer(self) -> MetadataWriter:
        return DummyMetadataWriter()

    def create_hash_loader(self) -> HashDictLoader:
        return DummyHashLoader()

    def create_hash_writer(self) -> HashDictWriter:
        return DummyHashWriter()


# 工厂函数占位
def get_registry_factory(backend: str, backend_conf: dict[str, Any]) -> RegistryFactory:
    # 暂时返回 Dummy 实现
    return DummyRegistryFactory(backend_conf)
