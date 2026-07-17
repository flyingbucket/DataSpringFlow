# DataSpringFlow

面向深度学习场景的数据集元数据管理工具。
DataSpringFlow 的核心目标是：**把数据集之间的派生关系组织为 DAG（有向无环图）**，并提供基于哈希的**一致性校验**能力，帮助您在多阶段数据处理流程中稳定管理数据集版本与依赖。

---

## 项目简介

### 功能特性

DataSpringFlow 是一个**轻量级、非侵入式**的数据集元数据管理系统。在各类研究任务中，一个原始数据集往往会经过多次加工，形成多个派生版本（如清洗版、特化版或与其他数据集混合、筛选所得的新数据集）。这些数据集之间天然存在"从上游派生到下游"的依赖关系。

DataSpringFlow 把这种关系显式建模为 **DAG（有向无环图）**，并围绕 DAG 提供以下核心能力：

1. **元数据统一注册**：使用 `name@tag` 唯一标识数据集，支持注册、查询、列出、删除元数据
2. **依赖关系追踪**：在注册时声明 `dependencies`，自动构建数据集间的派生关系图
3. **哈希一致性校验**：
   - 自身校验（`verify_self`）：校验单个数据集内容是否与登记状态一致
   - 深度校验（`verify_deep`）：沿依赖链深度校验整个子图的健康状态
4. **引用检查**：检查某数据集是否被其他数据集引用，防止误删
5. **Merkle 树支撑**：用于一致性校验结果的底层数据结构
6. **多后端架构**：支持用户私有后端和全局公有后端的堆叠视图
7. **Web UI 界面**：提供可视化的数据集浏览、详情查看和依赖拓扑图展示

### 技术特点

- **Rust + Python 混合架构**：核心逻辑用 Rust 实现，通过 PyO3 暴露 Python 接口,零python依赖，不会污染您的虚拟环境。
- **SQLite 后端**：轻量级数据库存储，无常驻进程，不额外占用服务器资源。
- **BLAKE3 哈希**：高性能加密哈希算法用于数据一致性校验
- **HTMX + Alpine.js 前端**：现代轻量级 Web 技术栈，无需复杂前端框架
- **跨后端查询**：支持同时查询私有和全局后端的数据集信息

---

## 安装方式

### 系统要求

- Python: `>=3.9`
- Linux/Unix 系统（依赖 XDG 目录规范）
*若从源码安装则另需rust工具链*

### 从源码安装

```bash
# 克隆仓库
git clone https://github.com/flyingbucket/DataSpringFlow.git
cd DataSpringFlow

# 安装 Python 包（自动构建 Rust 扩展）
pip install -e .

cd rust 
cargo build --release # 生成dsf cli工具
```

### 构建说明

- 构建系统：`maturin`
- Rust 模块名：`pydsf`
- 配置文件位置：
  - 用户配置：`$XDG_CONFIG_HOME/dataspringflow/config.yaml`
  - 全局配置：`/etc/dataspringflow/config.yaml`
  - 数据文件：`$XDG_DATA_HOME/dataspringflow/dsf.db` 或 `/var/lib/dataspringflow/dsf.db`

---

## 用户接口使用说明

### 1. Python API

#### 初始化服务

```python
from pydsf import DSFService, BackendAddr, DatasetStatus

# 创建服务实例（自动加载配置）
svc = DSFService()
```

#### 注册数据集

```python
# 注册一个"根数据集"（无依赖）
svc.register(
    name="raw_images",
    tag="v1",
    path="/data/raw_images_v1",
    script_path="/workspace/scripts/prepare_raw.py",
    owner_nickname="myteam",      # 可选：所有者昵称
    description_path=None,         # 可选：描述文件路径
    dependencies=[],               # 可选：依赖列表
    target_backend=BackendAddr.private(),  # 可选：指定后端
)

# 注册一个派生数据集（依赖 raw_images@v1）
svc.register(
    name="train_split",
    tag="v1",
    path="/data/train_split_v1",
    script_path="/workspace/scripts/make_train_split.py",
    dependencies=["raw_images@v1"],
    target_backend=BackendAddr.private(),
)
```

#### 查询元数据

```python
# 查询单个数据集
metas = svc.query_meta("train_split@v1")
for scoped_meta in metas:
    backend_addr = scoped_meta.backend  # 后端地址
    meta = scoped_meta.metadata          # 元数据对象
    print(f"ID: {meta.id()}")
    print(f"Path: {meta.path}")
    print(f"Hash: {meta.hash}")
    print(f"Dependencies: {meta.dependencies}")
```

#### 一致性校验

```python
# 校验自身一致性
res_self = svc.verify_self("train_split@v1", show_diff=True)
print(f"Status: {res_self.status}")  # Healthy, Broken, BrokenDeps, Unverified

# 深度校验（包含所有依赖）
res_deep = svc.verify_deep("train_split@v1", show_diff=True)
print(f"Dataset Status: {res_deep.status}")
print(f"Dependency Statuses: {res_deep.dep_status}")
```

#### 引用检查与删除

```python
# 检查是否被其他数据集引用
referrers = svc.check_is_referenced("raw_images@v1")
print(f"Referenced by: {referrers}")

# 删除元数据（不删除实际数据）
svc.delete_metadata("train_split@v1", force=False)  # force=True 强制删除
```

#### 列出所有数据集

```python
all_metas = svc.list_all_metadata()
for scoped_meta in all_metas:
    backend, meta = scoped_meta
    print(f"[{backend}] {meta.id()} -> {meta.path}")
```

#### 状态标记（高级用法）

```python
# 标记数据集为创建中状态
svc.mark_status(
    id="new_dataset@v1",
    status=DatasetStatus.BusyCreating,
    target_backend=BackendAddr.private()
)
```

---

### 2. CLI 命令行工具

安装后可直接使用 `dsf` 命令：

#### 初始化配置

```bash
# 用户级初始化（默认）
dsf init

# 全局初始化（需要 root 权限）
sudo dsf init --global
```

#### 查看配置

```bash
dsf show-config
```

#### 授予管理员权限

```bash
# 授予指定用户 DSFadmin 权限,<username>参数为空则默认使用$USER环境变量，即当前用户
dsf grant <username>
```

#### 查询数据集状态

```bash
# 查询元数据
dsf query mnist@v1

# 自我校验
dsf query mnist@v1 --level self-only

# 深度校验（包含依赖）
dsf query multimodal@v1 --level deep --show-diff

# 查询全局注册表
dsf query imagenet@v1 --global
```

#### 注册数据集

```bash
dsf register \
    --name mnist_blurred \
    --tag v1.0 \
    --path /data/mnist_blurred \
    --script-path /scripts/blur.py \
    --deps mnist@v1.0 \
    --owner-nickname dev_team
```

#### 更新哈希

```bash
# 重新计算并更新数据集哈希
dsf update mnist@v1

# 更新全局注册表中的数据集
dsf update mnist@v1 --global
```

#### 删除数据集

```bash
# 交互式删除
dsf delete mnist@v1

# 强制删除（忽略引用检查）
dsf delete mnist@v1 --force

# 非确认模式
dsf delete mnist@v1 --yes
```

#### 启动 Web UI

```bash
# 默认监听 0.0.0.0:8080
dsf serve

# 指定地址和端口
dsf serve --host 127.0.0.1 --port 3000
```

---

### 3. Web UI 界面

启动 Web 服务后，访问 `http://localhost:8080` 即可使用图形界面。

#### 首页功能

- **搜索框**：支持按名称、标签、所有者搜索数据集
- **数据集卡片**：显示所有已注册的数据集，点击可进入详情页

#### 工作台（Workspace）

访问 `/workspace?id=name@tag` 进入双栏工作界面：

- **左侧边栏**：
  - 迷你搜索框：快速筛选数据集列表
  - 数据集卡片：紧凑展示，点击切换右侧详情
  - URL 自动同步：切换数据集时更新浏览器地址

- **右侧详情面板**：
  1. **元数据详细信息**：名称、标签、路径、哈希值、所有者、依赖列表等
  2. **依赖拓扑图**：可视化展示上游依赖关系（DAG 结构）
  3. **下游引用列表**：显示哪些数据集依赖了当前数据集

#### 技术特性

- **HTMX 局部刷新**：无需整页重载，点击即更新详情
- **Alpine.js 交互**：轻量级前端状态管理
- **响应式设计**：适配不同屏幕尺寸

---

## 核心概念

### 数据集标识

使用 `name@tag` 格式唯一标识数据集：

- `name`：数据集名称（如 `mnist`, `imagenet_subset`）
- `tag`：版本标签（如 `v1`, `v1.0`, `2026-07`）

### 后端架构

- **私有后端**：每个用户独立的后端，位于用户家目录下
- **全局后端**：系统级共享后端，位于 `/var/lib/dataspringflow`
- **StackBackend**：将多个后端组合成虚拟单后端视图

### 权限模型

- 普通用户：对自己的私有后端有完整权限，对全局后端只有读权限
- DSFadmin 组成员：对本机全局后端有读写权限，对远程全局后端只有读权限

### 状态枚举

- `Healthy`：健康状态
- `Broken`：数据损坏或不一致
- `BrokenDeps`：依赖项有问题
- `Unverified`：未校验状态
- `BusyCreating/Reading/Modifying/Deleting`：忙碌状态（占用中）

---

## 最佳实践

1. **版本管理**：让 `tag` 明确表达版本语义，每次数据内容发生实质变化时创建新 tag
2. **脚本追溯**：保持 `script_path` 可追溯，便于团队理解派生过程
3. **训练前校验**：在训练前执行 `verify_self` 或 `verify_deep` 确保数据一致性
4. **依赖声明**：注册时准确声明 `dependencies`，确保 DAG 完整性
5. **删除谨慎**：删除前先用 `check_is_referenced` 检查是否有下游依赖

---

## 项目结构

```
DataSpringFlow/
├── rust/                    # Rust 核心代码
│   ├── dsf-core/           # 核心业务逻辑（后端、服务、DAG）
│   ├── dsf-py/             # Python 绑定（PyO3）
│   ├── dsf-cli/            # 命令行工具
│   ├── dsf-web/            # Web UI 服务
│   └── dsf-api/            # API 数据类型定义
├── src/pydsf/              # Python 包入口
├── examples/               # 使用示例
├── tests/                  # 测试用例
└── benchmarks/             # 性能基准测试
```

---

## 许可证

请参考仓库中的 `LICENSE` 文件。
