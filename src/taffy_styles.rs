use egui_taffy::taffy::{
    AlignContent, AlignItems, FlexDirection, JustifyContent, LengthPercentage, Size, Style,
    prelude::{auto, length},
};

pub trait StyleBuilder {
    /// Sets the flex direction to column
    fn column(self) -> Self;
    /// Sets the flex direction to row
    fn row(self) -> Self;
    /// Sets the gap between flex items
    fn gap(self, gap: f32) -> Self;
    /// Sets the padding around flex items
    fn padding(self, padding: f32) -> Self;
    /// Sets the width of the flex container
    fn width(self, width: f32) -> Self;
    /// Sets the height of the flex container
    fn height(self, height: f32) -> Self;
    /// Sets the height to auto
    fn auto_height(self) -> Self;
    /// Sets how much the flex item will grow relative to other flex items
    fn grow(self, grow: f32) -> Self;
    /// Sets the initial main size of the flex item
    fn basis(self, basis: f32) -> Self;
    /// Sets the direction of the main axis (row or column)
    fn flex_direction(self, direction: FlexDirection) -> Self;
    /// Aligns lines of content along the cross axis when there is extra space
    fn align_content(self, align: AlignContent) -> Self;
    /// Aligns lines of content along the cross axis when there is extra space
    fn align_items(self, align: AlignItems) -> Self;
    /// Aligns flex items along the main axis when there is extra space
    fn justify_content(self, justify: JustifyContent) -> Self;
    /// Sets the size of the gap between flex items using LengthPercentage
    fn gap_size(self, gap: Size<LengthPercentage>) -> Self;
}

impl StyleBuilder for Style {
    /// Sets flex direction to column layout
    fn column(mut self) -> Self {
        self.flex_direction = FlexDirection::Column;
        self
    }

    /// Sets flex direction to row layout
    fn row(mut self) -> Self {
        self.flex_direction = FlexDirection::Row;
        self
    }

    /// Sets gap spacing between flex items
    fn gap(mut self, gap: f32) -> Self {
        self.gap = length(gap);
        self
    }

    /// Sets padding around flex items
    fn padding(mut self, padding: f32) -> Self {
        self.padding = length(padding);
        self
    }

    /// Sets container width
    fn width(mut self, width: f32) -> Self {
        self.size.width = length(width);
        self
    }

    /// Sets container height
    fn height(mut self, height: f32) -> Self {
        self.size.height = length(height);
        self
    }

    /// Sets height to automatic sizing
    fn auto_height(mut self) -> Self {
        self.size.height = auto();
        self
    }

    /// Sets flex grow factor
    fn grow(mut self, grow: f32) -> Self {
        self.flex_grow = grow;
        self
    }

    /// Sets flex basis size
    fn basis(mut self, basis: f32) -> Self {
        self.flex_basis = length(basis);
        self
    }

    /// Sets main axis direction
    fn flex_direction(mut self, direction: FlexDirection) -> Self {
        self.flex_direction = direction;
        self
    }

    /// Sets cross-axis alignment
    fn align_content(mut self, align: AlignContent) -> Self {
        self.align_content = Some(align);
        self
    }

    fn align_items(mut self, align: AlignItems) -> Self {
        self.align_items = Some(align);
        self
    }

    /// Sets main-axis alignment
    fn justify_content(mut self, justify: JustifyContent) -> Self {
        self.justify_content = Some(justify);
        self
    }

    /// Sets gap size using LengthPercentage
    fn gap_size(mut self, gap: Size<LengthPercentage>) -> Self {
        self.gap = gap;
        self
    }
}

/// Creates a new Style instance with default values
pub fn style() -> Style {
    Style::default()
}

pub fn flex_row() -> Style {
    style().row()
}

pub fn flex_column() -> Style {
    style().column()
}
