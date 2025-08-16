FROM ubuntu:22.04

# 安装必要的构建工具和依赖
RUN apt-get update && apt-get install -y \
    curl \
    build-essential \
    musl-tools \
    musl-dev \
    libssl-dev \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

# 为musl目标创建OpenSSL符号链接
RUN ln -s /usr/include/openssl /usr/include/x86_64-linux-musl/openssl || true
RUN ln -s /usr/lib/x86_64-linux-gnu/libssl.a /usr/lib/x86_64-linux-musl/libssl.a || true
RUN ln -s /usr/lib/x86_64-linux-gnu/libcrypto.a /usr/lib/x86_64-linux-musl/libcrypto.a || true

# 安装Rust
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

# 添加musl目标
RUN rustup target add x86_64-unknown-linux-musl

# 设置环境变量以支持静态链接OpenSSL
ENV OPENSSL_STATIC=1
ENV OPENSSL_LIB_DIR=/usr/lib/x86_64-linux-musl
ENV OPENSSL_INCLUDE_DIR=/usr/include/openssl
ENV PKG_CONFIG_PATH=/usr/lib/x86_64-linux-gnu/pkgconfig:/usr/lib/pkgconfig:/usr/share/pkgconfig
ENV PKG_CONFIG_ALLOW_SYSTEM_CFLAGS=1
ENV CC_x86_64_unknown_linux_musl=x86_64-linux-musl-gcc
ENV CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER=x86_64-linux-musl-gcc

WORKDIR /app
COPY . .

# 使用vendored OpenSSL进行静态编译
RUN cargo build --release --target x86_64-unknown-linux-musl --features vendored