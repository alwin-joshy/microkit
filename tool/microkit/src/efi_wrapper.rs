#![no_main]
#![no_std]

use log::info;
use uefi::{boot::{AllocateType, MemoryType}, mem::memory_map::MemoryMap, prelude::*, system};
use core::{arch::asm, slice};

const bytes: &[u8] = include_bytes!(env!("LOADER_FILE"));
const load_addr_str: &str = env!("LOADER_ADDRESS");

#[entry]
fn main(_image_handle: Handle, system_table: SystemTable<Boot>) -> Status {
    uefi::helpers::init().unwrap();
    // let load_addr = load_addr_str.parse::<u64>().expect("Load address is not of correct format\n");
    let load_addr: u64 = u64::from_str_radix(load_addr_str.strip_prefix("0x").unwrap(), 16).expect("Load address was not a valid hex string");
    /* Allocate the region that we will load the image into */
    // TODO: What if this conflicts with where UEFI has loaded us (or something else that UEFI is using?)
    // We should:
    //      1. Allocate a random region of memory with size that is big enough to store the image
    //      2. Load the image there
    //      3. Have an assembly routine at the start of the image that relocates it to where it is meant to be executing prior to jumping to main.
    let allocate_res = system_table.boot_services().allocate_pages(AllocateType::Address(load_addr), MemoryType::LOADER_DATA, (bytes.len() / 4096) + 1).expect("Failed to alloc");

    /* Define the region as a slice so that we can access it */
    let load_region = unsafe {slice::from_raw_parts_mut(load_addr as *mut u8, bytes.len())};

    /* Exit boot services  */
    unsafe{system_table.exit_boot_services(MemoryType::LOADER_DATA)};

    /* Copy the image into the correct memory region */
    load_region.copy_from_slice(&bytes);

    /* Jump to the loader  */
    let kernel_start: unsafe extern "C" fn() = unsafe { core::mem::transmute(load_addr)};
    unsafe { (kernel_start)() };

    return Status::SUCCESS
}
