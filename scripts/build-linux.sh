#!/bin/bash

# プロジェクトルートに移動
cd "$(dirname "$0")/.."

echo "Docker を使用して Linux ビルドを開始します..."

# docker-compose がインストールされているか確認 (モダンなバージョンは 'docker compose' を使用)
DOCKER_COMPOSE="docker compose"
if ! command -v docker > /dev/null || ! docker compose version > /dev/null 2>&1; then
    if command -v docker-compose > /dev/null; then
        DOCKER_COMPOSE="docker-compose"
    else
        echo "エラー: Docker と Docker Compose が必要です。"
        exit 1
    fi
fi

# ビルドを実行
$DOCKER_COMPOSE run --rm build

echo "ビルドが完了しました。src-tauri/target/release/bundle/ を確認してください。"
