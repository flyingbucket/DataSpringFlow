import hashlib
from pathlib import Path


def _choose_chunk_size(file_size):
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
