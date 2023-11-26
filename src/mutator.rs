#[cfg(doc)]
use crate::WriteHandle;

/// Trait for providing interface to mutating data inside `WriteHandle`.
pub trait Mutator {
    /// Type that stores info about mutating object. The most simple one is Box<dyn Fn(&mut Self)>.
    type Operation;

    /// Method for mutating object by Operation. Used from `WriteHandle`.
    fn apply_operation(&mut self, operation: &Self::Operation);

    /// Method for mutating `operations_log` if something is known about `operation` (for example if it is enum). Used from `WriteHandle`.
    fn mutate_log(operation: &Self::Operation, operations_log: &mut Vec<Self::Operation>);

    /// Method to apply methods `apply_operation` and `mutate_log`. Change it we caution. Used from `WriteHandle`.
    fn mutate(&mut self, operation: &Self::Operation, operations_log: &mut Vec<Self::Operation>) {
        self.apply_operation(operation);
        Self::mutate_log(operation, operations_log);
    }
}
