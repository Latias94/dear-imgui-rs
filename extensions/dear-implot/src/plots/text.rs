//! Text plot implementation

use super::{
    PlotData, PlotDataLayout, PlotError, PlotItemStyle, PlotItemStyled, plot_spec_with_style,
};
use crate::{ItemFlags, TextFlags, sys};
use dear_imgui_rs::with_scratch_txt;

/// Builder for text plots with extensive customization options
///
/// Text plots allow placing text labels at specific coordinates in the plot area.
pub struct TextPlot<'a> {
    text: &'a str,
    x: f64,
    y: f64,
    style: PlotItemStyle,
    pix_offset_x: f64,
    pix_offset_y: f64,
    flags: TextFlags,
    item_flags: ItemFlags,
}

impl<'a> super::PlotItemStyled for TextPlot<'a> {
    fn style_mut(&mut self) -> &mut PlotItemStyle {
        &mut self.style
    }
}

impl<'a> TextPlot<'a> {
    /// Create a new text plot with the given text and position
    pub fn new(text: &'a str, x: f64, y: f64) -> Self {
        Self {
            text,
            x,
            y,
            style: PlotItemStyle::default(),
            pix_offset_x: 0.0,
            pix_offset_y: 0.0,
            flags: TextFlags::NONE,
            item_flags: ItemFlags::NONE,
        }
    }

    /// Set pixel offset for fine positioning
    pub fn with_pixel_offset(mut self, offset_x: f64, offset_y: f64) -> Self {
        self.pix_offset_x = offset_x;
        self.pix_offset_y = offset_y;
        self
    }

    /// Set text flags for customization
    pub fn with_flags(mut self, flags: TextFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Set common item flags for this plot item (applies to all plot types)
    pub fn with_item_flags(mut self, flags: ItemFlags) -> Self {
        self.item_flags = flags;
        self
    }

    /// Make text vertical instead of horizontal
    pub fn vertical(mut self) -> Self {
        self.flags |= TextFlags::VERTICAL;
        self
    }

    /// Check the label, plot position, and native pixel offset.
    pub fn validate(&self) -> Result<(), PlotError> {
        self.validated_pixel_offset().map(|_| ())
    }

    fn validated_pixel_offset(&self) -> Result<sys::ImVec2_c, PlotError> {
        if self.text.is_empty() {
            return Err(PlotError::InvalidData("Text cannot be empty".to_string()));
        }
        if self.text.contains('\0') {
            return Err(PlotError::StringConversion(
                "text contained null byte".to_string(),
            ));
        }
        if !self.x.is_finite() || !self.y.is_finite() {
            return Err(PlotError::InvalidData(
                "Text position must be finite".into(),
            ));
        }
        let pix_offset = sys::ImVec2_c {
            x: self.pix_offset_x as f32,
            y: self.pix_offset_y as f32,
        };
        if !pix_offset.x.is_finite() || !pix_offset.y.is_finite() {
            return Err(PlotError::InvalidData(
                "Text pixel offsets must convert to finite f32 values".into(),
            ));
        }
        Ok(pix_offset)
    }

    /// Plot the text, or skip it if validation fails.
    pub fn plot(self, plot_ui: &crate::PlotScope<'_>) {
        let Ok(pix_offset) = self.validated_pixel_offset() else {
            return;
        };
        plot_ui.with_bound_context(|| {
            with_scratch_txt(self.text, |text_ptr| unsafe {
                let spec = plot_spec_with_style(
                    plot_ui,
                    self.style,
                    self.flags.bits() | self.item_flags.bits(),
                    PlotDataLayout::DEFAULT,
                );
                sys::ImPlot_PlotText(text_ptr, self.x, self.y, pix_offset, spec);
            });
        })
    }
}

impl<'a> PlotData for TextPlot<'a> {
    fn label(&self) -> &str {
        self.text
    }

    fn data_len(&self) -> usize {
        1 // Text plot has one data point
    }

    fn validate(&self) -> Result<(), PlotError> {
        TextPlot::validate(self)
    }
}

/// Multiple text labels plot
pub struct MultiTextPlot<'a> {
    texts: Vec<&'a str>,
    positions: Vec<(f64, f64)>,
    pixel_offsets: Vec<(f64, f64)>,
    style: PlotItemStyle,
    flags: TextFlags,
    item_flags: ItemFlags,
}

impl<'a> super::PlotItemStyled for MultiTextPlot<'a> {
    fn style_mut(&mut self) -> &mut PlotItemStyle {
        &mut self.style
    }
}

impl<'a> MultiTextPlot<'a> {
    /// Create a new multi-text plot
    pub fn new(texts: Vec<&'a str>, positions: Vec<(f64, f64)>) -> Self {
        let pixel_offsets = vec![(0.0, 0.0); texts.len()];
        Self {
            texts,
            positions,
            pixel_offsets,
            style: PlotItemStyle::default(),
            flags: TextFlags::NONE,
            item_flags: ItemFlags::NONE,
        }
    }

    /// Set pixel offsets for all texts
    pub fn with_pixel_offsets(mut self, offsets: Vec<(f64, f64)>) -> Self {
        self.pixel_offsets = offsets;
        self
    }

    /// Set text flags for all texts
    pub fn with_flags(mut self, flags: TextFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Set common item flags for all text items (applies to all plot types)
    pub fn with_item_flags(mut self, flags: ItemFlags) -> Self {
        self.item_flags = flags;
        self
    }

    /// Make all texts vertical
    pub fn vertical(mut self) -> Self {
        self.flags |= TextFlags::VERTICAL;
        self
    }

    /// Validate the plot data
    pub fn validate(&self) -> Result<(), PlotError> {
        if self.texts.len() != self.positions.len() {
            return Err(PlotError::InvalidData(format!(
                "Text count ({}) must match position count ({})",
                self.texts.len(),
                self.positions.len()
            )));
        }

        if self.pixel_offsets.len() != self.texts.len() {
            return Err(PlotError::InvalidData(format!(
                "Pixel offset count ({}) must match text count ({})",
                self.pixel_offsets.len(),
                self.texts.len()
            )));
        }

        if self.texts.is_empty() {
            return Err(PlotError::EmptyData);
        }

        for (i, text) in self.texts.iter().enumerate() {
            if text.is_empty() {
                return Err(PlotError::InvalidData(format!(
                    "Text at index {} cannot be empty",
                    i
                )));
            }
        }

        Ok(())
    }

    /// Plot all texts
    pub fn plot(self, plot_ui: &crate::PlotScope<'_>) {
        if self.validate().is_err() {
            return;
        }
        for (i, &text) in self.texts.iter().enumerate() {
            let position = self.positions[i];
            let offset = self.pixel_offsets[i];

            let text_plot = TextPlot::new(text, position.0, position.1)
                .with_style(self.style)
                .with_pixel_offset(offset.0, offset.1)
                .with_flags(self.flags)
                .with_item_flags(self.item_flags);

            text_plot.plot(plot_ui);
        }
    }
}

impl<'a> PlotData for MultiTextPlot<'a> {
    fn label(&self) -> &str {
        "MultiText"
    }

    fn data_len(&self) -> usize {
        self.texts.len()
    }
}

/// Formatted text plot with dynamic content
pub struct FormattedTextPlot {
    text: String,
    x: f64,
    y: f64,
    style: PlotItemStyle,
    pix_offset_x: f64,
    pix_offset_y: f64,
    flags: TextFlags,
    item_flags: ItemFlags,
}

impl super::PlotItemStyled for FormattedTextPlot {
    fn style_mut(&mut self) -> &mut PlotItemStyle {
        &mut self.style
    }
}

impl FormattedTextPlot {
    /// Create a new formatted text plot
    pub fn new(text: String, x: f64, y: f64) -> Self {
        Self {
            text,
            x,
            y,
            style: PlotItemStyle::default(),
            pix_offset_x: 0.0,
            pix_offset_y: 0.0,
            flags: TextFlags::NONE,
            item_flags: ItemFlags::NONE,
        }
    }

    /// Create a formatted text plot from format arguments
    pub fn from_format(x: f64, y: f64, args: std::fmt::Arguments) -> Self {
        Self::new(format!("{}", args), x, y)
    }

    /// Set pixel offset for fine positioning
    pub fn with_pixel_offset(mut self, offset_x: f64, offset_y: f64) -> Self {
        self.pix_offset_x = offset_x;
        self.pix_offset_y = offset_y;
        self
    }

    /// Set text flags for customization
    pub fn with_flags(mut self, flags: TextFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Set common item flags for this plot item (applies to all plot types)
    pub fn with_item_flags(mut self, flags: ItemFlags) -> Self {
        self.item_flags = flags;
        self
    }

    /// Make text vertical
    pub fn vertical(mut self) -> Self {
        self.flags |= TextFlags::VERTICAL;
        self
    }

    /// Check the label, plot position, and native pixel offset.
    pub fn validate(&self) -> Result<(), PlotError> {
        self.as_text_plot().validate()
    }

    /// Plot the formatted text, or skip it if validation fails.
    pub fn plot(self, plot_ui: &crate::PlotScope<'_>) {
        self.as_text_plot().plot(plot_ui);
    }

    fn as_text_plot(&self) -> TextPlot<'_> {
        TextPlot {
            text: &self.text,
            x: self.x,
            y: self.y,
            style: self.style,
            pix_offset_x: self.pix_offset_x,
            pix_offset_y: self.pix_offset_y,
            flags: self.flags,
            item_flags: self.item_flags,
        }
    }
}

impl PlotData for FormattedTextPlot {
    fn label(&self) -> &str {
        &self.text
    }

    fn data_len(&self) -> usize {
        1
    }

    fn validate(&self) -> Result<(), PlotError> {
        FormattedTextPlot::validate(self)
    }
}

/// Convenience macro for creating formatted text plots
#[macro_export]
macro_rules! plot_text {
    ($x:expr, $y:expr, $($arg:tt)*) => {
        $crate::plots::text::FormattedTextPlot::from_format($x, $y, format_args!($($arg)*))
    };
}

/// Text annotation with automatic positioning
pub struct TextAnnotation<'a> {
    text: &'a str,
    x: f64,
    y: f64,
    style: PlotItemStyle,
    auto_offset: bool,
    flags: TextFlags,
    item_flags: ItemFlags,
}

impl<'a> super::PlotItemStyled for TextAnnotation<'a> {
    fn style_mut(&mut self) -> &mut PlotItemStyle {
        &mut self.style
    }
}

impl<'a> TextAnnotation<'a> {
    /// Create a new text annotation
    pub fn new(text: &'a str, x: f64, y: f64) -> Self {
        Self {
            text,
            x,
            y,
            style: PlotItemStyle::default(),
            auto_offset: true,
            flags: TextFlags::NONE,
            item_flags: ItemFlags::NONE,
        }
    }

    /// Disable automatic offset calculation
    pub fn no_auto_offset(mut self) -> Self {
        self.auto_offset = false;
        self
    }

    /// Set text flags
    pub fn with_flags(mut self, flags: TextFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Set common item flags for the annotation text
    pub fn with_item_flags(mut self, flags: ItemFlags) -> Self {
        self.item_flags = flags;
        self
    }

    /// Make text vertical
    pub fn vertical(mut self) -> Self {
        self.flags |= TextFlags::VERTICAL;
        self
    }

    /// Plot the annotation
    pub fn plot(self, plot_ui: &crate::PlotScope<'_>) {
        let offset = if self.auto_offset {
            // Simple auto-offset logic - could be enhanced
            (5.0, -5.0)
        } else {
            (0.0, 0.0)
        };

        let text_plot = TextPlot::new(self.text, self.x, self.y)
            .with_style(self.style)
            .with_pixel_offset(offset.0, offset.1)
            .with_flags(self.flags)
            .with_item_flags(self.item_flags);

        text_plot.plot(plot_ui);
    }
}

#[cfg(test)]
mod tests {
    use super::{FormattedTextPlot, PlotData, PlotError, TextPlot};

    #[test]
    fn text_validates_label_position_and_native_pixel_offset() {
        let text = |position_x, position_y| TextPlot::new("label", position_x, position_y);
        assert!(
            text(0.0, 1.0)
                .with_pixel_offset(-1.0, 2.0)
                .validate()
                .is_ok()
        );
        for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(matches!(
                text(invalid, 0.0).validate(),
                Err(PlotError::InvalidData(_))
            ));
            assert!(matches!(
                text(0.0, invalid).validate(),
                Err(PlotError::InvalidData(_))
            ));
        }
        for invalid in [f64::NAN, f64::INFINITY, f64::MAX, -f64::MAX] {
            assert!(
                text(0.0, 0.0)
                    .with_pixel_offset(invalid, 0.0)
                    .validate()
                    .is_err()
            );
            assert!(
                text(0.0, 0.0)
                    .with_pixel_offset(0.0, invalid)
                    .validate()
                    .is_err()
            );
        }
        assert!(matches!(
            TextPlot::new("", 0.0, 0.0).validate(),
            Err(PlotError::InvalidData(_))
        ));
        assert!(matches!(
            TextPlot::new("not\0valid", 0.0, 0.0).validate(),
            Err(PlotError::StringConversion(_))
        ));
        let formatted =
            FormattedTextPlot::new("label".into(), 0.0, 0.0).with_pixel_offset(f64::MAX, 0.0);
        assert!(matches!(
            PlotData::validate(&formatted),
            Err(PlotError::InvalidData(_))
        ));
    }
}
