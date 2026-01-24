import asyncio
import statistics
import time
import tempfile
from pathlib import Path

from dataspringflow.core.merkle import FileMerkleTree


def make_files(root: Path, n: int, size: int, prefix: str) -> None:
    root.mkdir(parents=True, exist_ok=True)
    chunk = b"a" * 4096
    for i in range(n):
        p = root / f"{prefix}_{i:06d}.bin"
        # 写固定内容即可；如果你担心重复内容影响压缩/缓存，可掺入 i
        with open(p, "wb") as f:
            remaining = size
            while remaining > 0:
                w = min(remaining, len(chunk))
                f.write(chunk[:w])
                remaining -= w


def time_call(fn, repeat: int = 3) -> float:
    times = []
    for _ in range(repeat):
        t0 = time.perf_counter()
        fn()
        t1 = time.perf_counter()
        times.append(t1 - t0)
    return statistics.median(times)


def bench_scenario(
    name: str, n: int, size: int, size_threshold: int, max_workers: int = 16
) -> None:
    with tempfile.TemporaryDirectory() as td:
        root = Path(td) / "dataset"
        make_files(root / "files", n=n, size=size, prefix="f")

        # 串行
        t_serial = time_call(lambda: FileMerkleTree(root)._serial_hash())

        # 多线程并行
        t_parallel = time_call(
            lambda: FileMerkleTree(root)._parallel_hash(max_workers=max_workers)
        )

        # asyncio 并发（如果你的实现内部用 to_thread）
        t_async = time_call(lambda: asyncio.run(FileMerkleTree(root)._async_hash()))

        print(f"\n== {name} ==")
        print(f"serial   : {t_serial:.3f}s")
        print(f"parallel : {t_parallel:.3f}s  (x{t_serial / t_parallel:.2f})")
        print(f"async    : {t_async:.3f}s  (x{t_serial / t_async:.2f})")

        # 可选：验证结果一致（别只测速度）
        h1 = FileMerkleTree(root)._serial_hash()
        h2 = FileMerkleTree(root)._parallel_hash(max_workers=max_workers)
        h3 = asyncio.run(FileMerkleTree(root)._async_hash())
        assert h1 == h2 == h3, "hash mismatch!"


if __name__ == "__main__":
    # 你提出的三种场景
    bench_scenario(
        "10k small (100KB)", n=10_000, size=100 * 1024, size_threshold=1, max_workers=16
    )
    bench_scenario(
        "1k medium (800KB)", n=1_000, size=800 * 1024, size_threshold=1, max_workers=16
    )
    bench_scenario(
        "1k large (1.5MB)",
        n=1_000,
        size=int(1.5 * 1024 * 1024),
        size_threshold=1,
        max_workers=16,
    )
