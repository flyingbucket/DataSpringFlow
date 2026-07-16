import os
from pydsf import DSFService, BackendAddr, DatasetStatus
from mock_creator import (
    create_shell_dir,
    generate_blurred_mnist,
    generate_imagenet_features,
    generate_multimodal_data,
)


def main():
    service = DSFService()
    # 声明私有后端及全局参考后端
    private_backend = BackendAddr.private()

    public_base = os.path.abspath("examples/example_data/public")
    private_base = os.path.abspath("examples/example_data/private")
    script_dir = os.path.abspath("examples/example_data/scripts")

    os.makedirs(private_base, exist_ok=True)

    derivatives = [
        {
            "name": "mnist_blurred",
            "tag": "v1.0",
            "path": os.path.join(private_base, "mnist_blurred_v1.0"),
            "script_path": os.path.join(script_dir, "blur_mnist.py"),
            "dependencies": ["mnist@v1.0"],
            "generator": lambda dest: generate_blurred_mnist(
                os.path.join(public_base, "mnist_v1.0"), dest
            ),
        },
        {
            "name": "imagenet_features",
            "tag": "v1.0",
            "path": os.path.join(private_base, "imagenet_features_v1.0"),
            "script_path": os.path.join(script_dir, "extract_resnet.py"),
            "dependencies": ["imagenet_subset@v1.0"],
            "generator": lambda dest: generate_imagenet_features(
                os.path.join(public_base, "imagenet_subset_v1.0"), dest
            ),
        },
        {
            "name": "multimodal_dataset",
            "tag": "v1.0",
            "path": os.path.join(private_base, "multimodal_dataset_v1.0"),
            "script_path": os.path.join(script_dir, "align_multimodal.py"),
            "dependencies": ["wikitext@v1.0", "imagenet_subset@v1.0"],
            "generator": lambda dest: generate_multimodal_data(
                os.path.join(public_base, "wikitext_v1.0"),
                os.path.join(public_base, "imagenet_subset_v1.0"),
                dest,
            ),
        },
    ]

    for ds in derivatives:
        print(f"\n🔒 === 开始注册私有衍生数据集: {ds['name']}@{ds['tag']} ===")
        print(f"⛓️  依赖链: {ds['dependencies']}")

        # 0. 写入伪 pipeline 脚本
        with open(ds["script_path"], "w") as f:
            f.write(f"#!/usr/bin/env python\n# Processing to generate {ds['name']}\n")

        # 1. 建立空壳目录并首次注册
        print("[Step 1/4] 创建私有空壳结构并初始注册...")
        create_shell_dir(ds["path"])
        service.register(
            name=ds["name"],
            tag=ds["tag"],
            path=ds["path"],
            script_path=ds["script_path"],
            owner_nickname="private_dev",
            dependencies=ds["dependencies"],
            target_backend=private_backend,
        )

        # 2. 标注为 Creating 状态
        ds_id = f"{ds['name']}@{ds['tag']}"
        print(f"[Step 2/4] 将 {ds_id} 标注为 BusyCreating...")
        service.mark_status(
            id=ds_id, status=DatasetStatus.BusyCreating, target_backend=private_backend
        )

        # 3. 运行生成流水线，生成派生文件
        print("[Step 3/4] 运行派生流水线产生实际衍生数据...")
        ds["generator"](ds["path"])

        # 4. 重新注册锁死
        print("[Step 4/4] 落盘完成，更新元数据和最终哈希...")
        service.register(
            name=ds["name"],
            tag=ds["tag"],
            path=ds["path"],
            script_path=ds["script_path"],
            owner_nickname="private_dev",
            dependencies=ds["dependencies"],
            target_backend=private_backend,
        )
        print(f"✅ {ds_id} 在私有后端注册成功！")

    # ================= 拓扑及数据深度校验验证 =================
    print("\n🔍 ===========================================")
    print("🚦 启动对 'multimodal_dataset@v1.0' 的深度依赖拓扑校验...")
    print("=============================================")

    # 跨后端校验：multimodal_dataset 在 private 中，而 wikitext 和 imagenet 在 global 中
    verify_res = service.verify_deep(
        id="multimodal_dataset@v1.0", show_diff=True, target_backend=private_backend
    )

    print(f"\n🎯 校验结果:")
    print(f"  - 衍生集状态: {verify_res.status}")
    print(f"  - 依赖集状态: {verify_res.dep_status}")


if __name__ == "__main__":
    main()
