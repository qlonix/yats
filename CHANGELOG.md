# Changelog

All notable changes to YATS (Yet Another Touchpad Shortcut) will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.2.1] - 2026-02-10

### Changed
- **大規模リファクタリング**: Windows と Linux のプラットフォーム固有コードを専用モジュールに分離
  - 新しい `platform` モジュール構造を作成（`platform/windows.rs`, `platform/linux.rs`）
  - タッチパッド監視の実装を `platform/touchpad/` に分離
  - `touchpad_monitor.rs` を 615行 → 155行 に削減（75%削減）
  - `keyboard_hook.rs` を 496行 → 290行 に削減（42%削減）
  - `lib.rs` を 534行 → 440行 に削減（18%削減）
  - 条件付きコンパイル（`#[cfg]`）ブロックを大幅に削減
- コードの保守性と可読性が向上

### Technical Details
- 入力シミュレーション、ウィンドウ管理、システム情報取得のための統一インターフェース（トレイト）を実装
- プラットフォーム固有の処理を抽象化し、将来的な拡張を容易に

---

## [1.2.0] - 以前のバージョン

過去のバージョンの変更履歴は追って記載予定
