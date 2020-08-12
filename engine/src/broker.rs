/// Distribute events of type `T` among readers.
pub struct EventBroker<T> {
    pool: Vec<T>,
}

impl<T> EventBroker<T> {
    pub fn new() -> Self {
        Self::with_capacity(1024)
    }

    pub fn with_capacity(capacity: usize) -> Self {
        EventBroker {
            pool: Vec::with_capacity(capacity),
        }
    }

    pub fn read<'a>(&'a self) -> impl ExactSizeIterator<Item = &'a T> + Clone {
        self.pool.iter()
    }

    pub fn add(&mut self, event: T) {
        self.pool.push(event);
    }

    pub fn clear(&mut self) {
        self.pool.clear();
    }
}
