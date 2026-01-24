from __future__ import annotations

import hashlib
import asyncio

from pathlib import Path
from functools import cached_property
from typing import Dict, Iterable, List, Generator
from concurrent.futures import ThreadPoolExecutor, as_completed

from ..utils import hash_file
from ..utils import walkdir


class Node:
    def __init__(self, path: Path, root_path: Path) -> None:
        self.path = path
        self.root_path = root_path
        self.childs: List[Node] = []

    def add_child(self, node: Node) -> None:
        self.childs.append(node)

    @cached_property
    def hash(self) -> str:
        if len(self.childs) == 0:
            if self.path.is_file():  # leaf node if a file
                return hash_file(self.path, self.root_path)
            else:  # leaf node if an empty folder
                return hashlib.md5(
                    self.path.relative_to(self.root_path).as_posix().encode("utf-8")
                ).hexdigest()
        h = hashlib.md5()
        for child in sorted(self.childs, key=lambda x: x.path.relative_to(self.path)):
            h.update(child.hash.encode(encoding="utf-8"))
        return h.hexdigest()


class FileMerkleTree:
    def __init__(self, root_path: Path) -> None:
        self.root_path = root_path
        self.root_node: Node = self._build()
        self._hash_cache: dict[int, str] = {}

    def _build(self) -> Node:
        nodes: Dict[Path, Node] = {}
        for path, dirs, files in walkdir(self.root_path):
            node = nodes.setdefault(path, Node(path, self.root_path))
            for dir in dirs:
                child = nodes.setdefault(path / dir, Node(path / dir, self.root_path))
                node.add_child(child)
            for file in files:
                child = nodes.setdefault(path / file, Node(path / file, self.root_path))
                node.add_child(child)
        return nodes[self.root_path]

    def iter_path_hash(self) -> Generator[tuple[Path, str]]:
        stack = [self.root_node]
        while stack:
            node = stack.pop()
            yield node.path, node.hash
            stack.extend(node.childs)

    def items(self) -> Iterable[tuple[Path, str]]:
        return self.iter_path_hash()

    def _serial_hash(self) -> str:
        return self.root_node.hash

    def _parallel_hash(self, max_workers: int = 16) -> str:
        def compute_node_hash(node: Node, executor: ThreadPoolExecutor) -> str:
            if len(node.childs) == 0:  # a leaf node
                if node.path.is_file():  # leaf node is a file
                    return hash_file(node.path, self.root_path)
                else:  # leaf node is an empty folder
                    return hashlib.md5(
                        node.path.relative_to(node.root_path).as_posix().encode("utf-8")
                    ).hexdigest()
            h = hashlib.md5()

            child_hashes: List[tuple[Path, str]] = []
            futures = {
                executor.submit(compute_node_hash, child, executor): child
                for child in node.childs
            }
            for future in as_completed(futures):
                child = futures[future]
                child_hash = future.result()
                child_hashes.append((child.path, child_hash))

            # 按路径排序
            for _, child_hash in sorted(
                child_hashes, key=lambda x: x[0].relative_to(node.path)
            ):
                h.update(child_hash.encode("utf-8"))
            return h.hexdigest()

        with ThreadPoolExecutor(max_workers=max_workers) as executor:
            root_hash = compute_node_hash(self.root_node, executor)
        return root_hash

    async def _async_hash(self) -> str:
        async def compute_node_hash(node: Node) -> str:
            if len(node.childs) == 0:  # a leaf node
                if node.path.is_file():  # leaf node is a file
                    return await asyncio.to_thread(hash_file, node.path, self.root_path)
                else:  # leaf node is an empty folder
                    return hashlib.md5(
                        node.path.relative_to(node.root_path).as_posix().encode("utf-8")
                    ).hexdigest()

            # 异步计算所有子节点
            child_tasks = [
                asyncio.create_task(compute_node_hash(child)) for child in node.childs
            ]
            child_hashes: List[tuple[Path, str]] = []
            for child, task in zip(node.childs, child_tasks):
                h = await task
                child_hashes.append((child.path, h))

            # 按路径排序
            h = hashlib.md5()
            for _, child_hash in sorted(
                child_hashes, key=lambda x: x[0].relative_to(node.path)
            ):
                h.update(child_hash.encode("utf-8"))
            return h.hexdigest()

        return await compute_node_hash(self.root_node)

    def get_hash(
        self, *, size_threshold: int = 100 * 1024, max_workers: int = 16
    ) -> str:
        """
        自动选择串行或并行版本：
        - 如果大部分文件 >= size_threshold（默认100KB），使用并行
        - 否则使用串行
        """
        if size_threshold in self._hash_cache:
            return self._hash_cache[size_threshold]
        total_files = 0
        large_files = 0

        stack = [self.root_node]
        while stack:
            node = stack.pop()
            if node.path.is_file():
                total_files += 1
                if node.path.stat().st_size >= size_threshold:
                    large_files += 1
            stack.extend(node.childs)

        # 判断大多数文件是否 >= 阈值
        if total_files == 0:
            h: str = self._serial_hash()  # 空目录或全目录文件夹
        elif large_files * 2 >= total_files:
            h: str = self._parallel_hash(max_workers=max_workers)
        else:
            h: str = self._serial_hash()
        self._hash_cache[size_threshold] = h
        return h

    async def get_hash_async(self, *, size_threshold: int = 100 * 1024) -> str:
        """
        async 版本，内部使用 asyncio + to_thread
        """
        total_files = 0
        large_files = 0
        stack = [self.root_node]
        while stack:
            node = stack.pop()
            if node.path.is_file():
                total_files += 1
                if node.path.stat().st_size >= size_threshold:
                    large_files += 1
            stack.extend(node.childs)

        if total_files == 0:
            return await self._async_hash()
        elif large_files * 2 >= total_files:
            return await self._async_hash()  # async 并发版本
        else:
            return await asyncio.to_thread(self._serial_hash)  # async 串行版本
