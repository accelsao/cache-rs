use std::ptr::NonNull;

pub enum CacheRegion {
    Window,
    MainProbation,
    MainProtected,
}

pub struct LinkedList<T> {
    head: Option<NonNull<Node<T>>>,
    tail: Option<NonNull<Node<T>>>,
    len: usize,
    // marker: PhantomData<Box<Node<T>>>,
}

struct Node<T> {
    elem: T,
    next: Option<NonNull<Node<T>>>,
    prev: Option<NonNull<Node<T>>>,
}

impl<T> Node<T> {
    fn new(elem: T) -> Self {
        Self {
            elem,
            next: None,
            prev: None,
        }
    }
    fn into_elem(self: Box<Self>) -> T {
        self.elem
    }
}

// private methods
impl<T> LinkedList<T> {
    #[inline]
    fn push_front_node(&mut self, mut node: Box<Node<T>>) {
        node.next = self.head;
        node.prev = None;
        let node = Some(Box::leak(node).into());
        unsafe {
            match self.head {
                None => self.tail = node,
                Some(head) => (*head.as_ptr()).prev = node,
            }
        }
        self.len += 1;
        self.head = node;
    }

    #[inline]
    fn pop_front_node(&mut self) -> Option<Box<Node<T>>> {
        self.head.map(|node| unsafe {
            let node = Box::from_raw(node.as_ptr());
            self.head = node.next;
            match self.head {
                None => self.tail = None,
                Some(head) => {
                    (*head.as_ptr()).prev = None;
                }
            }
            self.len -= 1;
            node
        })
    }

    #[inline]
    fn push_back_node(&mut self, mut node: Box<Node<T>>) {
        node.next = None;
        node.prev = self.tail;
        let node = Some(Box::leak(node).into());
        unsafe {
            match self.tail {
                None => self.head = node,
                Some(tail) => (*tail.as_ptr()).next = node,
            }
        }
        self.len += 1;
        self.tail = node;
    }

    #[inline]
    fn pop_back_node(&mut self) -> Option<Box<Node<T>>> {
        self.tail.map(|node| unsafe {
            let node = Box::from_raw(node.as_ptr());
            self.tail = node.prev;
            match self.tail {
                None => self.head = None,
                Some(tail) => (*tail.as_ptr()).next = None,
            }
            self.len -= 1;
            node
        })
    }

    #[inline]
    unsafe fn unlink_node(&mut self, mut node: NonNull<Node<T>>) {
        let node = node.as_mut();
        match node.prev {
            Some(prev) => (*prev.as_ptr()).next = node.next,
            None => self.head = node.next,
        }
        match node.next {
            Some(next) => (*next.as_ptr()).prev = node.prev,
            None => self.tail = node.prev,
        }
        self.len -= 1;
    }
}

impl<T> LinkedList<T> {
    pub const fn new() -> Self {
        Self {
            head: None,
            tail: None,
            len: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.head.is_none()
    }

    pub fn front(&self) -> Option<&T> {
        unsafe { self.head.as_ref().map(|head| &head.as_ref().elem) }
    }

    pub fn pop_front(&mut self) -> Option<T> {
        self.pop_front_node().map(Node::into_elem)
    }

    pub fn push_back(&mut self, elem: T) -> Option<NonNull<Node<T>>> {
        self.push_back_node(Box::new(Node::new(elem)));
        self.tail
    }

    pub fn move_to_back(&mut self, node: Option<NonNull<Node<T>>>) {
        match node {
            None => {}
            Some(unlinked_node) => unsafe {
                self.unlink_node(unlinked_node);
                let node = Box::from_raw(unlinked_node.as_ptr());
                self.push_back_node(node);
            },
        }
    }
}

unsafe impl<#[may_dangle] T> Drop for LinkedList<T> {
    fn drop(&mut self) {
        struct DropGuard<'a, T>(&'a mut LinkedList<T>);

        impl<'a, T> Drop for DropGuard<'a, T> {
            fn drop(&mut self) {
                // Continue the same loop we do below. This only runs when a destructor has
                // panicked. If another one panics this will abort.
                while self.0.pop_front_node().is_some() {}
            }
        }

        while let Some(node) = self.pop_front_node() {
            let guard = DropGuard(self);
            drop(node);
            std::mem::forget(guard);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::linked_list::LinkedList;

    #[test]
    fn basic() {
        let mut linkedlist = LinkedList::new();
        let n1 = linkedlist.push_back(3);
        let n2 = linkedlist.push_back(4);
        linkedlist.move_to_back(n1);
        assert_eq!(linkedlist.pop_front(), Some(4));
        assert_eq!(linkedlist.pop_front(), Some(3));
    }
}
