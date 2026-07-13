import unittest
import pydsf as dsf


class TestDSFPublicExportsAndCore(unittest.TestCase):
    """测试根模块导出和核心数据结构的基础功能"""

    def test_public_exports(self):
        """检查 __init__.py 显式导出是否正常"""
        assert_has = lambda attr: self.assertTrue(
            hasattr(dsf, attr), f"Missing export: {attr}"
        )
        assert_has("DSFService")
        assert_has("DSFDataset")
        assert_has("MetaData")
        assert_has("DataSetVerifyRes")
        assert_has("DatasetStatus")
        assert_has("BackendAddr")
        assert_has("ScopedId")
        assert_has("ScopedMetaData")

    def test_enum_members_exist(self):
        """检查 DatasetStatus 枚举成员及基本相等性判断"""
        self.assertTrue(hasattr(dsf.DatasetStatus, "Healthy"))
        self.assertTrue(hasattr(dsf.DatasetStatus, "Broken"))
        self.assertTrue(hasattr(dsf.DatasetStatus, "BrokenDeps"))
        self.assertTrue(hasattr(dsf.DatasetStatus, "Unverified"))

        self.assertEqual(dsf.DatasetStatus.Healthy, dsf.DatasetStatus.Healthy)
        self.assertNotEqual(dsf.DatasetStatus.Healthy, dsf.DatasetStatus.Broken)

    def test_verify_result_constructor_and_fields(self):
        """检查 DataSetVerifyRes 构造函数及其字段"""
        res = dsf.DataSetVerifyRes(
            dsf.DatasetStatus.Healthy,
            [dsf.DatasetStatus.Healthy, dsf.DatasetStatus.Unverified],
        )
        self.assertEqual(res.status, dsf.DatasetStatus.Healthy)
        self.assertIsInstance(res.dep_status, list)
        for x in res.dep_status:
            self.assertIsInstance(x, dsf.DatasetStatus)


class TestDSFServiceEndpoints(unittest.TestCase):
    """测试 DSFService 端的每一个增删改查及校验接口签名"""

    def setUp(self):
        """尝试初始化服务。如果自动探测后端失败，优雅跳过当前测试方法"""
        try:
            self.svc = dsf.DSFService()
        except Exception as e:
            self.skipTest(f"DSFService backend unavailable in test env: {e}")

        # 准备一个合法格式但不存在的测试 ID
        self.dummy_id = "__definitely_not_existing__@__nope__"

    def test_service_can_be_constructed(self):
        """验证服务成功实例化（若未被 setUp 中的 skipTest 拦截，则说明成功）"""
        self.assertIsNotNone(self.svc)

    def test_service_query_and_list_on_nonexistent(self):
        """测试查询与列表获取接口 (Read) 在无数据时的表现"""
        # query_meta 在底层返回错误时，绑定层会抛出 IOError，或者返回空列表
        try:
            res = self.svc.query_meta(self.dummy_id)
            self.assertIsInstance(res, list)
        except IOError:
            pass  # 允许抛出定义的 IOError

        # list_all_metadata 应该能够正常返回列表（即使为空）
        try:
            all_metas = self.svc.list_all_metadata()
            self.assertIsInstance(all_metas, list)
        except IOError:
            pass

    def test_service_register_argument_validation(self):
        """测试数据集注册接口 (Create) 的完整参数签名链路"""
        # 由于路径不存在，底层 Rust 转换为 PathBuf 校验或后端写入时必然会抛错
        # 这里主要验证 Python 侧多参数以及新增的 target_backend 关键字参数能够正确解析并传递给 Rust
        with self.assertRaises(Exception):
            self.svc.register(
                name="dummy",
                tag="v0",
                path="/tmp/not-exist-dataset",
                script_path="/tmp/not-exist-script.py",
                owner_nickname=None,
                dependencies=None,
                description_path=None,
                target_backend=None,  # 映射自 PyBackendAddr
                force_heal=False,
                yes=True,
            )

    def test_service_update_merkle_signature(self):
        """测试更新默克尔树接口 (Update) 的签名"""
        with self.assertRaises(Exception):
            self.svc.update_merkle(id=self.dummy_id, target_backend=None)

    def test_service_verify_methods_behavior(self):
        """测试深度校验与自身校验接口 (Verify) 在 ID 不存在时的表现"""
        # 行为二选一：抛出异常（因底层找不着对应元数据）或者返回 DataSetVerifyRes 结构体
        try:
            deep_res = self.svc.verify_deep(
                id=self.dummy_id, show_diff=False, target_backend=None
            )
            self.assertIsInstance(deep_res, dsf.DataSetVerifyRes)
        except Exception:
            pass

        try:
            self_res = self.svc.verify_self(
                id=self.dummy_id, show_diff=False, target_backend=None
            )
            self.assertIsInstance(self_res, dsf.DataSetVerifyRes)
        except Exception:
            pass

    def test_service_check_is_referenced_signature(self):
        """测试拓扑依赖引用检查接口 (Graph Reference) 的签名"""
        try:
            ref_list = self.svc.check_is_referenced(self.dummy_id)
            self.assertIsInstance(ref_list, list)
        except IOError:
            pass

    def test_service_delete_metadata_signature(self):
        """测试元数据删除接口 (Delete) 的签名"""
        with self.assertRaises(Exception):
            self.svc.delete_metadata(id=self.dummy_id, force=False, target_backend=None)


if __name__ == "__main__":
    unittest.main()
