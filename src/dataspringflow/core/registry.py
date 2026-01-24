from __future__ import annotations
import asyncio
import warnings
from typing import Any, Union, Optional
from ..protocols import HashSnapshot, RegistryFactory
from ..factory import get_registry_factory
from ..utils import parse_id, get_running_environment
from .metadata import Metadata
from .dataset import DSFdataset


class DSFRegistry:
    def __init__(
        self, backend: str = "yaml", backend_conf: Union[dict[str, Any], None] = None
    ) -> None:
        self.backend = backend
        self.backend_conf = backend_conf or {}
        factory: RegistryFactory = get_registry_factory(self.backend, self.backend_conf)
        self._metadata_loader = factory.create_metadata_loader()
        self._hash_loader = factory.create_hash_loader()
        # self._metadata_writer = factory.create_metadata_writer()
        # self._hash_writer = factory.create_hash_writer()
        self._atomic_writer = factory.create_atomic_writer()

    def get_info(self, id: str) -> Metadata:
        name, tag = parse_id(id)
        return self._metadata_loader.load(name, tag)

    def get_hash_snapshot(self, id: str) -> HashSnapshot:
        name, tag = parse_id(id)
        return self._hash_loader.load(name, tag)

    def get(self, id: str) -> DSFdataset:
        from .dag import DAG

        metadata = self.get_info(id)
        dag = DAG(id, self)
        return DSFdataset(metadata, dag)

    def save(
        self, metadata: Metadata, full_hash: Optional[HashSnapshot] = None
    ) -> bool:
        """
        同步版本：给普通用户 / CLI 用
        - 在普通 Python 脚本中调用安全
        - 在 async 环境（如 Jupyter）中会抛异常，提醒使用 save_async
        """
        try:
            _ = asyncio.get_running_loop()
        except RuntimeError:
            # 没有 event loop，可以安全同步调用
            # 通过线程封装 atomic_writer.save 保证不会阻塞 event loop
            return asyncio.run(self.save_async(metadata, full_hash))
        else:
            warnings.warn(
                "DSFRegistry.save() is called from an async context. \n"
                + f"Seems like you are running in {get_running_environment()}\n"
                + "This will block the running event loop. \n"
                + "Prefer `await save_async()`.\n",
                stacklevel=2,
            )
            return self._atomic_writer.save(metadata, full_hash)

    async def save_async(
        self, metadata: Metadata, full_hash: Optional[HashSnapshot] = None
    ) -> bool:
        """
        异步版本：给 async 用户用
        - 内部使用 asyncio.to_thread 调用原子写入
        """
        result = await asyncio.to_thread(self._atomic_writer.save, metadata, full_hash)
        return result
