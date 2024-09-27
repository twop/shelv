use std::{fmt::Pointer, marker::PhantomData, ops::Range, usize};

/// Ordered byte span
#[derive(Clone, Copy, PartialEq, Hash, Eq)]
pub struct ByteSpan {
    pub start: usize,
    pub end: usize,
    marker: PhantomData<()>,
}

impl std::fmt::Debug for ByteSpan {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ByteSpan({}..{})", self.start, self.end)
        // f.fmt("ByteSpan")
        //     .field(&self.start)
        //     .field(&self.end)
        //     .finish()
    }
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

    pub fn point(start_and_end: usize) -> Self {
        Self {
            start: start_and_end,
            end: start_and_end,
            marker: PhantomData,
        }
    }

    pub fn from_range(range: &Range<usize>) -> Self {
        Self::new(range.start, range.end)
    }

    pub fn range(&self) -> Range<usize> {
        self.start..self.end
    }

    pub fn contains_pos(&self, pos: usize) -> bool {
        self.range().contains(&pos)
    }

    pub fn contains(&self, other: ByteSpan) -> bool {
        self.start <= other.start && self.end >= other.end
    }

    pub fn is_empty(self) -> bool {
        self.end == self.start
    }

    pub fn unordered(&self) -> UnOrderedByteSpan {
        UnOrderedByteSpan::new(self.start, self.end)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

    pub fn ordered(&self) -> ByteSpan {
        ByteSpan::new(self.start, self.end)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum RangeRelation {
    Before,
    After,
    StartInside,
    EndInside,
    Inside,
    Contains,
    Equal,
}

impl ByteSpan {
    pub fn relative_to(&self, other: Self) -> RangeRelation {
        let (s_start, s_end) = (self.start, self.end);
        let (other_start, other_end) = (other.start, other.end);
        assert!(s_start <= s_end, "self: assumes left -> right direction");
        assert!(
            other_start <= other_end,
            "other: assumes left -> right direction"
        );

        if s_start == other_start && s_end == other_end {
            RangeRelation::Equal
        } else if s_end <= other_start {
            RangeRelation::Before
        } else if s_start >= other_end {
            RangeRelation::After
        } else if s_start >= other_start && s_end <= other_end {
            RangeRelation::Inside
        } else if s_start <= other_start && s_end >= other_end {
            RangeRelation::Contains
        }
        // note strict comparison here, due to range being not inclusive
        else if s_end > other_start && s_start < other_start {
            RangeRelation::EndInside
        } else if s_start < other_end && s_end > other_end {
            RangeRelation::StartInside
        } else {
            panic!("should be exhaustive")
        }
    }
}
