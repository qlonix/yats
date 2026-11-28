# YATS (Yet Another Touchpad Shortcut)

タッチパッドに触れながらキーを押すと、マウスボタン押下やキーマクロ、ウィンドウ操作などを実行できます。

## もう少し詳しく

このアプリケーションは、[ThumbSense](https://www.sonycsl.co.jp/projects/thumbsense/) の影響を強く受けています。

最近の Windows では ThumbSense がまともに動作しなくなってしまったため、代替案として作成しました。

YATS のトリガー検出は、プラットフォームで方式が異なります。

Linux では、ThumbSense と同様にタッチパッドへの接触そのものを検出します。そのため、カーソルが止まっていても、指が触れている間（および指を離してから Release Delay の間）のキー押下でトリガーが発火します。検出は標準の evdev インターフェース経由なのでベンダーに依存せず、接触状態を evdev で取得できるタッチパッドであれば、Synaptics / ALPS 製でなくても動作します。パームリジェクションとして、接触面積による手のひら判定も行います。

なお、Linux でも設定の「Trigger Detection」で検出方式を切り替えられます。「Movement (Any Device)」にすると、カーソルが動いてから Release Delay 以内のキー押下をトリガーとする従来方式になり、マウスやトラックポイントなどタッチパッド以外のポインティングデバイスでも使えます。「Auto (Both)」は接触検出と移動検出の併用です。

Windows では、マウスカーソルが動いているかどうか（+止まってから一定時間の猶予）をトリガーとしています。そのため ThumbSense ほど精度の高いトリガー検出はできませんが、その代わり Synaptics/ALPS 以外のタッチパッドでも動作することができます。タッチパッドである必要すらないので、トラックポイントや [Nape Pro](https://www.gizmodo.jp/2025/11/gizmart-nape-pro.html) と組み合わせての操作も可能です（Precision Touchpad 搭載機種では、接触情報をパームリジェクションにも利用します）。

## プラットフォームによる違い

OS のタッチパッド仕様の違いが、そのまま実装と操作感の違いになっています。

- **パーム判定の材料**: Windows の Precision Touchpad は、接触ごとに Confidence（本物の指かどうかの確信）ビットを付けて報告し、手のひらのような接触は Windows 側でポインタ操作から除外されます。YATS の Windows 版モニタも Confidence ありの接触だけを数えます。一方、Linux の evdev には Confidence 相当の情報がないため、YATS が接触面積の生値やタイピングの状況から自前で判定しています（改善中）。
- **タップでクリックとの関係**: Windows では Confidence のない接触はタップとしても扱われないため、「タップでクリック」を有効にしていてもタイピング中の誤タップはほとんど起きません。Linux では、置いた手のひらの動きを libinput がタップと判定してしまうことがあり、タイピング中にキャレットが飛ぶなどの誤動作の原因になります。YATS の利用時は、OS 側の「タップでクリック」を無効にするか、「入力中はタッチパッドを無効にする」（dwt）の併用を推奨します。
- **操作感**: Linux では「触れている間」が条件なので、カーソルが静止していても指を置いたままキー操作ができます。Windows では「動かした直後」が条件なので、完全に止まった状態からの押下は発火しませんが、その代わりタッチパッド以外のポインティングデバイスでも同じ操作ができます。

## 機能

- **カスタマイズ可能なアクション**:
  - マウスクリック (左、右、中、ダブルクリック)
  - キーボードマクロ
  - ウィンドウ操作 (閉じる、最大化/元に戻す、最小化、ドラッグ移動)
  - スクロール
- **スクロール調整 (Scroll Tuning)**:
  - 加速度カーブをグラフで編集でき、出力軸 (Min/Max Scroll Output Speed) と入力軸の上限 (Max Input Speed) を変更すると、カーブが形を保ったまま伸縮します
  - 入力速度の上限の初期値は、接続中のモニタ幅から自動決定されます
  - 調整中は、現在のスクロール速度に対応する点がグラフ上にリアルタイムで表示されます

## 動作環境

| 項目 | Windows | Linux |
|---|---|---|
| OS | Windows 10 / 11（64bit） | 64bit ディストリビューション（glibc 2.35 以降。Ubuntu 22.04 以降相当） |
| アーキテクチャ | x86_64 | x86_64 |
| その他 | Microsoft Edge WebView2 ランタイム（通常は OS に含まれています） | X11 セッション推奨（Wayland では一部機能に制限あり。下記「既知の制限」参照） |

- Windows のパームリジェクションは Precision Touchpad（PTP）搭載機種で動作します。それ以外のポインティングデバイスでも、カーソル移動をトリガーとする基本機能は利用できます。
- Linux ではデバイスアクセス権限の設定が必要です（下記「必須要件」参照）。

## インストール

### Windows

インストーラー（.msi または .exe）をダウンロードして実行してください。

> [!NOTE]
> **「Windows によって PC が保護されました」と表示される場合**  
> 本ソフトは個人開発のオープンソースソフトウェアであり、デジタル署名を施していないため、Windows SmartScreen によって警告が表示されることがあります。
> インストールを続行するには、画面上の**「詳細情報」**をクリックし、表示された**「実行」**ボタンを押してください。

### Linux

#### 必須要件

**重要**: Linuxでは、キーボード入力の監視と仮想デバイスの作成（リマップ）を行うために、適切なデバイスアクセス権限が必要です。

1. **ユーザーグループの追加**:
   現在のユーザーを `input` グループに追加してください。
   ```bash
   sudo usermod -a -G input $USER
   ```
   変更を反映するには、**一度ログアウトして再ログイン**（または再起動）してください。

2. **ランタイム依存パッケージのインストール**:
   ウィンドウ操作機能（閉じる、最大化など）を利用するには、以下のツールが必要です。
   ```bash
   # Debian/Ubuntu系
   sudo apt install xdotool wmctrl
   ```

3. **udev ルールの設定**:
   仮想キーボードの機能を利用するためには、各デバイスおよび `/dev/uinput` へのアクセス権限が必要です。（`.deb` パッケージを利用する場合は自動的に設定されますが、`.AppImage` の場合は手作業での導入が必要です）。

#### インストール方法

**Debian/Ubuntu系 (.deb)**:
```bash
sudo dpkg -i yats_*.deb
```

**その他のディストリビューション (.AppImage)**:

AppImage版を実行するには、事前に以下の udev ルールを手動でセットアップしてください。

```bash
# リポジトリからudevルールをダウンロード
sudo curl -o /etc/udev/rules.d/99-yats.rules \
  https://raw.githubusercontent.com/qlonix/yats/main/src-tauri/resources/99-yats-touchpad.rules

# udevルールを再読み込み
sudo udevadm control --reload-rules && sudo udevadm trigger
```

セットアップが完了したら、AppImageを実行可能にして起動します。
```bash
chmod +x yats_*.AppImage
./yats_*.AppImage
```

#### 既知の制限 (Linux)
- **パームリジェクション（発展途上）**: Linux 版のパームリジェクションは現在も改善中です。接触面積による手のひら判定と、タイピング直後のキー押下の抑止（Palm Delay）を組み合わせていますが、デバイスや使い方によっては、意図した操作が抑止されたり、タイピング中の誤発火が残ったりする場合があります。
- **Wayland 環境**: ウィンドウ操作（`xdotool` を使用する機能）は Wayland 上では動作しない場合があります。リマップやスクロール機能は動作します。
- **権限**: デバイスへのアクセス権限が不足していると、キーのリマップが機能しません。必ず上記の udev ルールまたはグループ設定を確認してください。

## 開発

このプロジェクトの初期のコード、ドキュメント等は **Antigravity** によって生成されました。その後はメンテナの監修のもと、複数の AI 支援ツールを活用して開発が継続されています。

## ライセンス

[MIT](LICENSE)

---

# YATS (Yet Another Touchpad Shortcut)

Press keys while touching the touchpad to perform mouse button clicks, key macros, window operations, and more.

## More Details

This application is heavily inspired by [ThumbSense](https://www.sonycsl.co.jp/projects/thumbsense/).

It was created as an alternative because ThumbSense no longer works properly on modern Windows.

How YATS detects triggers differs by platform.

On Linux, like ThumbSense, YATS detects actual contact with the touchpad. A key press therefore triggers while your finger is touching (and for the Release Delay after you lift it), even when the cursor is perfectly still. Detection goes through the standard evdev interface, so it is vendor-independent: any touchpad that exposes its contact state via evdev works, not just Synaptics/ALPS ones. Contacts are also classified by size for palm rejection.

On Linux you can also switch the detection method in the "Trigger Detection" setting. "Movement (Any Device)" uses the traditional method — a key press within the Release Delay after the cursor moved — which works with any pointing device such as a mouse or a TrackPoint. "Auto (Both)" combines contact and movement detection.

On Windows, YATS uses mouse cursor movement (plus a certain delay after it stops) as a trigger. Therefore, it cannot detect triggers with as much precision as ThumbSense, but it can work with touchpads other than Synaptics/ALPS. Since it doesn't even need to be a touchpad, it can also be used in combination with a TrackPoint or [Nape Pro](https://www.gizmodo.jp/2025/11/gizmart-nape-pro.html). (On machines with a Precision Touchpad, contact information is also used for palm rejection.)

## Platform Differences

Differences in the OS touchpad stacks translate directly into differences in implementation and feel.

- **Basis for palm rejection**: A Windows Precision Touchpad reports each contact with a Confidence bit (whether it is a real finger); palm-like contacts are excluded from pointer operation by Windows itself, and YATS's Windows monitor likewise counts only contacts with Confidence set. Linux evdev has no equivalent of Confidence, so YATS classifies contacts itself from raw contact size and typing context (still being improved).
- **Interaction with tap-to-click**: On Windows, a contact without Confidence is never treated as a tap, so tap-to-click can stay enabled with almost no stray taps while typing. On Linux, a resting palm that shifts can be classified as a tap by libinput, which moves the text caret mid-typing and garbles input. When using YATS, we recommend disabling OS tap-to-click, or combining it with "disable while typing" (dwt).
- **Feel**: On Linux the condition is "while touching", so you can operate with the cursor perfectly still, finger resting on the pad. On Windows the condition is "right after moving", so a press from a complete standstill does not trigger — but the same operation works with any pointing device, not just touchpads.

## Features

- **Customizable Actions**:
  - Mouse clicks (Left, Right, Middle, Double Click)
  - Keyboard macros
  - Window operations (Close, Maximize/Restore, Minimize, Drag)
  - Scrolling
- **Scroll Tuning**:
  - Edit the acceleration curve on a graph; changing the output axis range (Min/Max Scroll Output Speed) or the input axis limit (Max Input Speed) rescales the curve while preserving its shape
  - The input speed limit is initially determined automatically from your monitor width
  - While tuning, a live marker shows the point on the curve for your current scroll speed in real time

## System Requirements

| Item | Windows | Linux |
|---|---|---|
| OS | Windows 10 / 11 (64-bit) | 64-bit distribution (glibc 2.35 or later, equivalent to Ubuntu 22.04 or later) |
| Architecture | x86_64 | x86_64 |
| Other | Microsoft Edge WebView2 Runtime (normally included with the OS) | X11 session recommended (some features are limited on Wayland; see "Known Limitations" below) |

- Palm rejection on Windows works on machines with a Precision Touchpad (PTP). The basic features, which use cursor movement as the trigger, also work with other pointing devices.
- On Linux, device access permissions must be configured (see "Requirements" below).

## Installation

### Windows

Download and run the installer (.msi or .exe).

> [!NOTE]
> **If "Windows protected your PC" appears**  
> Since this is an open-source project by an individual developer without an expensive digital signature, Windows SmartScreen may show a warning.
> To proceed, click **"More info"** on the warning screen and then click the **"Run anyway"** button.

### Linux

#### Requirements

**Important**: On Linux, appropriate device access permissions are required to monitor keyboard input and create virtual devices (remapping).

1. **Add User Groups**:
   Add the current user to the `input` group.
   ```bash
   sudo usermod -a -G input $USER
   ```
   To apply changes, **log out and log back in** (or restart).

2. **Install Runtime Dependencies**:
   To use window operation functions (close, maximize, etc.), the following tools are required.
   ```bash
   # For Debian/Ubuntu-based
   sudo apt install xdotool wmctrl
   ```

3. **Configure udev Rules**:
   Access permissions to each device and `/dev/uinput` are required to use the virtual keyboard functionality. (It is automatically configured when using the `.deb` package, but manual setup is required for `.AppImage`).

#### Installation Method

**Debian/Ubuntu-based (.deb)**:
```bash
sudo dpkg -i yats_*.deb
```

**Other Distributions (.AppImage)**:

To run the AppImage version, set up the following udev rules manually beforehand.

```bash
# Download udev rules from the repository
sudo curl -o /etc/udev/rules.d/99-yats.rules \
  https://raw.githubusercontent.com/qlonix/yats/main/src-tauri/resources/99-yats-touchpad.rules

# Reload udev rules
sudo udevadm control --reload-rules && sudo udevadm trigger
```

After setup is complete, make the AppImage executable and launch it.
```bash
chmod +x yats_*.AppImage
./yats_*.AppImage
```

#### Known Limitations (Linux)
- **Palm Rejection (work in progress)**: Palm rejection on Linux is still being improved. It combines contact-size classification with suppression of key presses right after typing (Palm Delay), but depending on your device and usage, intended actions may occasionally be suppressed, or stray triggers may still slip through while typing.
- **Wayland Environment**: Window operations (functions using `xdotool`) may not work on Wayland. Remapping and scrolling functions will work.
- **Permissions**: If access permissions to the device are insufficient, key remapping will not function. Be sure to check the above udev rules or group settings.

## Development

The initial code, documentation, etc. for this project were generated by **Antigravity**. Development has since continued under the maintainer's supervision with the help of multiple AI-assisted tools.

## License

[MIT](LICENSE)

