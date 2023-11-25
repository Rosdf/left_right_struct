use arc_swap::ArcSwapAny;
use triomphe::Arc;

pub(crate) type ArcSwapOption<T> = ArcSwapAny<Option<Arc<T>>>;

pub(crate) fn option_ptr_compare<T>(first: Option<&Arc<T>>, second: Option<&Arc<T>>) -> bool {
    match (first, second) {
        (Some(f), Some(s)) => Arc::as_ptr(f) == Arc::as_ptr(s),
        (Some(_), None) | (None, Some(_)) => false,
        (None, None) => true,
    }
}
