# DataSpringFlow

面向深度学习场景的数据集元数据管理工具。  
DataSpringFlow 的核心目标是：**把数据集之间的派生关系组织为 DAG（有向无环图）**，并提供基于哈希的**一致性校验**能力，帮助你在多阶段数据处理流程中稳定管理数据集版本与依赖。

---

## Dev Tips

### StackBackend 与多后端配置

- [x] **StackBackend 核心定义**
  - 单个后端只提供 SQLite 实现，一个 SQLite 后端为一个实际存储数据的后端实例。
  - 后端实例分为两类：用户私有后端（在 `XDG_CONFIG_HOME`）和全局公有后端（在全局路径 `/etc` 配置文件 `/var`数据文件，或remote后端）。
  - 每个公有后端对应一台服务器。
  - StackBackend 将多个后端（用户私有后端、本机公有后端、可能存在的多个远程公有后端）组合起来，形成虚拟单后端视图。
  - 用户家目录下的那一个为该用户的唯一私有后端，其他全部为公有后端。

- [x] **多后端配置规范**
  - 每个后端实例对应一个配置文件。
  - 每个用户必须有一个私有后端及该私有后端的配置文件。
  - 配置文件记录该后端的参数：
    - **属性**：公有 or 私有，DB 文件路径。
    - **SQLite 参数**：连接池大小、WAL 等等。
    - **私有后端特有**：还应记录该用户所有可见的公有后端（以 IP 形式记录，本机全局后端为 `local`，远程后端则为具体 IP 地址）。

- [x] **权限模型设计**
  - 全局初始化时创建用户组 `DSFadmin`。
  - 普通用户对自己的私有后端有完整权限，对所有公有后端有读取权限。
  - 若用户在 `DSFadmin` 用户组，则对本机公有后端有完整读写权限，对远程公有后端仍然只有读权限。

- [x] 数据状态与一致性标记设计:
  - [x] 增加MetaData字段status
  - [x] status对应一个enum，包含free(实际用None表示)（可用）、creating（正在创建）、deleting（正在删除）、modifying（正在修改）、reading（读取并使用）
  - [x] 增加mark_status(id,status)方法来标记status字段
  - [x] 所有增删改查方法根据status字段进行判断是否可用

- [x] 增加mark_status到DatasetBackend trait

- [ ] 前端设计：
  
  - 两大部分：首页与详情页
  - 首页包含一个搜索框，下面就是所有数据一条一条从上到下堆叠起来的卡片。随着搜索，下面的展示卡片不断筛选变少。当进入某一个数据的详情页面之后，左侧的sidebar再显示搜索栏和其他所有数据（相当于小型主页）; 右侧为展示详情页
  - 详情页（detailed_panel)包含三个部分：元数据详细信息、依赖拓扑图、下游衍生数据集列表
  - 把detailed_panel设计成一个小型的框架，每个元素抽取出来（目前是详情、依赖拓扑图、下游衍生数据集列表，三个元素），将来再添加元素就另外设计该元素的html然后动态插入到detailed_panel中，

---

## 设计目标

在实际训练流程中，一个原始数据集往往会经过多次加工，形成多个派生版本,如清洗版、特化版或与其他数据集混合、筛选所得的新数据集。

这些数据集之间天然存在“从上游派生到下游”的依赖关系。  
DataSpringFlow 把这种关系显式建模为 DAG，并围绕 DAG 提供：

1. 元数据统一注册（`name@tag`）
2. 依赖关系追踪
3. 哈希一致性校验（自身 + 依赖子图）

## 当前能力概览

- **数据集标识**：使用 `name@tag` 唯一标识数据集
- **元数据管理**：注册、查询、列出、删除元数据
- **依赖关系建模**：在注册时声明 `dependencies`
- **一致性校验**：
  - 校验单个数据集（self）
  - 沿依赖链深度校验（deep）
- **引用检查**：检查某数据集是否被其他数据集引用
- **Merkle 树相关信息**：用于一致性校验结果支撑（而非删除恢复）

## 安装

### 从源码安装

```bash
git clone https://github.com/flyingbucket/DataSpringFlow.git
cd DataSpringFlow
pip install -e .
```

## Python 版本与构建

- Python: `>=3.9`
- 构建系统：`maturin`
- Rust 扩展模块：`dataspringflow.dataspringflow_rs`

项目通过 Rust + PyO3 暴露核心能力，Python 侧主要是 API 入口与类型接口。

## 快速开始

```python
from dataspringflow import DSFService

svc = DSFService()

# 1) 注册一个“根数据集”
svc.register(
    name="raw_images",
    tag="v1",
    path="/data/raw_images_v1",
    script_path="/workspace/scripts/prepare_raw.py",
    dependencies=None,           # 或 []
    description_path=None,
)

# 2) 注册一个派生数据集（依赖 raw_images@v1）
svc.register(
    name="train_split",
    tag="v1",
    path="/data/train_split_v1",
    script_path="/workspace/scripts/make_train_split.py",
    dependencies=["raw_images@v1"],
)

# 3) 查询元数据
meta = svc.query_meta("train_split@v1")
print(meta.id())          # train_split@v1
print(meta.path)
print(meta.dependencies)  # ["raw_images@v1"]

# 4) 校验自身一致性
res_self = svc.verify_self("train_split@v1", show_diff=True)
print(res_self.status)

# 5) 深度校验（包含依赖）
res_deep = svc.verify_deep("train_split@v1", show_diff=True)
print(res_deep.status, res_deep.dep_status)

# 6) 查看是否被引用
ref_by = svc.check_is_referenced("raw_images@v1")
print(ref_by)

# 7) 列出全部元数据
all_meta = svc.list_all_metadata()
for m in all_meta:
    print(m.id(), m.path)
```

## 核心对象与接口（当前代码）

## `DSFService`

服务入口，负责数据集元数据生命周期与校验调用。

主要方法：

- `query_meta(id: str) -> MetaData`
- `register(name, tag, path, script_path, dependencies=None, description_path=None, force_heal=False, yes=False) -> None`
- `update_merkle(id: str) -> None`
- `delete_metadata(id: str, force=False) -> None`
- `verify_deep(id: str, show_diff=False) -> DataSetVerifyRes`
- `verify_self(id: str, show_diff=False) -> DataSetVerifyRes`
- `list_all_metadata() -> list[MetaData]`
- `check_is_referenced(target_id: str) -> list[str]`

## `MetaData`

当前暴露字段（以实际类型声明为准）：

- `name: str`
- `tag: str`
- `hash: str`
- `path: str`
- `description_path: str`
- `script_path: str`
- `dependencies: list[str]`
- `merkle_tree_path: str`

方法：

- `id() -> str`（`name@tag`）

## `DatasetStatus`

当前状态枚举：

- `Healthy`
- `Broken`
- `BrokenDeps`
- `Unverified`

## `DataSetVerifyRes`

校验结果对象：

- `status: DatasetStatus`：目标数据集状态
- `dep_status: list[DatasetStatus]`：依赖相关状态集合

## `DSFDataset`

数据集对象（由底层返回）：

- `metadata` 属性
- `detailed_status` 属性
- `verify(_backend_auth, _show_diff=False)`

## DAG 组织约定

DataSpringFlow 的关键约定是：  
**每个数据集在注册时声明其上游依赖（`dependencies`），形成有向无环图。**

- 节点：数据集（`name@tag`）
- 边：`child -> parent`（或“当前数据集依赖哪些上游”）
- 语义：下游可追溯来源，上游变动可影响下游可信度

建议实践：

1. 让 `tag` 明确表达版本语义（如 `v1`, `v2`, `2026-07`）
2. 每次数据内容发生实质变化时创建新 tag
3. 保持 `script_path` 可追溯，便于团队理解派生过程
4. 在训练前执行 `verify_self` 或 `verify_deep`

## 关于哈希与一致性校验

当前版本中，哈希能力用于：

- 判断数据集内容是否与登记状态一致
- 辅助定位“当前数据集是否发生变动”
- 支撑依赖链校验中的健康判断

**不用于**数据删除后自动恢复或重建编排。

## 许可证

请参考仓库中的 `LICENSE` 文件。
