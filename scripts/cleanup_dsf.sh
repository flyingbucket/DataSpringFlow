#!/bin/bash

# 确保脚本是以 root 权限运行（因为涉及删除系统目录和修改用户组）
if [ "$EUID" -ne 0 ]; then
  echo -e "\e[31m\e[1m错误：此清理脚本需要 root 权限。请使用 sudo 运行！\e[0m"
  exit 1
fi

echo -e "\e[36m\e[1m[DSF Cleanup] 开始清理 DataSpringFlow 全局配置与数据...\e[0m"

# 1. 删除全局数据和配置文件目录
echo "正在删除系统数据和配置文件..."
rm -rf /var/lib/dataspringflow
rm -rf /etc/dataspringflow

# 2. 清理可能残留在 /tmp 中的临时配置文件
rm -f /tmp/dsf_global_config_temp.yaml

# 3. 将所有普通用户从 DSFadmin 组中移除
# 这一步是为了防止直接 delgroup 时报 "cannot remove the primary group of user"
# 或者用户残留附加组凭证
if getent group DSFadmin >/dev/null; then
  echo "正在从 DSFadmin 组中移除关联用户..."
  # 遍历 /etc/group 找到所有属于 DSFadmin 组的用户并将其移除
  USERS=$(getent group DSFadmin | cut -d: -f4 | tr ',' ' ')
  for user in $USERS; do
    if [ -n "$user" ]; then
      echo "  -> 正在移除用户: $user"
      gpasswd -d "$user" DSFadmin 2>/dev/null || deluser "$user" DSFadmin 2>/dev/null
    fi
  done

  # 4. 删除用户组 DSFadmin
  echo "正在删除用户组 DSFadmin..."
  if delgroup DSFadmin 2>/dev/null || groupdel DSFadmin 2>/dev/null; then
    echo -e "\e[32m成功删除用户组 DSFadmin。\e[0m"
  else
    echo -e "\e[31m警告：删除用户组 DSFadmin 失败，可能仍有进程占用该组。\e[0m"
  fi
else
  echo "未检测到 DSFadmin 用户组，跳过删除。"
fi

echo -e "\e[32m\e[1m[DSF Cleanup] 环境已成功恢复到初始化前的纯净状态！\e[0m"
echo -e "\e[33m 提示：为了让组权限彻底在当前 Session 卸载，请记得在测试终端运行 'su - \$USER' 刷新身份。\e[0m"
