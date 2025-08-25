#!/bin/bash

# 创建必要的日志目录
mkdir -p /var/log/mongodb /var/log/maccms

# 创建 MongoDB 数据目录
mkdir -p /var/lib/mongodb

# 生成随机密码和配置
RANDOM_PASSWORD=$(openssl rand -base64 12)
SESSION_KEY=$(openssl rand -hex 32)

# 创建 .env 文件
cat > /app/.env << EOF
DATABASE_URL=mongodb://localhost:27017
DATABASE_NAME=maccms_rust
SERVER_HOST=0.0.0.0
SERVER_PORT=8080
ADMIN_USER=admin
ADMIN_PASS=${RANDOM_PASSWORD}
SESSION_SECRET_KEY=${SESSION_KEY}
RUST_LOG=info
EOF

# 创建静态目录和凭证文件
mkdir -p /app/static
cat > /app/static/admin_credentials.txt << EOF
Admin Username: admin
Admin Password: ${RANDOM_PASSWORD}
Generated at: $(date)
EOF

# 复制 Supervisor 配置
cat > /etc/supervisor/conf.d/supervisord.conf <<EOF
[supervisord]
nodaemon=true

[program:mongodb]
command=bash -c "rm -rf /var/lib/mongodb/mongod.lock /tmp/mongodb-27017.sock /var/lib/mongodb/WiredTiger.lock && /usr/bin/mongod --dbpath /var/lib/mongodb --logpath /var/log/mongodb/mongodb.log"
priority=0
autostart=true
autorestart=true
stderr_logfile=/var/log/mongodb/mongodb_stderr.log
stdout_logfile=/var/log/mongodb/mongodb_stdout.log

[program:maccms_rust]
command=/app/maccms_rust
directory=/app
priority=1
autostart=true
autorestart=true
stdout_logfile=/var/log/maccms/maccms.log
stderr_logfile=/var/log/maccms/maccms.err
EOF

exec supervisord -c /etc/supervisor/conf.d/supervisord.conf
