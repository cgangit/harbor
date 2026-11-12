use std::collections::VecDeque;
use std::sync::Mutex;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccessUnit {
    pub bytes: Vec<u8>,
    pub pts_ns: i64,
    pub es: i32,
}

pub struct AuQueue {
    inner: Mutex<VecDeque<AccessUnit>>,
    cap: usize,
}

impl AuQueue {
    pub fn new(cap: usize) -> Self {
        Self {
            inner: Mutex::new(VecDeque::new()),
            cap: cap.max(1),
        }
    }

    pub fn try_push(&self, au: AccessUnit) -> Result<(), AccessUnit> {
        let mut q = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if q.len() >= self.cap {
            return Err(au);
        }
        q.push_back(au);
        Ok(())
    }

    pub fn pop(&self) -> Option<AccessUnit> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner()).pop_front()
    }

    pub fn len(&self) -> usize {
        self.inner.lock().unwrap_or_else(|e| e.into_inner()).len()
    }

    pub fn clear(&self) {
        self.inner.lock().unwrap_or_else(|e| e.into_inner()).clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_push_pops_fifo() {
        let q = AuQueue::new(2);
        let a = AccessUnit {
            bytes: vec![1],
            pts_ns: 0,
            es: 1,
        };
        let b = AccessUnit {
            bytes: vec![2],
            pts_ns: 1,
            es: 1,
        };
        let c = AccessUnit {
            bytes: vec![3],
            pts_ns: 2,
            es: 1,
        };
        assert!(q.try_push(a.clone()).is_ok());
        assert!(q.try_push(b.clone()).is_ok());
        assert_eq!(q.try_push(c).unwrap_err().bytes, vec![3]);
        assert_eq!(q.pop(), Some(a));
        assert_eq!(q.pop(), Some(b));
        assert!(q.pop().is_none());
    }
}
