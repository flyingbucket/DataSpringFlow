FROM ubuntu:24.04

ENV DEBIAN_FRONTEND=noninteractive

# 安装基础开发工具链
RUN apt-get update && apt-get install -y \
    acl \
    tree \
    ripgrep \
    curl \
    build-essential \
    python3-dev \
    python3-pip \
    python3-venv \
    sudo \
    git \
    && rm -rf /var/lib/apt/lists/*

# 安装 Rust 编译环境并配置全局 Path
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

# 安装 maturin 编译工具 (全局环境)
RUN pip3 install --break-system-packages maturin

# 创建带有独立家目录的两个普通用户
# 用户 A：服务器上拥有 sudo 权限的管理员同学
RUN useradd -m -s /bin/bash -g users admin && \
    echo "admin:password123" | chpasswd && \
    usermod -aG sudo admin

# 用户 B：模拟普通学生，无 sudo 权限，主组也是 users
RUN useradd -m -s /bin/bash -g users student && \
    echo "student:password123" | chpasswd

# 终端体验优化
RUN echo "export TERM=xterm-256color" >> /etc/bash.bashrc

# 注入高亮、带颜色的特定 PS1 提示符（兼容 root 和普通用户）
# 格式为：[时间] 用户@容器名:当前目录 [状态]
RUN printf 'export PS1="\\[\\e[34;1m\\][ \\t ]\\[\\e[0m\\] \\[\\e[32;1m\\]\\u\\[\\e[0m\\]\\[\\e[33;1m\\]@\\h\\[\\e[0m\\]:\\[\\e[36;1m\\]\\w\\[\\e[0m\\] \\[\\e[35;1m\\]❯\\[\\e[0m\\] "\n' >> /etc/bash.bashrc
# 开启 ls, grep 等基础命令的别名色彩
RUN echo "alias ls='ls --color=auto'" >> /etc/bash.bashrc && \
    echo "alias grep='grep --color=auto'" >> /etc/bash.bashrc && \
    echo "alias ll='ls -alF'" >> /etc/bash.bashrc

RUN echo "source /etc/bash.bashrc" >> /root/.bashrc && \ 
    echo "source /etc/bash.bashrc" >> /etc/skel/.bashrc && \
    echo "source /etc/bash.bashrc" >> /home/admin/.bashrc && \
    echo "source /etc/bash.bashrc" >> /home/student/.bashrc

# 设置工作目录
WORKDIR /app
CMD ["/bin/bash"]
