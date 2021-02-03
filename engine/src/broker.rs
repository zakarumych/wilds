use std::ops::Deref;

pub struct Ref<'a, T> {
    entry: &'a mut Option<T>,
}

impl<T> Deref for Ref<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        self.entry.as_ref().unwrap()
    }
}

impl<T> Ref<'_, T> {
    pub fn consume(mut self) -> T {
        self.entry.take().unwrap()
    }
}

pub struct Iter<'a, T> {
    entries: std::slice::IterMut<'a, Option<T>>,
}

impl<'a, T> Iterator for Iter<'a, T> {
    type Item = Ref<'a, T>;

    fn next(&mut self) -> Option<Ref<'a, T>> {
        loop {
            let entry = self.entries.next()?;
            if entry.is_some() {
                return Some(Ref { entry });
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, self.entries.size_hint().1)
    }
}

/// Distributes events of type `T` among readers.
pub struct EventBroker<T> {
    pool: Vec<Option<T>>,
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

    pub fn read(&mut self) -> Iter<'_, T> {
        Iter {
            entries: self.pool.iter_mut(),
        }
    }

    pub fn add(&mut self, event: T) {
        self.pool.push(Some(event));
    }

    pub fn clear(&mut self) {
        self.pool.clear();
    }
}
