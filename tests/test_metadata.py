import unittest
from pathlib import Path
from dataspringflow.core.metadata import Metadata


class TestMetadata(unittest.TestCase):
    def test_id_property(self):
        m = Metadata(
            name="dataset",
            tag="v1",
            path=Path("/tmp/data"),
            description_path=Path("/tmp/data/meta.yaml"),
            hash="abc123",
            dependencies=(),
        )
        self.assertEqual(m.id, "dataset@v1")

    def test_post_init_validation(self):
        with self.assertRaises(ValueError):
            Metadata(
                name="bad@name",
                tag="v1",
                path=Path("/tmp"),
                description_path=Path("/tmp/meta.yaml"),
                hash="h",
                dependencies=(),
            )
        with self.assertRaises(ValueError):
            Metadata(
                name="good",
                tag="v@1",
                path=Path("/tmp"),
                description_path=Path("/tmp/meta.yaml"),
                hash="h",
                dependencies=(),
            )

    def test_to_dict(self):
        m = Metadata(
            name="dataset",
            tag="v1",
            path=Path("/tmp/data"),
            description_path=Path("/tmp/data/meta.yaml"),
            hash="abc123",
            dependencies=("dep1",),
        )
        d = m.to_dict()
        self.assertEqual(d["name"], "dataset")
        self.assertEqual(d["tag"], "v1")
        self.assertEqual(d["dependencies"], ("dep1",))
        self.assertIsNone(d["script_path"])

    def test_script_path_optional(self):
        m = Metadata(
            name="dataset",
            tag="v1",
            path=Path("/tmp/data"),
            description_path=Path("/tmp/data/meta.yaml"),
            hash="abc123",
            dependencies=(),
            script_path=Path("/tmp/script.py"),
        )
        self.assertEqual(str(m.script_path), "/tmp/script.py")
