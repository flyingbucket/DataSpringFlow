import os
from pydsf import DSFService, BackendAddr, DatasetStatus
from mock_creator import (
    create_shell_dir,
    generate_mnist_data,
    generate_imagenet_data,
    generate_wikitext_data,
)


def main():
    service = DSFService()
    # 映射到 Local Global (全局数据库后端)
    global_backend = BackendAddr.local_global()

    base_dir = os.path.abspath("examples/example_data/public")
    script_dir = os.path.abspath("examples/example_data/scripts")
    os.makedirs(base_dir, exist_ok=True)
    os.makedirs(script_dir, exist_ok=True)

    public_datasets = [
        {
            "name": "mnist",
            "tag": "v1.0",
            "path": os.path.join(base_dir, "mnist_v1.0"),
            "script_path": os.path.join(script_dir, "build_mnist.sh"),
            "generator": generate_mnist_data,
        },
        {
            "name": "imagenet_subset",
            "tag": "v1.0",
            "path": os.path.join(base_dir, "imagenet_subset_v1.0"),
            "script_path": os.path.join(script_dir, "build_imagenet.sh"),
            "generator": generate_imagenet_data,
        },
        {
            "name": "wikitext",
            "tag": "v1.0",
            "path": os.path.join(base_dir, "wikitext_v1.0"),
            "script_path": os.path.join(script_dir, "build_wikitext.sh"),
            "generator": generate_wikitext_data,
        },
    ]

    for ds in public_datasets:
        print(f"\n🚀 === 开始注册公共数据集: {ds['name']}@{ds['tag']} ===")

        # 0. 生成占位 pipeline bash 脚本
        with open(ds["script_path"], "w") as f:
            f.write(f"#!/bin/bash\necho 'Constructing {ds['name']}'\n")

        # 1. 建立空壳目录并首次注册
        print("[Step 1/4] 创建空壳结构并初始注册...")
        create_shell_dir(ds["path"])
        service.register(
            name=ds["name"],
            tag=ds["tag"],
            path=ds["path"],
            script_path=ds["script_path"],
            owner_nickname="public_mirror",
            target_backend=global_backend,
        )

        # 2. 标注为 Creating 状态
        ds_id = f"{ds['name']}@{ds['tag']}"
        print(f"[Step 2/4] 将 {ds_id} 标注为 BusyCreating...")
        service.mark_status(
            id=ds_id, status=DatasetStatus.BusyCreating, target_backend=global_backend
        )

        # 3. 实际构造落盘数据 (与 metadata 解耦)
        print("[Step 3/4] 实际构建生成本地 mock 数据...")
        ds["generator"](ds["path"])

        # 4. 数据落盘完成后，再次注册更新最终元数据及哈希
        print("[Step 4/4] 落盘完成，重新注册更新 Merkle 哈希...")
        service.register(
            name=ds["name"],
            tag=ds["tag"],
            path=ds["path"],
            script_path=ds["script_path"],
            owner_nickname="public_mirror",
            target_backend=global_backend,
        )
        print(f"✅ {ds_id} 注册成功！")


if __name__ == "__main__":
    main()
