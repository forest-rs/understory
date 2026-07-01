// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

// After you edit the crate's doc comment, run this command, then check README.md for any missing links
// cargo rdme --workspace-project=understory_presentation_properties --heading-base-level=0

//! Dependency-property integration for resolved presentation primitives.
//!
//! `understory_presentation_properties` registers canonical dependency
//! properties for drawing surfaces and resolves them into
//! [`understory_presentation`] primitives. It is the adapter between
//! property/style resolution and the renderer-neutral presentation tree.
//!
//! The crate deliberately does **not** own property storage, selector matching,
//! theme resources, layout bounds, hit testing, CSS parsing, or renderer command
//! emission. Those responsibilities stay in `understory_property`,
//! `understory_style`, layout/hit crates, and backend lowerers.
//!
//! ## Why this crate exists
//!
//! Surface decoration spans several concerns:
//!
//! - property systems need typed longhands with defaults and invalidation
//!   metadata;
//! - style systems need independently-overridable values such as one border
//!   side's brush/style/width, one padding side, one corner radius, or one
//!   corner shape;
//! - presentation needs a resolved [`understory_presentation::SurfacePrimitive`]
//!   that can be cached in a paint tree;
//! - renderers need geometry helpers such as
//!   [`understory_presentation::SurfacePrimitive::decoration_geometry`] when
//!   final bounds are known.
//!
//! This crate owns the property-to-presentation step so callers do not need to
//! invent parallel property names, defaults, or invalidation policy.
//!
//! ## Surface pipeline
//!
//! ```text
//! understory_property + understory_style
//!         |
//!         v
//! SurfaceProperties::resolve_surface
//!         |
//!         v
//! understory_presentation::SurfacePrimitive
//!         |
//!         v
//! understory_box_decoration geometry helpers
//! ```
//!
//! ## Feature flags
//!
//! - `default`: enables `libm` so the crate builds as `no_std` by default.
//! - `libm`: forwards `kurbo/libm` and `understory_presentation/libm`.
//! - `std`: forwards `kurbo/std` and `understory_presentation/std`.
//!
//! If default features are disabled, callers must enable either `libm` or
//! `std`.
//!
//! ## Minimal example
//!
//! ```rust
//! use invalidation::Channel;
//! use understory_presentation::{BorderStyle, Brush, Color, CornerShape, Edges};
//! use understory_presentation_properties::{
//!     CornerRadius, StyleMatch, SurfacePropertyChannels, SurfaceProperties,
//! };
//! use understory_property::{DependencyObject, PropertyRegistry, PropertyStore};
//! use understory_style::{
//!     NoResolveParentLookup, ResolveCx, StyleBuilder, StyleCascadeBuilder,
//!     StyleOrigin, ThemeBuilder,
//! };
//!
//! const GEOMETRY: Channel = Channel::new(0);
//! const PAINT: Channel = Channel::new(1);
//!
//! let mut registry = PropertyRegistry::new();
//! let surface = SurfaceProperties::register(
//!     &mut registry,
//!     SurfacePropertyChannels::new(GEOMETRY.into_set(), PAINT.into_set()),
//! );
//!
//! struct Element {
//!     store: PropertyStore<u32>,
//! }
//!
//! impl DependencyObject<u32> for Element {
//!     fn property_store(&self) -> &PropertyStore<u32> { &self.store }
//!     fn property_store_mut(&mut self) -> &mut PropertyStore<u32> { &mut self.store }
//!     fn key(&self) -> u32 { self.store.owner() }
//!     fn parent_key(&self) -> Option<u32> { None }
//! }
//!
//! let element = Element { store: PropertyStore::new(1) };
//! let style = StyleBuilder::new()
//!     .set(surface.background, Some(Brush::from(Color::WHITE)))
//!     .set(surface.border_styles.top, BorderStyle::Solid)
//!     .set(surface.border_widths.top, 2.0)
//!     .set(surface.border_brushes.top, Some(Brush::from(Color::BLACK)))
//!     .set(surface.padding_widths.top, 6.0)
//!     .set(surface.corner_radii.top_left, CornerRadius::circular(6.0))
//!     .set(surface.corner_shapes.top_left, CornerShape::squircle())
//!     .build();
//! let cascade = StyleCascadeBuilder::new()
//!     .push_style(StyleOrigin::Base, style)
//!     .build();
//! let theme = ThemeBuilder::new().build();
//! let cx = ResolveCx::new(&registry, &theme, NoResolveParentLookup);
//!
//! let primitive = surface.resolve_surface(
//!     &cx,
//!     &element,
//!     Some(StyleMatch::new(&cascade, cascade.root_state())),
//! );
//!
//! assert_eq!(primitive.backgrounds.len(), 1);
//! assert_eq!(primitive.border.styles().top, BorderStyle::Solid);
//! assert_eq!(primitive.border.visible_widths(), Edges::new(2.0, 0.0, 0.0, 0.0));
//! assert_eq!(primitive.padding_widths.top, 6.0);
//! assert_eq!(primitive.corner_radii.top_left.width, 6.0);
//! assert_eq!(primitive.corner_shapes.top_left, CornerShape::squircle());
//! ```
//!
//! ## Roadmap
//!
//! The first slice covers resolved surface background, per-side border brushes,
//! styles, and widths, physical padding widths, per-corner elliptical radii,
//! and per-corner shapes. Future work should add length-percentage values,
//! `border-shape`, richer background layers, shadow properties, and
//! shape-related properties once a dedicated shape value crate exists. Those
//! additions should keep the same pattern: properties are longhand, resolution
//! produces presentation primitives, and geometry remains in
//! `understory_box_decoration`.

#![no_std]

mod surface;

pub use surface::{
    CornerRadius, StyleMatch, SurfaceProperties, SurfacePropertyChannels, SurfacePropertyValues,
};
