import asyncio
import shutil
import tempfile
import unittest
from pathlib import Path

from dataspringflow.core.merkle import FileMerkleTree


def _write_bytes(path: Path, data: bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(data)


def _make_dummy_dataset(root: Path) -> dict[str, Path]:
    """
    构造一个包含：
    - 多级目录
    - 空目录
    - 多个文件（大小不同）
    的数据集

    返回一些关键路径，方便测试里重命名/移动。
    """
    # 目录结构：
    # root/
    #   a/
    #     small.txt        (小文件)
    #     sub/
    #       medium.bin     (中等文件)
    #   b/
    #     empty_dir/       (空目录)
    #   c/                 (空目录)
    #   top.txt
    a = root / "a"
    b = root / "b"
    c = root / "c"
    empty_dir = b / "empty_dir"

    (a / "sub").mkdir(parents=True, exist_ok=True)
    empty_dir.mkdir(parents=True, exist_ok=True)
    c.mkdir(parents=True, exist_ok=True)

    # 文件内容：确定性，方便复现
    _write_bytes(root / "top.txt", b"TOP\n")
    _write_bytes(a / "small.txt", b"hello\n")
    # 让 medium.bin 大一点（比如 256KB）以便你 size_threshold 判断更容易覆盖“并行”分支
    _write_bytes(a / "sub" / "medium.bin", b"x" * (256 * 1024))

    return {
        "root": root,
        "file_small": a / "small.txt",
        "file_medium": a / "sub" / "medium.bin",
        "file_top": root / "top.txt",
        "empty_dir": empty_dir,
        "empty_dir_parent": b,
        "empty_dir_2": c,  # 另一个空目录
    }


class TestFileMerkleTree(unittest.TestCase):
    def test_serial_parallel_async_consistent(self) -> None:
        with tempfile.TemporaryDirectory() as td:
            root = Path(td) / "dataset"
            root.mkdir()
            _make_dummy_dataset(root)

            t = FileMerkleTree(root)

            # 1) 直接走串行
            h_serial = t._serial_hash()

            # 2) 直接走多线程并行
            h_parallel = t._parallel_hash(max_workers=8)

            # 3) 直接走 asyncio 并发（内部 to_thread）
            h_async = asyncio.run(t._async_hash())

            self.assertEqual(h_serial, h_parallel, "串行与多线程并行 hash 不一致")
            self.assertEqual(h_serial, h_async, "串行与 asyncio 并发 hash 不一致")

    def test_move_dataset_keeps_hash(self) -> None:
        with tempfile.TemporaryDirectory() as td:
            base = Path(td)

            src = base / "dataset_src"
            src.mkdir()
            _make_dummy_dataset(src)

            h1 = FileMerkleTree(src)._serial_hash()

            # mv 到另一位置（同一个临时目录下的另一个路径）
            dst = base / "dataset_dst"
            shutil.move(str(src), str(dst))
            self.assertTrue(dst.exists() and dst.is_dir())

            h2 = FileMerkleTree(dst)._serial_hash()
            self.assertEqual(
                h1, h2, "整体移动数据集后 hash 应保持不变（只依赖相对结构与内容）"
            )

    def test_empty_dir_rename_changes_hash(self) -> None:
        with tempfile.TemporaryDirectory() as td:
            root = Path(td) / "dataset"
            root.mkdir()
            paths = _make_dummy_dataset(root)

            h_before = FileMerkleTree(root)._serial_hash()

            empty_dir = paths["empty_dir"]
            empty_dir_parent = paths["empty_dir_parent"]
            renamed = empty_dir_parent / "empty_dir_renamed"

            # 重命名空目录
            empty_dir.rename(renamed)

            # 注意：必须重建树（Node.hash 是 cached_property，旧实例会缓存）
            h_after = FileMerkleTree(root)._serial_hash()

            self.assertNotEqual(h_before, h_after, "空目录改名后 hash 应发生变化")

    def test_file_rename_changes_hash(self) -> None:
        with tempfile.TemporaryDirectory() as td:
            root = Path(td) / "dataset"
            root.mkdir()
            paths = _make_dummy_dataset(root)

            h_before = FileMerkleTree(root)._serial_hash()

            file_small = paths["file_small"]
            renamed = file_small.with_name("small_renamed.txt")

            # 改名但内容不变
            file_small.rename(renamed)

            h_after = FileMerkleTree(root)._serial_hash()
            self.assertNotEqual(
                h_before,
                h_after,
                "文件改名后 hash 应发生变化（因为 hash_file 包含相对路径）",
            )

    def test_get_hash_branches_still_consistent(self) -> None:
        """
        额外覆盖一下 get_hash 自动选择分支：
        - 用不同 size_threshold 触发串行 or 并行
        - 验证最终 root hash 一致
        """
        with tempfile.TemporaryDirectory() as td:
            root = Path(td) / "dataset"
            root.mkdir()
            _make_dummy_dataset(root)

            t1 = FileMerkleTree(root)
            # threshold 很大 -> 绝大部分文件 < threshold -> 倾向串行
            h_serial_like = t1.get_hash(size_threshold=10 * 1024 * 1024, max_workers=8)

            t2 = FileMerkleTree(root)
            # threshold 很小 -> 绝大部分文件 >= threshold -> 倾向并行
            h_parallel_like = t2.get_hash(size_threshold=1, max_workers=8)

            t3 = FileMerkleTree(root)
            h_async = asyncio.run(t3.get_hash_async(size_threshold=1))

            self.assertEqual(
                h_serial_like, h_parallel_like, "get_hash 不同分支结果不一致"
            )
            self.assertEqual(
                h_parallel_like, h_async, "get_hash 与 get_hash_async 结果不一致"
            )


if __name__ == "__main__":
    unittest.main()
