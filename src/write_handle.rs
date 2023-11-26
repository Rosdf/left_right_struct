use crate::mutator::Mutator;
use crate::reader::ReadHandleInner;
use crate::utils::option_ptr_compare;
use std::collections::HashMap;
use std::ptr::NonNull;
use std::{hint, mem};
use triomphe::Arc;

/// Handle for mutating inner data.
#[derive(Debug)]
pub struct WriteHandle<T: Mutator> {
    writer_pointer: NonNull<T>,
    reader_pointer: NonNull<T>,
    operations_log: Vec<T::Operation>,
    read_handle_inner: Option<Arc<ReadHandleInner<T>>>,
}

impl<T: Mutator> WriteHandle<T> {
    /// #SAFETY
    /// pointers should point to different objects in same state
    pub(crate) unsafe fn new(
        writer_pointer: *mut T,
        reader_pointer: *mut T,
        read_handle_inner: Option<Arc<ReadHandleInner<T>>>,
    ) -> Self {
        Self {
            // SAFETY:
            // provided by caller
            writer_pointer: unsafe { NonNull::new_unchecked(writer_pointer) },
            // SAFETY:
            // provided by caller
            reader_pointer: unsafe { NonNull::new_unchecked(reader_pointer) },
            operations_log: Vec::new(),
            read_handle_inner,
        }
    }

    /// Method for mutating inner value.
    pub fn mutate(&mut self, operation: T::Operation) {
        // SAFETY:
        // only we have access to this pointer so it is safe to write to it
        let data = unsafe { self.writer_pointer.as_mut() };
        data.mutate(&operation, &mut self.operations_log);
        self.operations_log.push(operation);
    }

    /// Method for publishing updates to read handles. It is quite heavy on atomic operations, and might block
    /// for some time, if there are active reads.
    pub fn publish(&mut self) {
        if self.read_handle_inner.is_none() {
            return;
        }

        if self.operations_log.is_empty() {
            return;
        }

        self.remove_first_dead_readers();
        self.update_reader_pointers();

        let epoch_counters = self.get_epoch_counters();
        self.wait_epoch_counters(&epoch_counters);

        mem::swap(&mut self.reader_pointer, &mut self.writer_pointer);

        // SAFETY:
        // we swapped all reader pointers so we the only holder of this pointer and can write to it
        let writer = unsafe { self.writer_pointer.as_mut() };

        let operations = mem::take(&mut self.operations_log);

        for operation in operations {
            writer.apply_operation(&operation);
        }
    }

    fn clone_read_handle(&self) -> Option<Arc<ReadHandleInner<T>>> {
        self.read_handle_inner.as_ref().map(Arc::clone)
    }

    fn update_reader_pointers(&self) {
        let mut reader_option = self.clone_read_handle();

        while let Some(reader) = reader_option {
            reader.swap_pointer(self.writer_pointer.as_ptr());

            reader_option = reader.next.load_full();
        }
    }

    fn get_epoch_counters(&self) -> HashMap<*const ReadHandleInner<T>, usize> {
        let mut res = HashMap::new();

        let mut reader_ptr = self.clone_read_handle();

        while let Some(reader) = reader_ptr {
            let epoch_counter = reader.get_epoch();

            if epoch_counter % 2 != 0 {
                res.insert(Arc::as_ptr(&reader), epoch_counter);
            }
            reader_ptr = reader.next.load_full();
        }

        res
    }

    fn wait_epoch_counters(&mut self, epoch_counters: &HashMap<*const ReadHandleInner<T>, usize>) {
        let mut reader_ptr = self.clone_read_handle();

        while let Some(reader) = reader_ptr {
            // remove dead handles
            {
                let mut next_reader_ptr = reader.next.load();

                while let Some(next_reader) = next_reader_ptr.as_ref() {
                    if next_reader.is_active() {
                        break;
                    }
                    let swapped = reader
                        .next
                        .compare_and_swap(next_reader, next_reader.next.load_full());

                    if option_ptr_compare(swapped.as_ref(), Some(next_reader)) {
                        next_reader_ptr = reader.next.load();
                    } else {
                        break;
                    }
                }
            }

            epoch_counters
                .get(&Arc::as_ptr(&reader))
                .map_or((), |epoch_counter| {
                    while reader.get_epoch() == *epoch_counter {
                        hint::spin_loop();
                    }
                });

            reader_ptr = reader.next.load_full();
        }
    }

    fn remove_first_dead_readers(&mut self) {
        let mut current_reader_ptr = self.clone_read_handle();

        while let Some(current_reader) = current_reader_ptr {
            if current_reader.is_active() {
                current_reader_ptr = Some(current_reader);
                break;
            }

            current_reader_ptr = current_reader.next.load_full();
        }

        self.read_handle_inner = current_reader_ptr;
    }
}

impl<T: Mutator> Drop for WriteHandle<T> {
    fn drop(&mut self) {
        // SAFETY:
        // only we have access to this pointer so it is safe to drop it
        drop(unsafe { Box::from_raw(self.writer_pointer.as_ptr()) });
    }
}

impl<T: Mutator> AsRef<T> for WriteHandle<T> {
    fn as_ref(&self) -> &T {
        // SAFETY:
        // only we have access to this pointer and we can not initiate publish
        // because it requires mut reference
        unsafe { self.writer_pointer.as_ref() }
    }
}
