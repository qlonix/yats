# Changelog

All notable changes to YATS (Yet Another Touchpad Shortcut) will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.3.6] - 2026-07-19

### Fixed
- **Linuxパームリジェクションの改善**: Linux環境（`evdev`）において、手のひら（パーム）がタッチパッドに触れた際に発生するイベント（`ABS_MT_TOOL_TYPE` が `MT_TOOL_PALM` となるスロット）を検知して正しくフィルタリングするようロジックを修正。タイピング中の手のひら接触による意図しないクリック等の誤動作が防止されます。

## [1.3.5] - 2026-04-29

### Fixed
- **未使用警告の解消**: Windows環境で `platform` モジュールの関数が未使用と判定されていた問題を、`lib.rs` のロジックを統合・集約することで解消。

### Changed
- **プラットフォームの実装整理**:
  - `lib.rs` 内の Windows 固有ロジックを `platform\windows.rs` へ移動。
  - Windows のスタートアップ登録を、レジストリ方式からスタートアップフォルダへのショートカット作成方式（PowerShell経由）に統一。
  - AAPしきい値の取得・設定およびレジストリクリーンアップのロジックを `platform` モジュールに集約。

## [1.3.4] - 2026-04-19

### Changed
- **Documentation**: Updated README to include instructions on how to handle the Windows SmartScreen warning ("Windows protected your PC") during installation. Added both Japanese and English explanations.

## [1.3.1] - 2026-03-22

### Fixed
- **Linux Panic Fix**: Fixed an issue where the application would panic (`uinput init fail`) on startup if the user lacked permissions for `/dev/uinput`. The app now gracefully handles the permission error and logs it instead of crashing.
- **Documentation**: Updated README to clarify that `/dev/uinput` requires proper permissions (via `udev` rules) for the virtual keyboard simulation feature on Linux.
- **Udev Rules**: Added `KERNEL=="uinput", MODE="0666"` rule to `99-yats-touchpad.rules` to automatically grant access to the `uinput` module.

## [1.3.0] - 2026-02-16

### Fixed
- **Linux Scroll Granularity**: Fixed an issue where scroll events were being accumulated redundantly (once in the hook, once in the driver), causing multi-line jumps. Removed the driver-level accumulator to allow 1-to-1 mapping of scroll events.
- **Touchpad Latency**: Reduced the absolute position accumulation threshold (`ACCUM_THRESHOLD`) from 8 to 1 in the Linux touchpad driver. This eliminates the "dead zone" feeling where small initial movements were ignored.
- **Linux Tray Icon**: Fixed the "Pause" menu item on Zorin OS (and other GNOME-based distros) where `CheckMenuItem` was not rendering correctly. Replaced with a dynamic text label ("⏸ 一時停止" / "▶ 機能を再開").

### Changed
- **Default Scroll Parameters**: Updated default scroll settings based on user feedback (Sensitivity: 1, Speed: 5, Natural Scroll: On, Max Speed: 100).
- **UI Improvements**:
  - Moved "Natural Scroll Direction" checkbox to the top of the Scroll Tuning screen and inverted its logic (checked = natural/non-inverted).
  - Capped "Max Scroll Output Speed" slider to 200 (was 3000) for finer control.
  - Hidden "Advanced Settings" button to simplify the UI.
  - Disabled text selection globally via CSS (`user-select: none`).
- **Package Metadata**: Added comprehensive metadata (Homepage, License, Section, Priority) to the generated `.deb` package.

## [1.2.15] - 2026-02-14

### Added
- **ローカル Linux ビルド環境の構築**: Docker および WSL を使用して、GitHub Actions を介さずにローカルで Linux 版バイナリ（.deb, .AppImage）をビルドできる環境を追加。

### Fixed
- **Linux ビルドエラーの修正**: `keyboard_hook.rs` において `uinput` ビルダーの `with_keys` 戻り値の処理漏れによるコンパイルエラーを修正。
- **警告のクリーンアップ**: `linux.rs` 内の未使用変数 `last_dir_y` をリネームし、コンパイル警告を解消。

### Changed
- **ビルド環境のローカライズ**: `Dockerfile` やビルドスクリプト内のコメントを日本語に統一。

## [1.2.13] - 2026-02-14 [YANKED]

### Fixed
- **設定保存の正確性向上**: 設定保存時の競合（Race Condition）を解消し、スクロール詳細設定が確実に保存されるように修正。
- **注意**: このバージョンはビルドエラーのため使用できません。v1.2.14 を使用してください。

## [1.2.12] - 2026-02-14 [YANKED]

### Changed
- **Linux スクロール設定の統合**: 「Global Scroll Settings」と「Linux Scroll Tuning (Curve)」画面を統合。感度曲線（Curve）を基本モデルとし、UIを簡略化。
- **UIの改善**: `Max Scroll Output Speed` を感度曲線の y 軸上限と連動。設定項目（Sensitivity, Scroll Speed, Min Mouse Speed）を固定値化し UI から削除。
- **注意**: このバージョンはビルドエラーのため使用できません。v1.2.13 を使用してください。


### Fixed
- **Linux マウスロック問題の修正**: キーボードフック対象の自動判定ロジックを強化。相対/絶対座標を持つのポインティングデバイス（マウス・タッチパッド）を明示的に除外することで、起動時にマウスが操作不能になる問題を修正。

## [1.2.10] - 2026-02-14

### Fixed
- **Linux ビルド構成の修正**: `evdev` クレートの `uinput` フィーチャー指定を削除（v0.12 では不要なため）。これにより依存関係の解決エラーを修正。

## [1.2.9] - 2026-02-14

### Fixed
- **Linux ビルド環境への対応強化**: `evdev` の `uinput` ビルダーおよび `AttributeSet` の操作方法を、より互換性の高い構文に修正。

## [1.2.8] - 2026-02-14

### Fixed
- **Linux ビルドエラーの修正**: `evdev` クレートの `uinput` フィーチャーが不足していたため、GitHub Actions でのコンパイルに失敗していた問題を修正。

## [1.2.7] - 2026-02-14

### Fixed
- **Linux キーボード無反応問題の修正**: キーボードの排他制御（Grab）時に、リマップ対象外のイベントを物理デバイスへ戻すのではなく、`/dev/uinput` による仮想キーボード経由でOSへ再注入するように改善。これにより、アプリ起動中にマウスやキーボードが操作不能になる問題を根本解決。

## [1.2.6] - 2026-02-14

### Fixed
- **Linux スリープ復帰時の不具合修正**: Linux版のキーボードフックを X11 ベースの `rdev` からカーネルレベルの `evdev` 直接監視に移行。
  - セッション切断の影響を受けなくなり、スリープ復帰時も自動的にデバイスを再検知して動作を継続するよう改善。
  - キーボードデバイスの排他取得（Grab）により、より低遅延で安定したリマップを実現。

## [1.2.5] - 2026-02-14

### Fixed
- **スクロール速度反映の修正**: 計算ロジック内の不要な係数を削除し、カーブエディタで設定した出力速度が正確に反映されるように修正。

### Changed
- **感度曲線の平滑化**: カーブエディタの補間アルゴリズムを線形からスプライン曲線（Monotone Cubic Spline）に変更。つなぎ目がなだらかになり、より直感的なチューニングが可能。
- **UIのブラッシュアップ**: グラフの描画品質向上、目盛りラベルの追加、グリッドの調整。

## [1.2.4] - 2026-02-14

### Added
- **スクロール感度曲線（カーブエディタ）**: マウスの移動速度に対するスクロール速度をグラフ上で直感的に設定できる機能を追加（Linux版）。
  - ポイントのドラッグ、クリックによる追加、右クリックによる削除に対応。
  - 従来の線形設定（Standard）と感度曲線（Advanced）をいつでも切り替え可能。
- **データ構造の拡張**: `AppConfig` にカーブデータ保持用のフィールドを追加。

## [1.2.3] - 2026-02-14

### Fixed
- **Linuxスクロール不具合の修正**: 速度制限（クランプ）がかかった際に微小な移動量が切り捨てられる問題を、OS側送信関数への累積器（アキュムレータ）導入により解決。

### Changed
- **UIの改善 (別画面化)**: `Global Scroll Settings` モーダルからLinux専用詳細設定を分離。新設した `Linux Scroll Tuning` モーダルへ移動し、UIのオーバーフローを解消。
- **デフォルト値の調整**: `Max Scroll Output Speed` の初期値を 300 から 800 へ引き上げ。

## [1.2.2] - 2026-02-13

### Added
- **Linux専用スクロール詳細設定**: 移動量、速度、最小/最大出力速度のチューニング項目を `AppConfig` および UI に追加。
- **スクロールロジックの改善 (Linux)**: しきい値を超えるまで入力を抑制する「遊び」と、出力範囲のクランプ機能を実装。

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
