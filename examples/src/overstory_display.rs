// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Lowering helpers between Overstory, Understory Display, and Imaging.

use imaging::{
    Composite, Painter,
    record::{self, Glyph},
};
use kurbo::{Affine, Point, Stroke, Vec2};
use overstory::ResolvedElement;
use peniko::{BlendMode, Brush};
use understory_display::{
    BoxConstraints, DisplayAlign, DisplayEntry, DisplayList, DisplayNode, DisplayOp, DisplayTree,
    Insets, TextEngine, parley::Alignment,
};

/// Stateful Overstory -> display-list lowerer with reusable text shaping state.
#[derive(Clone, Default)]
pub struct OverstoryDisplayLowerer {
    text: TextEngine,
}

#[derive(Debug)]
struct ElementDisplayTree<'a> {
    element: &'a ResolvedElement,
    children: Vec<ElementDisplayTree<'a>>,
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
        let mut index = 0;
        let display_root = build_element_tree(snapshot.resolved(), &mut index)
            .expect("scene snapshot should contain a root resolved element");
        let mut tree = DisplayTree::new(Self::display_node_for(root_origin, &display_root));
        tree.layout(
            &mut self.text,
            root_origin,
            BoxConstraints::tight(snapshot.view_rect().size()),
        );
        tree.to_display_list()
    }

    fn display_node_for(parent_origin: Point, node: &ElementDisplayTree<'_>) -> DisplayNode {
        let element = node.element;
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

        children.extend(
            node.children
                .iter()
                .map(|child| Self::display_node_for(element.rect.origin(), child)),
        );

        DisplayNode::offset(
            Vec2::new(
                element.rect.x0 - parent_origin.x,
                element.rect.y0 - parent_origin.y,
            ),
            DisplayNode::fixed_frame(size, DisplayNode::stack(children)),
        )
    }
}

fn build_element_tree<'a>(
    resolved: &'a [ResolvedElement],
    index: &mut usize,
) -> Option<ElementDisplayTree<'a>> {
    let element = resolved.get(*index)?;
    let depth = element.depth;
    *index += 1;

    let mut children = Vec::new();
    while let Some(next) = resolved.get(*index) {
        if next.depth <= depth {
            break;
        }
        if next.depth != depth + 1 {
            panic!("resolved scene depth should advance one level at a time");
        }
        let child = build_element_tree(resolved, index).expect("child subtree should parse");
        children.push(child);
    }

    Some(ElementDisplayTree { element, children })
}

/// Lower one retained display list into an imaging recording.
#[must_use]
pub fn imaging_scene_from_display(list: &DisplayList) -> record::Scene {
    let mut scene = record::Scene::new();
    {
        let mut painter = Painter::new(&mut scene);
        let mut index = 0;
        record_entries(&mut painter, list.entries(), &mut index, Affine::IDENTITY);
    }
    scene
}

fn record_entries(
    painter: &mut Painter<'_, record::Scene>,
    entries: &[DisplayEntry],
    index: &mut usize,
    transform: Affine,
) {
    while *index < entries.len() {
        match &entries[*index] {
            DisplayEntry::Item(item) => {
                record_item(painter, item, transform);
                *index += 1;
            }
            DisplayEntry::PushClipRect(clip) => {
                *index += 1;
                painter.with_fill_clip_transformed(clip.rect, transform, |painter| {
                    record_entries(painter, entries, index, transform);
                });
            }
            DisplayEntry::PopClip => {
                *index += 1;
                return;
            }
            DisplayEntry::PushOpacity(opacity) => {
                *index += 1;
                painter.with_group(
                    imaging::GroupRef::new()
                        .with_composite(Composite::new(BlendMode::default(), opacity.opacity)),
                    |painter| {
                        record_entries(painter, entries, index, transform);
                    },
                );
            }
            DisplayEntry::PopOpacity => {
                *index += 1;
                return;
            }
            DisplayEntry::PushTransform(scope) => {
                *index += 1;
                record_entries(painter, entries, index, transform * scope.transform);
            }
            DisplayEntry::PopTransform => {
                *index += 1;
                return;
            }
        }
    }
}

fn record_item(
    painter: &mut Painter<'_, record::Scene>,
    item: &understory_display::DisplayItem,
    transform: Affine,
) {
    match &item.op {
        DisplayOp::FillRect { rect, brush } => {
            painter.fill(*rect, brush).transform(transform).draw();
        }
        DisplayOp::StrokeRect {
            rect,
            stroke,
            brush,
        } => {
            painter
                .stroke(*rect, stroke, brush)
                .transform(transform)
                .draw();
        }
        DisplayOp::FillRoundedRect { rect, brush } => {
            painter.fill(*rect, brush).transform(transform).draw();
        }
        DisplayOp::StrokeRoundedRect {
            rect,
            stroke,
            brush,
        } => {
            painter
                .stroke(*rect, stroke, brush)
                .transform(transform)
                .draw();
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
                .transform(transform)
                .font_size(run.font_size)
                .normalized_coords(&run.normalized_coords)
                .draw(&peniko::Style::Fill(peniko::Fill::NonZero), &glyphs);
        }
    }
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
            .filter_map(|item| match &item.op {
                DisplayOp::GlyphRun { run } => Some(run.glyphs.len()),
                _ => None,
            })
            .sum();
        assert!(glyph_count > 0, "expected at least one positioned glyph");
    }

    #[test]
    fn imaging_lowering_handles_structural_entries() {
        let mut text = TextEngine::new();
        let mut tree = DisplayTree::new(DisplayNode::clip_rect(DisplayNode::opacity(
            0.75,
            DisplayNode::transform(
                Affine::translate((8.0, 6.0)),
                DisplayNode::fixed_frame(
                    kurbo::Size::new(120.0, 40.0),
                    DisplayNode::stack(vec![
                        DisplayNode::fill_rounded_rect(
                            8.0,
                            Brush::Solid(Color::from_rgb8(240, 240, 240)),
                        ),
                        DisplayNode::align(
                            DisplayAlign::Start,
                            DisplayAlign::Center,
                            DisplayNode::padding(
                                Insets::symmetric(16.0, 0.0),
                                DisplayNode::text(
                                    "Layered",
                                    Brush::Solid(Color::BLACK),
                                    21.0,
                                    "sans-serif",
                                    Alignment::Start,
                                ),
                            ),
                        ),
                    ]),
                ),
            ),
        )));
        tree.layout(
            &mut text,
            Point::ORIGIN,
            BoxConstraints::tight(kurbo::Size::new(120.0, 40.0)),
        );

        let list = tree.to_display_list();
        let scene = imaging_scene_from_display(&list);
        assert!(
            !scene.commands().is_empty(),
            "expected retained imaging commands"
        );
    }

    #[test]
    fn overstory_lowering_preserves_parent_child_structure() {
        let mut ui = overstory::Ui::new(overstory::default_theme());
        ui.set_view_rect(kurbo::Rect::new(0.0, 0.0, 220.0, 140.0));
        ui.set_local(ui.root(), ui.properties().padding, 0.0);
        ui.set_local(ui.root(), ui.properties().gap, 10.0);

        let shell = ui.append_child(ui.root(), overstory::ElementKind::Row);
        ui.set_local(shell, ui.properties().padding, 0.0);
        ui.set_local(shell, ui.properties().gap, 10.0);

        let left = ui.append_child(shell, overstory::ElementKind::Panel);
        ui.set_local(left, ui.properties().width, 80.0);
        ui.set_local(left, ui.properties().padding, 8.0);
        ui.set_local(left, ui.properties().background, Color::from_rgb8(1, 2, 3));

        let child = ui.append_child(left, overstory::ElementKind::Button);
        ui.set_label(child, "Child");
        ui.set_local(child, ui.properties().height, 28.0);
        ui.set_local(
            child,
            ui.properties().background,
            Color::from_rgb8(10, 20, 30),
        );

        let right = ui.append_child(shell, overstory::ElementKind::Panel);
        ui.set_local(right, ui.properties().padding, 8.0);
        ui.set_local(
            right,
            ui.properties().background,
            Color::from_rgb8(200, 210, 220),
        );

        let root = ui.root();
        let snapshot = ui.scene();
        let mut index = 0;
        let display_root = build_element_tree(snapshot.resolved(), &mut index).expect("root tree");

        assert_eq!(display_root.element.id, root);
        assert_eq!(
            display_root.children.len(),
            1,
            "root should own the shell row"
        );
        let shell_tree = &display_root.children[0];
        assert_eq!(
            shell_tree.children.len(),
            2,
            "shell row should have two panels"
        );
        assert_eq!(
            shell_tree.children[0].element.id, left,
            "left panel should stay inside the shell subtree"
        );
        assert_eq!(
            shell_tree.children[1].element.id, right,
            "right panel should stay a sibling of the left panel"
        );
        assert_eq!(
            shell_tree.children[0].children.len(),
            1,
            "left panel should own its button child"
        );
        assert_eq!(
            shell_tree.children[0].children[0].element.id, child,
            "button should remain nested under the left panel"
        );
    }
}
