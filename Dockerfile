FROM ubuntu:22.04

# apt のプロンプトを回避
ENV DEBIAN_FRONTEND=noninteractive

# システム依存関係のインストール
RUN apt-get update && apt-get install -y \
    curl \
    wget \
    build-essential \
    libgtk-3-dev \
    libwebkit2gtk-4.1-dev \
    libayatana-appindicator3-dev \
    librsvg2-dev \
    libxdo-dev \
    libsoup-3.0-dev \
    libssl-dev \
    pkg-config \
    git \
    && rm -rf /var/lib/apt/lists/*

# Node.js のインストール
RUN curl -fsSL https://deb.nodesource.com/setup_20.x | bash - \
    && apt-get install -y nodejs

# Rust のインストール
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

# 作業ディレクトリの設定
WORKDIR /app

# デフォルトで実行するコマンド
CMD ["npm", "run", "tauri", "build"]
