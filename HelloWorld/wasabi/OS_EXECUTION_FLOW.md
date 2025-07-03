# WasabiOS - 自作OS実行フロー概要

## 概要
WasabiOSは、Rustで書かれたUEFIベースの自作OSです。USBキーボードとタブレットの入力をサポートし、非同期処理を使用してシステムを管理します。

## プロジェクト構造

```
wasabi/
├── src/
│   ├── main.rs          # メインエントリーポイント
│   ├── lib.rs           # モジュール定義
│   ├── init.rs          # 初期化処理
│   ├── uefi.rs          # UEFI関連
│   ├── graphics.rs      # グラフィックス処理
│   ├── keyboard.rs      # キーボード処理
│   ├── tablet.rs        # タブレット処理
│   ├── usb.rs           # USB処理
│   ├── xhci.rs          # xHCIコントローラー
│   ├── executor.rs      # 非同期実行器
│   └── ... (その他のモジュール)
├── Cargo.toml           # プロジェクト設定
└── scripts/
    └── launch_qemu.sh   # QEMU起動スクリプト
```

## 重要な概念

### UEFI (Unified Extensible Firmware Interface)
- 従来のBIOSの代替技術
- WasabiOSはUEFIアプリケーションとして起動
- `efi_main`関数がエントリーポイント

### No-Std環境
- 標準ライブラリを使用しない（`#![no_std]`）
- OSレベルの低レベル制御が必要
- ヒープアロケーターを独自実装

### 非同期処理
- `async/await`を使用
- 独自の非同期実行器（executor）を実装
- 複数のタスクを並行実行

## 実行フロー概要

### 1. 起動段階 (main.rs:38-55)
- UEFIエントリーポイント `efi_main` が呼ばれる
- システム情報の表示
- グラフィックス初期化（VRAM、ディスプレイ）

### 2. ランタイム移行 (main.rs:54-56)
- ACPIテーブル取得
- UEFIブートサービス終了
- OSが直接ハードウェア制御を開始

### 3. システム初期化 (main.rs:57-63)
- メモリ管理（アロケーター、ページング）
- 例外処理（GDT、IDT）
- ハードウェア初期化（HPET、PCI）

### 4. 非同期タスク実行 (main.rs:64-98)
- 3つのタスクを登録・実行
  - テストタスク1（1秒間隔）
  - テストタスク2（2秒間隔）
  - シリアルポート監視タスク
- 非同期実行器の開始

## 詳細ドキュメント

各機能の詳細な解説は以下の個別ドキュメントを参照してください：

1. **[起動・初期化詳細](01_BOOT_AND_INITIALIZATION.md)** - UEFI起動からシステム初期化まで
2. **[メモリ管理詳細](02_MEMORY_MANAGEMENT.md)** - アロケーター、ページング、メモリマップ
3. **[非同期処理詳細](03_ASYNC_EXECUTION.md)** - executor、Future、タスクスケジューリング
4. **[USB機能詳細](04_USB_FUNCTIONALITY.md)** - キーボード・タブレット制御
5. **[ハードウェア制御詳細](05_HARDWARE_CONTROL.md)** - ACPI、PCI、HPET制御

## 技術的特徴

- **No-Std環境**: 標準ライブラリを使わずにOS機能を実装
- **UEFI起動**: 従来のBIOSではなくUEFIから起動
- **直接制御**: ハードウェアを直接制御（ACPI、PCI、xHCI）
- **非同期処理**: カスタム実行器による協調的マルチタスク
- **メモリ安全**: Rustの所有権システムによる安全性

## 実行環境

### QEMU環境
```bash
# ビルド
cargo build --release

# QEMU起動
./scripts/launch_qemu.sh
```

- OVMF（Open Virtual Machine Firmware）を使用
- UEFIブートをエミュレート

このOSは教育目的の自作OSとして、最小限の機能でハードウェアを直接制御し、非同期処理によってリアルタイムな応答性を実現しています。