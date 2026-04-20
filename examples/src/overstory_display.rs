// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Lowering helpers between Overstory, Understory Display, and Imaging.

use imaging::{
    Painter,
    record::{self, Glyph},
};
use kurbo::{Affine, Point, Stroke, Vec2};
use overstory::ResolvedElement;
use peniko::Brush;
use understory_display::{
    BoxConstraints, DisplayAlign, DisplayList, DisplayNode, DisplayOp, DisplayTree, Insets,
    TextEngine, parley::Alignment,
};

/// Stateful Overstory -> display-list lowerer with reusable text shaping state.
#[derive(Clone, Default)]
pub struct OverstoryDisplayLowerer {
    text: TextEngine,
}

impl OverstoryDisplayLowerer {
    /// Creates a new lowerer.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Lowers one resolved Overstory snapshot into a retained display list.
    #[must_use]
    pub fn display_list_from_overstory(
        &mut self,
        snapshot: &overstory::SceneSnapshot,
    ) -> DisplayList {
        let root_origin = Point::new(snapshot.view_rect().x0, snapshot.view_rect().y0);
        let mut tree = DisplayTree::new(DisplayNode::fixed_frame(
            snapshot.view_rect().size(),
            DisplayNode::stack(
                snapshot
                    .resolved()
                    .iter()
                    .map(|element| Self::display_node_for(root_origin, element))
                    .collect(),
            ),
        ));
        tree.layout(
            &mut self.text,
            root_origin,
            BoxConstraints::tight(snapshot.view_rect().size()),
        );
        tree.to_display_list()
    }

    fn display_node_for(root_origin: Point, element: &ResolvedElement) -> DisplayNode {
        let mut children = Vec::new();
        let size = element.rect.size();

        if element.background.to_rgba8().a != 0 {
            let background = Brush::Solid(element.background);
            children.push(if element.corner_radius > 0.0 {
                DisplayNode::fill_rounded_rect(element.corner_radius, background)
            } else {
                DisplayNode::fill_rect(background)
            });
        }

        if element.border.width > 0.0 && element.border.color.to_rgba8().a != 0 {
            let border = Brush::Solid(element.border.color);
            let stroke = Stroke::new(element.border.width);
            children.push(if element.corner_radius > 0.0 {
                DisplayNode::stroke_rounded_rect(element.corner_radius, stroke, border)
            } else {
                DisplayNode::stroke_rect(stroke, border)
            });
        }

        if let Some(label) = element.label.as_deref() {
            children.push(DisplayNode::align(
                DisplayAlign::Start,
                DisplayAlign::Center,
                DisplayNode::padding(
                    Insets::symmetric(16.0, 0.0),
                    DisplayNode::text(
                        label,
                        Brush::Solid(element.foreground),
                        21.0,
                        "sans-serif",
                        Alignment::Start,
                    ),
                ),
            ));
        }

        DisplayNode::offset(
            Vec2::new(
                element.rect.x0 - root_origin.x,
                element.rect.y0 - root_origin.y,
            ),
            DisplayNode::fixed_frame(size, DisplayNode::stack(children)),
        )
    }
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
                DisplayOp::GlyphRun { run } => {
                    let glyphs = run
                        .glyphs
                        .iter()
                        .map(|glyph| Glyph {
                            id: glyph.id,
                            x: glyph.origin.x as f32,
                            y: glyph.origin.y as f32,
                        })
                        .collect::<Vec<_>>();
                    painter
                        .glyphs(&run.font, &run.brush)
                        .transform(Affine::IDENTITY)
                        .font_size(run.font_size)
                        .normalized_coords(&run.normalized_coords)
                        .draw(&peniko::Style::Fill(peniko::Fill::NonZero), &glyphs);
                }
            }
        }
    }
    scene
}

#[cfg(test)]
mod tests {
    use super::*;
    use peniko::Color;

    #[test]
    fn display_tree_text_lowering_produces_positioned_glyphs() {
        let mut text = TextEngine::new();
        let mut tree = DisplayTree::new(DisplayNode::fixed_frame(
            kurbo::Size::new(160.0, 48.0),
            DisplayNode::stack(vec![
                DisplayNode::fill_rounded_rect(10.0, Brush::Solid(Color::from_rgb8(240, 240, 240))),
                DisplayNode::align(
                    DisplayAlign::Start,
                    DisplayAlign::Center,
                    DisplayNode::padding(
                        Insets::symmetric(16.0, 0.0),
                        DisplayNode::text(
                            "Overstory",
                            Brush::Solid(Color::BLACK),
                            21.0,
                            "sans-serif",
                            Alignment::Start,
                        ),
                    ),
                ),
            ]),
        ));
        tree.layout(
            &mut text,
            Point::ORIGIN,
            BoxConstraints::tight(kurbo::Size::new(160.0, 48.0)),
        );

        let list = tree.to_display_list();
        let glyph_count: usize = list
            .items()
            .iter()
            .filter_map(|item| match &item.op {
                DisplayOp::GlyphRun { run } => Some(run.glyphs.len()),
                _ => None,
            })
            .sum();
        assert!(glyph_count > 0, "expected at least one positioned glyph");
    }
}
