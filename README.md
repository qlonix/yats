# YATS (Yet Another Touchpad Shortcut)

マウスカーソルを動かした直後にキーを押すと、マウスボタン押下やキーマクロ、ウィンドウ操作などを実行できます。

実際には、タッチパッドに触れながらキーを押すことを想定しています。

## もう少し詳しく

このアプリケーションは、[ThumbSense](https://www.sonycsl.co.jp/projects/thumbsense/) の影響を強く受けています。

最近の Windows では ThumbSense がまともに動作しなくなってしまったため、代替案として作成しました。

YATS ではタッチパッドへ触れているかどうかではなく、マウスカーソルが動いているかどうか（+止まってから一定時間の猶予）をトリガーとしています。

そのため、ThumbSense ほど精度の高いトリガー検出はできませんが、その代わり Synaptics/ALPS 以外のタッチパッドでも動作することができます。

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

**重要**: Linuxでは、タッチパッドにアクセスするために、使用するユーザーを `input` グループに追加する必要があります。

```bash
sudo usermod -a -G input $USER
```

変更を反映するには、**ログアウトして再ログイン**（または再起動）してください。

#### インストール方法

**Debian/Ubuntu系 (.deb)**:
```bash
sudo dpkg -i yats_*.deb
```

**その他のディストリビューション (.AppImage)**:
```bash
chmod +x yats_*.AppImage
./yats_*.AppImage
```

> **注意**: AppImageには以下の制約があります：
> - **推奨**: 可能であれば `.deb` パッケージの使用を推奨します
> - **udevルール**: AppImageではudevルールが自動インストールされないため、手動でセットアップが必要です：
>   ```bash
>   # リポジトリからudevルールをダウンロード
>   sudo curl -o /etc/udev/rules.d/99-yats-touchpad.rules \
>     https://raw.githubusercontent.com/qlonix/yats/main/src-tauri/resources/99-yats-touchpad.rules
>   
>   # udevルールを再読み込み
>   sudo udevadm control --reload-rules
>   sudo udevadm trigger
>   ```
> - **FUSE要件**: AppImageの実行にはFUSEが必要です（最近のディストリビューションには通常含まれています）

#### 確認方法

`input` グループに追加されたか確認するには:
```bash
groups
```

出力に `input` が含まれていれば成功です。

## 開発

このプロジェクトのコード、ドキュメント等はほぼすべて **Antigravity** によって生成されました。

## ライセンス

[MIT](LICENSE)
