// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Lowering helpers between Overstory, Understory Display, and Imaging.

use alloc::vec::Vec;

use imaging::{Painter, record};
use kurbo::{Rect, RoundedRect};
use overstory::ResolvedElement;
use peniko::Brush;
use understory_display::{DisplayList, DisplayListBuilder, DisplayOp};

extern crate alloc;

/// Lower one resolved Overstory snapshot into a retained display list.
#[must_use]
pub fn display_list_from_overstory(snapshot: &overstory::SceneSnapshot) -> DisplayList {
    let mut builder = DisplayListBuilder::new();
    for (z, element) in snapshot.resolved().iter().enumerate() {
        append_resolved_element(&mut builder, element, z as i32);
    }
    builder.build()
}

/// Lower one retained display list into an imaging recording.
#[must_use]
pub fn imaging_scene_from_display(list: &DisplayList) -> record::Scene {
    let mut scene = record::Scene::new();
    {
        let mut painter = Painter::new(&mut scene);
        for item in list.items() {
            match &item.op {
                DisplayOp::FillRect { rect, brush } => {
                    painter.fill_rect(*rect, brush);
                }
                DisplayOp::StrokeRect {
                    rect,
                    stroke,
                    brush,
                } => {
                    painter.stroke(*rect, stroke, brush).draw();
                }
                DisplayOp::FillRoundedRect { rect, brush } => {
                    painter.fill(*rect, brush).draw();
                }
                DisplayOp::StrokeRoundedRect {
                    rect,
                    stroke,
                    brush,
                } => {
                    painter.stroke(*rect, stroke, brush).draw();
                }
            }
        }
    }
    scene
}

fn append_resolved_element(builder: &mut DisplayListBuilder, element: &ResolvedElement, z: i32) {
    let background = Brush::Solid(element.background);
    if element.background.to_rgba8().a != 0 {
        if element.corner_radius > 0.0 {
            let _ = builder.fill_rounded_rect(
                RoundedRect::from_rect(element.rect, element.corner_radius),
                background,
                z,
                None,
            );
        } else {
            let _ = builder.fill_rect(element.rect, background, z, None);
        }
    }

    if element.border.width > 0.0 && element.border.color.to_rgba8().a != 0 {
        let border = Brush::Solid(element.border.color);
        let stroke = kurbo::Stroke::new(element.border.width);
        if element.corner_radius > 0.0 {
            let _ = builder.stroke_rounded_rect(
                RoundedRect::from_rect(element.rect, element.corner_radius),
                stroke,
                border,
                z,
                None,
            );
        } else {
            let _ = builder.stroke_rect(element.rect, stroke, border, z, None);
        }
    }

    if element.label.is_some() {
        let foreground = Brush::Solid(element.foreground);
        for placeholder in label_placeholders(element) {
            let _ = builder.fill_rounded_rect(
                RoundedRect::from_rect(placeholder, placeholder.height() * 0.5),
                foreground.clone(),
                z,
                None,
            );
        }
    }
}

fn label_placeholders(element: &ResolvedElement) -> Vec<Rect> {
    let Some(label) = element.label.as_deref() else {
        return Vec::new();
    };
    let inset_x = 16.0;
    let base_y = element.rect.y0 + element.rect.height() * 0.5 - 4.0;
    let max_width = (element.rect.width() - inset_x * 2.0).max(24.0);
    let label_width = (f64::from(label.len() as u32) * 7.0).min(max_width);
    vec![Rect::new(
        element.rect.x0 + inset_x,
        base_y,
        element.rect.x0 + inset_x + label_width,
        base_y + 8.0,
    )]
}
