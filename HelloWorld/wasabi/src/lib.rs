#![no_std]
#![feature(offset_of)]
#![feature(custom_test_frameworks)]
#![feature(sync_unsafe_cell)]
#![feature(const_caller_location)]
#![feature(const_location_fields)]
#![feature(option_get_or_insert_default)]
#![test_runner(crate::test_runner::test_runner)]
#![reexport_test_harness_main = "run_unit_tests"]
#![no_main]
pub mod allocator;
pub mod executor;
pub mod graphics;
pub mod qemu;
pub mod result;
pub mod serial;
pub mod uefi;
pub mod x86;
pub mod init;
pub mod print;
pub mod acpi;
pub mod hpet;
pub mod mutex;
pub mod pci;
pub mod xhci;
pub mod bits;
pub mod mmio;
pub mod volatile;
pub mod pin;
pub mod slice;

#[cfg(test)]
pub mod test_runner;

#[cfg(test)]
#[no_mangle]
fn efi_main(
    image_handle: uefi::EfiHandle,
    efi_system_table: &uefi::EfiSystemTable,
) {
    init::init_basic_runtime(image_handle, efi_system_table);
    run_unit_tests();
}
