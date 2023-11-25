pub trait Mutator {
    type Operation;

    fn apply_operation(&mut self, operation: &Self::Operation);
    fn mutate_log(&self, operation: &Self::Operation, operations_log: &mut Vec<Self::Operation>);

    fn mutate(&mut self, operation: &Self::Operation, operations_log: &mut Vec<Self::Operation>) {
        self.apply_operation(operation);
        self.mutate_log(operation, operations_log);
    }
}
