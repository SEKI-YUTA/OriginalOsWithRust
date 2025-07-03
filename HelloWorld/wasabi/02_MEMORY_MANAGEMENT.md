# WasabiOS - メモリ管理詳細解説

## 目次
1. [メモリ管理概要](#メモリ管理概要)
2. [UEFIブートサービス終了](#uefiブートサービス終了)
3. [メモリマップ解析](#メモリマップ解析)
4. [ヒープアロケーター](#ヒープアロケーター)
5. [ページング初期化](#ページング初期化)
6. [仮想メモリ管理](#仮想メモリ管理)
7. [メモリ保護](#メモリ保護)

---

## メモリ管理概要

WasabiOSのメモリ管理は以下の4つの層で構成されています：

1. **UEFI層**: UEFIファームウェアが提供するメモリサービス
2. **ランタイム層**: OSが直接制御するメモリ管理
3. **ページング層**: 仮想メモリによるメモリ保護
4. **ヒープ層**: 動的メモリ割り当て

---

## UEFIブートサービス終了

### main.rs:54-56 - ランタイム移行処理
```rust
let acpi = efi_system_table.acpi_table().expect("ACPI table not found");
let memory_map = init::init_basic_runtime(image_handle, efi_system_table);
info!("Hello, Non-UEFI world!");
```

#### init_basic_runtime関数詳細解析 (init.rs:25-33)
```rust
pub fn init_basic_runtime(
    image_handle: EfiHandle,
    efi_system_table: &EfiSystemTable,
) -> MemoryMapHolder {
    let mut memory_map = MemoryMapHolder::new();
    exit_from_efi_boot_services(image_handle, efi_system_table, &mut memory_map);
    ALLOCATOR.init_with_mmap(&memory_map);
    memory_map
}
```

#### 処理の詳細解説

##### 行28: MemoryMapHolder初期化
```rust
let mut memory_map = MemoryMapHolder::new();
```
- **MemoryMapHolder構造体** (uefi.rs:98-100):
  ```rust
  pub struct MemoryMapHolder {
      memory_map_buffer: [u8; MEMORY_MAP_BUFFER_SIZE],  // 32KB バッファ
      memory_map_size: usize,                           // 実際のサイズ
      memory_map_key: usize,                            // UEFI内部キー
      descriptor_size: usize,                           // 記述子サイズ
      descriptor_version: u32,                          // 記述子バージョン
  }
  ```
- **MEMORY_MAP_BUFFER_SIZE**: 32,768バイト（0x8000）
- **目的**: UEFIメモリマップを格納するバッファ

##### 行29: UEFIブートサービス終了
```rust
exit_from_efi_boot_services(image_handle, efi_system_table, &mut memory_map);
```

**exit_from_efi_boot_services()の内部処理**:
1. **GetMemoryMap呼び出し**: 現在のメモリマップを取得
2. **ExitBootServices呼び出し**: UEFIブートサービスを終了
3. **制御移譲**: OSが直接ハードウェア制御権を獲得

**UEFIブートサービス終了の意味**:
- UEFIが提供していたメモリ管理サービスが使用不可
- UEFIタイマー・割り込みサービスが停止
- OSが全ハードウェアの制御権を獲得
- この時点以降、UEFIの関数呼び出しは危険

##### 行30: アロケーター初期化
```rust
ALLOCATOR.init_with_mmap(&memory_map);
```
- グローバルアロケーターにメモリマップを渡す
- 以降、`Box`、`Vec`、`String`などが使用可能になる

---

## メモリマップ解析

### main.rs:57 - アロケーター詳細初期化
```rust
init_allocator(&memory_map);
```

#### init_allocator関数詳細解析 (init.rs:70-81)
```rust
pub fn init_allocator(memory_map: &MemoryMapHolder) {
    let mut total_memory_pages = 0;
    for e in memory_map.iter() {
        if e.memory_type() != EfiMemoryType::CONVENTIONAL_MEMORY {
            continue;
        }
        total_memory_pages += e.number_of_pages();
        info!("{e:?}");
    }
    let total_memory_size_mib = total_memory_pages * 4096 / 1024 / 1024;
    info!("Total memory size: {total_memory_size_mib} MiB");
}
```

#### EfiMemoryType解析 (uefi.rs:54-70)
```rust
#[repr(C)]
pub enum EfiMemoryType {
    RESERVED = 0,                    // 使用不可
    LOADER_CODE,                     // ローダーコード
    LOADER_DATA,                     // ローダーデータ
    BOOT_SERVICES_CODE,              // UEFIブートサービスコード
    BOOT_SERVICES_DATA,              // UEFIブートサービスデータ
    RUNTIME_SERVICES_CODE,           // UEFIランタイムサービスコード
    RUNTIME_SERVICES_DATA,           // UEFIランタイムサービスデータ
    CONVENTIONAL_MEMORY,             // OS使用可能メモリ ★重要★
    UNUSABLE_MEMORY,                 // 故障・使用不可メモリ
    ACPI_RECLAIM_MEMORY,             // ACPI終了後に回収可能
    ACPI_MEMORY_NVS,                 // ACPI不揮発性メモリ
    MEMORY_MAPPED_IO,                // メモリマップドI/O
    MEMORY_MAPPED_IO_PORT_SPACE,     // I/Oポート空間
    PAL_CODE,                        // プロセッサ固有コード
    PERSISTENT_MEMORY,               // 不揮発性メモリ
}
```

#### EfiMemoryDescriptor解析 (uefi.rs:73-81)
```rust
#[repr(C)]
pub struct EfiMemoryDescriptor {
    memory_type: EfiMemoryType,      // メモリタイプ
    physical_start: u64,             // 物理開始アドレス
    virtual_start: u64,              // 仮想開始アドレス
    number_of_pages: u64,            // ページ数（4KBページ）
    attribute: u64,                  // メモリ属性
}
```

**メモリマップ走査の流れ**:
1. **記述子反復**: メモリマップの各記述子をチェック
2. **タイプフィルタ**: `CONVENTIONAL_MEMORY`のみを対象
3. **サイズ計算**: `number_of_pages * 4096`でバイトサイズ計算
4. **統計出力**: 総メモリサイズをMiB単位で表示

---

## ヒープアロケーター

### ALLOCATOR構造体詳細解析

#### グローバルアロケーター定義 (allocator.rs:155-158)
```rust
#[global_allocator]
pub static ALLOCATOR: FirstFitAllocator = FirstFitAllocator {
    first_header: RefCell::new(None),
};
```
- **`#[global_allocator]`**: Rustの標準メモリ割り当て処理を置き換え
- **`FirstFitAllocator`**: 最初適合法によるメモリ割り当て
- **`RefCell`**: 内部可変性（シングルスレッド環境での共有可変アクセス）

#### FirstFitAllocator実装詳細

##### init_with_mmap関数 (allocator.rs:193-200)
```rust
pub fn init_with_mmap(&self, memory_map: &MemoryMapHolder) {
    for e in memory_map.iter() {
        if e.memory_type() != EfiMemoryType::CONVENTIONAL_MEMORY {
            continue;
        }
        self.add_free_from_descriptor(e);
    }
}
```

##### add_free_from_descriptor関数 (allocator.rs:201-225)
```rust
fn add_free_from_descriptor(&self, desc: &EfiMemoryDescriptor) {
    let mut start_addr = desc.physical_start() as usize;
    let mut size = desc.number_of_pages() as usize * 4096;
    
    // アドレス0を回避（nullポインタ検出のため）
    if start_addr == 0 {
        start_addr += 4096;
        size = size.saturating_sub(4096);
    }
    if size <= 4096 {
        return;  // 小さすぎる領域は無視
    }
    
    // フリーリストヘッダーを作成
    let mut header = unsafe { Header::new_from_addr(start_addr) };
    header.next_header = None;
    header.is_allocated = false;
    header.size = size;
    
    // フリーリストの先頭に挿入
    let mut first_header = self.first_header.borrow_mut();
    let prev_last = first_header.replace(header);
    drop(first_header);
    let mut header = self.first_header.borrow_mut();
    header.as_mut().unwrap().next_header = prev_last;
}
```

#### Header構造体詳細解析 (allocator.rs:43-48)
```rust
struct Header {
    next_header: Option<Box<Header>>,    // 次のヘッダーへのポインタ
    size: usize,                         // ブロックサイズ（ヘッダー含む）
    is_allocated: bool,                  // 割り当て済みフラグ
    _reserved: usize,                    // 予約領域（アライメント用）
}
```
- **サイズ**: 32バイト（64ビット境界に整列）
- **構造**: リンクリスト形式でフリーブロックを管理
- **アライメント**: 2の冪乗サイズに強制整列

#### メモリ割り当てアルゴリズム

##### provide関数詳細解析 (allocator.rs:85-132)
```rust
fn provide(&mut self, size: usize, align: usize) -> Option<*mut u8> {
    let size = max(round_up_to_nearest_pow2(size).ok()?, HEADER_SIZE);
    let align = max(align, HEADER_SIZE);
    
    if self.is_allocated() || !self.can_provide(size, align) {
        return None;  // 既に割り当て済みまたはサイズ不足
    }
    
    // メモリレイアウト計算
    let allocated_addr = (self.end_addr() - size) & !(align - 1);
    let mut header_for_allocated = unsafe { 
        Self::new_from_addr(allocated_addr - HEADER_SIZE) 
    };
    
    // 割り当て済みヘッダーを設定
    header_for_allocated.is_allocated = true;
    header_for_allocated.size = size + HEADER_SIZE;
    header_for_allocated.next_header = self.next_header.take();
    
    // パディングが必要な場合のヘッダー作成
    if header_for_allocated.end_addr() != self.end_addr() {
        let mut header_for_padding = unsafe {
            Self::new_from_addr(header_for_allocated.end_addr())
        };
        header_for_padding.is_allocated = false;
        header_for_padding.size = self.end_addr() - header_for_allocated.end_addr();
        header_for_padding.next_header = header_for_allocated.next_header.take();
        header_for_allocated.next_header = Some(header_for_padding);
    }
    
    // 現在のブロックを縮小
    self.size -= header_for_allocated.size;
    self.next_header = Some(header_for_allocated);
    
    Some(allocated_addr as *mut u8)
}
```

**アルゴリズムの特徴**:
1. **最初適合法**: フリーリストを先頭から検索
2. **後方割り当て**: ブロックの末尾から割り当て
3. **分割**: 大きなブロックを必要サイズに分割
4. **アライメント**: 指定境界に整列
5. **フラグメンテーション管理**: ヘッダーによるブロック管理

---

## ページング初期化

### main.rs:58-61 - ページング設定
```rust
let (_gdt, _idt) = init_exceptions();
init_paging(&memory_map);
flush_tlb();
```

#### init_paging関数詳細解析 (init.rs:35-58)
```rust
pub fn init_paging(memory_map: &MemoryMapHolder) {
    let mut table = PML4::new();
    let mut end_of_mem = 0x1_0000_0000u64;  // 初期値: 4GB
    
    // 使用可能なメモリ領域の最大アドレスを計算
    for e in memory_map.iter() {
        match e.memory_type() {
            EfiMemoryType::CONVENTIONAL_MEMORY
            | EfiMemoryType::LOADER_CODE
            | EfiMemoryType::LOADER_DATA => {
                end_of_mem = max(
                    end_of_mem,
                    e.physical_start() + e.number_of_pages() * PAGE_SIZE as u64,
                );
            }
            _ => {}
        }
    }
    
    // 直接マッピング（仮想=物理）を作成
    table
        .create_mapping(0, end_of_mem, 0, PageAttr::ReadWriteKernel)
        .expect("Failed to create initial page mapping");
    
    // ページ0をマップ解除（nullポインタアクセス検出）
    table
        .create_mapping(0, 4096, 0, PageAttr::NotPresent)
        .expect("Failed to unmap page 0");
    
    // ページテーブルをCPUに設定
    unsafe { write_cr3(Box::into_raw(table)) }
}
```

#### x86-64ページテーブル構造

##### PML4構造体 (x86.rs:200-240)
```rust
#[repr(C, align(4096))]
pub struct PML4 {
    entries: [PML4Entry; 512],  // 512エントリー × 8バイト = 4KB
}

pub type PML4Entry = Entry<4, PDPT>;  // Level 4 → PDPT
pub type PDPTEntry = Entry<3, PD>;    // Level 3 → PD  
pub type PDEntry = Entry<2, PT>;      // Level 2 → PT
pub type PTEntry = Entry<1, ()>;      // Level 1 → 物理ページ
```

##### Entry構造体詳細 (x86.rs:71-74)
```rust
#[repr(transparent)]
pub struct Entry<const LEVEL: usize, NEXT> {
    value: u64,                    // エントリー値（アドレス+属性）
    next_type: PhantomData<NEXT>,  // 型安全性のマーカー
}
```

##### PageAttr属性定義 (x86.rs:55-61)
```rust
#[derive(Debug, Clone, Copy)]
#[repr(u64)]
pub enum PageAttr {
    NotPresent = 0,                                                    // 存在しない
    ReadWriteKernel = ATTR_PRESENT | ATTR_WRITABLE,                   // カーネル読み書き
    ReadWriteIo = ATTR_PRESENT | ATTR_WRITABLE | 
                  ATTR_WRITE_THROUGH | ATTR_CACHE_DISABLE,            // I/O用（キャッシュ無効）
}
```

#### ページテーブル作成プロセス

##### create_mapping関数の処理フロー
1. **アドレス分解**: 仮想アドレスを各レベルのインデックスに分解
2. **テーブル走査**: PML4 → PDPT → PD → PT の順で走査
3. **テーブル作成**: 存在しないテーブルを動的に作成
4. **マッピング設定**: 最終的に物理ページにマッピング

##### 仮想アドレス分解（64ビット）
```
63        48 47      39 38      30 29      21 20      12 11       0
|   予約   | PML4idx | PDPTidx  |  PDidx   |  PTidx   | オフセット |
```
- **PML4idx**: bits 47-39 (9ビット, 0-511)
- **PDPTidx**: bits 38-30 (9ビット, 0-511) 
- **PDidx**: bits 29-21 (9ビット, 0-511)
- **PTidx**: bits 20-12 (9ビット, 0-511)
- **オフセット**: bits 11-0 (12ビット, 0-4095)

#### write_cr3関数とTLB (x86.rs:238-242)
```rust
pub unsafe fn write_cr3(table: *mut PML4) {
    asm!("mov cr3, rax", in("rax") table);
}

pub fn flush_tlb() {
    unsafe {
        asm!("mov rax, cr3", "mov cr3, rax", out("rax") _);
    }
}
```
- **CR3レジスタ**: ページテーブルのベースアドレスを格納
- **TLB**: Translation Lookaside Buffer（アドレス変換キャッシュ）
- **flush_tlb()**: TLBの内容を無効化（新しいページテーブルを反映）

---

## 仮想メモリ管理

### ページ保護の仕組み

#### nullポインタ保護 (init.rs:55-57)
```rust
table
    .create_mapping(0, 4096, 0, PageAttr::NotPresent)
    .expect("Failed to unmap page 0");
```
- **目的**: nullポインタ参照を検出
- **仕組み**: アドレス0-4095をマップ解除
- **効果**: nullポインタアクセス時にページフォルト発生

#### 直接マッピング戦略
```rust
table
    .create_mapping(0, end_of_mem, 0, PageAttr::ReadWriteKernel)
    .expect("Failed to create initial page mapping");
```
- **マッピング**: 仮想アドレス = 物理アドレス
- **範囲**: 0 から `end_of_mem` まで
- **利点**: アドレス変換が簡単、デバッグが容易
- **制限**: セキュリティが低い（将来は分離予定）

### メモリアクセス制御

#### Entry操作関数群 (x86.rs:76-139)
```rust
impl<const LEVEL: usize, NEXT> Entry<LEVEL, NEXT> {
    fn is_present(&self) -> bool {
        (self.read_value()) & (1 << 0) != 0
    }
    
    fn is_writable(&self) -> bool {
        (self.read_value()) & (1 << 1) != 0
    }
    
    fn is_user(&self) -> bool {
        (self.read_value()) & (1 << 2) != 0
    }
    
    fn set_page(&mut self, phys: u64, attr: PageAttr) -> Result<()> {
        if phys & ATTR_MASK != 0 {
            Err("Physical address must be aligned to page size")
        } else {
            self.value = phys | attr as u64;
            Ok(())
        }
    }
}
```

#### ページフォルト処理
現在の実装では基本的なページフォルト処理のみ：
1. **Present bit = 0**: ページが存在しない
2. **Write protection**: 読み取り専用ページへの書き込み
3. **User/Supervisor**: ユーザー/カーネルモード違反

---

## メモリ保護

### セキュリティ機能

#### アドレス0保護
```rust
// アロケーターでの0アドレス回避 (allocator.rs:206-209)
if start_addr == 0 {
    start_addr += 4096;
    size = size.saturating_sub(4096);
}

// ページングでの0ページ無効化 (init.rs:55-57)
table.create_mapping(0, 4096, 0, PageAttr::NotPresent)
```

#### メモリ境界チェック
```rust
// ヒープアロケーターでの範囲チェック
fn can_provide(&self, size: usize, align: usize) -> bool {
    self.size >= size + HEADER_SIZE * 2 + align
}

// ページテーブルでのアライメントチェック  
fn set_page(&mut self, phys: u64, attr: PageAttr) -> Result<()> {
    if phys & ATTR_MASK != 0 {
        Err("Physical address must be aligned to page size")
    } else {
        self.value = phys | attr as u64;
        Ok(())
    }
}
```

#### メモリリーク検出
```rust
impl Drop for Header {
    fn drop(&mut self) {
        panic!("Header should not be dropped!");
    }
}
```
- ヘッダーが誤ってドロップされた場合にパニック
- メモリ管理の整合性を保証

### デバッグ支援

#### メモリダンプ機能
- **hexdump()**: 任意の構造体をバイト単位でダンプ
- **Debug trait**: 各構造体の詳細情報を表示
- **アロケーター統計**: 総メモリサイズの表示

#### エラーハンドリング
- **Result型**: エラーの明示的な処理
- **アサーション**: 不変条件の検証
- **パニック**: 回復不可能なエラーでのシステム停止

---

## まとめ

WasabiOSのメモリ管理システムは以下の特徴を持ちます：

### 設計思想
1. **安全性優先**: nullポインタ保護、境界チェック
2. **シンプルさ**: 直接マッピング、最初適合法
3. **デバッグ容易性**: 詳細なログ、ダンプ機能

### 技術的特徴
1. **ページベース管理**: 4KBページでの仮想メモリ
2. **動的割り当て**: リンクリスト型ヒープ管理
3. **ハードウェア統合**: x86-64 MMU機能の活用

### 制限と将来課題
1. **セキュリティ**: カーネル/ユーザー分離未実装
2. **性能**: フラグメンテーション、TLB効率
3. **機能**: スワップ、共有メモリ未対応

このメモリ管理システムは教育目的のOSとして、基本的な機能を提供しながら安全性を保証する設計となっています。