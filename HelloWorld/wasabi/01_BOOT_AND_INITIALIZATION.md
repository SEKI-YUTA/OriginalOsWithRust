# WasabiOS - 起動・初期化詳細解説

## 目次
1. [Rustコンパイラ設定](#rustコンパイラ設定)
2. [モジュールimport](#モジュールimport)
3. [UEFIエントリーポイント](#uefiエントリーポイント)
4. [システム情報出力](#システム情報出力)
5. [グラフィックス初期化](#グラフィックス初期化)
6. [ログシステム](#ログシステム)
7. [パニックハンドラー](#パニックハンドラー)

---

## Rustコンパイラ設定

### main.rs:1-3 - コンパイラ属性
```rust
#![no_std]
#![no_main]
#![feature(offset_of)]
```

#### `#![no_std]` (main.rs:1)
- **目的**: Rust標準ライブラリを使用しない
- **詳細解説**:
  - 標準ライブラリ（std）はOSが提供するシステムコールに依存
  - OSを作成中なので、そのシステムコール自体が存在しない
  - `core`ライブラリのみ使用（アロケーション不要な基本機能）
  - `alloc`ライブラリは独自アロケーター実装後に使用可能

#### `#![no_main]` (main.rs:2)
- **目的**: Rust標準の`main`関数を使用しない
- **詳細解説**:
  - 通常のRustプログラムは`fn main()`から開始
  - OSでは `efi_main` が実際のエントリーポイント
  - Rustランタイムの初期化処理を回避

#### `#![feature(offset_of)]` (main.rs:3)
- **目的**: 不安定機能 `offset_of` の使用を許可
- **詳細解説**:
  - 構造体のフィールドオフセットを取得する機能
  - C言語の `offsetof` マクロと同等
  - ハードウェア構造体の正確なメモリレイアウト制御に必要

---

## モジュールimport

### main.rs:6-33 - use文解説
```rust
use core::panic::PanicInfo;
```
- **core::panic::PanicInfo**: パニック情報を含む構造体
- `no_std`環境でのパニックハンドラー実装に必要

```rust
use core::time::Duration;
```
- **core::time::Duration**: 時間間隔を表現する型
- `std::time::Duration`の`no_std`版
- 非同期sleep処理で使用

```rust
use wasabi::executor::sleep;
use wasabi::executor::spawn_global;
use wasabi::executor::start_global_executor;
```
- **executor系**: 独自実装の非同期実行器
- `sleep`: 指定時間の非同期待機
- `spawn_global`: タスクをグローバル実行器に登録
- `start_global_executor`: 実行器のメインループ開始

```rust
use wasabi::graphics::draw_test_pattern;
```
- **draw_test_pattern**: グラフィックステストパターン描画
- VRAM初期化後の動作確認用

```rust
use wasabi::hpet::global_timestamp;
```
- **global_timestamp**: HPET（高精度タイマー）から現在時刻取得
- 非同期タスクの時間測定に使用

```rust
use wasabi::init;
use wasabi::init::init_allocator;
use wasabi::init::init_display;
use wasabi::init::init_hpet;
use wasabi::init::init_paging;
use wasabi::init::init_pci;
```
- **init系**: システム初期化関数群
- 各ハードウェア・ソフトウェアコンポーネントの初期化

```rust
use wasabi::print::hexdump;
use wasabi::print::set_global_vram;
```
- **print系**: 出力・ログ機能
- `hexdump`: メモリダンプ表示
- `set_global_vram`: グローバルVRAM設定

```rust
use wasabi::qemu::exit_qemu;
use wasabi::qemu::QemuExitCode;
```
- **qemu系**: QEMU仮想化環境制御
- OSの終了処理

```rust
use wasabi::serial::SerialPort;
```
- **SerialPort**: シリアルポート通信
- デバッグ出力やQEMUモニター通信

```rust
use wasabi::uefi::init_vram;
use wasabi::uefi::EfiHandle;
use wasabi::uefi::EfiSystemTable;
use wasabi::uefi::locate_loaded_image_protocol;
```
- **uefi系**: UEFI（統合拡張ファームウェアインターフェース）関連
- UEFIブートサービスとの通信

```rust
use wasabi::x86::flush_tlb;
use wasabi::x86::init_exceptions;
```
- **x86系**: x86-64アーキテクチャ固有機能
- メモリ管理、例外処理

```rust
use wasabi::println;
use wasabi::warn;
use wasabi::info;
use wasabi::error;
```
- **ログマクロ**: カスタムロギングシステム
- ファイル名・行番号付きログ出力

---

## UEFIエントリーポイント

### main.rs:37-38 - 関数定義
```rust
#[no_mangle]
fn efi_main(image_handle: EfiHandle, efi_system_table: &EfiSystemTable) {
```

#### `#[no_mangle]` 属性 (main.rs:37)
- **目的**: 関数名のマングリング（名前変換）を防ぐ
- **詳細解説**:
  - Rustコンパイラは関数名を独自形式に変換（マングリング）
  - UEFIファームウェアは `efi_main` という正確な名前で関数を呼び出す
  - マングリングを無効にして、リンカーが正確な名前を保持

#### 関数パラメータ解説
```rust
image_handle: EfiHandle
```
- **型**: `EfiHandle` (= `u64`)
- **内容**: 実行可能ファイルのハンドル
- **用途**: UEFIプロトコルでの自身の識別

```rust
efi_system_table: &EfiSystemTable
```
- **型**: `&EfiSystemTable` (構造体参照)
- **内容**: UEFIシステムテーブルへのポインタ
- **用途**: UEFIサービス（メモリ、入出力等）へのアクセス

---

## システム情報出力

### main.rs:39-45 - 基本情報出力
```rust
println!("Booting WasabiOS...");
println!("image_handle: {:#018X}", image_handle);
println!("efi_system_table: {:#p}", efi_system_table);
```

#### `println!` マクロ解析 (print.rs:30-33)
```rust
#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}
```
- **展開例**: `println!("Hello")` → `print!("{}\n", format_args!("Hello"))`
- **実装**: `print!` マクロを呼び出し、改行文字を追加

#### `print!` マクロ解析 (print.rs:25-27)
```rust
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::print::global_print(format_args!($($arg)*)));
}
```
- **展開**: `global_print()` 関数を呼び出し

#### `global_print()` 関数解析 (print.rs:16-22)
```rust
pub fn global_print(args: fmt::Arguments) {
    let mut writer = SerialPort::default();
    fmt::write(&mut writer, args).unwrap();
    if let Some(w) = &mut *GLOBAL_VRAM_WRITER.lock() {
        fmt::write(w, args).expect("Failed to write to GLOBAL_VRAM_WRITER");
    }
}
```
- **行17**: シリアルポート（COM1）に出力
- **行18**: シリアルポートに書き込み
- **行19-21**: 画面（VRAM）にも出力（初期化後）

#### フォーマット指定子解説
```rust
println!("image_handle: {:#018X}", image_handle);
```
- **`{:#018X}`**: 
  - `#`: プレフィックス `0x` を付加
  - `018`: 18文字幅、先頭を0で埋める
  - `X`: 大文字16進数表示
  - 結果例: `0x000000000000ABCD`

```rust
println!("efi_system_table: {:#p}", efi_system_table);
```
- **`{:#p}`**: ポインタアドレスを16進数で表示

### main.rs:42-45 - LoadedImageProtocol取得
```rust
let loaded_image_protocol = locate_loaded_image_protocol(image_handle, efi_system_table)
    .expect("Failed to get LoadedImageProtocol");
println!("image_base: {:#018X}", loaded_image_protocol.image_base);
println!("image_size: {:#018X}", loaded_image_protocol.image_size);
```

#### `locate_loaded_image_protocol()` の内部動作
1. **UEFIプロトコル検索**: `image_handle` に関連するプロトコルを検索
2. **GUID照合**: `EFI_LOADED_IMAGE_PROTOCOL_GUID` と一致するプロトコルを取得
3. **構造体変換**: プロトコルデータを `LoadedImageProtocol` 構造体にキャスト
4. **情報取得**: 実行ファイルのメモリベースアドレス・サイズを取得

#### LoadedImageProtocol構造体の内容
- **image_base**: 実行ファイルがロードされたメモリアドレス
- **image_size**: 実行ファイルのバイトサイズ
- これらの情報はメモリ管理・デバッグに使用

---

## ログシステム

### main.rs:46-48 - ログレベルテスト
```rust
info!("info");
warn!("warn");
error!("error");
```

#### `info!` マクロ解析 (print.rs:36-39)
```rust
#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => ($crate::print!("[INFO]  {}:{:<3}: {}\n",
            file!(), line!(), format_args!($($arg)*)));
}
```
- **`file!()`**: コンパイル時にファイル名を取得
- **`line!()`**: コンパイル時に行番号を取得
- **`{:<3}`**: 左寄せ、3文字幅で行番号を表示
- **出力例**: `[INFO]  src/main.rs:46 : info`

#### ログレベル比較
| マクロ | プレフィックス | 用途 |
|--------|---------------|------|
| `info!` | `[INFO]` | 一般情報 |
| `warn!` | `[WARN]` | 警告 |
| `error!` | `[ERROR]` | エラー |

---

## グラフィックス初期化

### main.rs:49 - メモリダンプ表示
```rust
hexdump(efi_system_table);
```

#### `hexdump()` 関数解析 (print.rs:108-122)
```rust
pub fn hexdump<T>(ptr: *const T) {
    let t_size = size_of::<T>();
    let bytes = unsafe { slice::from_raw_parts(ptr.cast::<u8>(), t_size) };
    hexdump_bytes(bytes);
}
```
- **行109**: 構造体サイズを取得
- **行110**: ポインタからバイト配列スライスを作成
- **行111**: バイトデータをhexdumpで表示

#### `hexdump_bytes()` 関数詳細 (print.rs:53-100)
```rust
fn hexdump_bytes(bytes: &[u8]) {
    let mut i = 0;
    let mut ascii = [0u8; 16];
    let mut offset = 0;
    for v in bytes.iter() {
        if i == 0 {
            print!("{offset:08X}: ");  // オフセット表示
        }
        print!("{:02X} ", v);          // 16進数値表示
        ascii[i] = *v;                 // ASCII文字保存
        i += 1;
        if i == 16 {                   // 16バイト毎に改行
            print!("|");
            for c in ascii.iter() {
                print!("{}", 
                    match c {
                        0x20..=0x7e => *c as char,  // 印刷可能文字
                        _ => '.',                    // 非印刷文字は'.'
                    }
                );
            }
            println!("|");
            offset += 16;
            i = 0;
        }
    }
    // 末尾の不完全な行の処理...
}
```

### main.rs:50 - VRAM初期化
```rust
let mut vram = init_vram(efi_system_table).expect("init_vram failed");
```

#### `init_vram()` の内部処理
1. **GOP取得**: Graphics Output Protocol をUEFIから取得
2. **フレームバッファ情報取得**: ピクセル形式、解像度、メモリアドレス
3. **VramBufferInfo構造体作成**: フレームバッファの詳細情報を格納
4. **戻り値**: 画面描画に使用する構造体

#### VramBufferInfo構造体の内容
- **buffer**: フレームバッファのメモリアドレス
- **width/height**: 画面の解像度
- **pixels_per_line**: 1行あたりのピクセル数
- **bytes_per_pixel**: 1ピクセルあたりのバイト数（通常4: RGBA）

### main.rs:51 - テストパターン描画
```rust
draw_test_pattern(&mut vram);
```

#### `draw_test_pattern()` 関数の動作
1. **グラデーション描画**: RGB値を徐々に変化させる
2. **線描画**: 対角線や格子パターン
3. **色確認**: 基本色（赤・緑・青・白・黒）の表示
4. **ピクセル操作テスト**: VRAMへの直接書き込み確認

### main.rs:52-53 - ディスプレイ設定
```rust
init_display(&mut vram);
set_global_vram(vram);
```

#### `init_display()` の処理 (init.rs:83-88)
```rust
pub fn init_display(vram: &mut VramBufferInfo) {
    let vw = vram.width();
    let vh = vram.height();
    fill_rect(vram, 0x000000, 0, 0, vw, vh).expect("fill_rect failed");
    draw_test_pattern(vram);
}
```
- **行84-85**: 画面サイズを取得
- **行86**: 画面全体を黒色（0x000000）で塗りつぶし
- **行87**: テストパターンを再描画

#### `fill_rect()` 関数詳細 (graphics.rs:39-63)
```rust
pub fn fill_rect<T: Bitmap>(
    buf: &mut T,
    color: u32,
    px: i64, py: i64,
    w: i64, h: i64,
) -> Result<()> {
    // 範囲チェック
    if !buf.is_in_x_range(px) || !buf.is_in_y_range(py) 
        || !buf.is_in_x_range(px + w - 1) || !buf.is_in_y_range(py + h - 1) {
        return Err("Out of Range");
    }
    
    // ピクセル描画ループ
    for y in py..py + h {
        for x in px..px + w {
            unsafe {
                unchecked_draw_point(buf, color, x, y);
            }
        }
    }
    Ok(())
}
```

#### `set_global_vram()` の処理 (print.rs:11-15)
```rust
pub fn set_global_vram(vram: VramBufferInfo) {
    assert!(GLOBAL_VRAM_WRITER.lock().is_none());
    let w = BitmapTextWriter::new(vram);
    *GLOBAL_VRAM_WRITER.lock() = Some(w);
}
```
- **行12**: 既に設定済みでないことを確認
- **行13**: テキスト描画用のライターを作成
- **行14**: グローバル変数に設定（以降のprintln!で画面出力が有効）

---

## パニックハンドラー

### main.rs:101-105 - パニック処理
```rust
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    error!("PANIC: {info:?}");
    exit_qemu(QemuExitCode::Fail);
}
```

#### `#[panic_handler]` 属性
- **目的**: Rustのパニック発生時の処理を定義
- **必須**: `no_std`環境では必ず実装が必要
- **戻り値型**: `-> !` (never type: 決して戻らない)

#### パニック処理の流れ
1. **行103**: パニック情報をログ出力
   - `{info:?}`: PanicInfo構造体をDebug形式で表示
   - ファイル名、行番号、パニック理由が含まれる
2. **行104**: QEMUを終了コード1で終了
   - `QemuExitCode::Fail`: 異常終了を示す

#### PanicInfo構造体の内容
- **location**: パニック発生場所（ファイル名・行番号）
- **message**: パニックメッセージ
- **payload**: パニック時の追加情報

---

## まとめ

この起動・初期化フェーズでは以下の処理が行われます：

1. **Rust環境設定**: no_std/no_main でのOS開発環境
2. **UEFIエントリー**: ファームウェアからのOS起動
3. **システム情報取得**: 実行環境の詳細情報を収集
4. **ログシステム初期化**: シリアル出力とVRAM出力の設定
5. **グラフィックス初期化**: フレームバッファを使った画面制御
6. **エラーハンドリング**: パニック時の適切な処理

次のフェーズでは、ACPIテーブル取得とUEFIからOSランタイムへの移行が行われます。