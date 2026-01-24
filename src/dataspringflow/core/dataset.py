from __future__ import annotations
from typing import TYPE_CHECKING

from ..protocols import HashSnapshot
from .merkle import FileMerkleTree
from .metadata import Metadata

from ..utils import HashDiff, compute_hash_diff

if TYPE_CHECKING:
    from .dag import DAG
    from .registry import DSFRegistry


class DSFdataset:
    """
    面向用户的dataset接口，registry按name@tag查询后返回此类的实例，而不是在用户代码中完成实例化

    功能：
        - 以只读方式获取数据集metadata各属性
        - 校验数据集哈希
        - 获取依赖图
        - 获取上游依赖数据集的健康状况
    """

    def __init__(self, metadata: Metadata, dag: DAG, registry: DSFRegistry) -> None:
        self.info = metadata
        self.DAG = dag
        self._regi = registry

    def verify(
        self, *, size_threshold: int = 100 * 1024, max_workers: int = 16
    ) -> tuple[bool, HashDiff]:
        merkle = FileMerkleTree(self.info.path)
        current_hash = merkle.get_hash(
            size_threshold=size_threshold, max_workers=max_workers
        )
        if self.info.hash == current_hash:
            return (True, HashDiff(set(), set(), set()))
        else:
            old_snapshot: HashSnapshot = self._regi._hash_loader.load(
                self.info.name, self.info.tag
            )
            diff = compute_hash_diff(old_snapshot, merkle)
            return (False, diff)
