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

**重要**: Linuxでは、タッチパッド入力の監視と仮想キーボード操作（`uinput`）を行うために、適切なデバイスアクセス権限が必要です。

1. **ユーザーグループの追加**:
   現在のユーザーを `input` グループに追加してください。
   ```bash
   sudo usermod -a -G input $USER
   ```
   変更を反映するには、**ログアウトして再ログイン**（または再起動）してください。確認するには `groups` コマンドを実行し、出力に `input` が含まれるか確認します。

2. **udev ルールの設定**:
   仮想キーボードの機能を利用するためには、各デバイスおよび `/dev/uinput` へのアクセス権限が必要です。（`.deb` パッケージを利用する場合は自動的にインストールされますが、`.AppImage` の場合は手作業での導入が必要です）。

#### インストール方法

**Debian/Ubuntu系 (.deb)**:
```bash
sudo dpkg -i yats_*.deb
```

**その他のディストリビューション (.AppImage)**:

AppImage版を実行するには、事前に以下の udev ルールを手動でセットアップする必要があります。これを怠ると、権限エラーでYATSの一部機能（キーのリマップ等）が動作しません。

```bash
# リポジトリからudevルールをダウンロード
sudo curl -o /etc/udev/rules.d/99-yats-touchpad.rules \
  https://raw.githubusercontent.com/qlonix/yats/main/src-tauri/resources/99-yats-touchpad.rules

# udevルールを再読み込み
sudo udevadm control --reload-rules
sudo udevadm trigger
```

セットアップが完了したら、AppImageを実行可能にして起動します。
```bash
chmod +x yats_*.AppImage
./yats_*.AppImage
```

> **注意**: AppImageの実行にはFUSEが必要です（最近のディストリビューションには通常含まれています）。可能であれば `.deb` パッケージの使用を推奨します。

## 開発

このプロジェクトのコード、ドキュメント等はほぼすべて **Antigravity** によって生成されました。

## ライセンス

[MIT](LICENSE)
