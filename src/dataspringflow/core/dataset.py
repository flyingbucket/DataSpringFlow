from __future__ import annotations

import yaml

from dataclasses import dataclass
from pathlib import Path
from typing import List, Optional


@dataclass(frozen=True)
class Metadata:
    name: str
    tag: str
    path: Path
    description_path: Path
    hash: int
    dependencies: List[str]  # List[Metadata.id]
    script_path: Optional[Path] = None

    @property
    def id(self) -> str:
        return f"{self.name}@{self.tag}"

    def to_dict(self) -> dict:
        return {
            "name": self.name,
            "tag": self.tag,
            "path": str(self.path),
            "description_path": str(self.description_path),
            "hash": self.hash,
            "dependencies": list(self.dependencies),
            "script_path": str(self.script_path) if self.script_path else None,
        }


class MetadataLoader:
    @staticmethod
    def load_from_dict(metadata_dict) -> Metadata:
        return Metadata(
            name=metadata_dict["name"],
            tag=metadata_dict["tag"],
            path=Path(metadata_dict["path"]),
            description_path=Path(metadata_dict["description_path"]),
            hash=metadata_dict["hash"],
            dependencies=metadata_dict.get("dependencies", []),
            script_path=Path(metadata_dict["script_path"])
            if metadata_dict.get("script_path")
            else None,
        )

    @staticmethod
    def load_from_yaml(name: str, tag: str,root: Path = Path("/opt/DSFRegistry")) -> Metadata:
        metadata_path = root / name / tag / "metadata.yaml"
        with open(metadata_path, "r") as f:
            metadata_dict = yaml.safe_load(f)
        return MetadataLoader.load_from_dict(metadata_dict)

    @staticmethod
    def load_from_db(**kwargs) -> Metadata:
        raise NotImplementedError("Database metadata loader not implemented yet")

    @staticmethod
    def load(
        name: str,
        tag: str,
        *,
        backend: str = "yaml",
        root: Path = Path("/opt/DSFRegistry"),
    ) -> Metadata:
        if backend == "yaml":
            return MetadataLoader.load_from_yaml(
                name,tag,root
            )
        elif backend == "database":
            return MetadataLoader.load_from_db()
        else:
            raise ValueError(
                f"Invalid session type {backend}. Should be 'yaml' or 'database' "
            )


class DSFdataset:
    def __init__(self, metadata: Metadata)) -> None:
        self.metadata = metadata
    
