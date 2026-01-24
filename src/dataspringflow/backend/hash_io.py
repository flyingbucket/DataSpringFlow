import joblib  # type: ignore
from pathlib import Path

from ..protocols import HashSnapshot
from ..core.merkle import FileMerkleTree


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


class HashLoader:
    def load(self, name: str, tag: str) -> HashSnapshot:
        raise NotImplementedError
