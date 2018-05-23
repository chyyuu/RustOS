#![feature(alloc)]
#![feature(const_fn)]
#![cfg_attr(feature = "ucore", feature(allocator_api, global_allocator, lang_items))]
#![no_std]

#[cfg(test)]
#[macro_use]
extern crate std;
extern crate spin;
#[macro_use]
extern crate alloc;
extern crate bit_set;
#[cfg(feature = "ucore")]
#[macro_use]
extern crate bitflags;
#[macro_use]
extern crate static_assertions;

#[cfg(not(test))]
macro_rules! eprintln {
    () => ();
    ($fmt:expr) => ();
    ($fmt:expr, $($arg:tt)*) => ();
}

mod dirty;
mod util;
mod blocked_device;
mod vfs;
mod sfs;
mod structs;
#[cfg(feature = "ucore")]
pub mod c_interface;
#[cfg(test)]
mod tests;

pub use sfs::*;
pub use vfs::*;
pub use blocked_device::BlockedDevice;

#[cfg(feature = "ucore")]
#[global_allocator]
pub static UCORE_ALLOCATOR: c_interface::UcoreAllocator = c_interface::UcoreAllocator{};