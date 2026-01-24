from pathlib import Path


def get_running_environment() -> str:
    """
    判断当前运行环境
    返回：
        'script'   - 普通 Python 脚本
        'notebook' - Jupyter Notebook
        'ipython'  - IPython REPL
        'python'   - 普通 Python REPL
    """
    try:
        _ = Path(__file__)
        return "script"
    except NameError:
        try:
            from IPython.core.getipython import get_ipython

            ipy = get_ipython()
            if ipy is None:
                return "python"  # 普通 REPL
            elif "IPKernelApp" in ipy.config:
                return "notebook"  # Jupyter Notebook
            else:
                return "ipython"  # IPython REPL
        except ImportError:
            return "python"  # 没有 IPython，普通 REPL
