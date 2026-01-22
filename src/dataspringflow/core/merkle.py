from __future__ import annotations

import hashlib
import joblib  # type: ignore

from pathlib import Path
from functools import cached_property
from typing import Dict, List, Generator, Iterator
from concurrent.futures import ThreadPoolExecutor, as_completed

from ..utils.hash import hash_file


def walkdir(root: Path) -> Generator[tuple[Path, list[str], list[str]], None, None]:
    root = Path(root)

    dirs: list[str] = []
    files: list[str] = []

    for child in root.iterdir():
        if child.is_dir():
            dirs.append(child.name)
        elif child.is_file():
            files.append(child.name)

    yield root, dirs, files

    for d in dirs:
        yield from walkdir(root / d)


class Node:
    def __init__(self, path: Path) -> None:
        self.path = path
        self.childs: List[Node] = []

    def add_child(self, node: Node) -> None:
        self.childs.append(node)

    @cached_property
    def hash(self) -> str:
        if self.path.is_file():
            return hash_file(self.path)
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
            node = nodes.setdefault(path, Node(path))
            for dir in dirs:
                child = nodes.setdefault(path / dir, Node(path / dir))
                node.add_child(child)
            for file in files:
                child = nodes.setdefault(path / file, Node(path / file))
                node.add_child(child)
        return nodes[self.root_path]

    def iter_path_hash(self) -> Iterator[tuple[Path, str]]:
        stack = [self.root_node]
        while stack:
            node = stack.pop()
            yield node.path, node.hash
            stack.extend(node.childs)

    def _serial_hash(self) -> str:
        return self.root_node.hash

    def _parallel_hash(self, max_workers: int = 16) -> str:
        def compute_node_hash(node: Node, executor: ThreadPoolExecutor) -> str:
            if node.path.is_file():
                return hash_file(node.path)
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

    def _async_hash(self) -> str:
        """
        not used now
        """
        import asyncio

        async def compute_node_hash(node: Node) -> str:
            if node.path.is_file():
                # 异步执行同步 hash_file
                return await asyncio.to_thread(hash_file, node.path)

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

        # 同步封装：在内部创建/运行事件循环
        try:
            loop = asyncio.get_running_loop()
        except RuntimeError:
            loop = None

        if loop and loop.is_running():
            # 如果已经在 asyncio 环境里，直接创建任务并等待结果
            return loop.run_until_complete(compute_node_hash(self.root_node))
        else:
            # 普通同步调用，创建新事件循环
            return asyncio.run(compute_node_hash(self.root_node))

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
        if large_files / total_files >= 0.5:
            h: str = self._parallel_hash(max_workers=max_workers)
        else:
            h: str = self._serial_hash()
        self._hash_cache[size_threshold] = h
        return h


class HashWriter:
    def __init__(self, merkle_tree: FileMerkleTree):
        self.tree = merkle_tree

    def to_dict(self) -> dict[Path, str]:
        return dict(self.tree.iter_path_hash())

    def to_json(self, file_path: Path) -> None:
        import json

        d = {str(p): h for p, h in self.to_dict().items()}
        with open(file_path, "w") as f:
            json.dump(d, f, indent=2)

    def to_joblib(self, file_path: Path) -> None:
        joblib.dump(self.to_dict(), file_path)  # type: ignore

    def to_db(self) -> None:
        raise NotImplementedError
