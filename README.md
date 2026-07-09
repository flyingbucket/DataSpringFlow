# DataSpringFlow

一个用于深度学习数据集管理的 Python 工具，提供数据集版本控制、完整性校验、依赖追踪和**可恢复删除**功能。

## 设计背景

### 问题场景

在高校深度学习课题组中，算力资源通常采用**单服务器、多用户**的共享模式。在实际研究过程中，一个原始数据集往往会经过多次处理产生多种变体：

```
原始数据集
    ├── 数据清洗 → 清洗后数据集
    ├── 图片切割 → 切割后数据集
    ├── 数据增强 → 增强后数据集
    └── 标注修正 → 修正后数据集
         └── 格式转换 → 最终训练集
```

这些"子数据集"通常由**与项目逻辑耦合的处理脚本**生成。随着项目的推进，派生数据集会不断累积，占用大量磁盘空间。

### 核心痛点

1. **磁盘空间紧张** - 派生数据集占用大量存储，但项目结束后再次使用的概率很低
2. **依赖关系混乱** - 难以追踪数据集之间的派生关系
3. **数据完整性风险** - 共享环境下数据可能被意外修改或删除
4. **恢复困难** - 删除后的数据集难以重建

### 解决方案

DataSpringFlow 将原始数据集比作“泉水”，将子数据集比作“河流”，采用以下设计来解决上述问题：

| 组件 | 功能 | 解决的问题 |
|------|------|-----------|
| **元数据注册表** | 记录数据集路径、名称、标签、生成脚本等信息 | 依赖关系混乱 |
| **DAG 依赖图** | 使用有向无环图描述数据集之间的派生关系 | 依赖关系追踪 |
| **Merkle 哈希树** | 对数据集进行一致性和完整性校验 | 数据完整性风险 |
| **可恢复删除** | 保留元数据和生成脚本，支持按需重建数据 | 磁盘空间 & 恢复困难 |

### 工作流程

```
┌─────────────────────────────────────────────────────────────────┐
│                     DataSpringFlow 工作流程                       │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  1. 注册阶段                                                     │
│     ┌──────────┐    处理脚本    ┌──────────┐                    │
│     │ 父数据集  │ ────────────→ │ 子数据集  │                    │
│     └──────────┘               └──────────┘                    │
│           │                          │                          │
│           └──────── 注册元数据 ────────┘                          │
│                         ↓                                       │
│              ┌───────────────────┐                              │
│              │  Registry (注册表) │                              │
│              │  - 路径、名称、标签  │                              │
│              │  - Merkle 哈希树   │                              │
│              │  - 依赖关系 (DAG)  │                              │
│              │  - 生成脚本路径    │                              │
│              └───────────────────┘                              │
│                                                                 │
│  2. 使用阶段                                                     │
│     - 通过 name@tag 查询数据集                                    │
│     - 使用 Merkle 树校验数据完整性                                 │
│     - 遍历 DAG 检查上游依赖健康状况                                │
│                                                                 │
│  3. 清理阶段（项目结束后）                                         │
│     - 删除子数据集文件，释放磁盘空间                                │
│     - 保留元数据和生成脚本信息                                     │
│                                                                 │
│  4. 恢复阶段（需要时）                                            │
│     - 根据 DAG 找到父数据集                                       │
│     - 执行生成脚本重建子数据集                                     │
│     - 使用 Merkle 树校验恢复后的数据一致性                          │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

## 特性

- **Merkle Tree 哈希校验** - 使用 Merkle 树结构对数据集进行快速完整性校验
- **数据集版本管理** - 通过 `name@tag` 的方式管理数据集的不同版本
- **依赖关系追踪 (DAG)** - 构建数据集之间的有向无环图，追踪上下游派生关系
- **可恢复删除** - 删除数据集后保留元数据，支持通过生成脚本按需重建
- **并行哈希计算** - 支持多线程并行计算大文件哈希，提升性能
- **可插拔后端** - 支持不同的元数据存储后端（如 YAML）
- **异步支持** - 提供同步和异步 API，适配不同使用场景（包括 Jupyter Notebook）

## 安装

### 从源码安装

```bash
git clone https://github.com/flyingbucket/DataSpringFlow.git
cd DataSpringFlow
pip install -e .
```

### 依赖项

- Python >= 3.9
- pyyaml
- joblib

## 快速开始

### 创建数据集注册表

```python
from dataspringflow.core.registry import DSFRegistry
from dataspringflow.core.metadata import MetadataBuilder
from pathlib import Path

# 初始化注册表
registry = DSFRegistry(backend="yaml")

# 创建数据集元数据
builder = MetadataBuilder(
    path=Path("/path/to/your/dataset"),
    name="my_dataset",
    tag="v1.0",
    dependencies=()  # 依赖的其他数据集 ID，如 ("other_dataset@v1.0",)
)

# 构建并保存
metadata, hash_snapshot = builder.build()
registry.save(metadata, hash_snapshot)
```

### 获取数据集

```python
# 通过 name@tag 获取数据集
dataset = registry.get("my_dataset@v1.0")

# 访问元数据
print(dataset.info.name)
print(dataset.info.tag)
print(dataset.info.path)
print(dataset.info.hash)
```

### 校验数据集完整性

```python
# 校验单个数据集
is_valid, diff = dataset.verify()

if is_valid:
    print("数据集完整性校验通过！")
else:
    print("数据集已被修改：")
    print(f"  新增文件: {diff.added}")
    print(f"  删除文件: {diff.removed}")
    print(f"  修改文件: {diff.modified}")
```

### 校验依赖链

```python
# 校验所有上游依赖数据集
is_healthy, broken_datasets = dataset.DAG.verify()

if is_healthy:
    print("所有依赖数据集完整性校验通过！")
else:
    print(f"以下数据集已损坏: {broken_datasets}")
```

### 遍历依赖图

```python
# 遍历所有依赖
for dep_metadata in dataset.DAG.iter_DAG():
    print(f"依赖: {dep_metadata.id}")
```

## 项目结构

```
dataspringflow/
├── core/
│   ├── dataset.py    # 数据集用户接口
│   ├── merkle.py     # Merkle 树实现
│   ├── metadata.py   # 元数据定义与构建
│   ├── registry.py   # 数据集注册表
│   └── dag.py        # 依赖关系图
├── backend/
│   ├── hash_io.py    # 哈希数据 I/O
│   └── metadata_io.py # 元数据 I/O
├── utils/
│   ├── env.py        # 运行环境检测
│   ├── fs.py         # 文件系统工具
│   └── hash.py       # 哈希计算工具
├── protocols.py      # 协议定义（接口）
└── factory.py        # 工厂模式实现
```

## 核心概念

### Metadata（元数据）

每个数据集包含以下元数据：

| 字段 | 描述 |
|------|------|
| `name` | 数据集名称 |
| `tag` | 版本标签 |
| `path` | 数据集路径 |
| `hash` | Merkle 树根哈希 |
| `dependencies` | 依赖的数据集 ID 列表 |
| `script_path` | 生成脚本路径（可选） |

### FileMerkleTree（文件 Merkle 树）

使用 MD5 哈希构建目录的 Merkle 树：

- 叶子节点：文件的内容哈希 或 空目录的路径哈希
- 非叶子节点：子节点哈希的有序组合

支持并行计算以提升大型数据集的处理性能。

### DAG（有向无环图）

数据集之间的依赖关系通过 DAG 表示，支持：

- 遍历所有上游依赖
- 批量校验依赖链的完整性

## 后端扩展

DataSpringFlow 使用协议（Protocol）定义后端接口，您可以实现自定义后端：

```python
from dataspringflow.protocols import MetadataLoader, HashDictLoader, AtomicWriter, RegistryFactory

class MyCustomLoader(MetadataLoader):
    def load(self, name: str, tag: str) -> Metadata:
        # 自定义加载逻辑
        ...

class MyCustomFactory(RegistryFactory):
    def create_metadata_loader(self) -> MetadataLoader:
        return MyCustomLoader()
    # ... 实现其他方法
```
