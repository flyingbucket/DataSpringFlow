import unittest
from unittest.mock import MagicMock, AsyncMock, patch
import asyncio

from dataspringflow.core.registry import DSFRegistry, parse_id
from dataspringflow.core.metadata import Metadata
from dataspringflow.core.dataset import DSFdataset
from dataspringflow.core.dag import DAG


class TestDSFRegistry(unittest.TestCase):
    def setUp(self):
        # patch get_registry_factory 返回一个 mock factory
        patcher = patch("dataspringflow.core.registry.get_registry_factory")
        self.addCleanup(patcher.stop)
        self.mock_factory_fn = patcher.start()

        # 创建 mock loader/writer
        self.mock_metadata_loader = MagicMock()
        self.mock_metadata_writer = MagicMock()
        self.mock_hash_loader = MagicMock()
        self.mock_hash_writer = MagicMock()

        mock_factory = MagicMock()
        mock_factory.create_metadata_loader.return_value = self.mock_metadata_loader
        mock_factory.create_metadata_writer.return_value = self.mock_metadata_writer
        mock_factory.create_hash_loader.return_value = self.mock_hash_loader
        mock_factory.create_hash_writer.return_value = self.mock_hash_writer

        self.mock_factory_fn.return_value = mock_factory

        # 创建 registry 实例
        self.registry = DSFRegistry(backend="mock")

    def test_parse_id_valid(self):
        name, tag = parse_id("dataset@v1")
        self.assertEqual(name, "dataset")
        self.assertEqual(tag, "v1")

    def test_parse_id_invalid(self):
        with self.assertRaises(ValueError):
            parse_id("invalid_string")

    def test_get_info_calls_loader(self):
        # mock 返回一个 Metadata
        mock_meta = Metadata(
            name="dataset",
            tag="v1",
            path="p",
            description_path="d",
            hash=123,
            dependencies=[],
        )
        self.mock_metadata_loader.load.return_value = mock_meta

        result = self.registry.get_info("dataset@v1")
        self.mock_metadata_loader.load.assert_called_once_with("dataset", "v1")
        self.assertEqual(result, mock_meta)

    def test_get_hash_snapshot_calls_loader(self):
        mock_snapshot = {"hash": "abc123"}
        self.mock_hash_loader.load.return_value = mock_snapshot

        result = self.registry.get_hash_snapshot("dataset@v1")
        self.mock_hash_loader.load.assert_called_once_with("dataset", "v1")
        self.assertEqual(result, mock_snapshot)

    def test_get_returns_dsf_dataset(self):
        mock_meta = Metadata(
            name="dataset",
            tag="v1",
            path="p",
            description_path="d",
            hash=123,
            dependencies=[],
        )
        self.mock_metadata_loader.load.return_value = mock_meta

        dsf_dataset = self.registry.get("dataset@v1")
        self.assertIsInstance(dsf_dataset, DSFdataset)
        self.assertEqual(dsf_dataset.info, mock_meta)
        self.assertIsInstance(dsf_dataset.DAG, DAG)

    def test_save_calls_writers(self):
        mock_meta = Metadata(
            name="dataset",
            tag="v1",
            path="p",
            description_path="d",
            hash=123,
            dependencies=[],
        )

        # 模拟 writer.save 返回值
        self.mock_metadata_writer.save.return_value = 1
        self.mock_hash_writer.save.return_value = 2

        async def run_test():
            result = await self.registry.save(mock_meta)
            self.mock_metadata_writer.save.assert_called_once_with(mock_meta)
            self.mock_hash_writer.save.assert_called_once_with("dataset", "v1")
            self.assertEqual(result, {"metadata": 1, "hash": 2})

        asyncio.run(run_test())


if __name__ == "__main__":
    unittest.main()
