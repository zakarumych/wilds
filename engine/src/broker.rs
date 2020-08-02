use std::{collections::VecDeque, convert::TryFrom as _};

struct Event<T> {
    value: T,
    unread: usize,
}

/// Distribute events of type `T` among readers.
pub struct EventBroker<T> {
    offset: u64,
    queue: VecDeque<Event<T>>,
    readers: usize,
}

pub struct EventReader {
    offset: u64,
}

impl<T> EventBroker<T> {
    pub fn new() -> Self {
        Self::with_capacity(1024)
    }

    pub fn with_capacity(capacity: usize) -> Self {
        EventBroker {
            offset: 0,
            queue: VecDeque::with_capacity(capacity),
            readers: 0,
        }
    }

    pub fn subscribe(&mut self) -> EventReader {
        self.readers += 1;
        EventReader {
            offset: u64::try_from(self.queue.len())
                .ok()
                .and_then(|l| l.checked_add(self.offset))
                .expect("Too many events"),
        }
    }

    pub fn unsubscribe(&mut self, reader: EventReader) {
        drop(reader);

        while let Some(event) = self.queue.pop_front() {
            if event.unread > 1 {
                self.queue.push_front(event);
                break;
            }
        }

        for event in &mut self.queue {
            event.unread -= 1;
        }
    }

    pub fn read<'a>(
        &'a mut self,
        reader: &'a mut EventReader,
    ) -> EventReaderIter<'a, T> {
        if self.offset > reader.offset {
            reader.offset = self.offset;
        }

        EventReaderIter {
            broker: self,
            reader,
        }
    }

    pub fn add(&mut self, event: T) {
        if self.queue.len() == self.queue.capacity() {
            self.queue.pop_front();
            self.offset += 1;
            tracing::warn!("Unread event dropped");
        }

        self.queue.push_back(Event {
            value: event,
            unread: self.readers,
        });
    }
}

pub struct EventReaderIter<'a, T> {
    broker: &'a mut EventBroker<T>,
    reader: &'a mut EventReader,
}

impl<'a, T> Iterator for EventReaderIter<'a, T>
where
    T: Clone,
{
    type Item = T;

    fn next(&mut self) -> Option<T> {
        let offset = self.reader.offset - self.broker.offset;
        let offset = usize::try_from(offset).expect("Too large offset");
        if self.broker.queue.len() > offset {
            let event = &mut self.broker.queue[offset];
            self.reader.offset += 1;
            let value = event.value.clone();
            event.unread -= 1;
            if offset == 0 && event.unread == 0 {
                self.broker.queue.pop_front();
                self.broker.offset += 1;
            }
            Some(value)
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len();
        (len, Some(len))
    }
}

impl<'a, T> std::iter::ExactSizeIterator for EventReaderIter<'a, T>
where
    T: Clone,
{
    fn len(&self) -> usize {
        let offset = self.reader.offset - self.broker.offset;
        let offset = usize::try_from(offset).expect("Too large offset");
        assert!(self.broker.queue.len() >= offset);
        self.broker.queue.len() - offset
    }
}
