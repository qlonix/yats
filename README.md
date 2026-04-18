# YATS (Yet Another Touchpad Shortcut)

マウスカーソルを動かした直後にキーを押すと、マウスボタン押下やキーマクロ、ウィンドウ操作などを実行できます。

実際には、タッチパッドに触れながらキーを押すことを想定しています。

## もう少し詳しく

このアプリケーションは、[ThumbSense](https://www.sonycsl.co.jp/projects/thumbsense/) の影響を強く受けています。

最近の Windows では ThumbSense がまともに動作しなくなってしまったため、代替案として作成しました。

YATS ではタッチパッドへ触れているかどうかではなく、マウスカーソルが動いているかどうか（+止まってから一定時間の猶予）をトリガーとしています。

そのため、ThumbSense ほど精度の高いトリガー検出はできませんが、その代わり Synaptics/ALPS 以外のタッチパッドでも動作することができます。

タッチパッドである必要すらないので、トラックポイントや [Nape Pro](https://www.gizmodo.jp/2025/11/gizmart-nape-pro.html) と組み合わせての操作も可能です。

## 機能

- **カスタマイズ可能なアクション**:
  - マウスクリック (左、右、中、ダブルクリック)
  - キーボードマクロ
  - ウィンドウ操作 (閉じる、最大化/元に戻す、最小化)
  - スクロール

## インストール

### Windows

インストーラー（.msi または .exe）をダウンロードして実行してください。

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
- **Wayland 環境**: ウィンドウ操作（`xdotool` を使用する機能）は Wayland 上では動作しない場合があります。リマップやスクロール機能は動作します。
- **権限**: デバイスへのアクセス権限が不足していると、キーのリマップが機能しません。必ず上記の udev ルールまたはグループ設定を確認してください。

## 開発

このプロジェクトのコード、ドキュメント等はほぼすべて **Antigravity** によって生成されました。

## ライセンス

[MIT](LICENSE)
