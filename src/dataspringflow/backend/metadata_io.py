import yaml

from pathlib import Path
from typing import Any
from ..core.metadata import Metadata, MetadataDict


class MetadataLoader:
    @staticmethod
    def load_from_dict(metadata_dict: MetadataDict) -> Metadata:
        script_path_str = metadata_dict.get("script_path")
        script_path = Path(script_path_str) if script_path_str is not None else None
        return Metadata(
            name=metadata_dict["name"],
            tag=metadata_dict["tag"],
            path=Path(metadata_dict["path"]),
            description_path=Path(metadata_dict["description_path"]),
            hash=metadata_dict["hash"],
            dependencies=metadata_dict[
                "dependencies"
            ],  # dependencies 在 MetadataDict 中是必填的
            script_path=script_path,
        )

    @staticmethod
    def load_from_yaml(
        name: str, tag: str, root: Path = Path("/opt/DSFRegistry")
    ) -> Metadata:
        metadata_path = root / name / tag / "metadata.yaml"
        with open(metadata_path, "r") as f:
            metadata_dict = yaml.safe_load(f)
        return MetadataLoader.load_from_dict(metadata_dict)

    @staticmethod
    def load_from_db(**kwargs: Any) -> Metadata:
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
            return MetadataLoader.load_from_yaml(name, tag, root)
        elif backend == "database":
            return MetadataLoader.load_from_db()
        else:
            raise ValueError(
                f"Invalid session type {backend}. Should be 'yaml' or 'database' "
            )
