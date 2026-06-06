// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Resolve dependency properties into a presentation surface.
//!
//! Run:
//! - `cargo run -p understory_examples --example presentation_properties`

use invalidation::Channel;
use kurbo::Rect;
use understory_presentation::{Brush, Color, CornerShape, Edges, PresentationStore};
use understory_presentation_properties::{
    CornerRadius, StyleMatch, SurfaceProperties, SurfacePropertyChannels,
};
use understory_property::{DependencyObject, PropertyRegistry, PropertyStore};
use understory_style::{
    NoResolveParentLookup, ResolveCx, StyleBuilder, StyleCascadeBuilder, StyleOrigin, ThemeBuilder,
};

const GEOMETRY: Channel = Channel::new(0);
const PAINT: Channel = Channel::new(1);

#[derive(Debug)]
struct Widget {
    id: u32,
    store: PropertyStore<u32>,
}

impl Widget {
    fn new(id: u32) -> Self {
        Self {
            id,
            store: PropertyStore::new(id),
        }
    }
}

impl DependencyObject<u32> for Widget {
    fn property_store(&self) -> &PropertyStore<u32> {
        &self.store
    }

    fn property_store_mut(&mut self) -> &mut PropertyStore<u32> {
        &mut self.store
    }

    fn key(&self) -> u32 {
        self.id
    }

    fn parent_key(&self) -> Option<u32> {
        None
    }
}

fn main() {
    let mut registry = PropertyRegistry::new();
    let surface = SurfaceProperties::register(
        &mut registry,
        SurfacePropertyChannels::new(GEOMETRY.into_set(), PAINT.into_set()),
    );

    let button_style = StyleBuilder::new()
        .set(surface.background, Some(Brush::from(Color::WHITE)))
        .set(surface.border_widths.top, 2.0)
        .set(surface.border_widths.right, 2.0)
        .set(surface.border_widths.bottom, 3.0)
        .set(surface.border_widths.left, 2.0)
        .set(surface.border_brushes.top, Some(Brush::from(Color::BLACK)))
        .set(
            surface.border_brushes.right,
            Some(Brush::from(Color::BLACK)),
        )
        .set(
            surface.border_brushes.bottom,
            Some(Brush::from(Color::BLACK)),
        )
        .set(surface.border_brushes.left, Some(Brush::from(Color::BLACK)))
        .set(surface.padding_widths.top, 6.0)
        .set(surface.padding_widths.right, 10.0)
        .set(surface.padding_widths.bottom, 6.0)
        .set(surface.padding_widths.left, 10.0)
        .set(surface.corner_radii.top_left, CornerRadius::circular(8.0))
        .set(surface.corner_radii.top_right, CornerRadius::circular(8.0))
        .set(
            surface.corner_radii.bottom_right,
            CornerRadius::new(12.0, 6.0),
        )
        .set(
            surface.corner_radii.bottom_left,
            CornerRadius::new(12.0, 6.0),
        )
        .set(surface.corner_shapes.top_left, CornerShape::squircle())
        .set(surface.corner_shapes.top_right, CornerShape::squircle())
        .build();
    let cascade = StyleCascadeBuilder::new()
        .push_style(StyleOrigin::Base, button_style)
        .build();

    let theme = ThemeBuilder::new().build();
    let cx = ResolveCx::new(&registry, &theme, NoResolveParentLookup);
    let button = Widget::new(42);

    let resolved_surface = surface.resolve_surface(
        &cx,
        &button,
        Some(StyleMatch::new(&cascade, cascade.root_state())),
    );
    let geometry = resolved_surface.decoration_geometry(Rect::new(0.0, 0.0, 120.0, 44.0));

    let mut presentation: PresentationStore<u32, u32> = PresentationStore::new();
    presentation.insert(1, button.key());
    *presentation
        .surface_mut(1)
        .expect("presentation node was inserted") = resolved_surface;

    let dirty: Vec<_> = presentation.take_dirty().collect();
    println!("dirty presentation nodes: {dirty:?}");
    println!(
        "visible border widths: {:?}",
        presentation
            .node(1)
            .and_then(|node| node.surface())
            .map(|surface| surface.border.visible_widths())
            .unwrap_or(Edges::ZERO)
    );
    println!(
        "padding box after border widths: {:?}",
        geometry.padding_box
    );
    println!(
        "content box after padding widths: {:?}",
        geometry.content_box
    );
    println!("fitted border contour: {:?}", geometry.border_edge);
}
