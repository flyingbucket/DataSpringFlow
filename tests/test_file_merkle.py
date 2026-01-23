import unittest
import tempfile
import shutil
import time
from pathlib import Path
from dataspringflow.core.merkle import FileMerkleTree


class TestFileMerkleTree(unittest.TestCase):
    def setUp(self):
        self.temp_dir = Path(tempfile.mkdtemp())

    def tearDown(self):
        shutil.rmtree(self.temp_dir)

    def _create_files(self, num_files: int, file_size_kb: int = 1):
        """创建指定数量和大小的文件"""
        for i in range(num_files):
            file_path = self.temp_dir / f"file{i}.txt"
            file_path.write_bytes(b"x" * 1024 * file_size_kb)

    def _run_test(self, num_files: int, file_size_kb: int = 1):
        print(f"\n--- 测试 {num_files} 个文件，每个 {file_size_kb}KB ---")
        self._create_files(num_files, file_size_kb)

        # 创建两个独立的树实例，保证缓存不干扰
        tree_serial = FileMerkleTree(self.temp_dir)
        tree_parallel = FileMerkleTree(self.temp_dir)
        tree_asyncio = FileMerkleTree(self.temp_dir)

        # 串行 hash
        start = time.time()
        serial_hash = tree_serial._serial_hash()
        serial_time = time.time() - start

        # 并行 hash
        start = time.time()
        parallel_hash = tree_parallel._parallel_hash()
        parallel_time = time.time() - start

        # asyncio hash
        start = time.time()
        asycnio_hash = tree_asyncio._async_hash()
        asycnio_time = time.time() - start
        print(f"Serial hash time: {serial_time:.4f}s")
        print(f"Parallel hash time: {parallel_time:.4f}s")
        print(f"Asyncio hash time: {asycnio_time:.4f}s")

        # 验证一致性
        self.assertEqual(
            len({serial_hash, parallel_hash, asycnio_hash}), 1, "三个 hash 值不一致"
        )
        # 清空临时目录（防止下一轮文件残留）
        for f in self.temp_dir.iterdir():
            if f.is_file():
                f.unlink()
            elif f.is_dir():
                shutil.rmtree(f)

    def test_small_files(self):
        self._run_test(num_files=1000, file_size_kb=50)

    def test_medium_files(self):
        self._run_test(num_files=1000, file_size_kb=800)

    def test_large_dir(self):
        self._run_test(num_files=10000, file_size_kb=200)

    def test_large_files(self):
        self._run_test(num_files=1000, file_size_kb=3 * 1024)

    def test_very_large_files(self):
        self._run_test(num_files=100, file_size_kb=10 * 1024)


if __name__ == "__main__":
    unittest.main()
