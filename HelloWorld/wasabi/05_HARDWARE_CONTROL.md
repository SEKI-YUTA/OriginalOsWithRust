# WasabiOS - ハードウェア制御詳細解説

## 目次
1. [ハードウェア制御概要](#ハードウェア制御概要)
2. [ACPI (Advanced Configuration and Power Interface)](#acpi-advanced-configuration-and-power-interface)
3. [PCI (Peripheral Component Interconnect)](#pci-peripheral-component-interconnect)
4. [HPET (High Precision Event Timer)](#hpet-high-precision-event-timer)
5. [x86-64 アーキテクチャ制御](#x86-64-アーキテクチャ制御)
6. [例外・割り込み処理](#例外割り込み処理)
7. [I/O ポート制御](#ioポート制御)
8. [ハードウェア統合](#ハードウェア統合)

---

## ハードウェア制御概要

WasabiOSのハードウェア制御サブシステムは、現代的なPC/AT互換機の主要コンポーネントを直接制御します：

### システム構成図
```
┌─────────────────────────────────────────────────────────────────┐
│                        WasabiOS Kernel                         │
├─────────────────┬─────────────────┬─────────────────┬──────────┤
│      ACPI       │       PCI       │      HPET       │   x86    │
│   (Power Mgmt)  │   (Device Bus)  │    (Timer)      │  (CPU)   │
└─────────────────┴─────────────────┴─────────────────┴──────────┘
         │                 │                 │             │
    ┌────────────┐    ┌──────────────┐  ┌──────────┐  ┌──────────┐
    │ ACPI Tables│    │ PCI Devices  │  │   HPET   │  │ CPU Regs │
    │  - MCFG    │    │  - xHCI      │  │ Registers│  │  - CR3   │
    │  - HPET    │    │  - Graphics  │  │          │  │  - GDT   │
    │  - ...     │    │  - Network   │  │          │  │  - IDT   │
    └────────────┘    └──────────────┘  └──────────┘  └──────────┘
```

### 初期化シーケンス (main.rs:58-63)
```rust
let (_gdt, _idt) = init_exceptions();    // x86例外処理初期化
init_paging(&memory_map);               // 仮想メモリ初期化
flush_tlb();                            // TLB無効化
init_hpet(acpi);                        // 高精度タイマー初期化
init_pci(acpi);                         // PCIデバイス初期化
```

---

## ACPI (Advanced Configuration and Power Interface)

### ACPI システム概要

ACPI（Advanced Configuration and Power Interface）は、PC/AT互換機におけるハードウェア設定・電源管理の標準仕様です。

#### ACPI取得プロセス (main.rs:54)
```rust
let acpi = efi_system_table.acpi_table().expect("ACPI table not found");
```

### ACPI構造体詳細

#### AcpiRsdpStruct の役割
```rust
// ACPI Root System Description Pointer
// システム内のACPIテーブルの場所を示すルートポインタ
pub struct AcpiRsdpStruct {
    // RSDP → XSDT → 各種ACPIテーブル の階層構造
}
```

#### システム記述テーブルヘッダー (acpi.rs:6-22)
```rust
#[repr(packed)]
#[derive(Clone, Copy, Debug)]
struct SystemDescriptionTableHeader {
    signature: [u8; 4],      // テーブル識別子（"MCFG", "HPET"等）
    length: u32,             // テーブル全体の長さ
    _unused: [u8; 28],       // その他のフィールド（リビジョン、チェックサム等）
}
```

### XSDT (Extended System Description Table)

#### XSDT構造体 (acpi.rs:53-81)
```rust
#[repr(packed)]
struct Xsdt {
    header: SystemDescriptionTableHeader,
    // この後に可変長のテーブルポインタ配列が続く
}

impl Xsdt {
    fn num_of_entries(&self) -> usize {
        (self.header.length as usize - self.header_size()) / size_of::<*const u8>()
    }
    
    unsafe fn entry(&self, index: usize) -> *const u8 {
        ((self as *const Self as *const u8).add(self.header_size())
            as *const *const u8)
            .add(index)
            .read_unaligned()
    }
}
```

#### XSDTイテレーター (acpi.rs:24-50)
```rust
impl<'a> Iterator for XsdtIterator<'a> {
    type Item = &'static SystemDescriptionTableHeader;
    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.table.num_of_entries() {
            None
        } else {
            self.index += 1;
            Some(unsafe {
                &*(self.table.entry(self.index - 1) as *const SystemDescriptionTableHeader)
            })
        }
    }
}
```

### ACPI テーブル検索

#### AcpiTable trait (acpi.rs:83-96)
```rust
trait AcpiTable {
    const SIGNATURE: &'static [u8; 4];
    type Table;
    fn new(header: &SystemDescriptionTableHeader) -> &Self::Table {
        header.expect_signature(Self::SIGNATURE);  // シグネチャ検証
        let table: &Self::Table = unsafe {
            &*(header as *const SystemDescriptionTableHeader as *const Self::Table)
        };
        table
    }
}
```

#### テーブル検索プロセス
1. **RSDP特定**: UEFIからACPIルートポインタを取得
2. **XSDT取得**: RSDPからXSDTテーブルを取得
3. **テーブル列挙**: XSDTから各種ACPIテーブルを列挙
4. **シグネチャ一致**: 目的のテーブル（MCFG, HPET等）を検索
5. **型変換**: ヘッダーから具体的なテーブル構造体にキャスト

---

## PCI (Peripheral Component Interconnect)

### PCI初期化プロセス

#### init_pci関数 (init.rs:90-100)
```rust
pub fn init_pci(acpi: &AcpiRsdpStruct) {
    if let Some(mcfg) = acpi.mcfg() {              // MCFG テーブル取得
        for i in 0..mcfg.num_of_entries() {
            if let Some(e) = mcfg.entry(i) {
                info!("{}", e)                      // MCFG エントリ表示
            }
        }
        let pci = Pci::new(mcfg);                   // PCI ドライバー作成
        pci.probe_devices();                        // デバイス検出開始
    }
}
```

### MCFG (Memory Mapped Configuration)

#### MCFG テーブルの役割
```
MCFG（Memory Mapped Configuration）テーブル:
- PCIエクスプレスの設定領域をメモリマップドI/Oで提供
- 従来のI/Oポートベース設定に代わる高速アクセス方式
- 各PCIデバイスの設定レジスタに直接メモリアクセス可能

PCIエクスプレス設定領域:
Base Address + (Bus << 20) + (Device << 15) + (Function << 12) + Register
```

### BusDeviceFunction 構造体

#### BDF (Bus/Device/Function) 管理 (pci.rs:37-89)
```rust
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct BusDeviceFunction {
    id: u16,    // Bus(8bit) + Device(5bit) + Function(3bit)
}

impl BusDeviceFunction {
    pub fn new(bus: usize, device: usize, function: usize) -> Result<Self> {
        if !(0..256).contains(&bus) 
        || !(0..32).contains(&device) 
        || !(0..8).contains(&function) {
            return Err("PCI bus device function out of range");
        }
        Ok(Self {
            id: ((bus << 8) | (device << 3) | function) as u16,
        })
    }
    
    pub fn bus(&self) -> usize {
        ((self.id as usize) & 0xFF00) >> 8
    }
    pub fn device(&self) -> usize {
        ((self.id as usize) & 0x00F8) >> 3
    }
    pub fn function(&self) -> usize {
        (self.id as usize) & 0x0007
    }
}
```

### PCIデバイス検出

#### 全PCI空間スキャン
```rust
impl BusDeviceFunction {
    pub fn iter() -> BusDeviceFunctionIterator {
        BusDeviceFunctionIterator { next_id: 0 }
    }
}

// 使用例：全PCIデバイスをスキャン
for bdf in BusDeviceFunction::iter() {
    if let Some(vendor_device_id) = pci.read_vendor_device_id(bdf) {
        // デバイス発見時の処理
        info!("Found PCI device: {vendor_device_id} at {bdf}");
    }
}
```

#### VendorDeviceId構造体 (pci.rs:12-35)
```rust
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct VendorDeviceId {
    pub vendor: u16,    // ベンダーID（Intel: 0x8086, QEMU: 0x1b36等）
    pub device: u16,    // デバイスID（製品固有）
}
```

### PCIデバイス初期化

#### PCIドライバー接続プロセス
```rust
// 例：xHCIコントローラーの場合
if PciXhciDriver::supports(vendor_device_id) {
    PciXhciDriver::attach(&pci, bdf)?;
}

// attach関数内での処理
pub fn attach(pci: &Pci, bdf: BusDeviceFunction) -> Result<()> {
    pci.disable_interrupt(bdf)?;        // レガシー割り込み無効化
    pci.enable_bus_master(bdf)?;        // DMAアクセス許可
    let bar0 = pci.try_bar0_mem64(bdf)?; // BAR0メモリ領域取得
    // デバイス固有の初期化...
}
```

---

## HPET (High Precision Event Timer)

### HPET 概要

HPET（High Precision Event Timer）は、x86-64システムにおける高精度タイマーハードウェアです。

#### HPET初期化 (init.rs:60-68)
```rust
pub fn init_hpet(acpi: &AcpiRsdpStruct) {
    let hpet = acpi.hpet().expect("Failed to get HPET from ACPI");
    let hpet = hpet.base_address().expect("failed to get HPET base address");
    info!("HPET is at {hpet:#p}");
    let hpet = Hpet::new(hpet);
    set_global_hpet(hpet);
}
```

### HPET レジスタ構造

#### HpetRegisters構造体 (hpet.rs:25-34)
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

#### メインカウンタ読み取り (hpet.rs:75-77)
```rust
pub fn main_counter(&self) -> u64 {
    unsafe { read_volatile(&self.registers.main_counter_value) }
}
```

### HPET初期化プロセス

#### Hpet::new関数 (hpet.rs:43-66)
```rust
pub fn new(registers: &'static mut HpetRegisters) -> Self {
    // 1. 機能情報の取得
    let fs_per_count = registers.capabilities_and_id >> 32;
    let num_of_timers = ((registers.capabilities_and_id >> 8) & 0b11111) as usize + 1;
    let freq = 1_000_000_000_000_000 / fs_per_count;  // フェムト秒からHz変換
    
    let mut hpet = Self { registers, num_of_timers, freq };
    
    unsafe {
        // 2. HPET無効化
        hpet.globally_disable();
        
        // 3. 全タイマーの無効化
        for i in 0..hpet.num_of_timers {
            let timer = &mut hpet.registers.timers[i];
            let mut config = read_volatile(&timer.configuration_and_capability);
            config &= !(TIMER_CONFIG_INT_ENABLE | TIMER_CONFIG_USE_PERIODIC | (0b11111 << 9));
            timer.write_config(config);
        }
        
        // 4. メインカウンタリセット
        write_volatile(&mut hpet.registers.main_counter_value, 0);
        
        // 5. HPET有効化
        hpet.globally_enable();
    }
    hpet
}
```

### 時刻取得機能

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

#### 計算詳細
```
HPET周波数計算:
fs_per_count = capabilities_and_id[63:32]  // フェムト秒/カウント
freq = 10^15 / fs_per_count                // Hz単位の周波数

時刻計算:
counter_value = main_counter()             // カウンタ値
nanoseconds = counter_value * 10^9 / freq  // ナノ秒変換
```

---

## x86-64 アーキテクチャ制御

### CPU制御関数

#### アセンブリインライン関数 (x86.rs:14-30)
```rust
pub fn hlt() {
    unsafe { asm!("hlt") }                   // CPU停止（割り込み待機）
}

pub fn busy_loop_hint() {
    unsafe { asm!("pause") }                 // スピンループ最適化ヒント
}

pub fn read_io_port_u8(port: u16) -> u8 {
    let mut data: u8;
    unsafe { asm!("in al, dx", out("al") data, in("dx") port) }
    data
}

pub fn write_io_port_u8(port: u16, data: u8) {
    unsafe { asm!("out dx, al", in("al") data, in("dx") port) }
}
```

### ページング制御

#### CR3レジスタ操作 (x86.rs:32-38)
```rust
pub fn read_cr3() -> *mut PML4 {
    let mut cr3: *mut PML4;
    unsafe {
        asm!("mov rax, cr3", out("rax") cr3);
    }
    cr3
}

pub unsafe fn write_cr3(table: *mut PML4) {
    asm!("mov cr3, rax", in("rax") table);
}
```

#### TLB (Translation Lookaside Buffer) 制御
```rust
pub fn flush_tlb() {
    unsafe {
        asm!("mov rax, cr3", "mov cr3, rax", out("rax") _);
    }
}
```
- CR3レジスタの再書き込みによりTLBを無効化
- ページテーブル変更後の整合性保証

### ページ属性定義

#### PageAttr列挙型 (x86.rs:55-61)
```rust
#[derive(Debug, Clone, Copy)]
#[repr(u64)]
pub enum PageAttr {
    NotPresent = 0,                                  // ページ存在しない
    ReadWriteKernel = ATTR_PRESENT | ATTR_WRITABLE, // カーネル読み書き可能
    ReadWriteIo = ATTR_PRESENT | ATTR_WRITABLE |     // I/O用（キャッシュ無効）
                  ATTR_WRITE_THROUGH | ATTR_CACHE_DISABLE,
}
```

#### ページ属性ビット定義
```
x86-64 ページテーブルエントリ（64ビット）:
┌─────┬─────┬─────┬─────┬─────┬─────┬─────┬─────┐
│ 63  │ ... │ 12  │ 11  │ ... │  4  │  3  │  2  │
├─────┼─────┼─────┼─────┼─────┼─────┼─────┼─────┤
│ NX  │ ... │ PFN │ AVL │ ... │ PCD │ PWT │ U/S │
└─────┴─────┴─────┴─────┴─────┴─────┴─────┴─────┘
┌─────┬─────┐
│  1  │  0  │
├─────┼─────┤
│ R/W │  P  │
└─────┴─────┘

P (Present): 0 = ページ存在しない
R/W (Read/Write): 1 = 書き込み可能
U/S (User/Supervisor): 0 = カーネルモードのみ
PWT (Page Write Through): 1 = ライトスルーキャッシュ
PCD (Page Cache Disable): 1 = キャッシュ無効
```

### Entry構造体とページテーブル操作

#### Entry構造体 (x86.rs:71-140)
```rust
#[repr(transparent)]
pub struct Entry<const LEVEL: usize, NEXT> {
    value: u64,                      // エントリ値（アドレス+属性）
    next_type: PhantomData<NEXT>,    // 型安全性マーカー
}

impl<const LEVEL: usize, NEXT> Entry<LEVEL, NEXT> {
    fn is_present(&self) -> bool {
        (self.value) & (1 << 0) != 0         // Present ビットチェック
    }
    
    fn is_writable(&self) -> bool {
        (self.value) & (1 << 1) != 0         // Writable ビットチェック
    }
    
    fn set_page(&mut self, phys: u64, attr: PageAttr) -> Result<()> {
        if phys & ATTR_MASK != 0 {
            Err("Physical address must be aligned to page size")
        } else {
            self.value = phys | attr as u64;  // 物理アドレス + 属性設定
            Ok(())
        }
    }
}
```

---

## 例外・割り込み処理

### GDT・IDT初期化

#### init_exceptions関数 (main.rs:58)
```rust
let (_gdt, _idt) = init_exceptions();
```

### x86-64例外処理の概要

#### 例外種別
```
x86-64 例外一覧:
Vector  名前                        エラーコード  説明
0       Divide Error                No           ゼロ除算
1       Debug Exception             No           デバッグ例外
2       NMI                         No           マスクできない割り込み
3       Breakpoint                  No           ブレークポイント
4       Overflow                    No           オーバーフロー
5       BOUND Range Exceeded        No           境界チェック
6       Invalid Opcode              No           不正命令
7       Device Not Available        No           FPU使用不可
8       Double Fault                Yes          二重障害
9       Reserved                    No           予約
10      Invalid TSS                 Yes          不正TSS
11      Segment Not Present         Yes          セグメント不在
12      Stack Segment Fault         Yes          スタックセグメント障害
13      General Protection Fault    Yes          一般保護違反
14      Page Fault                  Yes          ページフォルト
15      Reserved                    No           予約
16      x87 FPU Error              No           FPU エラー
17      Alignment Check             Yes          アライメントチェック
18      Machine Check               No           マシンチェック
19      SIMD Exception              No           SIMD例外
20-31   Reserved                    No           予約
32-255  User Defined                Varies       ユーザー定義（割り込み）
```

#### ページフォルト処理
```rust
// ページフォルトが発生する条件：
// 1. Present = 0 のページへのアクセス
// 2. 書き込み専用ページへの読み取り
// 3. カーネルページへのユーザーアクセス

// CR2レジスタに障害アドレスが格納される
// エラーコードで詳細な原因を特定可能
```

---

## I/Oポート制御

### I/Oポートアクセス関数

#### シリアルポート制御例 (serial.rs:21-36)
```rust
pub fn init(&mut self) {
    write_io_port_u8(self.base + 1, 0x00);      // 割り込み無効化
    write_io_port_u8(self.base + 3, 0x80);      // DLAB有効（ボーレート設定）
    
    const BAUD_DIVISOR: u16 = 0x0001;           // 115200 bps
    write_io_port_u8(self.base, (BAUD_DIVISOR & 0xff) as u8);
    write_io_port_u8(self.base + 1, (BAUD_DIVISOR >> 8) as u8);
    
    write_io_port_u8(self.base + 3, 0x03);      // 8N1設定
    write_io_port_u8(self.base + 2, 0xC7);      // FIFO有効
    write_io_port_u8(self.base + 4, 0x0B);      // DTR/RTS設定
}
```

#### UART レジスタマップ
```
COM1ポート (0x3F8 ベース):
Offset  DLAB=0    DLAB=1    説明
+0      RBR/THR   DLL       受信/送信バッファ / 分周器下位
+1      IER       DLM       割り込み有効 / 分周器上位
+2      IIR/FCR   IIR/FCR   割り込み識別 / FIFO制御
+3      LCR       LCR       ライン制御
+4      MCR       MCR       モデム制御
+5      LSR       LSR       ライン状態
+6      MSR       MSR       モデム状態
+7      SR        SR        スクラッチレジスタ
```

---

## ハードウェア統合

### 統合アーキテクチャ

#### 初期化依存関係
```
1. ACPI初期化
   ↓
2. PCI初期化 (MCFGテーブル使用)
   ↓
3. HPET初期化 (HPETテーブル使用)
   ↓
4. PCIデバイス検出
   ↓
5. デバイスドライバー初期化
```

### MMIO (Memory Mapped I/O)

#### Mmio構造体による安全なレジスタアクセス
```rust
pub struct Mmio<T> {
    ptr: *mut T,
    _phantom: PhantomData<T>,
}

impl<T> Mmio<T> {
    pub unsafe fn from_raw(ptr: *mut T) -> Self {
        Self {
            ptr,
            _phantom: PhantomData,
        }
    }
    
    pub fn as_ref(&self) -> &T {
        unsafe { &*self.ptr }
    }
    
    pub fn as_mut(&mut self) -> &mut T {
        unsafe { &mut *self.ptr }
    }
}
```

### 割り込み管理

#### 現在の制限と将来の拡張
```rust
// 現在：ポーリングベース
loop {
    if let Some(data) = device.try_read() {
        // データ処理
    }
    yield_execution().await;
}

// 将来：割り込みベース
async fn interrupt_handler() {
    // 割り込み発生時の処理
    let data = device.read();
    process_data(data);
}
```

### キャッシュ制御

#### PCIデバイス用キャッシュ無効化
```rust
// レジスタアクセス用領域のキャッシュを無効化
bar0.disable_cache();

// ページテーブルレベルでの制御
PageAttr::ReadWriteIo = ATTR_PRESENT | ATTR_WRITABLE |
                        ATTR_WRITE_THROUGH | ATTR_CACHE_DISABLE;
```

---

## まとめ

WasabiOSのハードウェア制御システムは以下の特徴を持ちます：

### アーキテクチャの優位点
1. **標準準拠**: ACPI、PCI、UEFIなど業界標準に準拠
2. **階層化設計**: ハードウェア抽象化とデバイス固有処理の分離
3. **型安全性**: Rustによる安全なハードウェアアクセス
4. **拡張性**: 新しいハードウェアサポートの追加が容易

### 技術的特徴
1. **直接制御**: ファームウェア層を介さない高速アクセス
2. **メモリマップドI/O**: 現代的なデバイスアクセス方式
3. **精密タイミング**: HPET による高精度時刻管理
4. **効率的検出**: PCIバス全体の体系的スキャン

### 教育的価値
1. **ハードウェア理解**: PC/ATアーキテクチャの詳細な学習
2. **低レベルプログラミング**: レジスタ操作とメモリ管理
3. **システムプログラミング**: OS とハードウェアの境界
4. **現代技術**: 最新の PC ハードウェア標準

このハードウェア制御システムは、教育用OSとして現代的なPC環境での低レベルプログラミングを学ぶのに適した実装となっています。