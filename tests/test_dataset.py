import unittest
from unittest.mock import MagicMock, patch
from pathlib import Path

from dataspringflow.core.metadata import Metadata
from dataspringflow.core.dataset import DSFdataset
from dataspringflow.core.hash_utils import HashDiff


class TestDSFdataset(unittest.TestCase):
    def setUp(self):
        # 创建一个 Metadata mock
        self.meta = Metadata(
            name="dataset",
            tag="v1",
            path=Path("/tmp/dataset"),
            description_path=Path("/tmp/dataset/meta.yaml"),
            hash="correct_hash",
            dependencies=[],
        )

        # DAG mock
        self.mock_dag = MagicMock()

        # dataset 实例
        self.dataset = DSFdataset(self.meta, self.mock_dag)

    def test_init_assigns_attributes(self):
        self.assertEqual(self.dataset.info, self.meta)
        self.assertEqual(self.dataset.DAG, self.mock_dag)

    @patch("dataspringflow.core.dataset.FileMerkleTree")
    def test_verify_hash_matches(self, mock_merkle_cls):
        # 模拟 FileMerkleTree.get_hash 返回正确 hash
        mock_merkle = MagicMock()
        mock_merkle.get_hash.return_value = "correct_hash"
        mock_merkle_cls.return_value = mock_merkle

        ok, diff = self.dataset.verify()
        self.assertTrue(ok)
        self.assertIsInstance(diff, HashDiff)
        self.assertEqual(diff.added, set())
        self.assertEqual(diff.removed, set())
        self.assertEqual(diff.modified, set())

    @patch("dataspringflow.core.dataset.FileMerkleTree")
    @patch("dataspringflow.core.dataset.HashLoader")
    @patch("dataspringflow.core.dataset.compute_hash_diff")
    def test_verify_hash_mismatch(
        self, mock_compute_diff, mock_hash_loader_cls, mock_merkle_cls
    ):
        # 模拟 FileMerkleTree.get_hash 返回错误 hash
        mock_merkle = MagicMock()
        mock_merkle.get_hash.return_value = "wrong_hash"
        mock_merkle_cls.return_value = mock_merkle

        # 模拟 HashLoader 返回老快照
        mock_loader = MagicMock()
        mock_loader.load.return_value = "old_snapshot"
        mock_hash_loader_cls.return_value = mock_loader

        # 模拟 compute_hash_diff 返回自定义 diff
        expected_diff = HashDiff(added={"file1"}, removed={"file2"}, modified={"file3"})
        mock_compute_diff.return_value = expected_diff

        ok, diff = self.dataset.verify()
        self.assertFalse(ok)
        self.assertEqual(diff, expected_diff)

        # 验证 HashLoader.load 被调用
        mock_loader.load.assert_called_once_with(name="dataset", tag="v1")
        # 验证 compute_hash_diff 被调用
        mock_compute_diff.assert_called_once_with("old_snapshot", mock_merkle)


if __name__ == "__main__":
    unittest.main()
