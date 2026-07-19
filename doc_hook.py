import shutil
from pathlib import Path


def on_post_build(config, **kwargs):
    site_dir = Path(config["site_dir"])
    rustdoc_src = Path("rust/target/doc")
    rustdoc_dst = site_dir / "rustdoc"

    if rustdoc_src.exists():
        # 如果已存在先清理
        if rustdoc_dst.exists():
            shutil.rmtree(rustdoc_dst)

        # 复制静态资源
        shutil.copytree(rustdoc_src, rustdoc_dst)

        # 写入重定向首页
        redirect_html = (
            '<meta http-equiv="refresh" content="0; url=dsf_core/index.html">'
        )
        (rustdoc_dst / "index.html").write_text(redirect_html)

        print(
            "INFO    -  Successfully integrated Cargo docs into MkDocs site container."
        )
