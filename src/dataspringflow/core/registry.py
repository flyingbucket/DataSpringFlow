from __future__ import annotations
from pathlib import Path
from typing import Any, Union
from ..protocols import HashSnapshot, RegistryFactory
from ..factory import get_registry_factory
from .metadata import Metadata
from .dataset import DSFdataset
from .dag import DAG


def parse_id(id_str: str) -> tuple[str, str]:
    """
    将 id 字符串 'name@tag' 解析为 (name, tag)
    """
    if "@" not in id_str:
        raise ValueError(f"Invalid id string: {id_str}")
    name, tag = id_str.split("@", 1)  # 只分一次，防止 name 中有 @
    return name, tag


class DataRegistry:
    def __init__(
        self, backend: str = "yaml", backend_conf: Union[dict[str, Any], None] = None
    ) -> None:
        self.backend = backend
        self.backend_conf = backend_conf or {}
        factory: RegistryFactory = get_registry_factory(self.backend, self.backend_conf)
        self._metadata_loader = factory.create_metadata_loader()
        self._metadata_writer = factory.create_metadata_writer()
        self._hash_loader = factory.create_hash_loader()
        self._hash_writer = factory.create_hash_writer()

    def get_info(self, id: str) -> Metadata:
        name, tag = parse_id(id)
        return self._metadata_loader.load(name, tag)

    def get_hashDict(self, name: str, tag: str) -> HashSnapshot:
        return self._hash_loader.load(name, tag)

    def get(self, id: str) -> DSFdataset:
        metadata = self.get_info(id)
        dag = DAG(id, self)
        return DSFdataset(metadata, dag)
