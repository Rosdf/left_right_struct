use crate::reader::guard::{new_guard, Guard};
use crate::reader::read_handle_inner::ReadHandleInner;
use std::cell::Cell;
use std::marker::PhantomData;
use std::process::abort;
use triomphe::Arc;

/// Provides interface for reading inner data.
#[derive(Debug)]
pub struct ReadHandle<T> {
    inner: Arc<ReadHandleInner<T>>,
    ref_counter: Cell<usize>,

    // `ReadHandle` is _only_ Send if T is Sync. If T is !Sync, then it's not okay for us to expose
    // references to it to other threads! Since negative impls are not available on stable, we pull
    // this little hack to make the type not auto-impl Send, and then explicitly add the impl when
    // appropriate.
    _unimpl_send: PhantomData<*const T>,
}

// SAFETY:
// T implements Sync, so it is safe to share reference between threads
unsafe impl<T> Send for ReadHandle<T> where T: Sync {}

impl<T> Clone for ReadHandle<T> {
    fn clone(&self) -> Self {
        let inner = self.inner.clone_as_ptr();

        Self {
            inner,
            ref_counter: Cell::new(0),
            _unimpl_send: PhantomData,
        }
    }
}

impl<T> Drop for ReadHandle<T> {
    fn drop(&mut self) {
        let inner = self.inner.as_ref();
        inner.set_inactive();
    }
}

impl<T> ReadHandle<T> {
    const MAX_REFCOUNT: usize = isize::MAX as usize;

    pub(crate) fn new(inner: Arc<ReadHandleInner<T>>) -> Self {
        Self {
            inner,
            ref_counter: Cell::new(0),
            _unimpl_send: PhantomData,
        }
    }

    /// Returns guard for accessing inner data.
    pub fn reference(&self) -> Guard<T, impl Fn() + '_> {
        let refs = self.ref_counter.get();

        if refs > Self::MAX_REFCOUNT {
            abort();
        }

        if refs == 0 {
            self.inner.as_ref().increase_counter();
        }

        self.ref_counter.set(refs + 1);

        let inner = self.inner.as_ref();
        // SAFETY:
        // we already increased epoch counter, so writer can not access this pointer
        unsafe { new_guard(inner, &self.ref_counter) }
    }
}
