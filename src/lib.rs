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

//! # `LeftRightStruct`
//!
//! `left_right_struct` is a library for lockfree concurrent reads and writes
//! in situations with frequent reads and rare writes.
//! It is mostly inspired by [left_right](https://crates.io/crates/left-right).
//!
//! ## Example
//!
//! Firstly you should implement `Mutator` trait on the struct you wish to use. The simplest way to do it is so
//! ```rust
//! use left_right_struct::{create_handles_from_clone, Mutator};
//!
//! #[derive(Clone, PartialEq, Eq, Debug, Default)]
//! struct MyStruct(String);
//!
//! impl Mutator for MyStruct {
//! type Operation = Box<dyn Fn(&mut Self)>;
//!
//! fn apply_operation(&mut self, operation: &Self::Operation) {
//!         (*operation)(self);
//!     }
//!
//! fn mutate_log(operation: &Self::Operation, operations_log: &mut Vec<Self::Operation>) {}
//! }
//! ```
//!
//! Or if you just want to implement it with Operation = Box<dyn Fn(&mut Self)>
//!
//! ```rust
//! use left_right_struct::{create_handles_from_clone, impl_simple_mutator};
//!
//! #[derive(Clone, PartialEq, Eq, Debug, Default)]
//! struct MyStruct(String);
//!
//! impl_simple_mutator!(MyStruct);
//! ```
//!
//! Than you can use it this way.
//!
//! ```rust
//! use std::thread;
//! use left_right_struct::{create_handles_from_clone, create_handles_from_default, impl_simple_mutator};
//!
//! #[derive(Clone, PartialEq, Eq, Debug, Default)]
//! struct MyStruct(String);
//!
//! impl_simple_mutator!(MyStruct);
//!
//! let (rh, mut wh) = create_handles_from_default::<MyStruct>();
//!
//! thread::spawn(move || {
//!     let guard = rh.reference();
//!     let ref_str = guard.0.as_str();
//!     println!("{}", ref_str);
//!     // inner str will be "" if we got reference before wh.publish()
//!     // or "1" if after
//!     assert!(ref_str == "" || ref_str == "1");
//! });
//!
//! wh.mutate(Box::new(|s| s.0.push('1')));
//!
//! // in WriteHandle we see changes immediately
//! assert_eq!(wh.as_ref().0.as_str(), "1");
//! wh.publish();
//! ```

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

/// # Safety
///
/// pointers should point to different objects in identical state
unsafe fn create_handles_from_raw<T: Mutator>(
    read_ptr: *mut T,
    write_ptr: *mut T,
) -> (ReadHandle<T>, WriteHandle<T>) {
    let reader_counter = Arc::new(AtomicUsize::new(1));

    // SAFETY:
    // there is no writer at creation, so it is safe to pass pointer
    let reader_inner = Arc::new(unsafe { ReadHandleInner::new(read_ptr, reader_counter, None) });

    let reader = ReadHandle::new(Arc::clone(&reader_inner));

    // SAFETY:
    // provided by caller
    let writer = unsafe { WriteHandle::new(write_ptr, read_ptr, Some(reader_inner)) };

    (reader, writer)
}

/// Creates read and write handles from value by cloning. Clone should create identical objects
pub fn create_handles_from_clone<T: Clone + Mutator>(value: T) -> (ReadHandle<T>, WriteHandle<T>) {
    let read_ptr = Box::into_raw(Box::new(value.clone()));
    let write_ptr = Box::into_raw(Box::new(value));

    // SAFETY:
    // pointers point to different objects in identical state due to Clone trait
    unsafe { create_handles_from_raw(read_ptr, write_ptr) }
}

/// Creates read and write handles from default values. Default should create identical objects
pub fn create_handles_from_default<T: Default + Mutator>() -> (ReadHandle<T>, WriteHandle<T>) {
    let read_ptr = Box::into_raw(Box::default());
    let write_ptr = Box::into_raw(Box::default());

    // SAFETY:
    // pointers point to different objects in identical state due to Default trait
    unsafe { create_handles_from_raw(read_ptr, write_ptr) }
}

/// Creates simplest implementation for `Mutator` trait, where Operation is Box<dyn Fn(&mut Self)>
#[macro_export(local_inner_macros)]
macro_rules! impl_simple_mutator {
    ($TypeName:ty) => {
        impl $crate::Mutator for $TypeName {
            type Operation = Box<dyn Fn(&mut Self)>;

            fn apply_operation(&mut self, operation: &Self::Operation) {
                (*operation)(self);
            }

            fn mutate_log(_: &Self::Operation, _: &mut Vec<Self::Operation>) {}
        }
    };
}

#[cfg(test)]
mod test {
    use crate::create_handles_from_default;

    impl_simple_mutator!(String);

    #[test]
    fn basic_test() {
        let (rh, mut wh) = create_handles_from_default::<String>();

        assert_eq!(rh.reference().as_str(), "");

        wh.mutate(Box::new(|s| s.push('1')));

        assert_eq!(wh.as_ref().as_str(), "1");
        assert_eq!(rh.reference().as_str(), "");

        wh.publish();

        assert_eq!(rh.reference().as_str(), "1");
    }

    #[test]
    fn many_handles() {
        let (rh1, mut wh) = create_handles_from_default::<String>();
        #[allow(clippy::redundant_clone)]
        let rh2 = rh1.clone();
        #[allow(clippy::redundant_clone)]
        let rh3 = rh1.clone();

        wh.mutate(Box::new(|s| s.push('1')));

        assert_eq!(rh1.reference().as_str(), "");
        assert_eq!(rh2.reference().as_str(), "");
        assert_eq!(rh3.reference().as_str(), "");

        wh.publish();

        assert_eq!(rh1.reference().as_str(), "1");
        assert_eq!(rh2.reference().as_str(), "1");
        assert_eq!(rh3.reference().as_str(), "1");
    }

    #[test]
    fn many_handles_and_drop() {
        let (rh1, mut wh) = create_handles_from_default::<String>();
        #[allow(clippy::redundant_clone)]
        let rh2 = rh1.clone();
        #[allow(clippy::redundant_clone)]
        let rh3 = rh1.clone();

        wh.mutate(Box::new(|s| s.push('1')));

        assert_eq!(rh1.reference().as_str(), "");
        assert_eq!(rh2.reference().as_str(), "");
        assert_eq!(rh3.reference().as_str(), "");

        drop(rh1);
        drop(rh3);

        wh.publish();

        assert_eq!(rh2.reference().as_str(), "1");
    }
}
