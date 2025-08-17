# 使用 Ubuntu 22.04 作为基础镜像
FROM ubuntu:22.04

# 设置环境变量
ENV DEBIAN_FRONTEND=noninteractive
ENV MONGODB_VERSION=8.0
ENV MONGODB_PACKAGE=mongodb-org

# 安装必要的工具
RUN apt-get update && apt-get install -y \
    wget \
    unzip \
    curl \
    gnupg \
    systemctl \
    supervisor \
    openssl \
    && rm -rf /var/lib/apt/lists/*

# 添加 MongoDB GPG 密钥
RUN curl -fsSL https://www.mongodb.org/static/pgp/server-${MONGODB_VERSION}.asc | gpg -o /usr/share/keyrings/mongodb-server-${MONGODB_VERSION}.gpg --dearmor

# 添加 MongoDB 仓库
RUN echo "deb [ arch=amd64,arm64 signed-by=/usr/share/keyrings/mongodb-server-${MONGODB_VERSION}.gpg ] https://repo.mongodb.org/apt/ubuntu jammy/mongodb-org/${MONGODB_VERSION} multiverse" | tee /etc/apt/sources.list.d/mongodb-org-${MONGODB_VERSION}.list

# 安装 MongoDB
RUN apt-get update && apt-get install -y ${MONGODB_PACKAGE} \
    && rm -rf /var/lib/apt/lists/*

# 创建应用目录
WORKDIR /app

# 下载最新的 MacCMS Rust 版本
RUN wget -O linux.zip https://github.com/TFTG-CLOUD/maccms-rust/releases/latest/download/linux.zip \
    && unzip linux.zip \
    && rm linux.zip

# 生成随机密码
RUN RANDOM_PASSWORD=$(openssl rand -base64 12) \
    && echo "DATABASE_URL=mongodb://localhost:27017" > .env \
    && echo "DATABASE_NAME=maccms_rust" >> .env \
    && echo "SERVER_HOST=0.0.0.0" >> .env \
    && echo "SERVER_PORT=8080" >> .env \
    && echo "ADMIN_USER=admin" >> .env \
    && echo "ADMIN_PASS=${RANDOM_PASSWORD}" >> .env \
    && echo "SESSION_SECRET_KEY=$(openssl rand -hex 32)" >> .env \
    && echo "RUST_LOG=info" >> .env \
    && mkdir -p /app/static \
    && echo "Admin Username: admin" > /app/static/admin_credentials.txt \
    && echo "Admin Password: ${RANDOM_PASSWORD}" >> /app/static/admin_credentials.txt \
    && echo "Generated at: $(date)" >> /app/static/admin_credentials.txt

# 创建必要的目录
RUN mkdir -p /app/static/images \
    && mkdir -p /var/log/mongodb \
    && mkdir -p /var/log/maccms \
    && mkdir -p /var/log/supervisor \
    && mkdir -p /var/lib/mongodb

# 复制 MongoDB 配置文件
COPY <<EOF /etc/mongod.conf
storage:
  dbPath: /var/lib/mongodb
  journal:
    enabled: true
systemLog:
  destination: file
  logAppend: true
  path: /var/log/mongodb/mongod.log
net:
  port: 27017
  bindIp: 127.0.0.1
processManagement:
  timeZoneInfo: /usr/share/zoneinfo
EOF

# 复制 Supervisor 配置
COPY <<EOF /etc/supervisor/conf.d/supervisord.conf
[supervisord]
nodaemon=true
user=root

[program:mongodb]
command=/usr/bin/mongod --config /etc/mongod.conf
autostart=true
autorestart=true
stdout_logfile=/var/log/supervisor/mongodb.log
stderr_logfile=/var/log/supervisor/mongodb.err

[program:maccms_rust]
command=/app/maccms_rust
directory=/app
autostart=true
autorestart=true
stdout_logfile=/var/log/supervisor/maccms.log
stderr_logfile=/var/log/supervisor/maccms.err
user=root
environment=DATABASE_URL="mongodb://localhost:27017/maccms_rust",SERVER_HOST="0.0.0.0",SERVER_PORT="8080",RUST_LOG="info"
EOF

# 设置二进制文件执行权限
RUN chmod +x /app/maccms_rust

# 暴露端口
EXPOSE 8080

# 启动 Supervisor
CMD ["/usr/bin/supervisord", "-c", "/etc/supervisor/conf.d/supervisord.conf"]

# 健康检查
HEALTHCHECK --interval=30s --timeout=10s --start-period=60s --retries=3 \
    CMD curl -f http://localhost:8080/api/health || exit 1