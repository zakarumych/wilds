use {
    bumpalo::{boxed::Box as BBox, Bump},
    std::cell::{Cell, UnsafeCell},
};

pub struct BumpaloCellList<'a, T> {
    bump: &'a Bump,
    root: Cell<Node<'a, T>>,
}

enum Node<'a, T> {
    Nil,
    Cons(BBox<'a, Cons<'a, T>>),
}

struct Cons<'a, T> {
    value: UnsafeCell<T>,
    next: Node<'a, T>,
}

impl<'a, T> BumpaloCellList<'a, T> {
    pub const fn new_in(bump: &'a Bump) -> Self {
        BumpaloCellList {
            bump,
            root: Cell::new(Node::Nil),
        }
    }

    pub fn push(&self, value: T) -> &mut T {
        let mut node = BBox::new_in(
            Cons {
                value: value.into(),
                next: Node::Nil,
            },
            self.bump,
        );

        let value = unsafe {
            // Won't be accessed until `Drop`.
            // `&mut self` in `Drop::drop` guarantees that this reference is dropped.
            &mut *node.value.get()
        };

        // Extracted root cannot be dropped before integration back into list
        // as `Cell::replace` never panics.
        node.next = self.root.replace(Node::Nil);
        self.root.replace(Node::Cons(node));
        value
    }
}
