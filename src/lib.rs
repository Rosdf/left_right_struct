#![warn(
    clippy::pedantic,
    clippy::nursery,
    clippy::cargo,
    clippy::clone_on_ref_ptr,
    clippy::dbg_macro,
    clippy::exhaustive_enums,
    clippy::undocumented_unsafe_blocks
)]
#![allow(clippy::redundant_pub_crate, clippy::must_use_candidate)]
#![deny(missing_debug_implementations, missing_docs)]

mod mutator;
mod reader;
mod utils;
mod write_handle;

use std::sync::atomic::AtomicUsize;
use triomphe::Arc;

pub use crate::mutator::Mutator;
pub use crate::reader::ReadHandle;
use crate::reader::ReadHandleInner;
pub use crate::write_handle::WriteHandle;

pub fn create_handles<T: Clone + Mutator>(value: T) -> (ReadHandle<T>, WriteHandle<T>) {
    let read_ptr = Box::into_raw(Box::new(value.clone()));
    let write_ptr = Box::into_raw(Box::new(value));

    let reader_counter = Arc::new(AtomicUsize::new(1));

    // SAFETY:
    // there is no writer at creation, so it is safe to pass pointer
    let reader_inner = Arc::new(unsafe { ReadHandleInner::new(read_ptr, reader_counter, None) });

    let reader = ReadHandle::new(Arc::clone(&reader_inner));

    // SAFETY:
    // pointers point to objects in same state due to Clone
    let writer = unsafe { WriteHandle::new(write_ptr, read_ptr, Some(reader_inner)) };

    (reader, writer)
}
