#![no_std]
#![no_main]
#![feature(offset_of)]

use core::fmt::Write;
use core::panic::PanicInfo;
use core::time::Duration;
use core::writeln;
use wasabi::executor::Executor;
use wasabi::executor::Task;
use wasabi::executor::TimeoutFuture;
use wasabi::executor::block_on;
use wasabi::executor::yield_execution;
use wasabi::graphics::BitmapTextWriter;
use wasabi::graphics::draw_test_pattern;
use wasabi::graphics::fill_rect;
use wasabi::graphics::Bitmap;
use wasabi::hpet::Hpet;
use wasabi::hpet::global_timestamp;
use wasabi::hpet::set_global_hpet;
use wasabi::init;
use wasabi::init::init_allocator;
use wasabi::init::init_basic_runtime;
use wasabi::init::init_display;
use wasabi::init::init_hpet;
use wasabi::init::init_paging;
use wasabi::print::hexdump;

use wasabi::print::set_global_vram;
use wasabi::qemu::exit_qemu;
use wasabi::qemu::QemuExitCode;
use wasabi::uefi::init_vram;
use wasabi::uefi::EfiHandle;
use wasabi::uefi::EfiMemoryType;
use wasabi::uefi::EfiSystemTable;
use wasabi::uefi::locate_loaded_image_protocol;
use wasabi::x86::PageAttr;
use wasabi::x86::flush_tlb;
use wasabi::x86::hlt;
use wasabi::println;
use wasabi::warn;
use wasabi::info;
use wasabi::error;
use wasabi::x86::init_exceptions;
use wasabi::x86::read_cr3;
use wasabi::x86::trigger_debug_interrupt;

static mut GLOBAL_HPET: Option<Hpet> = None;

#[no_mangle]
fn efi_main(image_handle: EfiHandle, efi_system_table: &EfiSystemTable) {
    println!("Booting WasabiOS...");
    println!("image_handle: {:#018X}", image_handle);
    println!("efi_system_table: {:#p}", efi_system_table);
    let loaded_image_protocol = locate_loaded_image_protocol(image_handle, efi_system_table)
    .expect("Failed to get LoadedImageProtocol");
    println!("image_base: {:#018X}", loaded_image_protocol.image_base);
    println!("image_size: {:#018X}", loaded_image_protocol.image_size);
    info!("info");
    warn!("warn");
    error!("error");
    hexdump(efi_system_table);
    let mut vram = init_vram(efi_system_table).expect("init_vram failed");
    draw_test_pattern(&mut vram);
    init_display(&mut vram);
    set_global_vram(vram);
    let acpi = efi_system_table.acpi_table().expect("ACPI table not found");
    let memory_map = init::init_basic_runtime(image_handle, efi_system_table);
    info!("Hello, Non-UEFI world!");
    init_allocator(&memory_map);
    let (_gdt, _idt) = init_exceptions();
    init_paging(&memory_map);

    flush_tlb();
    init_hpet(acpi);
    let t0 = global_timestamp();
    let task1 = Task::new(async move {
        for i in 100..=103 {
            info!("{i} hpet.main_counter = {:?}", global_timestamp() - t0);
            // yield_execution().await;
            TimeoutFuture::new(Duration::from_millis(1000)).await;
        }
        Ok(())
    });
    let task2 = Task::new(async move {
        for i in 200..=203 {
            info!("{i} hpet.main_counter = {:?}", global_timestamp() - t0);
            // yield_execution().await;
            TimeoutFuture::new(Duration::from_millis(2000)).await;
        }
        Ok(())
    });
    let mut executor = Executor::new();
    executor.enqueue(task1);
    executor.enqueue(task2);
    Executor::run(executor)
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    error!("PANIC: {info:?}");
    exit_qemu(QemuExitCode::Fail);
}
