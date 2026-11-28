# Changelog

All notable changes to YATS (Yet Another Touchpad Shortcut) will be documented in this file.
Each version's entry is written in both Japanese and English.

YATS (Yet Another Touchpad Shortcut) の主な変更はすべてこのファイルに記録します。
各バージョンのエントリは日本語と英語で併記しています。

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

**日本語**

### Fixed
- Linux の接触検出で、タッチパッドに触れたまま指を静止させていると接触が切れてしまう問題を修正。静止した指は evdev イベントを出さないため、従来は Release Delay 経過後に接触が期限切れになっていた。接触状態をイベントの新しさではなく接触の有無そのもので判定するようにした
- Ctrl などの修飾キーを押しながらトリガーキーを押すと、パームリジェクションに「タイピング直後」と誤判定されて発火しない問題を修正。修飾キーとロックキー（Caps Lock / Num Lock / Scroll Lock）の押下・解放はタイピングとして数えないようにしたため、Ctrl+クリック（リンクを新しいタブで開く）や Shift+クリックが使えるようになった。Caps Lock を Ctrl に割り当てている環境（Linux では evdev レベルで Caps Lock として届く）でも有効

**English**

### Fixed
- Fixed contact detection on Linux dropping the contact while a finger rests still on the touchpad. A still finger produces no evdev events, so the contact previously expired after the Release Delay. The contact state is now based on contact presence itself rather than event recency
- Fixed trigger keys being rejected as "recent typing" by palm rejection when pressed while a modifier key (Ctrl, Shift, Alt, Meta) is held. Modifier and lock key (Caps Lock / Num Lock / Scroll Lock) presses and releases no longer count as typing, so Ctrl+click (open link in new tab) and Shift+click now work, including setups where Caps Lock is remapped to Ctrl (on Linux it still arrives as Caps Lock at the evdev level)

## [1.4.1] - 2026-11-28

**日本語**

### Added
- 設定画面のアクション選択肢に「Browser Back」「Browser Forward」を追加。バックエンドには専用アクションとして実装済みだったが、UI からは選べなかった
- トリガー検出方式を設定から切り替えられるようにした（Trigger Detection、Windows / Linux 共通）。「Contact (Touchpad)」は接触検出、「Movement (Any Device)」はカーソルが動いてから Release Delay 以内のキー押下をトリガーとする方式で、マウスやトラックポイントなどタッチパッド以外のポインティングデバイスでも使える。「Auto (Both)」は両方の併用。Windows の Contact は Precision Touchpad が必要。未設定時の既定は従来どおり（Linux は接触検出、Windows は移動検出）

### Fixed
- Linux の移動検出（Movement / Auto）でマウスを使うと、スクロールモードに入ってもスクロールできなかった問題を修正。マウスやトラックポイントの evdev デバイスも直接監視し、移動デルタと移動検出のタイムスタンプをデバイスレートで供給するようにした（従来は X のポインタ位置ポーリングのみで、デルタ自体は供給されていなかった）
- パームリジェクションに「タイピングモード」を追加。タッチパッドに触れながら一定時間タイプし続けると（間隔 1.5 秒以内の打鍵が 8 回以上、かつ 3 秒以上継続）、トリガーキーも通常のキーとして通すようになった。長文入力中の「考える間」で打鍵が数秒空いても、Palm Delay の時間切れで誤発火しなくなった。タイピング中の接触の細切れ（フリッカー）では解除されず、解除は「カーソルを大きく動かしてから押したとき」「接触が終わって打鍵も止んだとき」「10 秒間打鍵がないとき」に限られる。短いコマンド入力ではモードに入らない

**English**

### Added
- Added "Browser Back" and "Browser Forward" to the action choices in the settings UI. They were already implemented as dedicated actions in the backend but could not be selected from the UI
- The trigger detection method can now be switched in the settings (Trigger Detection, on both Windows and Linux). "Contact (Touchpad)" is contact detection; "Movement (Any Device)" triggers on a key press within the Release Delay after the cursor moved, and works with any pointing device such as a mouse or a TrackPoint; "Auto (Both)" combines the two. On Windows, Contact requires a Precision Touchpad. When unset, the default is unchanged (contact detection on Linux, movement detection on Windows)

### Fixed
- Fixed scrolling not working with a mouse under movement detection (Movement / Auto) on Linux. Mouse and TrackPoint evdev devices are now monitored directly, supplying movement deltas and movement-detection timestamps at device rate (previously only the X pointer position was polled and no deltas were supplied at all)
- Added a "typing mode" to palm rejection. When you keep typing while touching the touchpad (8+ keystrokes no more than 1.5 s apart, sustained for at least 3 s), trigger keys are passed through as regular keys. Stray triggers no longer slip through thinking pauses of several seconds, which the time-based Palm Delay could not cover. Brief contact flicker during typing does not end the mode; it ends only when you press after a deliberate cursor move, when the contact ends and typing has stopped, or after 10 s without a keystroke. Short command input does not enter it

## [1.4.0] - 2026-11-22

**日本語**

### Added
- アクション種別「Window Drag」を追加。本家の ThumbSense と同様に、カーソルが対象ウィンドウの中にあればタイトルバー以外でも、トリガーキーを押しながらタッチパッドを動かすとウィンドウを移動できる。対象ウィンドウが最大化されている場合は最大化を解除してから移動する。Linux は X11 セッションのみ対応（Wayland では仕様上利用できない）

**English**

### Added
- New "Window Drag" action: like the original ThumbSense, holding the trigger key and moving on the touchpad drags the window under the cursor from anywhere inside it, not just the title bar. A maximized target window is restored first, then dragged. On Linux this works in X11 sessions only (not possible on Wayland by design)

**日本語**

### Fixed
- Linux で、接触面積の解像度を報告しないタッチパッド（実測で指先は最大 450 前後、手のひらは 550 以上）でもパームリジェクションの面積判定が働くようにした。生値のしきい値（500）で判定し、手のひらと確定した接触しか残っていない場合はタッチ状態を即座に解除する
- Linux のスクロールモードの送信を、対象ウィンドウで切り替えるようにした。ブラウザ（WM_CLASS で判定）は uinput の仮想ホイールから高解像度ホイールイベントを送り、細かい刻みで滑らかにスクロールする。それ以外は従来どおりボタンイベントで送る（仮想ホイールの valuator は GTK 系アプリではスクロールとして解釈されないため）
- Windows で、しばらく使うとクリックやスクロールが一切効かなくなる問題を修正。接触開始時刻がマウス移動などの活動でも記録されていたため、最初のタイピング以降は「接触の方が先」と判定され続け、すべてのアクションがパームリジェクションで抑止されていた。接触の開始・終了を HID レポートの接触遷移（指が触れた／離れた）だけで管理するよう変更した
- パームリジェクションの監査ログに抑止理由を記録するようにした

**English**

### Fixed
- On Linux, palm rejection by contact size now also works on touchpads that report no size resolution (measured: fingertips peak around 450 raw, palms at 550+), using a raw-value threshold of 500. When only palm-classified contacts remain, the touched state is cleared immediately
- On Linux, scroll mode now picks its send path by the target window: browsers (detected via WM_CLASS) receive hi-res wheel events from a uinput virtual wheel for fine-grained smooth scrolling, while other apps keep receiving button events as before (GTK apps do not interpret the virtual wheel's valuators as scrolling)
- Fixed clicks and scroll stopping entirely on Windows after a while. The contact start time was recorded on any activity including mouse movement, so after the first typing every action was judged "contact started before typing" and suppressed by palm rejection. Contact start/end is now tracked only from HID contact transitions (finger down / finger up)
- Palm rejection audit log entries now record the rejection reason

## [1.3.10] - 2026-11-21

**日本語**

### Fixed
- スクロール中にマウスカーソルが一瞬移動して元の位置に戻るちらつきを修正。カーソルを事後的に戻すのではなく、スクロール中はカーソルが動かないようにした（Linux: タッチパッドデバイスをグラブして入力を OS へ渡さない。Windows: カーソルをスクロール開始位置にクリップ）
- パームリジェクションを強化。タイピングを始める前からタッチパッドに触れ続けている接触（置かれた手のひらなど）ではアクションが発火しないようにした。Linux では接触面積 (ABS_MT_TOUCH_MAJOR) によるパーム判定も追加（対応デバイスのみ）
- パームリジェクションの接触判定を改良。接触開始時刻をタッチパッドの接触状態 (evdev の TRACKING_ID / BTN_TOUCH) から取るようにし、タイピング前から置かれている手のひらがわずかに動いても、新規の接触と誤認せず抑止が続くようにした
- Windows でスクロールが Linux より極端に遅くなる問題を修正。ホイール量の単位が揃っておらず（Windows は 120 で 1 ノッチ、Linux は 1 単位 = 1 クリック）、Windows では 1/120 しか送られていなかった。Windows 側の送信量を 120 倍にし、同じ設定で同じスクロール量になるよう統一した
- Scroll Tuning の Min Scroll Output Speed のスライダーが他の項目より短く表示される問題を修正し、3 つのスライダーのレイアウトを揃えた
- Windows で細かい動きのスクロール量が最小でも大きすぎる問題を修正。ホイール量を 1 ノッチ (120) 単位でしか送っていなかったため、Windows ではノッチ未満の量も送れるよう送信解像度を 120 倍にし、わずかな動きでも少しずつスクロールできるようにした

### Changed
- Windows の NSIS インストーラーのインストールモードを「現在のユーザー」に明示した。管理者権限なしで AppData\Local へインストールされる（従来も既定で同じ動作だったが、設定として固定した）。なお MSI 版はマシン全体へのインストール（管理者権限が必要）で、NSIS 版と混在させると上書きが競合するため、どちらか一方への統一を推奨
- トレイアイコンのツールチップ（ホバー表示）を「YATS」から「YATS Ver. X.X.X」（現在のバージョン番号）へ変更
- Linux でもトレイアイコンのバージョンが見えるようにした。Linux のトレイ実装ではツールチップがサポートされないため、「YATS Ver. X.X.X」をアイコン横のラベルにも表示する（Windows では従来どおりツールチップに表示）
- トレイアイコンの右クリックメニューの一番下に「Ver. X.X.X」（現在のバージョン）を表示するようにした

**English**

### Fixed
- Fixed the mouse cursor flickering during scroll, where it momentarily moved and snapped back. Instead of moving the cursor back after the fact, the cursor is now kept from moving during scroll (Linux: the touchpad device is grabbed so its input does not reach the OS. Windows: the cursor is clipped to the scroll start position)
- Strengthened palm rejection. A touch that was already in contact before typing started (such as a resting palm) no longer triggers actions. On Linux, palms are also detected by contact size (ABS_MT_TOUCH_MAJOR) on devices that report it
- Improved the palm rejection contact tracking. The contact start time is now taken from the touchpad's actual contact state (evdev TRACKING_ID / BTN_TOUCH), so a palm that rests on the pad from before typing is not mistaken for a fresh touch when it shifts slightly, and stays rejected
- Fixed scrolling on Windows being far slower than on Linux. The wheel amount units were inconsistent (Windows: 120 per notch, Linux: 1 unit = 1 click), so Windows only sent 1/120 of the intended amount. The Windows send amount is now multiplied by 120 so the same settings produce the same scroll amount on both platforms
- Fixed the Min Scroll Output Speed slider in Scroll Tuning being shorter than the other sliders, and aligned the layout of all three sliders
- Fixed the minimum scroll amount on Windows being too large for fine movements. Wheel amounts were only sent in whole-notch (120) steps; the send resolution on Windows is now 120 times finer so sub-notch amounts are sent and small movements scroll gradually

### Changed
- Pinned the Windows NSIS installer mode to "current user" explicitly. It installs into AppData\Local without administrator rights (this was already the default behavior, now fixed in configuration). Note that the MSI installer is per-machine (requires administrator rights), and mixing it with the NSIS install makes upgrades conflict, so sticking to one of them is recommended
- Changed the tray icon tooltip (hover text) from "YATS" to "YATS Ver. X.X.X" (the current version number)
- Made the tray icon version visible on Linux as well. Tooltips are not supported by the Linux tray implementation, so "YATS Ver. X.X.X" is also shown as a label next to the icon (on Windows it appears in the tooltip as before)
- Added the current version ("Ver. X.X.X") at the bottom of the tray icon's right-click menu

## [1.3.9] - 2026-11-20

**日本語**

### Added
- カーブグラフの X 軸 (入力マウス速度) の上限を設定可能にした。初期値は接続中のモニタ幅から自動決定され、上限を変えるとカーブが形を保ったまま横方向に伸縮する

### Changed
- **ドキュメント**: README の機能一覧に Scroll Tuning (カーブ編集・軸範囲・ライブマーカー) の説明を追加（日本語・英語）。

**English**

### Added
- Made the upper limit of the curve graph's X axis (input mouse speed) configurable. The initial value is determined automatically from the connected monitor's width, and changing the limit rescales the curve horizontally while preserving its shape

### Changed
- **Documentation**: Added a description of Scroll Tuning (curve editing, axis ranges, live marker) to the README feature list (Japanese and English).

## [1.3.8] - 2026-11-19

**日本語**

### Added
- Scroll Tuning のカーブグラフにライブマーカーを追加。モーダルを開いた状態で他のウィンドウでスクロールすると、現在の入力速度とカーブ上の出力位置がリアルタイムで表示される

### Fixed
- Close Window アクションで YATS 自身のウィンドウを閉じるとアプリが終了してしまう問題を修正。対象が自ウィンドウの場合は外部クローズを送らず、トレイ常駐の既定動作どおりウィンドウを隠すようにした
- **Windows版のコンパイルエラーを修正**: `HidP_GetButtonCaps` の第3引数（PUSHORT = `*mut u16`）に ULONG の参照を渡していた型不一致を修正。現行コードのままでは Windows 版がビルドできない状態だった。
- スクロールの速度計測を実測ベース（8ms バッチの移動量 ÷ 実経過時間）に改修。従来は計測間隔が不安定で、カーブがピーキーになったり反応しなかったりした。あわせてカーブを Windows でも有効化し、Scroll Speed スライダーとキー別スクロール設定（感度・反転）がカーブ使用時にも反映されるようにした。
- Linux: スクロールとブラウザ戻る/進むキーの送信を rdev（Xlib）から enigo（x11rb）に変更。X サーバーとの接続が切れた際に Xlib の致命的エラーハンドラでプロセスごと終了し得る経路を排除した。
- Linux: アプリが突然終了する問題への対策として、panic 内容の監査ログへの記録、ロックのポイズン耐性、タッチパッド監視スレッドの自動再起動、uinput 初期化失敗時のリトライを追加した。

### Changed
- **ドキュメント**: README に動作環境（対応 OS・アーキテクチャ・前提条件）を明記（日本語・英語）。
- **開発運用**: Windows 版のビルドを GitHub Actions で行う方針に変更し、`.agent/rules.md` を実態に合わせて更新。
- Min/Max Scroll Output Speed を Acceleration Curve の出力軸の範囲として統合。軸の範囲を変えるとカーブが形を保ったまま伸縮するようになり、バックエンド側のクランプとの競合を解消した。
- 無効化されていた Min Mouse Distance と Min Mouse Speed の設定項目を削除（既存の `config.json` にキーが残っていても読み込みに影響しない）。
- スクロール出力の値 (Min/Max Scroll Output Speed、カーブの点、グラフの軸ラベル) を整数に統一し、小数表示をなくした

**English**

### Added
- Added a live marker to the Scroll Tuning curve graph. While the modal is open, scrolling in another window shows the current input speed and the corresponding output position on the curve in real time

### Fixed
- Fixed an issue where using the Close Window action on YATS's own window terminated the app. When the target is its own window, YATS no longer sends an external close and instead hides the window, following its default tray-resident behavior
- **Windows build error fix**: Fixed a type mismatch where a reference to a ULONG was passed as the third argument of `HidP_GetButtonCaps` (PUSHORT = `*mut u16`). As the code stood, the Windows build could not compile.
- Reworked scroll speed measurement to be measurement-based (movement per ~8ms batch ÷ actual elapsed time). Previously the measurement interval was unstable, making the curve peaky or unresponsive. The curve is now also enabled on Windows, and the Scroll Speed slider and per-key scroll settings (sensitivity, inversion) now apply when using the curve.
- Linux: Switched scroll and browser back/forward key sending from rdev (Xlib) to enigo (x11rb), removing a path where losing the connection to the X server could kill the process via Xlib's fatal error handler.
- Linux: As countermeasures against the app suddenly exiting, added panic logging to the audit log, poison-tolerant locks, automatic restart of the touchpad monitoring thread, and retry on uinput initialization failure.

### Changed
- **Documentation**: Documented system requirements (supported OS, architecture, prerequisites) in the README (Japanese and English).
- **Development**: Switched Windows builds to GitHub Actions and updated `.agent/rules.md` accordingly.
- Unified Min/Max Scroll Output Speed as the output axis range of the Acceleration Curve. Changing the axis range now rescales the curve while preserving its shape, resolving the conflict with backend clamping.
- Removed the disabled Min Mouse Distance and Min Mouse Speed settings (keys left in an existing `config.json` are ignored on load).
- Unified scroll output values (Min/Max Scroll Output Speed, curve points, graph axis labels) to integers, removing decimal displays

## [1.3.7] - 2026-07-19

**日本語**

### Added
- **Windows版パームリジェクションの改善**: Windows環境において、Raw InputのHID生データ（PTP規格）を解析し、手のひらによる誤操作（Confidenceビットが0のタッチ）を直接判定して除外するロジックを実装。タイピング中などの意図しない手のひら接触によるクリック等の誤動作が防止されます。

**English**

### Added
- **Improved Windows palm rejection**: On Windows, YATS now parses raw HID data from Raw Input (PTP spec) and directly detects and excludes palm touches (touches whose Confidence bit is 0). This prevents accidental clicks and other actions caused by unintended palm contact while typing.

## [1.3.6] - 2026-07-19

**日本語**

### Fixed
- **Linuxパームリジェクションの改善**: Linux環境（`evdev`）において、手のひら（パーム）がタッチパッドに触れた際に発生するイベント（`ABS_MT_TOOL_TYPE` が `MT_TOOL_PALM` となるスロット）を検知して正しくフィルタリングするようロジックを修正。タイピング中の手のひら接触による意図しないクリック等の誤動作が防止されます。

**English**

### Fixed
- **Improved Linux palm rejection**: On Linux (`evdev`), the logic now correctly detects and filters events from slots whose `ABS_MT_TOOL_TYPE` is `MT_TOOL_PALM`, which occur when a palm touches the touchpad. This prevents accidental clicks and other actions caused by palm contact while typing.

## [1.3.4] - 2026-04-19

**日本語**

> **注意**: このバージョンはタグ・GitHub リリースが存在せず、以下の変更は v1.3.6 でまとめて公開された。本ファイルに記載のあった「1.3.5」は、実際にはバージョン番号が付与されなかった（2026-04-29 のプラットフォーム実装整理を含む）ため、ここに統合した。

### Fixed
- **未使用警告の解消**: Windows環境で `platform` モジュールの関数が未使用と判定されていた問題を、`lib.rs` のロジックを統合・集約することで解消。

### Changed
- **プラットフォームの実装整理**:
  - `lib.rs` 内の Windows 固有ロジックを `platform\windows.rs` へ移動。
  - Windows のスタートアップ登録を、レジストリ方式からスタートアップフォルダへのショートカット作成方式（PowerShell経由）に統一。
  - AAPしきい値の取得・設定およびレジストリクリーンアップのロジックを `platform` モジュールに集約。
- **ドキュメント**: README に、インストール時の Windows SmartScreen 警告（「Windows によって PC が保護されました」）への対処方法を追加（日本語・英語）。

**English**

> **Note**: This version has no tag or GitHub release; the changes below were published together in v1.3.6. The "1.3.5" previously listed in this file never actually had a version number assigned (including the platform implementation cleanup of 2026-04-29) and has been merged here.

### Fixed
- **Resolved unused warnings**: Fixed an issue where functions in the `platform` module were flagged as unused on Windows, by consolidating the logic in `lib.rs`.

### Changed
- **Platform implementation cleanup**:
  - Moved Windows-specific logic from `lib.rs` into `platform\windows.rs`.
  - Unified Windows startup registration from the registry approach to creating a shortcut in the Startup folder (via PowerShell).
  - Consolidated the AAP threshold get/set logic and registry cleanup into the `platform` module.
- **Documentation**: Updated README to include instructions on how to handle the Windows SmartScreen warning ("Windows protected your PC") during installation. Added both Japanese and English explanations.

## [1.3.3] - 2026-04-18

**日本語**

### Changed
- リリースワークフローに Windows ビルドを追加し、Windows 版も GitHub Releases で配布するようにした
- About 画面のバージョン表示を更新

**English**

### Changed
- Added a Windows build to the release workflow so Windows builds are also distributed via GitHub Releases
- Updated the version display on the About screen

## [1.3.2] - 2026-04-18

**日本語**

### Fixed
- Linux で入力状態が固着（スタック）する問題を修正

**English**

### Fixed
- Fixed an issue on Linux where the input state could get stuck

## [1.3.1] - 2026-03-22

**日本語**

### Fixed
- **Linux のパニック修正**: `/dev/uinput` への権限がないユーザーの場合、起動時にアプリがパニック（`uinput init fail`）する問題を修正。権限エラーを適切に処理し、クラッシュせずにログへ記録するようになった
- **ドキュメント**: Linux の仮想キーボードシミュレーション機能には `/dev/uinput` への適切な権限（udev ルール経由）が必要であることを README に明記
- **udev ルール**: `99-yats-touchpad.rules` に `KERNEL=="uinput", MODE="0666"` ルールを追加し、`uinput` モジュールへのアクセスを自動で許可するようになった

**English**

### Fixed
- **Linux Panic Fix**: Fixed an issue where the application would panic (`uinput init fail`) on startup if the user lacked permissions for `/dev/uinput`. The app now gracefully handles the permission error and logs it instead of crashing.
- **Documentation**: Updated README to clarify that `/dev/uinput` requires proper permissions (via `udev` rules) for the virtual keyboard simulation feature on Linux.
- **Udev Rules**: Added `KERNEL=="uinput", MODE="0666"` rule to `99-yats-touchpad.rules` to automatically grant access to the `uinput` module.

## [1.3.0] - 2026-02-16

**日本語**

### Fixed
- **Linux スクロールの粒度**: スクロールイベントが二重に累積され（フック側とドライバ側で1回ずつ）、複数行ジャンプが発生する問題を修正。ドライバ側のアキュムレータを削除し、スクロールイベントの 1 対 1 マッピングを可能にした
- **タッチパッドの遅延**: Linux タッチパッドドライバの絶対位置の累積しきい値（`ACCUM_THRESHOLD`）を 8 から 1 に削減。小さな初期の動きが無視される「デッドゾーン」感を解消
- **Linux トレイアイコン**: Zorin OS（および他の GNOME 系ディストリビューション）で「一時停止」メニュー項目の `CheckMenuItem` が正しく描画されない問題を修正。動的テキストラベル（「⏸ 一時停止」/「▶ 機能を再開」）に置き換えた

### Changed
- **デフォルトスクロールパラメータ**: ユーザーフィードバックに基づき、デフォルトのスクロール設定を更新（Sensitivity: 1、Speed: 5、Natural Scroll: On、Max Speed: 100）
- **UI の改善**:
  - "Natural Scroll Direction" チェックボックスを Scroll Tuning 画面の上部へ移動し、ロジックを反転（チェック = ナチュラル/非反転）
  - より細かい調整のため "Max Scroll Output Speed" スライダーの上限を 200 に制限（従来 3000）
  - UI を簡素化するため "Advanced Settings" ボタンを非表示にした
  - CSS でテキスト選択を全体に無効化（`user-select: none`）
- **パッケージメタデータ**: 生成される `.deb` パッケージに包括的なメタデータ（Homepage、License、Section、Priority）を追加

**English**

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

**日本語**

### Added
- **ローカル Linux ビルド環境の構築**: Docker および WSL を使用して、GitHub Actions を介さずにローカルで Linux 版バイナリ（.deb, .AppImage）をビルドできる環境を追加。

### Fixed
- **Linux ビルドエラーの修正**: `keyboard_hook.rs` において `uinput` ビルダーの `with_keys` 戻り値の処理漏れによるコンパイルエラーを修正。
- **警告のクリーンアップ**: `linux.rs` 内の未使用変数 `last_dir_y` をリネームし、コンパイル警告を解消。

### Changed
- **ビルド環境のローカライズ**: `Dockerfile` やビルドスクリプト内のコメントを日本語に統一。

**English**

### Added
- **Local Linux build environment**: Added an environment for building Linux binaries (.deb, .AppImage) locally using Docker and WSL, without going through GitHub Actions.

### Fixed
- **Linux build error fix**: Fixed a compile error in `keyboard_hook.rs` caused by mishandling the return value of the `uinput` builder's `with_keys`.
- **Warning cleanup**: Renamed the unused variable `last_dir_y` in `linux.rs` to clear compile warnings.

### Changed
- **Build environment localization**: Unified comments in the `Dockerfile` and build scripts to Japanese.

## [1.2.14] - 2026-02-14

**日本語**

### Fixed
- **Linux ビルドエラーの修正**: `evdev` の Key 構文エラーによりコンパイルに失敗していた問題を修正。

**English**

### Fixed
- **Linux build error fix**: Fixed a compile failure caused by an `evdev` Key syntax error.

## [1.2.13] - 2026-02-14 [YANKED]

**日本語**

### Fixed
- **設定保存の正確性向上**: 設定保存時の競合（Race Condition）を解消し、スクロール詳細設定が確実に保存されるように修正。
- **注意**: このバージョンはビルドエラーのため使用できません。v1.2.14 を使用してください。

**English**

### Fixed
- **More reliable config saving**: Resolved a race condition when saving settings, so scroll tuning settings are now saved reliably.
- **Note**: This version is unusable due to a build error. Please use v1.2.14.

## [1.2.12] - 2026-02-14 [YANKED]

**日本語**

### Changed
- **Linux スクロール設定の統合**: 「Global Scroll Settings」と「Linux Scroll Tuning (Curve)」画面を統合。感度曲線（Curve）を基本モデルとし、UIを簡略化。
- **UIの改善**: `Max Scroll Output Speed` を感度曲線の y 軸上限と連動。設定項目（Sensitivity, Scroll Speed, Min Mouse Speed）を固定値化し UI から削除。
- **注意**: このバージョンはビルドエラーのため使用できません。v1.2.13 を使用してください。

**English**

### Changed
- **Unified Linux scroll settings**: Merged the "Global Scroll Settings" and "Linux Scroll Tuning (Curve)" screens. The sensitivity curve (Curve) is now the basic model, and the UI was simplified.
- **UI improvements**: Linked `Max Scroll Output Speed` to the y-axis limit of the sensitivity curve. The settings (Sensitivity, Scroll Speed, Min Mouse Speed) were fixed in value and removed from the UI.
- **Note**: This version is unusable due to a build error. Please use v1.2.13.

## [1.2.11] - 2026-02-14

**日本語**

### Fixed
- **Linux マウスロック問題の修正**: キーボードフック対象の自動判定ロジックを強化。相対/絶対座標を持つのポインティングデバイス（マウス・タッチパッド）を明示的に除外することで、起動時にマウスが操作不能になる問題を修正。

**English**

### Fixed
- **Linux mouse lock fix**: Strengthened the automatic detection of keyboard hook targets. Pointing devices with relative/absolute coordinates (mice, touchpads) are now explicitly excluded, fixing an issue where the mouse became unusable at startup.

## [1.2.10] - 2026-02-14

**日本語**

### Fixed
- **Linux ビルド構成の修正**: `evdev` クレートの `uinput` フィーチャー指定を削除（v0.12 では不要なため）。これにより依存関係の解決エラーを修正。

**English**

### Fixed
- **Linux build configuration fix**: Removed the `uinput` feature specification from the `evdev` crate (not required in v0.12), fixing a dependency resolution error.

## [1.2.9] - 2026-02-14

**日本語**

### Fixed
- **Linux ビルド環境への対応強化**: `evdev` の `uinput` ビルダーおよび `AttributeSet` の操作方法を、より互換性の高い構文に修正。

**English**

### Fixed
- **Stronger Linux build support**: Rewrote the `evdev` `uinput` builder and `AttributeSet` operations in a more compatible syntax.

## [1.2.8] - 2026-02-14

**日本語**

### Fixed
- **Linux ビルドエラーの修正**: `evdev` クレートの `uinput` フィーチャーが不足していたため、GitHub Actions でのコンパイルに失敗していた問題を修正。

**English**

### Fixed
- **Linux build error fix**: Fixed a GitHub Actions compile failure caused by a missing `uinput` feature in the `evdev` crate.

## [1.2.7] - 2026-02-14

**日本語**

### Fixed
- **Linux キーボード無反応問題の修正**: キーボードの排他制御（Grab）時に、リマップ対象外のイベントを物理デバイスへ戻すのではなく、`/dev/uinput` による仮想キーボード経由でOSへ再注入するように改善。これにより、アプリ起動中にマウスやキーボードが操作不能になる問題を根本解決。

**English**

### Fixed
- **Linux keyboard unresponsiveness fix**: When grabbing a keyboard exclusively, events not subject to remapping are now re-injected into the OS through a virtual keyboard via `/dev/uinput`, instead of being returned to the physical device. This fundamentally fixes the issue where the mouse and keyboard became unusable while the app was running.

## [1.2.6] - 2026-02-14

**日本語**

### Fixed
- **Linux スリープ復帰時の不具合修正**: Linux版のキーボードフックを X11 ベースの `rdev` からカーネルレベルの `evdev` 直接監視に移行。
  - セッション切断の影響を受けなくなり、スリープ復帰時も自動的にデバイスを再検知して動作を継続するよう改善。
  - キーボードデバイスの排他取得（Grab）により、より低遅延で安定したリマップを実現。

**English**

### Fixed
- **Linux sleep/resume fix**: Migrated the Linux keyboard hook from the X11-based `rdev` to direct kernel-level `evdev` monitoring.
  - No longer affected by session disconnects; devices are automatically re-detected after resuming from sleep so operation continues.
  - Exclusive device grab (Grab) enables lower-latency, more stable remapping.

## [1.2.5] - 2026-02-14

**日本語**

### Fixed
- **スクロール速度反映の修正**: 計算ロジック内の不要な係数を削除し、カーブエディタで設定した出力速度が正確に反映されるように修正。

### Changed
- **感度曲線の平滑化**: カーブエディタの補間アルゴリズムを線形からスプライン曲線（Monotone Cubic Spline）に変更。つなぎ目がなだらかになり、より直感的なチューニングが可能。
- **UIのブラッシュアップ**: グラフの描画品質向上、目盛りラベルの追加、グリッドの調整。

**English**

### Fixed
- **Scroll speed application fix**: Removed an unnecessary factor from the calculation logic so the output speed set in the curve editor is applied accurately.

### Changed
- **Smoother sensitivity curve**: Changed the curve editor's interpolation algorithm from linear to a spline curve (Monotone Cubic Spline), making joins smoother and tuning more intuitive.
- **UI polish**: Improved graph rendering quality, added tick labels, and adjusted the grid.

## [1.2.4] - 2026-02-14

**日本語**

### Added
- **スクロール感度曲線（カーブエディタ）**: マウスの移動速度に対するスクロール速度をグラフ上で直感的に設定できる機能を追加（Linux版）。
  - ポイントのドラッグ、クリックによる追加、右クリックによる削除に対応。
  - 従来の線形設定（Standard）と感度曲線（Advanced）をいつでも切り替え可能。
- **データ構造の拡張**: `AppConfig` にカーブデータ保持用のフィールドを追加。

**English**

### Added
- **Scroll sensitivity curve (Curve Editor)**: Added a feature to intuitively set scroll speed relative to mouse movement speed on a graph (Linux).
  - Supports dragging points, adding by click, and deleting by right-click.
  - The conventional linear setting (Standard) and the sensitivity curve (Advanced) can be switched at any time.
- **Data structure extension**: Added fields to `AppConfig` for holding curve data.

## [1.2.3] - 2026-02-14

**日本語**

### Fixed
- **Linuxスクロール不具合の修正**: 速度制限（クランプ）がかかった際に微小な移動量が切り捨てられる問題を、OS側送信関数への累積器（アキュムレータ）導入により解決。

### Changed
- **UIの改善 (別画面化)**: `Global Scroll Settings` モーダルからLinux専用詳細設定を分離。新設した `Linux Scroll Tuning` モーダルへ移動し、UIのオーバーフローを解消。
- **デフォルト値の調整**: `Max Scroll Output Speed` の初期値を 300 から 800 へ引き上げ。

**English**

### Fixed
- **Linux scroll fix**: Resolved an issue where small movements were truncated when the speed limit (clamp) applied, by adding an accumulator to the OS-level sending function.

### Changed
- **UI improvement (separate screen)**: Separated the Linux-only advanced settings from the `Global Scroll Settings` modal into a new `Linux Scroll Tuning` modal, resolving UI overflow.
- **Default adjustment**: Raised the default of `Max Scroll Output Speed` from 300 to 800.

## [1.2.2] - 2026-02-13

**日本語**

### Added
- **Linux専用スクロール詳細設定**: 移動量、速度、最小/最大出力速度のチューニング項目を `AppConfig` および UI に追加。
- **スクロールロジックの改善 (Linux)**: しきい値を超えるまで入力を抑制する「遊び」と、出力範囲のクランプ機能を実装。

**English**

### Added
- **Linux-only scroll tuning settings**: Added tuning items for movement amount, speed, and minimum/maximum output speed to `AppConfig` and the UI.
- **Scroll logic improvements (Linux)**: Implemented a "play" (deadzone) that suppresses input until a threshold is exceeded, and output range clamping.

## [1.2.1] - 2026-02-10

**日本語**

### Changed
- **大規模リファクタリング**: Windows と Linux のプラットフォーム固有コードを専用モジュールに分離
  - 新しい `platform` モジュール構造を作成（`platform/windows.rs`, `platform/linux.rs`）
  - タッチパッド監視の実装を `platform/touchpad/` に分離
  - `touchpad_monitor.rs` を 615行 → 155行 に削減（75%削減）
  - `keyboard_hook.rs` を 496行 → 290行 に削減（42%削減）
  - `lib.rs` を 534行 → 440行 に削減（18%削減）
  - 条件付きコンパイル（`#[cfg]`）ブロックを大幅に削減
- コードの保守性と可読性が向上

### Fixed
- **Linux スリープ復帰時のフック回復**: スリープ/サスペンドから復帰した後にキーボードフック（rdev::grab）が失敗した場合、自動でリトライするよう修正。

### Technical Details
- 入力シミュレーション、ウィンドウ管理、システム情報取得のための統一インターフェース（トレイト）を実装
- プラットフォーム固有の処理を抽象化し、将来的な拡張を容易に

**English**

### Changed
- **Major refactoring**: Separated Windows- and Linux-specific code into dedicated modules
  - Created a new `platform` module structure (`platform/windows.rs`, `platform/linux.rs`)
  - Separated the touchpad monitoring implementation into `platform/touchpad/`
  - Reduced `touchpad_monitor.rs` from 615 lines to 155 (75% reduction)
  - Reduced `keyboard_hook.rs` from 496 lines to 290 (42% reduction)
  - Reduced `lib.rs` from 534 lines to 440 (18% reduction)
  - Greatly reduced conditional compilation (`#[cfg]`) blocks
- Code maintainability and readability improved

### Fixed
- **Linux hook recovery after sleep**: The keyboard hook (rdev::grab) now retries automatically if it fails after resuming from sleep/suspend.

### Technical Details
- Implemented unified interfaces (traits) for input simulation, window management, and system information retrieval
- Abstracted platform-specific processing, making future extension easier

---

## [1.2.0] - 2026-02-10

**日本語**

### Added
- **Linux 版の正式サポート**: Ubuntu / Linux Mint などで動作する Linux 版を追加し、GitHub Releases で .deb / AppImage の配布を開始。タッチパッド検出の改善（除外キーワードによるキーボード誤検出の防止、Apple MacBook の bcm5974 タッチパッド対応）、udev ルールによるデバイスアクセス権限の設定、xdotool によるウィンドウ操作を含む
- **Scroll Speed 設定**: Sensitivity とは独立したスクロール速度の設定（10〜500%）を追加

### Fixed
- Linux のスクロールを Windows と同様の線形な挙動に改善。スムージング（デッドゾーン、アキュムレータ、方向ヒステリシス）と移動量のクランプ・フィルタリングにより、スクロールのジャンプや飛びを防止
- Linux のタッチパッド監視スレッドに panic ハンドリングとログ出力を追加し、起動時の問題を追跡可能にした

**English**

### Added
- **Official Linux support**: Added a Linux version that runs on Ubuntu, Linux Mint, and similar systems, and started distributing .deb / AppImage binaries via GitHub Releases. Includes improved touchpad detection (preventing keyboard misdetection via exclusion keywords, support for the Apple MacBook bcm5974 touchpad), device access permissions via udev rules, and window operations via xdotool
- **Scroll Speed setting**: Added a scroll speed setting (10–500%) independent of Sensitivity

### Fixed
- Made Linux scrolling linear, like Windows. Smoothing (deadzone, accumulator, direction hysteresis) and clamping/filtering of movement prevent scroll jumps and skips
- Added panic handling and logging to the Linux touchpad monitoring thread, making startup problems traceable

## [1.1.0] - 2026-02-06

**日本語**

### Changed
- ブランディングの洗練（名称・アイコンまわりの整理）
- メインウィンドウのサイズを 800x600 に拡大し、UI レイアウトを調整
- スタートアップ登録の状態とアプリ表示の同期を改善
- シェルウィンドウの表示ガードを追加

**English**

### Changed
- Refined branding (name, icons, and related elements)
- Enlarged the main window to 800x600 and adjusted the UI layout
- Improved synchronization between the startup registration state and the app display
- Added a guard for shell window display

## [1.0.1] - 2026-02-04

**日本語**

### Added
- ライセンス（LICENSE）ファイルを追加

### Fixed
- スクロールの挙動を修正

**English**

### Added
- Added the license (LICENSE) file

### Fixed
- Fixed scroll behavior

## [1.0.0] - 2026-02-04

**日本語**

### Added
- 初の正式リリース。タッチパッドの操作をトリガーに、あらかじめ割り当てたアクションを実行する
  - マウスクリック (左、右、中、ダブルクリック)
  - キーボードマクロ
  - ウィンドウ操作 (閉じる、最大化/元に戻す、移動)
  - スクロール
- 本バージョンは、復元された v0.4.8 のコードベースを基に整備したもの

**English**

### Added
- First official release. Performs pre-assigned actions triggered by touchpad operations
  - Mouse clicks (Left, Right, Middle, Double Click)
  - Keyboard macros
  - Window operations (Close, Maximize/Restore, Move)
  - Scrolling
- This version was prepared based on the restored v0.4.8 codebase
