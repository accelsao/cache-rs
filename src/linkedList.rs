use std::ptr::NonNull;

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

    pub fn push_back(&mut self, elem: T) {
        self.push_back_node(Box::new(Node::new(elem)));
    }

    pub unsafe fn move_to_back(&mut self, node: NonNull<Node<T>>) {
        self.unlink_node(node);
        let node = Box::from_raw(node.as_ptr());
        self.push_back_node(node);
    }
}
