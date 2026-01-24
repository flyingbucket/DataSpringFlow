from __future__ import annotations

import warnings
from dataclasses import dataclass
from pathlib import Path
from typing import Optional, Tuple, TypedDict, Union

from ..protocols import HashSnapshot
from ..utils import get_running_environment

from .merkle import FileMerkleTree


def default_script_path() -> Path:
    """
    获取默认 script_path
    - 普通脚本 -> __file__
    - Notebook / REPL -> Path.cwd() 并给出警告
    """
    try:
        return Path(__file__).resolve()
    except NameError:
        env = get_running_environment()
        if env == "notebook":
            warnings.warn(
                "Running in a Jupyter Notebook. default script_path will be set to cwd().\n"
                "Consider setting script_path explicitly if you need accurate metadata.",
                stacklevel=2,
            )
        elif env in ("ipython", "python"):
            warnings.warn(
                "Running in an interactive shell. default script_path will be set to cwd().\n "
                "Consider setting script_path explicitly if you need accurate metadata.",
                stacklevel=2,
            )
        return Path.cwd()


@dataclass(frozen=True)
class Metadata:
    name: str
    tag: str
    path: Path
    description_path: Path
    hash: str
    dependencies: Tuple[str]  # Tuple[Metadata.id]
    script_path: Optional[Path] = None

    def __post_init__(self) -> None:
        if "@" in self.name:
            raise ValueError(f"Metadata.name must not contain '@': {self.name}")
        if "@" in self.tag:
            raise ValueError(f"Metadata.tag must not contain '@': {self.tag}")

    @property
    def id(self) -> str:
        return f"{self.name}@{self.tag}"

    def to_dict(self) -> MetadataDict:
        return {
            "name": self.name,
            "tag": self.tag,
            "path": str(self.path),
            "description_path": str(self.description_path),
            "hash": self.hash,
            "dependencies": self.dependencies,
            "script_path": str(self.script_path) if self.script_path else None,
        }


class MetadataBuilder:
    def __init__(
        self,
        path: Path,
        name: str,
        tag: str,
        dependencies: tuple[str],
        script_path: Optional[Path] = None,
    ):
        self.path = path
        self.name = name
        self.tag = tag
        self.dependencies: tuple[str] = dependencies
        self.script_path = script_path if script_path else default_script_path()

    def build(self) -> tuple[Metadata, HashSnapshot]:
        tree = FileMerkleTree(self.path)
        metadata = Metadata(
            name=self.name,
            tag=self.tag,
            path=self.path,
            description_path=self.path / "description.json",
            hash=tree.get_hash(),
            dependencies=self.dependencies,
            script_path=self.script_path,
        )
        return metadata, tree


class MetadataDict(TypedDict):
    name: str
    tag: str
    path: str
    description_path: str
    hash: str
    dependencies: Tuple[str]
    script_path: Union[str, None]
