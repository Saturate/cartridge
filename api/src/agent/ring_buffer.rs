pub struct RingBuffer {
    buf: Vec<u8>,
    capacity: usize,
    write_pos: usize,
    total_written: u64,
}

pub struct ReadResult {
    pub data: Vec<u8>,
    pub next_offset: u64,
    pub wrapped: bool,
    pub lost_bytes: u64,
}

impl RingBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            buf: vec![0u8; capacity],
            capacity,
            write_pos: 0,
            total_written: 0,
        }
    }

    pub fn append(&mut self, data: &[u8]) {
        for &byte in data {
            self.buf[self.write_pos] = byte;
            self.write_pos = (self.write_pos + 1) % self.capacity;
        }
        self.total_written += data.len() as u64;
    }

    pub fn total_written(&self) -> u64 {
        self.total_written
    }

    fn oldest_offset(&self) -> u64 {
        self.total_written.saturating_sub(self.capacity as u64)
    }

    pub fn read_since(&self, since: u64) -> ReadResult {
        let oldest = self.oldest_offset();

        if since >= self.total_written {
            return ReadResult {
                data: Vec::new(),
                next_offset: self.total_written,
                wrapped: false,
                lost_bytes: 0,
            };
        }

        let (effective_start, wrapped, lost_bytes) = if since < oldest {
            (oldest, true, oldest - since)
        } else {
            (since, false, 0)
        };

        let len = (self.total_written - effective_start) as usize;
        let start_pos = if self.total_written <= self.capacity as u64 {
            effective_start as usize
        } else {
            (self.write_pos + self.capacity - (self.total_written - effective_start) as usize) % self.capacity
        };

        let mut data = Vec::with_capacity(len);
        for i in 0..len {
            data.push(self.buf[(start_pos + i) % self.capacity]);
        }

        ReadResult {
            data,
            next_offset: self.total_written,
            wrapped,
            lost_bytes,
        }
    }

    pub fn read_all(&self) -> ReadResult {
        self.read_since(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_buffer() {
        let rb = RingBuffer::new(16);
        let result = rb.read_since(0);
        assert!(result.data.is_empty());
        assert_eq!(result.next_offset, 0);
        assert!(!result.wrapped);
    }

    #[test]
    fn simple_write_read() {
        let mut rb = RingBuffer::new(16);
        rb.append(b"hello");
        let result = rb.read_since(0);
        assert_eq!(result.data, b"hello");
        assert_eq!(result.next_offset, 5);
        assert!(!result.wrapped);
    }

    #[test]
    fn incremental_read() {
        let mut rb = RingBuffer::new(16);
        rb.append(b"hello");
        let r1 = rb.read_since(0);
        assert_eq!(r1.data, b"hello");

        rb.append(b" world");
        let r2 = rb.read_since(r1.next_offset);
        assert_eq!(r2.data, b" world");
        assert_eq!(r2.next_offset, 11);
    }

    #[test]
    fn wrap_around() {
        let mut rb = RingBuffer::new(8);
        rb.append(b"12345678"); // fills exactly
        rb.append(b"ab");       // wraps, overwrites "12"

        let result = rb.read_since(0);
        assert!(result.wrapped);
        assert_eq!(result.lost_bytes, 2);
        assert_eq!(result.data, b"345678ab");
    }

    #[test]
    fn read_since_overwritten_offset() {
        let mut rb = RingBuffer::new(8);
        rb.append(b"12345678ab"); // 10 bytes, capacity 8

        let result = rb.read_since(1);
        assert!(result.wrapped);
        assert_eq!(result.lost_bytes, 1); // offset 1 is gone, oldest is 2
        assert_eq!(result.data, b"345678ab");
    }

    #[test]
    fn read_since_future_offset() {
        let mut rb = RingBuffer::new(16);
        rb.append(b"hello");
        let result = rb.read_since(100);
        assert!(result.data.is_empty());
        assert_eq!(result.next_offset, 5);
    }

    #[test]
    fn multiple_wraps() {
        let mut rb = RingBuffer::new(4);
        rb.append(b"abcdefghijkl"); // 12 bytes, capacity 4
        let result = rb.read_all();
        assert_eq!(result.data, b"ijkl");
        assert!(result.wrapped);
        assert_eq!(result.lost_bytes, 8);
    }
}
