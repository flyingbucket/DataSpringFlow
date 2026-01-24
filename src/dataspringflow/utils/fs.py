from pathlib import Path
from typing import Generator


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


def parse_id(id_str: str) -> tuple[str, str]:
    """
    将 id 字符串 'name@tag' 解析为 (name, tag)
    """
    if "@" not in id_str:
        raise ValueError(f"Invalid id string: {id_str}")
    name, tag = id_str.split("@", 1)  # 只分一次，防止 name 中有 @
    return name, tag
