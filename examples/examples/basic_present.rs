// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Build and mutate a tiny presentation store.
//!
//! This example keeps geometry out of the picture on purpose. A toolkit would
//! usually use box-tree node ids as presentation keys and then paint by walking
//! the box tree in z-order.
//!
//! Run:
//! - `cargo run -p understory_examples --example basic_present`

use understory_presentation::{
    Border, Brush, Color, FontWeight, PresentationStore, Primitive, RoundedRectRadii, TextAlign,
    TextContent,
};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct GeometryNode(u32);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Widget(&'static str);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Role {
    Background,
    Content,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Source {
    widget: Widget,
    role: Role,
}

fn main() {
    let button = Widget("button");
    let button_background = GeometryNode(1);
    let button_label = GeometryNode(2);
    let geometry_order = [button_background, button_label];

    let mut presentation: PresentationStore<GeometryNode, Source> = PresentationStore::new();
    presentation.insert(
        button_background,
        Source {
            widget: button,
            role: Role::Background,
        },
    );
    presentation.insert(
        button_label,
        Source {
            widget: button,
            role: Role::Content,
        },
    );

    let surface = presentation
        .surface_mut(button_background)
        .expect("background part was inserted");
    surface.set_background(Color::from_rgb8(38, 92, 142));
    surface.border = Border::uniform(Color::from_rgb8(15, 39, 64), 1.0);
    surface.corner_radii = RoundedRectRadii::from_single_radius(6.0);

    let text = presentation
        .plain_text_mut(button_label)
        .expect("content part was inserted");
    text.content = TextContent::plain("Run");
    text.foreground = Some(Brush::from(Color::WHITE));
    text.style.font_size = 15.0;
    text.style.font_weight = FontWeight::MEDIUM;
    text.layout.align = TextAlign::Center;

    println!("== initial present pass ==");
    print_dirty(&mut presentation);
    print_paint_walk(&geometry_order, &presentation);

    let surface = presentation
        .surface_mut(button_background)
        .expect("background part was inserted");
    surface.set_background(Color::from_rgb8(29, 122, 91));

    println!();
    println!("== property/style change ==");
    print_dirty(&mut presentation);
    print_paint_walk(&geometry_order, &presentation);

    presentation.remove(button_label);

    println!();
    println!("== template teardown ==");
    print_dirty(&mut presentation);
    print_paint_walk(&geometry_order, &presentation);
}

fn print_dirty(presentation: &mut PresentationStore<GeometryNode, Source>) {
    let dirty: Vec<_> = presentation.take_dirty().collect();
    println!("dirty keys: {dirty:?}");
}

fn print_paint_walk(
    geometry_order: &[GeometryNode],
    presentation: &PresentationStore<GeometryNode, Source>,
) {
    for key in geometry_order {
        let Some(node) = presentation.node(*key) else {
            continue;
        };

        let source = node.source();
        println!(
            "paint {:?}: widget={} role={:?}",
            key, source.widget.0, source.role
        );
        for primitive in node.primitives() {
            match primitive {
                Primitive::Surface(surface) => println!(
                    "  surface backgrounds={} border_empty={} radii={:?}",
                    surface.backgrounds.len(),
                    surface.border.is_empty(),
                    surface.corner_radii
                ),
                Primitive::Text(text) => {
                    let Some(text) = text.as_plain() else {
                        continue;
                    };
                    println!(
                        "  text content={:?} foreground={} font_size={} weight={} align={:?}",
                        text.content.as_str(),
                        text.foreground.is_some(),
                        text.style.font_size,
                        text.style.font_weight.value(),
                        text.layout.align
                    );
                }
                _ => println!("  unknown primitive"),
            }
        }
    }
}
