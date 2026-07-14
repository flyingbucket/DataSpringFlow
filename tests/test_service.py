import unittest
import os
import tempfile
from pydsf import (
    DSFService,
    BackendAddr,
    DatasetStatus,
    DataSetVerifyRes,
    MetaData,
    ScopedMetaData,
    ScopedId,
)


class TestDSFService(unittest.TestCase):
    def setUp(self):
        """每个测试用例执行前的初始化"""
        # 初始化服务层
        self.service = DSFService()

        # 准备一些模拟的路径和数据集信息
        self.temp_dir = tempfile.TemporaryDirectory()
        self.dummy_path = self.temp_dir.name
        self.dummy_script = os.path.join(self.dummy_path, "process.py")
        self.dummy_desc = os.path.join(self.dummy_path, "README.md")

        # 写入空文件以防底层校验需要文件存在
        with open(self.dummy_script, "w") as f:
            f.write("# dummy script")
        with open(self.dummy_desc, "w") as f:
            f.write("# dummy description")

        # 使用 Private 后端用于隔离测试
        self.backend = BackendAddr.private()

        self.dataset_name = "mnist"
        self.dataset_tag = "v1.0"
        self.dataset_id = f"{self.dataset_name}@{self.dataset_tag}"

    def tearDown(self):
        """每个测试用例执行后的清理"""
        self.temp_dir.cleanup()

    def test_1_register(self):
        """1. 测试数据集的注册 (Create)"""
        # 注册一个基础的数据集
        try:
            self.service.register(
                name=self.dataset_name,
                tag=self.dataset_tag,
                path=self.dummy_path,
                script_path=self.dummy_script,
                owner_nickname="tester",
                dependencies=[],
                description_path=self.dummy_desc,
                target_backend=self.backend,
                force_heal=False,
            )
        except Exception as e:
            self.fail(f"register 抛出了非预期的异常: {e}")

    def test_2_query_and_list_metadata(self):
        """2. 测试元数据的查询与列表获取 (Read)"""
        # 先确保数据被注册
        self.service.register(
            name=self.dataset_name,
            tag=self.dataset_tag,
            path=self.dummy_path,
            script_path=self.dummy_script,
            target_backend=self.backend,
        )

        # 测试 query_meta
        scoped_metas = self.service.query_meta(self.dataset_id, None)
        self.assertIsInstance(scoped_metas, list)
        self.assertGreater(len(scoped_metas), 0)

        # 验证 ScopedMetaData 结构与内嵌的 MetaData 属性
        scoped_meta = scoped_metas[0]
        self.assertIsInstance(scoped_meta, ScopedMetaData)
        self.assertIsInstance(scoped_meta.metadata, MetaData)
        self.assertEqual(scoped_meta.metadata.name, self.dataset_name)
        self.assertEqual(scoped_meta.metadata.tag, self.dataset_tag)
        self.assertEqual(scoped_meta.metadata.id(), self.dataset_id)

        # 测试 list_all_metadata
        all_metas = self.service.list_all_metadata()
        self.assertIsInstance(all_metas, list)
        # 确保刚刚注册的 id 在全部列表里
        ids = [sm.metadata.id() for sm in all_metas]
        self.assertIn(self.dataset_id, ids)

    def test_3_update_merkle(self):
        """3. 测试更新默克尔树哈希 (Update)"""
        self.service.register(
            name=self.dataset_name,
            tag=self.dataset_tag,
            path=self.dummy_path,
            script_path=self.dummy_script,
            target_backend=self.backend,
        )

        try:
            self.service.update_merkle(id=self.dataset_id, target_backend=self.backend)
        except Exception as e:
            self.fail(f"update_merkle 抛出了非预期的异常: {e}")

    def test_4_verification(self):
        """4. 测试自身校验与深度校验 (Verify)"""
        self.service.register(
            name=self.dataset_name,
            tag=self.dataset_tag,
            path=self.dummy_path,
            script_path=self.dummy_script,
            target_backend=self.backend,
        )

        # 测试 verify_self
        res_self = self.service.verify_self(
            id=self.dataset_id, show_diff=False, target_backend=self.backend
        )
        self.assertIsInstance(res_self, DataSetVerifyRes)
        self.assertIn(
            res_self.status,
            [DatasetStatus.Healthy, DatasetStatus.Broken, DatasetStatus.Unverified],
        )

        # 测试 verify_deep
        res_deep = self.service.verify_deep(
            id=self.dataset_id, show_diff=False, target_backend=self.backend
        )
        self.assertIsInstance(res_deep, DataSetVerifyRes)
        self.assertIsInstance(res_deep.dep_status, list)

    def test_5_check_is_referenced(self):
        """5. 测试依赖引用检查 (Graph Reference)"""
        # 注册一个下游数据集，其依赖于上面的 mnist
        downstream_name = "mnist_processed"
        downstream_tag = "v1.0"

        self.service.register(
            name=self.dataset_name,
            tag=self.dataset_tag,
            path=self.dummy_path,
            script_path=self.dummy_script,
            target_backend=self.backend,
        )

        self.service.register(
            name=downstream_name,
            tag=downstream_tag,
            path=self.dummy_path,
            script_path=self.dummy_script,
            dependencies=[self.dataset_id],  # 依赖 mnist@v1.0
            target_backend=self.backend,
        )

        # 检查是谁引用了 mnist@v1.0
        referenced_by = self.service.check_is_referenced(target_id=self.dataset_id)
        self.assertIsInstance(referenced_by, list)
        self.assertGreater(len(referenced_by), 0)

        # 验证返回的是 ScopedId 列表
        scoped_id = referenced_by[0]
        self.assertIsInstance(scoped_id, ScopedId)
        self.assertEqual(scoped_id.id, f"{downstream_name}@{downstream_tag}")

    def test_6_delete_metadata(self):
        """6. 测试元数据的删除 (Delete)"""
        self.service.register(
            name=self.dataset_name,
            tag=self.dataset_tag,
            path=self.dummy_path,
            script_path=self.dummy_script,
            target_backend=self.backend,
        )

        # 删除元数据
        try:
            self.service.delete_metadata(
                id=self.dataset_id, force=True, target_backend=self.backend
            )
        except Exception as e:
            self.fail(f"delete_metadata 抛出了非预期的异常: {e}")

        # 验证删除后是否依然能够查到（取决于具体的后端实现，一般查出来的列表里不应包含该 id）
        all_metas = self.service.list_all_metadata()
        ids = [sm.metadata.id() for sm in all_metas]
        # 如果你的实现中带有 backend 过滤且隔离成功，这里应该断言不在里面
        self.assertNotIn(self.dataset_id, ids)


if __name__ == "__main__":
    unittest.main()
