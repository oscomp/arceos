#![cfg_attr(feature = "axstd", no_std)]
#![cfg_attr(feature = "axstd", no_main)]

mod loader;

use axstd::println;
use loader::load_file;

#[cfg_attr(feature = "axstd", unsafe(no_mangle))]
fn main() {
    load_file("/");
    println!("Hello, world!");
}
