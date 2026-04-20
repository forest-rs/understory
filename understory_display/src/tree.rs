// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Retained display-tree fragments with simple measure/place support.

extern crate alloc;

use alloc::{boxed::Box, vec::Vec};

use kurbo::{Insets as KurboInsets, Point, Rect, RoundedRect, Size, Stroke, Vec2};
use peniko::Brush;

use crate::{DisplayGlyphRun, DisplayList, DisplayListBuilder, SemanticId};
#[cfg(feature = "std")]
use crate::{TextEngine, TextRunRequest, parley::Alignment};

/// Box constraints used during display-tree measure/place.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct BoxConstraints {
    /// Minimum allowed size.
    pub min: Size,
    /// Maximum allowed size.
    pub max: Size,
}

impl BoxConstraints {
    /// Creates constraints from minimum and maximum sizes.
    #[must_use]
    pub const fn new(min: Size, max: Size) -> Self {
        Self { min, max }
    }

    /// Creates tight constraints for one exact size.
    #[must_use]
    pub fn tight(size: Size) -> Self {
        Self {
            min: size,
            max: size,
        }
    }

    /// Creates loose constraints with a zero minimum and the given maximum.
    #[must_use]
    pub fn loose(max: Size) -> Self {
        Self {
            min: Size::ZERO,
            max,
        }
    }

    /// Clamps a candidate size into this constraint range.
    #[must_use]
    pub fn constrain(self, size: Size) -> Size {
        Size::new(
            size.width.clamp(self.min.width, self.max.width),
            size.height.clamp(self.min.height, self.max.height),
        )
    }

    /// Returns constraints shrunk by the given insets.
    #[must_use]
    pub fn shrink(self, insets: Insets) -> Self {
        let dx = (insets.x_value()).max(0.0);
        let dy = (insets.y_value()).max(0.0);
        let min = Size::new(
            (self.min.width - dx).max(0.0),
            (self.min.height - dy).max(0.0),
        );
        let max = Size::new(
            (self.max.width - dx).max(0.0),
            (self.max.height - dy).max(0.0),
        );
        Self { min, max }
    }
}

/// Insets for padding-like display nodes.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Insets {
    /// Left inset.
    pub left: f64,
    /// Top inset.
    pub top: f64,
    /// Right inset.
    pub right: f64,
    /// Bottom inset.
    pub bottom: f64,
}

impl Insets {
    /// Creates insets from one uniform amount.
    #[must_use]
    pub const fn uniform(value: f64) -> Self {
        Self {
            left: value,
            top: value,
            right: value,
            bottom: value,
        }
    }

    /// Creates insets from horizontal and vertical amounts.
    #[must_use]
    pub const fn symmetric(horizontal: f64, vertical: f64) -> Self {
        Self {
            left: horizontal,
            top: vertical,
            right: horizontal,
            bottom: vertical,
        }
    }

    /// Returns the total horizontal inset.
    #[must_use]
    pub const fn x_value(self) -> f64 {
        self.left + self.right
    }

    /// Returns the total vertical inset.
    #[must_use]
    pub const fn y_value(self) -> f64 {
        self.top + self.bottom
    }
}

impl From<Insets> for KurboInsets {
    fn from(value: Insets) -> Self {
        Self::new(value.left, value.top, value.right, value.bottom)
    }
}

/// Alignment along one axis.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum DisplayAlign {
    /// Align to the minimum edge.
    #[default]
    Start,
    /// Center within the available space.
    Center,
    /// Align to the maximum edge.
    End,
    /// Fill the available space.
    Fill,
}

/// Final layout information for one retained display node.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct DisplayLayout {
    rect: Rect,
    bounds: Rect,
}

impl DisplayLayout {
    /// Returns the node rectangle in display/user space.
    #[must_use]
    pub fn rect(self) -> Rect {
        self.rect
    }

    /// Returns the conservative bounds for the node and descendants.
    #[must_use]
    pub fn bounds(self) -> Rect {
        self.bounds
    }
}

/// Retained display tree with one root node.
#[derive(Clone, Debug, PartialEq)]
pub struct DisplayTree {
    root: DisplayNode,
}

impl DisplayTree {
    /// Creates a retained display tree from one root node.
    #[must_use]
    pub fn new(root: DisplayNode) -> Self {
        Self { root }
    }

    /// Returns the root node.
    #[must_use]
    pub fn root(&self) -> &DisplayNode {
        &self.root
    }

    /// Returns the mutable root node.
    #[must_use]
    pub fn root_mut(&mut self) -> &mut DisplayNode {
        &mut self.root
    }

    /// Lays out the tree with the given origin and constraints using the supplied text engine.
    #[cfg(feature = "std")]
    pub fn layout(&mut self, text: &mut TextEngine, origin: Point, constraints: BoxConstraints) {
        layout_node(&mut self.root, text, origin, constraints);
    }

    /// Lowers the placed tree into a flat retained display list.
    #[must_use]
    pub fn to_display_list(&self) -> DisplayList {
        let mut builder = DisplayListBuilder::new();
        let mut z = 0_i32;
        lower_node(&self.root, &mut builder, &mut z);
        builder.build()
    }
}

/// One retained display-tree node.
#[derive(Clone, Debug, PartialEq)]
pub struct DisplayNode {
    /// Optional semantic/provenance identifier.
    pub semantic_id: Option<SemanticId>,
    layout: DisplayLayout,
    kind: DisplayNodeKind,
}

impl DisplayNode {
    /// Creates a stack node that overlays children in order.
    #[must_use]
    pub fn stack(children: Vec<Self>) -> Self {
        Self {
            semantic_id: None,
            layout: DisplayLayout::default(),
            kind: DisplayNodeKind::Stack { children },
        }
    }

    /// Creates a padding node.
    #[must_use]
    pub fn padding(insets: Insets, child: Self) -> Self {
        Self {
            semantic_id: None,
            layout: DisplayLayout::default(),
            kind: DisplayNodeKind::Padding {
                insets,
                child: Box::new(child),
            },
        }
    }

    /// Creates an alignment node.
    #[must_use]
    pub fn align(horizontal: DisplayAlign, vertical: DisplayAlign, child: Self) -> Self {
        Self {
            semantic_id: None,
            layout: DisplayLayout::default(),
            kind: DisplayNodeKind::Align {
                horizontal,
                vertical,
                child: Box::new(child),
            },
        }
    }

    /// Creates an offset node.
    #[must_use]
    pub fn offset(translation: Vec2, child: Self) -> Self {
        Self {
            semantic_id: None,
            layout: DisplayLayout::default(),
            kind: DisplayNodeKind::Offset {
                translation,
                child: Box::new(child),
            },
        }
    }

    /// Creates a fixed frame node.
    #[must_use]
    pub fn fixed_frame(size: Size, child: Self) -> Self {
        Self {
            semantic_id: None,
            layout: DisplayLayout::default(),
            kind: DisplayNodeKind::FixedFrame {
                size,
                child: Box::new(child),
            },
        }
    }

    /// Creates a fill-rect leaf that fills its assigned layout rectangle.
    #[must_use]
    pub fn fill_rect(brush: Brush) -> Self {
        Self {
            semantic_id: None,
            layout: DisplayLayout::default(),
            kind: DisplayNodeKind::FillRect { brush },
        }
    }

    /// Creates a stroke-rect leaf that fills its assigned layout rectangle.
    #[must_use]
    pub fn stroke_rect(stroke: Stroke, brush: Brush) -> Self {
        Self {
            semantic_id: None,
            layout: DisplayLayout::default(),
            kind: DisplayNodeKind::StrokeRect { stroke, brush },
        }
    }

    /// Creates a fill-rounded-rect leaf that fills its assigned layout rectangle.
    #[must_use]
    pub fn fill_rounded_rect(corner_radius: f64, brush: Brush) -> Self {
        Self {
            semantic_id: None,
            layout: DisplayLayout::default(),
            kind: DisplayNodeKind::FillRoundedRect {
                corner_radius,
                brush,
            },
        }
    }

    /// Creates a stroke-rounded-rect leaf that fills its assigned layout rectangle.
    #[must_use]
    pub fn stroke_rounded_rect(corner_radius: f64, stroke: Stroke, brush: Brush) -> Self {
        Self {
            semantic_id: None,
            layout: DisplayLayout::default(),
            kind: DisplayNodeKind::StrokeRoundedRect {
                corner_radius,
                stroke,
                brush,
            },
        }
    }

    /// Creates a text leaf.
    #[cfg(feature = "std")]
    #[must_use]
    pub fn text(
        text: impl Into<Box<str>>,
        brush: Brush,
        font_size: f32,
        font_family: impl Into<Box<str>>,
        alignment: Alignment,
    ) -> Self {
        Self {
            semantic_id: None,
            layout: DisplayLayout::default(),
            kind: DisplayNodeKind::Text(DisplayText::new(
                text,
                brush,
                font_size,
                font_family,
                alignment,
            )),
        }
    }

    /// Attaches a semantic/provenance id to the node.
    #[must_use]
    pub fn with_semantic_id(mut self, semantic_id: SemanticId) -> Self {
        self.semantic_id = Some(semantic_id);
        self
    }

    /// Returns the current layout information.
    #[must_use]
    pub fn layout(&self) -> DisplayLayout {
        self.layout
    }

    /// Returns the node kind.
    #[must_use]
    pub fn kind(&self) -> &DisplayNodeKind {
        &self.kind
    }
}

/// Retained display-tree node variants.
#[derive(Clone, Debug, PartialEq)]
pub enum DisplayNodeKind {
    /// Overlay children in source order.
    Stack {
        /// Children to overlay.
        children: Vec<DisplayNode>,
    },
    /// Apply padding around a child.
    Padding {
        /// Insets applied to the child.
        insets: Insets,
        /// Child node.
        child: Box<DisplayNode>,
    },
    /// Align a child within the assigned rectangle.
    Align {
        /// Horizontal alignment.
        horizontal: DisplayAlign,
        /// Vertical alignment.
        vertical: DisplayAlign,
        /// Child node.
        child: Box<DisplayNode>,
    },
    /// Translate a child subtree without changing its measured size.
    Offset {
        /// Translation applied to the child origin.
        translation: Vec2,
        /// Child node.
        child: Box<DisplayNode>,
    },
    /// Force one exact frame size for the child subtree.
    FixedFrame {
        /// Exact frame size.
        size: Size,
        /// Child node.
        child: Box<DisplayNode>,
    },
    /// Fill the assigned layout rectangle.
    FillRect {
        /// Fill brush.
        brush: Brush,
    },
    /// Stroke the assigned layout rectangle.
    StrokeRect {
        /// Stroke style.
        stroke: Stroke,
        /// Stroke brush.
        brush: Brush,
    },
    /// Fill the assigned layout rectangle as a rounded rect.
    FillRoundedRect {
        /// Corner radius.
        corner_radius: f64,
        /// Fill brush.
        brush: Brush,
    },
    /// Stroke the assigned layout rectangle as a rounded rect.
    StrokeRoundedRect {
        /// Corner radius.
        corner_radius: f64,
        /// Stroke style.
        stroke: Stroke,
        /// Stroke brush.
        brush: Brush,
    },
    /// One retained text node.
    #[cfg(feature = "std")]
    Text(DisplayText),
}

/// Retained text node state.
#[cfg(feature = "std")]
#[derive(Clone, Debug, PartialEq)]
pub struct DisplayText {
    text: Box<str>,
    brush: Brush,
    font_size: f32,
    font_family: Box<str>,
    alignment: Alignment,
    cached_max_advance: Option<f32>,
    runs: Vec<DisplayGlyphRun>,
    measured_size: Size,
}

#[cfg(feature = "std")]
impl DisplayText {
    /// Creates a new retained text node.
    #[must_use]
    pub fn new(
        text: impl Into<Box<str>>,
        brush: Brush,
        font_size: f32,
        font_family: impl Into<Box<str>>,
        alignment: Alignment,
    ) -> Self {
        Self {
            text: text.into(),
            brush,
            font_size,
            font_family: font_family.into(),
            alignment,
            cached_max_advance: None,
            runs: Vec::new(),
            measured_size: Size::ZERO,
        }
    }

    fn ensure_shaped(&mut self, text: &mut TextEngine, max_advance: Option<f32>) {
        if self.cached_max_advance == max_advance {
            return;
        }

        self.runs = text.shape_text(&TextRunRequest {
            text: &self.text,
            brush: self.brush.clone(),
            font_size: self.font_size,
            font_family: &self.font_family,
            max_advance,
            alignment: self.alignment,
        });
        self.cached_max_advance = max_advance;
        let bounds = union_run_bounds(&self.runs).unwrap_or(Rect::ZERO);
        self.measured_size = bounds.size();
    }
}

#[cfg(feature = "std")]
fn measure_node(
    node: &mut DisplayNode,
    text: &mut TextEngine,
    constraints: BoxConstraints,
) -> Size {
    match &mut node.kind {
        DisplayNodeKind::Stack { children } => children
            .iter_mut()
            .map(|child| measure_node(child, text, constraints))
            .fold(Size::ZERO, |size, child| {
                Size::new(size.width.max(child.width), size.height.max(child.height))
            }),
        DisplayNodeKind::Padding { insets, child } => {
            let child_size = measure_node(child, text, constraints.shrink(*insets));
            constraints.constrain(Size::new(
                child_size.width + insets.x_value(),
                child_size.height + insets.y_value(),
            ))
        }
        DisplayNodeKind::Align { child, .. } => measure_node(child, text, constraints),
        DisplayNodeKind::Offset { child, .. } => measure_node(child, text, constraints),
        DisplayNodeKind::FixedFrame { size, .. } => constraints.constrain(*size),
        DisplayNodeKind::FillRect { .. }
        | DisplayNodeKind::StrokeRect { .. }
        | DisplayNodeKind::FillRoundedRect { .. }
        | DisplayNodeKind::StrokeRoundedRect { .. } => Size::ZERO,
        DisplayNodeKind::Text(display_text) => {
            let max_advance = max_advance(constraints.max.width);
            display_text.ensure_shaped(text, max_advance);
            constraints.constrain(display_text.measured_size)
        }
    }
}

#[cfg(feature = "std")]
fn layout_node(
    node: &mut DisplayNode,
    text: &mut TextEngine,
    origin: Point,
    constraints: BoxConstraints,
) -> Size {
    let size = match node.kind {
        DisplayNodeKind::FillRect { .. }
        | DisplayNodeKind::StrokeRect { .. }
        | DisplayNodeKind::FillRoundedRect { .. }
        | DisplayNodeKind::StrokeRoundedRect { .. } => constraints.constrain(constraints.max),
        _ => measure_node(node, text, constraints),
    };
    let rect = Rect::from_origin_size(origin, size);

    match &mut node.kind {
        DisplayNodeKind::Stack { children } => {
            let mut bounds = rect;
            for child in children {
                let child_size = layout_node(child, text, origin, BoxConstraints::tight(size));
                bounds = bounds.union(child.layout.bounds());
                let _ = child_size;
            }
            node.layout = DisplayLayout { rect, bounds };
        }
        DisplayNodeKind::Padding { insets, child } => {
            let child_origin = Point::new(origin.x + insets.left, origin.y + insets.top);
            let child_constraints = BoxConstraints::tight(Size::new(
                (size.width - insets.x_value()).max(0.0),
                (size.height - insets.y_value()).max(0.0),
            ));
            let _ = layout_node(child, text, child_origin, child_constraints);
            node.layout = DisplayLayout {
                rect,
                bounds: rect.union(child.layout.bounds()),
            };
        }
        DisplayNodeKind::Align {
            horizontal,
            vertical,
            child,
        } => {
            let child_measured = measure_node(child, text, BoxConstraints::loose(size));
            let child_size = Size::new(
                axis_extent(*horizontal, size.width, child_measured.width),
                axis_extent(*vertical, size.height, child_measured.height),
            );
            let child_origin = Point::new(
                origin.x + axis_offset(*horizontal, size.width, child_size.width),
                origin.y + axis_offset(*vertical, size.height, child_size.height),
            );
            let _ = layout_node(child, text, child_origin, BoxConstraints::tight(child_size));
            node.layout = DisplayLayout {
                rect,
                bounds: rect.union(child.layout.bounds()),
            };
        }
        DisplayNodeKind::Offset { translation, child } => {
            let _ = layout_node(
                child,
                text,
                origin + *translation,
                BoxConstraints::tight(size),
            );
            node.layout = DisplayLayout {
                rect,
                bounds: rect.union(child.layout.bounds()),
            };
        }
        DisplayNodeKind::FixedFrame { child, .. } => {
            let _ = layout_node(child, text, origin, BoxConstraints::tight(size));
            node.layout = DisplayLayout {
                rect,
                bounds: rect.union(child.layout.bounds()),
            };
        }
        DisplayNodeKind::FillRect { .. } | DisplayNodeKind::FillRoundedRect { .. } => {
            node.layout = DisplayLayout { rect, bounds: rect };
        }
        DisplayNodeKind::StrokeRect { stroke, .. }
        | DisplayNodeKind::StrokeRoundedRect { stroke, .. } => {
            let half = stroke.width * 0.5;
            let bounds = Rect::new(
                rect.x0 - half,
                rect.y0 - half,
                rect.x1 + half,
                rect.y1 + half,
            );
            node.layout = DisplayLayout { rect, bounds };
        }
        DisplayNodeKind::Text(display_text) => {
            let max_advance = max_advance(size.width);
            display_text.ensure_shaped(text, max_advance);
            let text_bounds = union_run_bounds(&display_text.runs).unwrap_or(Rect::ZERO);
            let delta = origin - text_bounds.origin();
            for run in &mut display_text.runs {
                run.translate(delta);
            }
            node.layout = DisplayLayout {
                rect,
                bounds: union_run_bounds(&display_text.runs).unwrap_or(rect),
            };
        }
    }

    size
}

fn lower_node(node: &DisplayNode, builder: &mut DisplayListBuilder, z: &mut i32) {
    match &node.kind {
        DisplayNodeKind::Stack { children } => {
            for child in children {
                lower_node(child, builder, z);
            }
        }
        DisplayNodeKind::Padding { child, .. }
        | DisplayNodeKind::Align { child, .. }
        | DisplayNodeKind::Offset { child, .. }
        | DisplayNodeKind::FixedFrame { child, .. } => {
            lower_node(child, builder, z);
        }
        DisplayNodeKind::FillRect { brush } => {
            let _ = builder.fill_rect(node.layout.rect, brush.clone(), *z, node.semantic_id);
            *z += 1;
        }
        DisplayNodeKind::StrokeRect { stroke, brush } => {
            let _ = builder.stroke_rect(
                node.layout.rect,
                stroke.clone(),
                brush.clone(),
                *z,
                node.semantic_id,
            );
            *z += 1;
        }
        DisplayNodeKind::FillRoundedRect {
            corner_radius,
            brush,
        } => {
            let _ = builder.fill_rounded_rect(
                RoundedRect::from_rect(node.layout.rect, *corner_radius),
                brush.clone(),
                *z,
                node.semantic_id,
            );
            *z += 1;
        }
        DisplayNodeKind::StrokeRoundedRect {
            corner_radius,
            stroke,
            brush,
        } => {
            let _ = builder.stroke_rounded_rect(
                RoundedRect::from_rect(node.layout.rect, *corner_radius),
                stroke.clone(),
                brush.clone(),
                *z,
                node.semantic_id,
            );
            *z += 1;
        }
        #[cfg(feature = "std")]
        DisplayNodeKind::Text(display_text) => {
            for run in &display_text.runs {
                let _ = builder.glyph_run(run.clone(), *z, node.semantic_id);
                *z += 1;
            }
        }
    }
}

#[cfg(feature = "std")]
fn union_run_bounds(runs: &[DisplayGlyphRun]) -> Option<Rect> {
    runs.iter()
        .map(|run| run.bounds)
        .reduce(|acc, rect| acc.union(rect))
}

fn axis_extent(alignment: DisplayAlign, available: f64, child: f64) -> f64 {
    match alignment {
        DisplayAlign::Fill => available,
        DisplayAlign::Start | DisplayAlign::Center | DisplayAlign::End => child.min(available),
    }
}

fn axis_offset(alignment: DisplayAlign, available: f64, child: f64) -> f64 {
    match alignment {
        DisplayAlign::Start | DisplayAlign::Fill => 0.0,
        DisplayAlign::Center => (available - child) * 0.5,
        DisplayAlign::End => available - child,
    }
}

fn finite_width(width: f64) -> Option<f64> {
    width.is_finite().then_some(width.max(0.0))
}

#[cfg(feature = "std")]
fn max_advance(width: f64) -> Option<f32> {
    finite_width(width).map(f64_to_f32_width)
}

#[cfg(feature = "std")]
fn f64_to_f32_width(width: f64) -> f32 {
    #[allow(
        clippy::cast_possible_truncation,
        reason = "Parley max advance is f32; widths are clamped before narrowing."
    )]
    {
        width.min(f64::from(f32::MAX)) as f32
    }
}

#[cfg(feature = "std")]
#[cfg(test)]
mod tests {
    use alloc::vec;

    use super::*;
    use peniko::Color;

    #[test]
    fn text_node_measures_and_lowers() {
        let mut text = TextEngine::new();
        let mut tree = DisplayTree::new(DisplayNode::text(
            "Display",
            Brush::Solid(Color::BLACK),
            14.0,
            "sans-serif",
            Alignment::Start,
        ));
        tree.layout(
            &mut text,
            Point::new(10.0, 20.0),
            BoxConstraints::loose(Size::new(200.0, 50.0)),
        );

        let list = tree.to_display_list();
        assert!(!list.is_empty());
        assert!(matches!(
            &list.items()[0].op,
            crate::DisplayOp::GlyphRun { .. }
        ));
    }

    #[test]
    fn fixed_frame_stack_places_text_inside_frame() {
        let mut text = TextEngine::new();
        let mut tree = DisplayTree::new(DisplayNode::fixed_frame(
            Size::new(160.0, 48.0),
            DisplayNode::stack(vec![
                DisplayNode::fill_rounded_rect(10.0, Brush::Solid(Color::from_rgb8(240, 240, 240))),
                DisplayNode::align(
                    DisplayAlign::Start,
                    DisplayAlign::Center,
                    DisplayNode::padding(
                        Insets::symmetric(16.0, 0.0),
                        DisplayNode::text(
                            "Button",
                            Brush::Solid(Color::BLACK),
                            14.0,
                            "sans-serif",
                            Alignment::Start,
                        ),
                    ),
                ),
            ]),
        ));

        tree.layout(
            &mut text,
            Point::new(0.0, 0.0),
            BoxConstraints::loose(Size::new(160.0, 48.0)),
        );

        let list = tree.to_display_list();
        assert_eq!(list.len(), 2);
        assert_eq!(list.items()[0].bounds, Rect::new(0.0, 0.0, 160.0, 48.0));
    }
}
