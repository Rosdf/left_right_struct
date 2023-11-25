use crate::utils::{option_ptr_compare, ArcSwapOption};
use std::process::abort;
use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicUsize, Ordering};
use triomphe::Arc;

pub(crate) struct ReadHandleInner<T> {
    reader_pointer: AtomicPtr<T>,
    epoch_counter: AtomicUsize,
    is_active: AtomicBool,
    reader_counter: Arc<AtomicUsize>,
    pub(crate) next: ArcSwapOption<ReadHandleInner<T>>,
}

impl<T> ReadHandleInner<T> {
    const MAX_REFCOUNT: usize = isize::MAX as usize;

    /// #Safety
    /// there should be no writers to pointer at creation
    pub(crate) unsafe fn new(
        reader_pointer: *mut T,
        reader_counter: Arc<AtomicUsize>,
        next: Option<Arc<Self>>,
    ) -> Self {
        Self {
            reader_pointer: AtomicPtr::new(reader_pointer),
            epoch_counter: AtomicUsize::new(0),
            is_active: AtomicBool::new(true),
            reader_counter,
            next: ArcSwapOption::new(next),
        }
    }

    pub(crate) fn increase_counter(&self) {
        self.epoch_counter.fetch_add(1, Ordering::Release);
    }

    /// # SAFETY
    /// pointer inside should be valid and have no writers.
    pub(crate) unsafe fn load_pointer(&self) -> &T {
        // SAFETY:
        // provided by caller
        unsafe { &*self.reader_pointer.load(Ordering::Acquire) }
    }

    pub(crate) fn swap_pointer(&self, new_pointer: *mut T) {
        self.reader_pointer.swap(new_pointer, Ordering::Release);
    }

    pub(crate) fn get_epoch(&self) -> usize {
        self.epoch_counter.load(Ordering::Acquire)
    }

    pub(crate) fn clone_as_ptr(&self) -> Arc<Self> {
        let mut old_next = self.next.load();
        let current_reader = ptr::null_mut();
        let old_count = self.reader_counter.fetch_add(1, Ordering::AcqRel);

        if old_count > Self::MAX_REFCOUNT {
            abort();
        }

        // SAFETY:
        // we change pointer later if it is not changed by writer
        let mut new = Arc::new(unsafe {
            Self::new(
                current_reader,
                Arc::clone(&self.reader_counter),
                old_next.clone(),
            )
        });

        while !option_ptr_compare(
            self.next
                .compare_and_swap(&old_next, Some(Arc::clone(&new)))
                .as_ref(),
            old_next.as_ref(),
        ) {
            old_next = self.next.load();
            let mut_new = Arc::get_mut(&mut new).unwrap();
            mut_new.next = ArcSwapOption::new(old_next.clone());
        }

        // change reader pointer if it has not been changed by writer
        let _ = new.reader_pointer.compare_exchange(
            current_reader,
            self.reader_pointer.load(Ordering::Acquire),
            Ordering::Release,
            Ordering::Relaxed,
        );

        new
    }

    pub(crate) fn set_inactive(&self) {
        // we publish rarely and this flag is for cleaning data, so we use Relaxed order
        self.is_active.store(false, Ordering::Relaxed);
    }

    pub(crate) fn is_active(&self) -> bool {
        // we publish rarely and this flag is for cleaning data, so we use Relaxed order
        self.is_active.load(Ordering::Relaxed)
    }
}

impl<T> Drop for ReadHandleInner<T> {
    fn drop(&mut self) {
        if self
            .reader_counter
            .compare_exchange(1, 0, Ordering::AcqRel, Ordering::Relaxed)
            .is_ok()
        {
            // SAFETY:
            // we checked that we are the only holder of reade handle
            // and no more can be created because we are holding mutable reference to self
            drop(unsafe { Box::from_raw(self.reader_pointer.load(Ordering::Acquire)) });
        }
    }
}
