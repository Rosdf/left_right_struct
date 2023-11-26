use crate::reader::ReadHandleInner;
use std::cell::Cell;
use std::ops::Deref;

/// Guard class for managing reference to inner data
#[derive(Debug)]
pub struct Guard<'a, T, U: Fn() + 'a> {
    inner: &'a T,
    drop_callback: U,
}

/// # Safety
/// Pointer in `read_handel_inner` should be valid and have no writers
pub(crate) unsafe fn new_guard<'a, T>(
    read_handel_inner: &'a ReadHandleInner<T>,
    ref_counter: &'a Cell<usize>,
) -> Guard<'a, T, impl Fn() + 'a> {
    Guard {
        // SAFETY:
        // provided by caller
        inner: unsafe { read_handel_inner.load_pointer() },
        drop_callback: || {
            let refs = ref_counter.get() - 1;
            ref_counter.set(refs);
            if refs == 0 {
                read_handel_inner.increase_counter();
            }
        },
    }
}

impl<'a, T, U: Fn()> Drop for Guard<'a, T, U> {
    fn drop(&mut self) {
        (self.drop_callback)();
    }
}

impl<'a, T, U: Fn()> AsRef<T> for Guard<'a, T, U> {
    fn as_ref(&self) -> &T {
        self.inner
    }
}

impl<'a, T, U: Fn()> Deref for Guard<'a, T, U> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.inner
    }
}
