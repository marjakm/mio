//! A thread safe linked list that allows for removal

use std::{mem, ptr};

pub struct LinkedList<T> {
    head: Link<T>,
    tail: Rawlink<Node<T>>,
}

impl<T: Send + Sync> LinkedList<T> {
    pub fn new() -> LinkedList<T> {
        LinkedList {
            head: None,
            tail: Rawlink::none(),
        }
    }
}

impl<T> LinkedList<T> {
    pub fn is_empty(&self) -> bool {
        self.head.is_none()
    }

    pub fn get_mut(&mut self, entry: &Entry<T>) -> &mut T {
        self.ensure_same_ll(&entry);

        unsafe {
            // Save because &mut self has mutable access to the entire
            // LinkedList
            let n: Option<&mut Node<T>> = mem::transmute(entry.node);
            &mut n.unwrap().value
        }
    }

    pub fn push(&mut self, el: T) -> Entry<T> {
        let mut node = Box::new(Node::new(el));
        let entry = Entry::new(self, &mut node);

        match unsafe { self.tail.resolve_mut() } {
            None => self.push_front(node),
            Some(tail) => {
                tail.set_next(node);
                self.tail = Rawlink::from(&mut tail.next);
            }
        }

        entry
    }

    pub fn remove(&mut self, mut entry: Entry<T>) -> T {
        self.ensure_same_ll(&entry);

        let (mut prev, next) = unsafe {
            let node = entry.node.resolve_mut().expect("invalid entry");
            (node.prev, node.next.take())
        };

        // Unlink previous pointer
        let removed = match unsafe { (prev.resolve_mut(), next) } {
            (Some(p), Some(mut next)) => {
                next.prev = prev;
                *mem::replace(&mut p.next, Some(next)).take().unwrap()
            }
            (None, Some(mut next)) => {
                next.prev = Rawlink::none();
                *mem::replace(&mut self.head, Some(next)).take().unwrap()
            }
            (Some(p), None) => {
                self.tail = prev;
                *p.next.take().unwrap()
            }
            (None, None) => {
                self.tail = Rawlink::none();
                *self.head.take().unwrap()
            }
        };

        debug_assert!(removed.next.is_none());
        removed.value
    }

    pub fn iter(&self) -> Iter<T> {
        Iter { curr: &self.head }
    }

    fn push_front(&mut self, mut new_head: Box<Node<T>>) {
        match self.head {
            None => {
                self.head = link_no_prev(new_head);
                self.tail = Rawlink::from(&mut self.head);
            }
            Some(ref mut head) => {
                new_head.prev = Rawlink::none();
                head.prev = Rawlink::some(&mut *new_head);
                mem::swap(head, &mut new_head);
                head.next = Some(new_head);
            }
        }
    }

    fn ensure_same_ll(&self, entry: &Entry<T>) {
        assert!(entry.ll == self as *const LinkedList<T> as *mut LinkedList<T>, "entry belongs to a different LinkedList");
    }
}

unsafe impl<T> Send for LinkedList<T> {}

pub struct Entry<T> {
    ll: *mut LinkedList<T>,
    node: Rawlink<Node<T>>,
}

impl<T> Entry<T> {
    fn new(ll: &mut LinkedList<T>, node: &mut Node<T>) -> Entry<T> {
        Entry {
            ll: ll as *mut LinkedList<T>,
            node: Rawlink::some(node),
        }
    }
}

/// An iterator over references to the items of a `LinkedList`.
pub struct Iter<'a, T:'a> {
    curr: &'a Link<T>,
}

impl<'a, A> Iterator for Iter<'a, A> {
    type Item = &'a A;

    fn next(&mut self) -> Option<&'a A> {
        self.curr.as_ref().map(|curr| {
            self.curr = &curr.next;
            &curr.value
        })
    }
}

unsafe impl<T> Send for Entry<T> {}
unsafe impl<T> Sync for Entry<T> {}

type Link<T> = Option<Box<Node<T>>>;

/// Clear the .prev field on `next`, then return `Some(next)`
fn link_no_prev<T>(mut next: Box<Node<T>>) -> Link<T> {
    next.prev = Rawlink::none();
    Some(next)
}

struct Rawlink<T> {
    p: *mut T,
}

impl<T> Rawlink<T> {
    /// Like Option::None for Rawlink
    fn none() -> Rawlink<T> {
        Rawlink{p: ptr::null_mut()}
    }

    /// Like Option::Some for Rawlink
    fn some(n: &mut T) -> Rawlink<T> {
        Rawlink{p: n}
    }

    /// Convert the `Rawlink` into an Option value
    ///
    /// **unsafe** because:
    ///
    /// - Dereference of raw pointer.
    /// - Returns reference of arbitrary lifetime.
    unsafe fn resolve_mut<'a>(&mut self) -> Option<&'a mut T> {
        mem::transmute(self.p)
    }
}

impl<'a, T> From<&'a mut Link<T>> for Rawlink<Node<T>> {
    fn from(node: &'a mut Link<T>) -> Self {
        match node.as_mut() {
            None => Rawlink::none(),
            Some(ptr) => Rawlink::some(ptr),
        }
    }
}

impl<T> Copy for Rawlink<T> {}

impl<T> Clone for Rawlink<T> {
    fn clone(&self) -> Rawlink<T> {
        Rawlink { p: self.p }
    }
}

struct Node<T> {
    next: Link<T>,
    prev: Rawlink<Node<T>>,
    value: T,
}

impl<T> Node<T> {
    fn new(v: T) -> Node<T> {
        Node {
            value: v,
            next: None,
            prev: Rawlink::none(),
        }
    }

    /// Update the `prev` link on `next`, then set self's next pointer.
    ///
    /// `self.next` should be `None` when you call this
    /// (otherwise a Node is probably being dropped by mistake).
    fn set_next(&mut self, mut next: Box<Node<T>>) {
        debug_assert!(self.next.is_none());
        next.prev = Rawlink::some(self);
        self.next = Some(next);
    }
}

unsafe impl<T> Send for Node<T> {}
unsafe impl<T> Sync for Node<T> {}
