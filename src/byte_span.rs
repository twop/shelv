use std::{marker::PhantomData, ops::Range};

/// Ordered byte span
#[derive(Debug, Clone, Copy)]
pub struct ByteSpan {
    pub start: usize,
    pub end: usize,
    marker: PhantomData<()>,
}

impl ByteSpan {
    pub fn new(start: usize, end: usize) -> Self {
        let (start, end) = if start <= end {
            (start, end)
        } else {
            (end, start)
        };

        Self {
            start,
            end,
            marker: PhantomData,
        }
    }

    pub fn range(&self) -> Range<usize> {
        self.start..self.end
    }
}

#[derive(Debug, Clone, Copy)]
pub struct UnOrderedByteSpan {
    pub start: usize,
    pub end: usize,
    marker: PhantomData<()>,
}

impl UnOrderedByteSpan {
    pub fn new(start: usize, end: usize) -> Self {
        Self {
            start,
            end,
            marker: PhantomData,
        }
    }

    pub fn range(&self) -> Range<usize> {
        self.start..self.end
    }

    pub fn ordered(&self) -> ByteSpan {
        ByteSpan::new(self.start, self.end)
    }
}
