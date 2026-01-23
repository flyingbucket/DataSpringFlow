from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
from typing import Optional, Tuple, TypedDict, Union


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


class MetadataDict(TypedDict):
    name: str
    tag: str
    path: str
    description_path: str
    hash: str
    dependencies: Tuple[str]
    script_path: Union[str, None]
