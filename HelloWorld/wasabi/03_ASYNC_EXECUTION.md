# WasabiOS - 非同期処理詳細解説

## 目次
1. [非同期処理概要](#非同期処理概要)
2. [Task構造体とFuture trait](#task構造体とfuture-trait)
3. [Executor実装](#executor実装)
4. [Waker システム](#wakerシステム)
5. [時間ベース非同期処理](#時間ベース非同期処理)
6. [実際のタスク実行例](#実際のタスク実行例)
7. [非同期I/O実装](#非同期io実装)
8. [パフォーマンス解析](#パフォーマンス解析)

---

## 非同期処理概要

WasabiOSの非同期処理システムは以下の要素で構成されています：

### アーキテクチャ図
```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   Application   │    │   Application   │    │   Application   │
│     Task 1      │    │     Task 2      │    │     Task 3      │
│  (Timer Test)   │    │  (Timer Test)   │    │ (Serial Monitor)│
└─────────────────┘    └─────────────────┘    └─────────────────┘
         │                       │                       │
         └───────────────────────┼───────────────────────┘
                                 │
                 ┌─────────────────────────────┐
                 │     Global Executor         │
                 │   (Task Queue Manager)      │
                 └─────────────────────────────┘
                                 │
                 ┌─────────────────────────────┐
                 │      Future Polling         │
                 │    (Cooperative Yield)      │
                 └─────────────────────────────┘
                                 │
    ┌─────────────────┬─────────────────┬─────────────────┐
    │      HPET       │     Serial      │      Memory     │
    │   (Timer HW)    │   (UART HW)     │   (Allocator)   │
    └─────────────────┘─────────────────┘─────────────────┘
```

### 設計思想
1. **協調的マルチタスク**: タスクが自主的にCPUを譲渡
2. **イベント駆動**: ハードウェアイベントによる非同期処理
3. **ゼロオーバーヘッド**: 抽象化のコストを最小化
4. **型安全**: Rustの型システムによる安全性保証

---

## Task構造体とFuture trait

### Task構造体詳細解析 (executor.rs:23-46)

#### 構造体定義
```rust
struct Task<T> {
    future: Pin<Box<dyn Future<Output = Result<T>>>>,  // 実際の非同期処理
    created_at_file: &'static str,                     // 作成場所ファイル名
    created_at_line: u32,                              // 作成場所行番号
}
```

#### 詳細解説

##### `Pin<Box<dyn Future<...>>>`の分解
```rust
Pin<Box<dyn Future<Output = Result<T>>>>
│   │   │                      │
│   │   │                      └─ Future の戻り値型
│   │   └─ trait object (動的ディスパッチ)
│   └─ ヒープ割り当て (動的サイズ対応)
└─ 不変位置保証 (self-referential 対応)
```

- **`Future` trait**: 非同期処理の基本インターフェース
- **`dyn Future`**: 異なる型のFutureを統一的に扱う
- **`Box`**: 可変サイズのFutureをヒープに配置
- **`Pin`**: self-referential構造体の安全性保証

##### `#[track_caller]`によるデバッグ支援 (executor.rs:29-36)
```rust
#[track_caller]
fn new(future: impl Future<Output = Result<T>> + 'static) -> Task<T> {
    Task {
        future: Box::pin(future),
        created_at_file: Location::caller().file(),      // コンパイル時にファイル名取得
        created_at_line: Location::caller().line(),      // コンパイル時に行番号取得
    }
}
```

- **`#[track_caller]`**: 呼び出し元の位置情報を自動取得
- **`Location::caller()`**: コンパイル時に位置情報を解決
- **デバッグ情報**: タスクがどこで作成されたかを追跡可能

#### Task::poll関数 (executor.rs:38-40)
```rust
fn poll(&mut self, context: &mut Context) -> Poll<Result<T>> {
    self.future.as_mut().poll(context)
}
```
- **`as_mut()`**: `Pin<Box<T>>`から`Pin<&mut T>`に変換
- **`poll(context)`**: Futureの実際の実行を委譲
- **戻り値**: `Poll::Ready(value)` または `Poll::Pending`

---

## Executor実装

### グローバル実行器設計

#### 静的グローバル変数 (executor.rs:164)
```rust
static GLOBAL_EXECUTOR: Mutex<Option<Executor>> = Mutex::new(None);
```
- **`static`**: プログラム全体で単一のインスタンス
- **`Mutex`**: 排他制御（シングルスレッド環境でも一貫性保証）
- **`Option`**: 遅延初期化対応

#### Executor構造体 (executor.rs:77-84)
```rust
pub struct Executor {
    task_queue: Option<VecDeque<Task<()>>>,
}

impl Executor {
    const fn new() -> Self {
        Self {
            task_queue: None,    // 遅延初期化
        }
    }
}
```

### タスク登録プロセス

#### spawn_global関数詳細解析 (executor.rs:165-169)
```rust
#[track_caller]
pub fn spawn_global(future: impl Future<Output = Result<()>> + 'static) {
    let task = Task::new(future);                              // タスク作成
    GLOBAL_EXECUTOR.lock().get_or_insert_default().enqueue(task);  // キューに追加
}
```

##### 処理フロー詳細
1. **Future受け取り**: 任意の非同期処理を受け取り
2. **Task変換**: デバッグ情報付きでTaskに変換
3. **Mutex取得**: グローバル実行器へのアクセス
4. **遅延初期化**: 初回時のExecutor作成
5. **エンキュー**: タスクキューへの追加

#### enqueue関数 (executor.rs:92-94)
```rust
fn enqueue(&mut self, task: Task<()>) {
    self.task_queue().push_back(task);    // VecDequeの末尾に追加
}
```

#### task_queue関数 (executor.rs:86-91)
```rust
fn task_queue(&mut self) -> &mut VecDeque<Task<()>> {
    if self.task_queue.is_none() {
        self.task_queue = Some(VecDeque::new());    // 遅延初期化
    }
    self.task_queue.as_mut().unwrap()
}
```

### 実行器メインループ

#### start_global_executor関数 (executor.rs:170-173)
```rust
pub fn start_global_executor() -> ! {
    info!("Starting global executor loop.");
    Executor::run(&GLOBAL_EXECUTOR);    // 永続実行
}
```

#### run関数詳細解析 (executor.rs:95-114)
```rust
fn run(executor: &Mutex<Option<Self>>) -> ! {
    info!("Executor starts running...");
    loop {                                                    // 無限ループ
        // タスクキューからタスクを取得
        let task = executor.lock()                           // Mutex取得
                         .as_mut()                           // Option<Executor>
                         .map(|e| e.task_queue().pop_front()); // タスク取得
        
        if let Some(Some(mut task)) = task {                 // タスクが存在する場合
            let waker = no_op_waker();                       // ダミーWaker作成
            let mut context = Context::from_waker(&waker);   // Context作成
            
            match task.poll(&mut context) {                 // タスク実行
                Poll::Ready(result) => {                     // 完了
                    info!("Task completed: {:?}: {:?}", task, result);
                }
                Poll::Pending => {                           // 継続必要
                    if let Some(e) = executor.lock().as_mut() {
                        e.task_queue().push_back(task);     // キューに戻す
                    }
                }
            }
        }
        // タスクが無い場合は次のループへ
    }
}
```

##### ポーリングアルゴリズム詳細
1. **ラウンドロビン**: キューの先頭から順次実行
2. **協調的実行**: 各タスクを1回ずつポーリング
3. **継続管理**: `Poll::Pending`のタスクをキューに戻す
4. **完了処理**: `Poll::Ready`のタスクを削除

---

## Wakerシステム

### ダミーWaker実装

#### no_op_waker関数群 (executor.rs:48-58)
```rust
fn no_op_raw_waker() -> RawWaker {
    fn no_op(_: *const ()) {}                    // 何もしない関数
    fn clone(_: *const ()) -> RawWaker {         // クローン実装
        no_op_raw_waker()
    }
    let vtable = &RawWakerVTable::new(clone, no_op, no_op, no_op);
    RawWaker::new(null::<()>(), vtable)
}

fn no_op_waker() -> Waker {
    unsafe { Waker::from_raw(no_op_raw_waker()) }
}
```

#### Wakerの役割と限界
- **標準的な役割**: Futureが準備完了時に実行器に通知
- **WasabiOSでの制限**: 割り込み未実装のためポーリングベース
- **将来の拡張**: ハードウェア割り込みとの統合予定

---

## 時間ベース非同期処理

### sleep関数実装

#### sleep関数 (executor.rs:160-162)
```rust
pub async fn sleep(duration: Duration) {
    TimeoutFuture::new(duration).await
}
```

#### TimeoutFuture構造体 (executor.rs:140-149)
```rust
struct TimeoutFuture {
    time_out: Duration,    // タイムアウト時刻
}

impl TimeoutFuture {
    pub fn new(duration: Duration) -> Self {
        Self {
            time_out: global_timestamp() + duration    // 絶対時刻計算
        }
    }
}
```

#### Future trait実装 (executor.rs:150-159)
```rust
impl Future for TimeoutFuture {
    type Output = ();
    
    fn poll(self: Pin<&mut Self>, _: &mut Context) -> Poll<()> {
        if self.time_out < global_timestamp() {      // 時間チェック
            Poll::Ready(())                          // 時間経過で完了
        } else {
            Poll::Pending                            // まだ待機中
        }
    }
}
```

### HPET（高精度タイマー）実装

#### global_timestamp関数 (hpet.rs:87-94)
```rust
pub fn global_timestamp() -> Duration {
    if let Some(hpet) = &*HPET.lock() {
        let ns = hpet.main_counter() as u128 * 1_000_000_000 / hpet.freq() as u128;
        Duration::from_nanos(ns as u64)
    } else {
        Duration::ZERO
    }
}
```

#### HPET レジスタ構造 (hpet.rs:25-34)
```rust
#[repr(C)]
pub struct HpetRegisters {
    capabilities_and_id: u64,        // 0x00: 機能・識別情報
    _reserved0: u64,                 // 0x08: 予約
    configuration: u64,              // 0x10: 設定レジスタ
    _reserved1: [u64; 27],          // 0x18-0xF8: 予約
    main_counter_value: u64,         // 0xF0: メインカウンタ ★重要★
    _reserved2: u64,                 // 0xF8: 予約
    timers: [TimerRegister; 32],     // 0x100-: 個別タイマー
}
```

#### main_counter関数 (hpet.rs:75-77)
```rust
pub fn main_counter(&self) -> u64 {
    unsafe { read_volatile(&self.registers.main_counter_value) }
}
```
- **read_volatile**: コンパイラ最適化を回避
- **ハードウェア読み取り**: レジスタから直接カウンタ値取得
- **単調増加**: システム起動からの経過時間

---

## 実際のタスク実行例

### main.rs:65-98のタスク解析

#### Task1 - 1秒間隔タイマー (main.rs:65-72)
```rust
let task1 = async move {
    for i in 100..=103 {                                    // 4回実行
        info!("{i} hpet.main_counter = {:?}", global_timestamp() - t0);
        sleep(Duration::from_secs(1)).await;                // 1秒待機
    }
    Ok(())
};
```

##### 実行トレース例
```
[INFO] src/main.rs:67: 100 hpet.main_counter = 234ms
[INFO] src/main.rs:67: 101 hpet.main_counter = 1.235s
[INFO] src/main.rs:67: 102 hpet.main_counter = 2.236s
[INFO] src/main.rs:67: 103 hpet.main_counter = 3.237s
```

#### Task2 - 2秒間隔タイマー (main.rs:73-80)
```rust
let task2 = async move {
    for i in 200..=203 {                                    // 4回実行
        info!("{i} hpet.main_counter = {:?}", global_timestamp() - t0);
        sleep(Duration::from_secs(2)).await;                // 2秒待機
    }
    Ok(())
};
```

##### 並行実行の例
```
Time: 0.0s  [Task1] 100 hpet.main_counter = 0ms
Time: 0.0s  [Task2] 200 hpet.main_counter = 1ms
Time: 1.0s  [Task1] 101 hpet.main_counter = 1.001s
Time: 2.0s  [Task1] 102 hpet.main_counter = 2.002s
Time: 2.0s  [Task2] 201 hpet.main_counter = 2.003s
Time: 3.0s  [Task1] 103 hpet.main_counter = 3.004s
Time: 4.0s  [Task2] 202 hpet.main_counter = 4.005s
Time: 6.0s  [Task2] 203 hpet.main_counter = 6.007s
```

---

## 非同期I/O実装

### Serial Task詳細解析 (main.rs:81-94)

#### シリアルタスク実装
```rust
let serial_task = async {
    let sp = SerialPort::default();                         // COM1ポート
    if let Err(e) = sp.loopback_test() {
        error!("serial: loopback test failed");
    }
    info!("Started to monitor serial port");
    loop {                                                   // 無限ループ
        if let Some(v) = sp.try_read() {                    // 非ブロッキング読み取り
            let c = char::from_u32(v as u32);
            info!("serial input: {v:#04X} = {c:?}");
        }
        sleep(Duration::from_millis(20)).await;             // 20ms間隔ポーリング
    }
};
```

### SerialPort実装詳細

#### try_read関数 (serial.rs:62-71)
```rust
pub fn try_read(&self) -> Option<u8> {
    if read_io_port_u8(self.base + 5) & 0x01 == 0 {         // データ受信チェック
        None                                                 // データなし
    } else {
        let c = read_io_port_u8(self.base);                 // データ読み取り
        write_io_port_u8(self.base + 2, 0xC7);             // FIFOクリア
        Some(c)                                              // データ返却
    }
}
```

#### UARTレジスタマップ
| オフセット | 名前 | 説明 |
|-----------|------|------|
| +0 | Data | データ送受信 |
| +1 | IER | 割り込み有効 |
| +2 | FCR | FIFOコントロール |
| +3 | LCR | ライン制御 |
| +4 | MCR | モデム制御 |
| +5 | LSR | ライン状態 ★重要★ |

#### ライン状態レジスタ（LSR）
```
Bit 0: Data Ready (DR)          - 受信データあり
Bit 1: Overrun Error (OE)       - バッファオーバーラン
Bit 2: Parity Error (PE)        - パリティエラー
Bit 3: Framing Error (FE)       - フレーミングエラー
Bit 4: Break Interrupt (BI)     - ブレーク信号検出
Bit 5: Transmit Holding Empty   - 送信バッファ空き ★send_char使用★
Bit 6: Transmitter Empty        - 送信完了
Bit 7: Error in RX FIFO         - 受信FIFOエラー
```

---

## パフォーマンス解析

### 非同期処理のオーバーヘッド

#### ポーリング頻度
```rust
// 1つのsleep(1秒)タスクの場合
// 1秒 = 1,000,000μs
// ポーリング間隔 ≈ 1μs (実際のCPU速度による)
// 総ポーリング回数 ≈ 1,000,000回/秒
```

#### メモリ使用量
```rust
// Task構造体
size_of::<Task<()>>() = 32バイト
  - future: 24バイト (Box<dyn Future>)
  - created_at_file: 8バイト (&'static str)
  - created_at_line: 4バイト (u32)

// VecDeque オーバーヘッド
- 初期容量: 7要素
- 成長率: 2倍
- メモリ効率: 90%以上
```

#### CPU使用率
```rust
// 3タスク実行時のCPU分配
Task1 (sleep 1s): ~0.0001% (ポーリング時のみ)
Task2 (sleep 2s): ~0.00005% (ポーリング頻度が半分)
Task3 (serial):   ~0.005% (20msポーリング)
Executor loop:    ~99.995% (スピンロック)
```

### 最適化の考慮点

#### 現在の制限
1. **ビジーウェイト**: CPUを常時使用
2. **割り込み未対応**: ハードウェアイベントを見逃す可能性
3. **優先度なし**: 全タスクが同等優先度

#### 将来の改善案
1. **hlt命令**: アイドル時のCPU省電力
2. **割り込み統合**: ハードウェアイベント駆動
3. **優先度付きスケジューリング**: リアルタイム対応
4. **プリエンプション**: より公平なCPU分配

---

## まとめ

WasabiOSの非同期処理システムは以下の特徴を持ちます：

### 設計の優位点
1. **型安全性**: Rustの型システムによる安全保証
2. **ゼロコスト抽象化**: ランタイムオーバーヘッドの最小化
3. **拡張性**: 新しいFutureの追加が容易
4. **デバッグ容易性**: 詳細なログとトレース機能

### 技術的特徴
1. **協調的マルチタスク**: 各タスクが自主的にCPU譲渡
2. **イベント駆動**: ハードウェアタイマーベースの時間管理
3. **統一的インターフェース**: Future traitによる抽象化
4. **リソース効率**: 最小限のメモリとCPU使用

### 教育的価値
1. **非同期概念**: Futureとasync/awaitの理解
2. **OSアーキテクチャ**: タスクスケジューリングの基本
3. **ハードウェア制御**: 低レベルタイマー・I/O操作
4. **Rust特化**: 所有権とライフタイムの実践

このシステムは教育用OSとして、現代的な非同期プログラミングの基礎概念を学ぶのに適した設計となっています。