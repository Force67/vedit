use std::cell::RefCell;
use std::collections::VecDeque;

/// Simple object pool for reusing allocations
pub struct ObjectPool<T> {
    pool: RefCell<VecDeque<T>>,
    create_fn: fn() -> T,
    reset_fn: fn(&mut T),
}

impl<T> ObjectPool<T> {
    /// Create a new object pool with creation and reset functions
    pub fn new(create_fn: fn() -> T, reset_fn: fn(&mut T)) -> Self {
        Self {
            pool: RefCell::new(VecDeque::new()),
            create_fn,
            reset_fn,
        }
    }

    /// Get an object from the pool, or create a new one if empty
    pub fn get(&self) -> PooledObject<'_, T> {
        let mut pool = self.pool.borrow_mut();
        let mut object = pool.pop_front().unwrap_or_else(|| (self.create_fn)());
        (self.reset_fn)(&mut object);
        PooledObject {
            object: Some(object),
            pool: &self.pool,
        }
    }

    /// Pre-populate the pool with a number of objects
    pub fn warm(&self, count: usize) {
        let mut pool = self.pool.borrow_mut();
        for _ in 0..count {
            pool.push_back((self.create_fn)());
        }
    }
}

/// A pooled object that returns itself to the pool when dropped
pub struct PooledObject<'a, T> {
    object: Option<T>,
    pool: &'a RefCell<VecDeque<T>>,
}

impl<T> std::ops::Deref for PooledObject<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.object.as_ref().unwrap()
    }
}

impl<T> std::ops::DerefMut for PooledObject<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.object.as_mut().unwrap()
    }
}

impl<T> Drop for PooledObject<'_, T> {
    fn drop(&mut self) {
        if let Some(object) = self.object.take() {
            let mut pool = self.pool.borrow_mut();
            if pool.len() < 50 {
                // Prevent unbounded growth
                pool.push_back(object);
            }
        }
    }
}

// Pre-defined pools for common types using a simpler approach
pub fn get_pooled_usize_vec() -> Vec<usize> {
    thread_local! {
        static USIZE_VEC_POOL: std::cell::RefCell<Vec<Vec<usize>>> = std::cell::RefCell::new(Vec::new());
    }

    USIZE_VEC_POOL.with(|pool| {
        let mut pool = pool.borrow_mut();
        pool.pop().unwrap_or_else(|| Vec::with_capacity(100))
    })
}

pub fn return_usize_vec(mut vec: Vec<usize>) {
    thread_local! {
        static USIZE_VEC_POOL: std::cell::RefCell<Vec<Vec<usize>>> = std::cell::RefCell::new(Vec::new());
    }

    vec.clear();
    USIZE_VEC_POOL.with(|pool| {
        let mut pool = pool.borrow_mut();
        if pool.len() < 50 {
            // Prevent unbounded growth
            pool.push(vec);
        }
    })
}

pub fn get_pooled_string() -> String {
    thread_local! {
        static STRING_POOL: std::cell::RefCell<Vec<String>> = std::cell::RefCell::new(Vec::new());
    }

    STRING_POOL.with(|pool| {
        let mut pool = pool.borrow_mut();
        pool.pop().unwrap_or_else(|| String::with_capacity(20))
    })
}

pub fn return_string(mut string: String) {
    thread_local! {
        static STRING_POOL: std::cell::RefCell<Vec<String>> = std::cell::RefCell::new(Vec::new());
    }

    string.clear();
    STRING_POOL.with(|pool| {
        let mut pool = pool.borrow_mut();
        if pool.len() < 50 {
            // Prevent unbounded growth
            pool.push(string);
        }
    })
}
