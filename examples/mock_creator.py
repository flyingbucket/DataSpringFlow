import os
import shutil


def create_shell_dir(path: str):
    """创建空壳数据集目录，置入临时占位符以确保路径在注册时存在"""
    if os.path.exists(path):
        shutil.rmtree(path)
    os.makedirs(path, exist_ok=True)
    with open(os.path.join(path, ".shell_placeholder"), "w") as f:
        f.write("temporary_shell_for_registration")


def generate_mnist_data(path: str):
    """模拟 MNIST 原始数据落盘"""
    os.makedirs(path, exist_ok=True)
    # 清理占位符
    shutil.rmtree(path)
    os.makedirs(path)
    with open(os.path.join(path, "train-images.idx3-ubyte"), "wb") as f:
        f.write(b"\x00\x00\x08\x03" + b"\x00\x00\x00\n" + b"\x1c" * 28 * 28)
    with open(os.path.join(path, "train-labels.idx1-ubyte"), "wb") as f:
        f.write(b"\x00\x00\x08\x01" + b"\x00\x00\x00\n" + b"\x05" * 10)


def generate_imagenet_data(path: str):
    """模拟 ImageNet 原始子集落盘"""
    os.makedirs(path, exist_ok=True)
    shutil.rmtree(path)
    os.makedirs(path)
    for category in ["cat", "dog"]:
        cat_dir = os.path.join(path, category)
        os.makedirs(cat_dir, exist_ok=True)
        for i in range(2):
            with open(os.path.join(cat_dir, f"img_{i}.jpg"), "wb") as f:
                f.write(
                    b"MOCK_JPEG_DATA_"
                    + bytes(category, "utf-8")
                    + bytes(str(i), "utf-8")
                )


def generate_wikitext_data(path: str):
    """模拟 WikiText 文本数据落盘"""
    os.makedirs(path, exist_ok=True)
    shutil.rmtree(path)
    os.makedirs(path)
    with open(os.path.join(path, "wiki.train.tokens"), "w", encoding="utf-8") as f:
        f.write(
            "= Nankai University =\nDataSpringFlow is a lightweight metadata management system.\n"
        )


def generate_blurred_mnist(src_path: str, dest_path: str):
    """模拟对 MNIST 应用高斯模糊的任务流"""
    os.makedirs(dest_path, exist_ok=True)
    shutil.rmtree(dest_path)
    os.makedirs(dest_path)
    with open(os.path.join(dest_path, "blurred_images.idx3-ubyte"), "wb") as f:
        f.write(b"BLURRED_DATA_STREAM_" + b"\x1c" * 28 * 28)


def generate_imagenet_features(src_path: str, dest_path: str):
    """模拟 ResNet 提取 ImageNet 特征向量的任务流"""
    os.makedirs(dest_path, exist_ok=True)
    shutil.rmtree(dest_path)
    os.makedirs(dest_path)
    with open(
        os.path.join(dest_path, "resnet50_embeddings.json"), "w", encoding="utf-8"
    ) as f:
        f.write(
            '{"cat/img_0.jpg": [0.12, -0.45, 0.88], "dog/img_0.jpg": [0.05, 0.99, -0.12]}'
        )


def generate_multimodal_data(text_path: str, img_path: str, dest_path: str):
    """模拟跨模态对齐关联任务流"""
    os.makedirs(dest_path, exist_ok=True)
    shutil.rmtree(dest_path)
    os.makedirs(dest_path)
    with open(os.path.join(dest_path, "aligned_pairs.csv"), "w", encoding="utf-8") as f:
        f.write("text,image_path\n'Nankai statistics description', 'cat/img_0.jpg'\n")
