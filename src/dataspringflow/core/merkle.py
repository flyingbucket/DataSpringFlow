from __future__ import annotations
import hashlib
from pathlib import Path
from typing import List, Optional
from functools import cached_property

from ..utils.hash import hash_file


def walkdir(root: Path):
    root = Path(root)
    dirs = []
    files = []

    # 首先列出当前目录下的文件和子目录
    for child in root.iterdir():
        if child.is_dir():
            dirs.append(child.name)
        elif child.is_file():
            files.append(child.name)
    yield root, dirs, files

    # 递归遍历子目录
    for d in dirs:
        yield from walkdir(root / d)


class Node:
    def __init__(self, path: Path, childs: Optional[List[Node]] = None) -> None:
        self.path = path
        self.childs = sorted(childs or [], key=lambda c: c.path.name)

    @cached_property
    def hash(self) -> str:
        if self.path.is_file():
            return hash_file(self.path)
        h = hashlib.md5()
        for child in self.childs:
            h.update(child.hash.encode(encoding="utf-8"))
        return h.hexdigest()
