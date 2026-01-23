from __future__ import annotations

from dataspringflow.core.dag import DAG
from ..protocols import HashSnapshot
from .merkle import FileMerkleTree
from .metadata import Metadata
from .dag import DAG
from .hash_utils import HashDiff, compute_hash_diff
from ..backend.hash_io import HashLoader


class DSFdataset:
    """
    面向用户的dataset接口，registry按name@tag查询后返回此类的实例，而不是在用户代码中完成实例化

    功能：
        - 以只读方式获取数据集metadata各属性
        - 校验数据集哈希
        - 计算并解析依赖图
        - 获取上游依赖数据集的健康状况
    """

    def __init__(self, metadata: Metadata, dag: DAG) -> None:
        self.info = metadata
        self.DAG = dag

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
            old_snapshot = HashLoader().load(name=self.info.name, tag=self.info.tag)
            diff = compute_hash_diff(old_snapshot, merkle)
            return (False, diff)
