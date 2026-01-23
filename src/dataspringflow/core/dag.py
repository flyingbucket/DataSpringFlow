from __future__ import annotations
from typing import Generator, Union

from dataspringflow.core.metadata import Metadata

from .merkle import FileMerkleTree
from .registry import DSFRegistry


class DAG:
    def __init__(self, root: str, registry: DSFRegistry) -> None:
        self.root = root
        self._regi = registry

    def _iter_deps(
        self, root_id: str, visited: Union[set[str], None] = None
    ) -> Generator[Metadata]:
        if visited is None:
            visited = set()

        root = self._regi.get_info(root_id)
        for d_id in root.dependencies:
            if d_id in visited:
                continue
            visited.add(d_id)
            dep = self._regi.get_info(d_id)
            yield dep
            yield from self._iter_deps(dep.id, visited)

    def iter_DAG(self):
        return self._iter_deps(self.root)

    def verify(
        self, *, size_threshold: int = 100 * 1024, max_workers: int = 16
    ) -> tuple[bool, list[str]]:
        broken_datasets: list[str] = []
        for dep in self.iter_DAG():
            merkle = FileMerkleTree(dep.path)
            current_hash = merkle.get_hash(
                size_threshold=size_threshold, max_workers=max_workers
            )
            if current_hash != dep.hash:
                broken_datasets.append(dep.id)
        if len(broken_datasets) > 0:
            return False, broken_datasets
        else:
            return True, []
