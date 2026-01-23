import hashlib
from pathlib import Path
from dataclasses import dataclass


def _choose_chunk_size(file_size: int):
    if file_size < 10 * 1024**2:  # <10 MB
        return 1024 * 16
    elif file_size < 100 * 1024**2:  # 10~100 MB
        return 1024 * 64
    elif file_size < 1024 * 1024**2:  # 100 MB~1 GB
        return 1024 * 256
    else:  # >1 GB
        return 1024 * 1024  # 1 MB


def hash_file(path: Path) -> str:
    chunk_size = _choose_chunk_size(path.stat().st_size)
    h = hashlib.md5()
    with open(path, "rb") as f:
        chunk = f.read(chunk_size)
        while chunk:
            h.update(chunk)
            chunk = f.read(chunk_size)
    return h.hexdigest()


@dataclass(frozen=True)
class HashDiff:
    added: set[Path]
    removed: set[Path]
    modified: set[Path]

    @property
    def ok(self) -> bool:
        return not (self.added or self.removed or self.modified)


def compute_hash_diff(
    old: HashSnapshot,
    new: HashSnapshot,
) -> HashDiff:
    old_map = dict(old.items())
    new_map = dict(new.items())

    old_paths = set(old_map)
    new_paths = set(new_map)

    added = new_paths - old_paths
    removed = old_paths - new_paths
    modified = {p for p in old_paths & new_paths if old_map[p] != new_map[p]}

    return HashDiff(
        added=added,
        removed=removed,
        modified=modified,
    )
