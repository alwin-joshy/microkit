#![no_main]
#![no_std]
#![feature(const_option)]
#![feature(const_int_from_str)]

#[macro_use]
mod util;

use core::ffi::CStr;
use core::mem::MaybeUninit;
use core::slice;
use uefi::boot::{open_protocol_exclusive, AllocateType, MemoryType, ScopedProtocol};
use uefi::proto::network::pxe::BaseCode;
use uefi::{prelude::*, Guid};

const bytes: &[u8] = include_bytes!(env!("LOADER_FILE"));
const tftp_path: &CStr = const_cstr!(env!("TFTP_PATH"));
const load_addr: u64 = match u64::from_str_radix(
    const_str::unwrap!(const_str::strip_prefix!(env!("LOAD_ADDRESS"), "0x")),
    16,
) {
    Ok(x) => x,
    Err(_) => panic!("Invalid hex string was provided for load address"),
};

#[entry]
fn main(_image_handle: Handle, system_table: SystemTable<Boot>) -> Status {
    uefi::helpers::init().unwrap();

    /* TODO: Can we do this at build time */

    /* Search for the PXE protocol */

    /* Create a buffer to store the results */
    let handle_buf_raw = system_table
        .boot_services()
        .allocate_pages(AllocateType::AnyPages, MemoryType::LOADER_DATA, 1)
        .expect("Failed to allocate buffer");

    /* Convert this to a slice so that we can use it */
    let handle_buf = unsafe {
        slice::from_raw_parts_mut(
            handle_buf_raw as *mut MaybeUninit<Handle>,
            0x1000 / core::mem::size_of::<MaybeUninit<Handle>>(),
        )
    };

    /* Find the handle for the protocol */
    let handles = uefi::boot::locate_handle(
        uefi::boot::SearchType::ByProtocol(&Guid::new(
            0x03c4e603_u32.to_le_bytes(),
            0xac28_u16.to_le_bytes(),
            0x11d3_u16.to_le_bytes(),
            0x9A_u8,
            0x2D_u8,
            [0x00, 0x90, 0x27, 0x3F, 0xC1, 0x4D],
        )),
        handle_buf,
    )
    .expect("Failed to locate handle for PXE protocol");

    /* Allocate the region that we will load the image into */
    // TODO: What if this conflicts with where UEFI has loaded us (or something else that UEFI is using?)
    // We should:
    //      1. Allocate a random region of memory with size that is big enough to store the image
    //      2. Load the image there
    //      3. Have an assembly routine at the start of the image that relocates it to where it is meant to be executing prior to jumping to main.
    let image_buf_raw = system_table.boot_services().allocate_pages(
        AllocateType::Address(load_addr),
        MemoryType::LOADER_DATA,
        (bytes.len() / 4096) + 1,
    );
    let image_slice = unsafe { slice::from_raw_parts_mut(load_addr as *mut u8, bytes.len()) };
    let mut loaded = false;

    /* If we recieve multiple handles, try all of them */
    for handle in &handles[1..] {
        /* Open the protocol */
        let mut proto: ScopedProtocol<BaseCode> =
            unwrap_or_continue!(open_protocol_exclusive(*handle));

        // TODO: this fails for some reason?
        proto.start(false);

        /* Perform DHCP */
        unwrap_or_continue!(proto.dhcp(false));

        /* Read the image into the buffer from the TFTP server */
        unwrap_or_continue!(proto.tftp_read_file(
            &uefi::proto::network::IpAddress::new_v4([172, 16, 0, 2]),
            tftp_path.try_into().unwrap(),
            Some(image_slice)
        ));

        proto.stop();

        /* The image is loaded, so we now jump to it */

        /* Exit boot services  */
        unsafe { system_table.exit_boot_services(MemoryType::LOADER_DATA) };

        /* Jump to the loader  */
        let kernel_start: unsafe extern "C" fn() = unsafe { core::mem::transmute(load_addr) };
        unsafe { (kernel_start)() };

        return Status::SUCCESS;
    }

    panic!("Was unable to load image");
}
