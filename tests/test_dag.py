import unittest
from unittest.mock import MagicMock, patch
from pathlib import Path

from dataspringflow.core.metadata import Metadata
from dataspringflow.core.dag import DAG


class TestDAG(unittest.TestCase):
    def setUp(self):
        # ✅ 创建 mock registry
        self.mock_registry = MagicMock()

        # ✅ 模拟 Metadata 实例
        self.meta_root = Metadata(
            name="root",
            tag="v1",
            path=Path("/tmp/root"),
            description_path=Path("/tmp/root/meta.yaml"),
            hash="hash_root",
            dependencies=["dep1@v1", "dep2@v1"],  # 完整 id
        )
        self.meta_dep1 = Metadata(
            name="dep1",
            tag="v1",
            path=Path("/tmp/dep1"),
            description_path=Path("/tmp/dep1/meta.yaml"),
            hash="hash_dep1",
            dependencies=[],
        )
        self.meta_dep2 = Metadata(
            name="dep2",
            tag="v1",
            path=Path("/tmp/dep2"),
            description_path=Path("/tmp/dep2/meta.yaml"),
            hash="hash_dep2",
            dependencies=[],
        )

        # ✅ registry.get_info 返回对应 metadata
        def get_info_side_effect(id_str):
            mapping = {
                "root@v1": self.meta_root,
                "dep1@v1": self.meta_dep1,
                "dep2@v1": self.meta_dep2,
            }
            return mapping[id_str]

        self.mock_registry.get_info.side_effect = get_info_side_effect

        # ✅ 创建 DAG 实例
        self.dag = DAG("root@v1", self.mock_registry)

    def test_iter_deps_yields_all(self):
        deps = list(self.dag.iter_DAG())
        self.assertIn(self.meta_dep1, deps)
        self.assertIn(self.meta_dep2, deps)
        self.assertEqual(len(deps), 2)  # 确保去重

    @patch("dataspringflow.core.dag.FileMerkleTree")
    def test_verify_all_ok(self, mock_merkle_cls):
        # 模拟 get_hash 返回正确 hash
        mock_merkle = MagicMock()
        # dep1 和 dep2 hash 都匹配
        mock_merkle.get_hash.side_effect = ["hash_dep1", "hash_dep2"]
        mock_merkle_cls.side_effect = [mock_merkle, mock_merkle]

        # 强制 _iter_deps 遍历依赖
        self.dag._iter_deps = MagicMock(return_value=[self.meta_dep1, self.meta_dep2])

        ok, broken = self.dag.verify()
        self.assertTrue(ok)
        self.assertEqual(broken, [])

    @patch("dataspringflow.core.dag.FileMerkleTree")
    def test_verify_some_broken(self, mock_merkle_cls):
        mock_merkle = MagicMock()
        # dep1 hash 匹配，dep2 hash 不匹配
        mock_merkle.get_hash.side_effect = ["hash_dep1", "wrong_hash"]
        mock_merkle_cls.side_effect = [mock_merkle, mock_merkle]

        self.dag._iter_deps = MagicMock(return_value=[self.meta_dep1, self.meta_dep2])
        ok, broken = self.dag.verify()
        self.assertFalse(ok)
        self.assertEqual(broken, [self.meta_dep2.id])


if __name__ == "__main__":
    unittest.main()
