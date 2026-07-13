#!/usr/bin/env bash
set -e

echo "========================================="
echo "   DataSpringFlow (DSF) 安装分发向导"
echo "========================================="

# 1. 识别当前用户权限与安装模式
IS_SUDO=0
if [ "$EUID" -eq 0 ]; then
  IS_SUDO=1
  echo "[模式] 检测到 root/sudo 权限，将执行 -> 全局系统级安装"
else
  echo "[模式] 检测到普通用户权限，将执行 -> 个人局部级安装"
fi

# 2. 准备临时解压目录
TMP_DIR=$(mktemp -d)
trap 'rm -rf "$TMP_DIR"' EXIT

# 3. 定位自解压边界并提取 tar.gz 数据
MATCH=$(grep -a -n '^__ARCHIVE_BELOW__$' "$0" | cut -d ':' -f 1)
PAYLOAD_LINE=$((MATCH + 1))

tail -n +"${PAYLOAD_LINE}" "$0" | tar -xzf - -C "$TMP_DIR"

# 4. 根据模式执行差异化安装逻辑
if [ "$IS_SUDO" -eq 1 ]; then
  # ==================== 全局安装逻辑 ====================
  BIN_DEST="/usr/local/bin/dsf"
  DATA_DIR="/var/lib/dataspringflow"
  PY_DIR="${DATA_DIR}/py"

  echo "--> 1. 复制二进制文件到系统路径..."
  cp "${TMP_DIR}/dsf" "$BIN_DEST"
  chmod +x "$BIN_DEST"

  echo "--> 2. 执行全局服务初始化 (sudo dsf init --global)..."
  # 调用你的 Rust CLI 进行全局目录和内部数据库建立
  dsf init --global

  echo "--> 3. 部署并共享全局 Python Wheel 包..."
  mkdir -p "$PY_DIR"
  cp ${TMP_DIR}/*.whl "$PY_DIR/"

  echo "--> 4. 规范全局数据目录和共享包的读取权限..."
  chmod -R 755 "$DATA_DIR"

  echo ""
  echo "========================================="
  echo "        DSF 全局系统级安装完成！"
  echo "========================================="
  echo "• 管理员 CLI 路径: $BIN_DEST"
  echo "• 全局共享 Wheel 仓库: $PY_DIR"
  echo "• 用户免网本地安装命令："
  echo "  pip install ${PY_DIR}/dataspringflow-*.whl"

else
  # ==================== 个人用户安装逻辑 ====================
  # 遵循 XDG 规范，优先读取环境变量，否则 fallback 到 ~/.local/share
  XDG_DATA_HOME="${XDG_DATA_HOME:-$HOME/.local/share}"
  USER_BIN_DIR="$HOME/.local/bin"
  BIN_DEST="${USER_BIN_DIR}/dsf"
  DATA_DIR="${XDG_DATA_HOME}/dataspringflow"
  PY_DIR="${DATA_DIR}/py"

  echo "--> 1. 复制二进制文件到用户局部路径..."
  mkdir -p "$USER_BIN_DIR"
  cp "${TMP_DIR}/dsf" "$BIN_DEST"
  chmod +x "$BIN_DEST"

  # 提示普通用户检查自己的 PATH
  if [[ ":$PATH:" != *":$USER_BIN_DIR:"* ]]; then
    echo "警告: $USER_BIN_DIR 不在你的 PATH 环境变量中，请稍后将其加入 ~/.bashrc 或 ~/.config/fish/config.fish"
  fi

  echo "--> 2. 执行用户级服务初始化 (dsf init)..."
  # 调用你的 Rust CLI 建立基于 ProjectDirs 的私有配置与数据库
  "$BIN_DEST" init

  echo "--> 3. 部署私有 Python Wheel 包..."
  mkdir -p "$PY_DIR"
  cp ${TMP_DIR}/*.whl "$PY_DIR/"
  chmod -R 700 "$DATA_DIR" # 个人数据保持私有隔离

  echo ""
  echo "========================================="
  echo "        DSF 个人用户级安装完成！"
  echo "========================================="
  echo "• 个人 CLI 路径: $BIN_DEST"
  echo "• 本地 Wheel 备份路径: $PY_DIR"
  echo "• 免网本地安装命令："
  echo "  pip install ${PY_DIR}/dataspringflow-*.whl"
fi

exit 0

__ARCHIVE_BELOW__
