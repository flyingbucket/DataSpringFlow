import shutil
import unittest
from pathlib import Path

from pydsf import DSFService, BusyStatus, DatasetStatus
import logging

logging.basicConfig(level=logging.DEBUG)


class TestConcurrencyFenceE2E(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        """在当前工作目录下创建真实的物理文件与脚本，用于端到端测试"""
        cls.test_dir = Path("/data/e2e_test_env")
        cls.data_dir = cls.test_dir / "data"
        cls.scripts_dir = cls.test_dir / "scripts"

        cls.data_dir.mkdir(parents=True, exist_ok=True)
        cls.scripts_dir.mkdir(parents=True, exist_ok=True)

        cls.script_path = cls.scripts_dir / "process.py"
        cls.script_path.write_text("print('processing data...')", encoding="utf-8")
        cls.raw_data_dir = cls.test_dir / "raw_dataset"
        cls.clean_data_dir = cls.test_dir / "clean_dataset"

        cls.raw_data_dir.mkdir(parents=True, exist_ok=True)
        cls.clean_data_dir.mkdir(parents=True, exist_ok=True)

        # 文件写在各自的目录内
        cls.raw_data_path = cls.raw_data_dir / "raw_corpus.txt"
        cls.raw_data_path.write_text("initial raw corpus data V1", encoding="utf-8")

        cls.child_data_path = cls.clean_data_dir / "clean_corpus.txt"
        cls.child_data_path.write_text("cleaned corpus data V1", encoding="utf-8")
        cls.service = DSFService()

        # 注册上游基础数据集
        cls.service.register(
            name="test_raw",
            tag="v1",
            path=str(cls.raw_data_dir),
            script_path=str(cls.script_path),
            force_heal=True,
        )

        # 注册下游派生数据集（依赖于 test_raw@v1）
        cls.service.register(
            name="test_clean",
            tag="v1",
            path=str(cls.clean_data_dir),
            script_path=str(cls.script_path),
            dependencies=["test_raw@v1"],
            force_heal=True,
        )

    @classmethod
    def tearDownClass(cls):
        """测试完成后清理注册元数据与物理磁盘文件"""
        try:
            cls.service.mark_status("test_clean@v1", BusyStatus.Free)
            cls.service.mark_status("test_raw@v1", BusyStatus.Free)
            cls.service.delete_metadata("test_clean@v1", force=True)
            cls.service.delete_metadata("test_raw@v1", force=True)
        except Exception as e:
            print(f"Cleanup metadata warning: {e}")
        finally:
            if cls.test_dir.exists():
                shutil.rmtree(cls.test_dir)

    def setUp(self):
        """确保每个测试开始前，栅栏均处于 Free 闲置状态"""
        self.service.mark_status("test_raw@v1", BusyStatus.Free)
        self.service.mark_status("test_clean@v1", BusyStatus.Free)

    def test_01_short_circuit_and_prevent_false_broken_alarm(self):
        """测试核心作用 1 & 2：物理文件被修改时，栅栏能否短路哈希计算并阻止误报 Broken"""
        target_id = "test_raw@v1"

        # 1. 正常状态下验证，应当处于 Healthy
        res_initial = self.service.verify_self(target_id)
        self.assertEqual(res_initial.status, DatasetStatus.Healthy)

        # 2. 立起栅栏，标记为 Modifying
        self.service.mark_status(target_id, BusyStatus.Modifying)

        # 3. 模拟真实的并发写入：对物理磁盘上的文件进行大规模篡改
        with open(self.raw_data_path, "a", encoding="utf-8") as f:
            f.write("\n--- injecting heavy modifications into disk files ---")

        # 4. 在文件哈希已经改变的情况下执行验证
        res_during_mod = self.service.verify_self(target_id)

        # 5. 核心断言：由于 Fence 的存在，系统绝不能误报 Broken！
        # 此时应该短路返回 Busy 状态（在底层可能转化为 busy_modifying 字符串映射或专属状态）
        self.assertNotEqual(
            res_during_mod.status,
            DatasetStatus.Broken,
            "栅栏失效：系统在 Modifying 状态下进行了底层哈希比对并误报了 Broken！",
        )
        self.assertIn(
            "busy",
            str(res_during_mod.status).lower(),
            "状态返回不符合预期，应当反映出数据集当前正处于 busy_modifying 状态",
        )

        # 6. 重新计算封印 Merkle Tree 并在数据库中更新哈希，随后拆除栅栏
        self.service.update_merkle(target_id)
        self.service.mark_status(target_id, BusyStatus.Free)

        # 7. 再次验证，确认新的哈希已经被识别，状态平稳回归 Healthy
        res_final = self.service.verify_self(target_id)
        self.assertEqual(res_final.status, DatasetStatus.Healthy)

    def test_02_dag_cascading_fence_protection(self):
        """测试核心作用 3：上游数据集立起栅栏时，能否级联保护下游 DAG 的深度验证"""
        parent_id = "test_raw@v1"
        child_id = "test_clean@v1"

        # 1. 对上游父数据集立起删除/改造栅栏
        self.service.mark_status(parent_id, BusyStatus.Deleting)

        # 2. 对下游派生数据集执行深度拓扑验证（verify_deep）
        res_child = self.service.verify_deep(child_id)

        # 3. 核心断言：由于依赖树中的父节点正处于 Deleting 忙状态，
        # 下游的验证不应盲目返回 Healthy，其依赖树状态 (dep_status) 必定捕捉到了上游的 Busy 锁
        parent_is_busy = any(
            "busy" in str(status).lower() for status in res_child.dep_status
        )
        self.assertTrue(
            parent_is_busy or res_child.status != DatasetStatus.Healthy,
            "DAG 级联栅栏失效：上游处于 Deleting 状态时，下游 verify_deep 未能捕获锁定信号！",
        )

        # 4. 上游解除栅栏
        self.service.mark_status(parent_id, BusyStatus.Free)

        # 5. 下游再次进行深度验证，DAG 恢复全链路健康
        res_child_recovered = self.service.verify_deep(child_id)
        self.assertEqual(res_child_recovered.status, DatasetStatus.Healthy)
        self.assertTrue(
            all(
                status == DatasetStatus.Healthy
                for status in res_child_recovered.dep_status
            )
        )

    def test_03_read_fence_allows_verification(self):
        """测试边界逻辑：当栅栏为纯读模式 (Reading) 时，不应阻断常规验证"""
        target_id = "test_raw@v1"

        # 设置为并发读取状态
        self.service.mark_status(target_id, BusyStatus.Reading)

        # Reading 状态下磁盘文件是只读安全的，verify 应当允许比对哈希并返回正常状态
        res = self.service.verify_self(target_id)
        self.assertEqual(
            res.status,
            DatasetStatus.Healthy,
            "纯读栅栏 (Reading) 逻辑异常，不应阻断合法的读操作和健康验证",
        )


if __name__ == "__main__":
    unittest.main(verbosity=2)
