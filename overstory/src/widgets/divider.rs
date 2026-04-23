// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Decorative divider widget.

use kurbo::Size;

use crate::{MeasureCtx, Widget};

/// Orientation for a decorative divider.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum DividerAxis {
    /// Horizontal rule separating stacked content.
    #[default]
    Horizontal,
    /// Vertical rule separating side-by-side content.
    Vertical,
}

/// Simple divider that measures to a thin rule along one axis.
///
/// The divider itself does not emit custom display nodes. It relies on the
/// element's resolved background color to paint the rule inside the measured
/// rectangle.
#[derive(Clone, Debug, PartialEq)]
pub struct Divider {
    axis: DividerAxis,
    thickness: f64,
}

impl Default for Divider {
    fn default() -> Self {
        Self {
            axis: DividerAxis::Horizontal,
            thickness: 1.0,
        }
    }
}

impl Divider {
    /// Creates a horizontal divider.
    #[must_use]
    pub fn horizontal() -> Self {
        Self::default()
    }

    /// Creates a vertical divider.
    #[must_use]
    pub fn vertical() -> Self {
        Self {
            axis: DividerAxis::Vertical,
            ..Self::default()
        }
    }

    /// Sets the divider thickness.
    #[must_use]
    pub fn with_thickness(mut self, thickness: f64) -> Self {
        self.thickness = thickness.max(1.0);
        self
    }

    fn measured_size(&self, available: Size) -> Size {
        match self.axis {
            DividerAxis::Horizontal => Size::new(available.width.max(0.0), self.thickness),
            DividerAxis::Vertical => Size::new(self.thickness, available.height.max(0.0)),
        }
    }
}

impl Widget for Divider {
    fn measure(&self, available: Size, _ctx: &mut MeasureCtx<'_>) -> Option<Size> {
        Some(self.measured_size(available))
    }

    crate::impl_widget_any!();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn horizontal_divider_measures_to_requested_thickness() {
        let divider = Divider::horizontal().with_thickness(2.0);
        assert_eq!(
            divider.measured_size(Size::new(320.0, 40.0)),
            Size::new(320.0, 2.0)
        );
    }

    #[test]
    fn vertical_divider_measures_to_requested_thickness() {
        let divider = Divider::vertical().with_thickness(3.0);
        assert_eq!(
            divider.measured_size(Size::new(320.0, 40.0)),
            Size::new(3.0, 40.0)
        );
    }
}
