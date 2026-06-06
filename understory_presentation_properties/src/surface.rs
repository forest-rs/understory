// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use invalidation::ChannelSet;
use kurbo::Size;
use understory_presentation::{
    BackgroundLayer, Border, BorderSide, Brush, CornerRadii, CornerShape, CornerShapes, Corners,
    Edges, SurfacePrimitive,
};
use understory_property::{DependencyObject, Property, PropertyMetadataBuilder, PropertyRegistry};
use understory_style::{MatchState, ResolveCx, ResolveParentLookup, StyleCascade};

/// Style cascade state passed to property resolution.
///
/// The [`StyleCascade`] and [`MatchState`] pair is produced by
/// `understory_style` while walking the caller's subject tree. Keep this value
/// with the object or part it was produced for, then pass `Some(style_match)`
/// to [`SurfaceProperties::resolve_surface`].
#[derive(Copy, Clone, Debug)]
pub struct StyleMatch<'a> {
    cascade: &'a StyleCascade,
    state: MatchState,
}

impl<'a> StyleMatch<'a> {
    /// Creates a style match from a cascade and a match state produced by that
    /// cascade.
    ///
    /// `MatchState` handles are scoped to the [`StyleCascade`] that created
    /// them. Passing a state from another cascade is a logic error enforced by
    /// `understory_style` debug checks.
    #[must_use]
    pub const fn new(cascade: &'a StyleCascade, state: MatchState) -> Self {
        Self { cascade, state }
    }

    /// Returns the style cascade.
    #[must_use]
    pub const fn cascade(self) -> &'a StyleCascade {
        self.cascade
    }

    /// Returns the match state produced by [`StyleMatch::cascade`].
    #[must_use]
    pub const fn state(self) -> MatchState {
        self.state
    }

    fn as_resolve_pair(self) -> (&'a StyleCascade, MatchState) {
        (self.cascade, self.state)
    }
}

impl<'a> From<(&'a StyleCascade, MatchState)> for StyleMatch<'a> {
    fn from((cascade, state): (&'a StyleCascade, MatchState)) -> Self {
        Self::new(cascade, state)
    }
}

/// Invalidation channels used by the canonical surface properties.
///
/// Brush-only properties affect paint. Shape properties such as border widths,
/// padding widths, corner radii, and corner shapes affect both `geometry` and
/// `paint`, because they can change clipping, hit regions, border paths, and
/// the pixels that are drawn.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct SurfacePropertyChannels {
    /// Channels affected when the surface shape or decoration geometry changes.
    pub geometry: ChannelSet,
    /// Channels affected when only painted pixels need to be refreshed.
    pub paint: ChannelSet,
}

impl SurfacePropertyChannels {
    /// Creates surface property channel metadata.
    #[must_use]
    pub const fn new(geometry: ChannelSet, paint: ChannelSet) -> Self {
        Self { geometry, paint }
    }

    /// Creates metadata where every surface property affects the same channels.
    #[must_use]
    pub const fn all(channels: ChannelSet) -> Self {
        Self {
            geometry: channels,
            paint: channels,
        }
    }

    const fn paint(self) -> ChannelSet {
        self.paint
    }

    fn geometry_and_paint(self) -> ChannelSet {
        self.geometry | self.paint
    }
}

/// Elliptical radius value for one corner.
///
/// This is the property-level longhand value. It stores the horizontal and
/// vertical radius before box-size-dependent fitting. Resolution combines four
/// corner values into [`CornerRadii`], and `understory_box_decoration` scales
/// those radii once a final border box is known.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct CornerRadius {
    /// Horizontal radius in logical pixels.
    pub horizontal: f64,
    /// Vertical radius in logical pixels.
    pub vertical: f64,
}

impl CornerRadius {
    /// A zero radius.
    pub const ZERO: Self = Self::new(0.0, 0.0);

    /// Creates an elliptical corner radius.
    #[must_use]
    pub const fn new(horizontal: f64, vertical: f64) -> Self {
        Self {
            horizontal,
            vertical,
        }
    }

    /// Creates a circular corner radius.
    #[must_use]
    pub const fn circular(radius: f64) -> Self {
        Self::new(radius, radius)
    }

    /// Returns this radius with negative and non-finite components set to zero.
    #[must_use]
    pub fn clamped_non_negative(self) -> Self {
        Self::new(
            finite_non_negative(self.horizontal),
            finite_non_negative(self.vertical),
        )
    }

    fn to_size(self) -> Size {
        let radius = self.clamped_non_negative();
        Size::new(radius.horizontal, radius.vertical)
    }
}

impl From<f64> for CornerRadius {
    fn from(radius: f64) -> Self {
        Self::circular(radius)
    }
}

impl From<Size> for CornerRadius {
    fn from(radius: Size) -> Self {
        Self::new(radius.width, radius.height)
    }
}

/// Registered dependency properties for a surface primitive.
///
/// Register this once in the caller's [`PropertyRegistry`], then share the
/// returned handles anywhere a widget, template part, or style rule needs to
/// write surface decoration values.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct SurfaceProperties {
    /// Optional background brush for the first background layer.
    pub background: Property<Option<Brush>>,
    /// Per-side border brush properties.
    pub border_brushes: Edges<Property<Option<Brush>>>,
    /// Per-side border width properties.
    pub border_widths: Edges<Property<f64>>,
    /// Physical padding width properties.
    ///
    /// These should resolve to the same physical padding values a toolkit uses
    /// for layout content insets when the same surface participates in layout.
    /// Keeping separate layout and decoration padding sources can make
    /// [`SurfacePrimitive::decoration_geometry`] disagree with actual content
    /// placement.
    pub padding_widths: Edges<Property<f64>>,
    /// Per-corner radius properties.
    pub corner_radii: Corners<Property<CornerRadius>>,
    /// Per-corner shape properties.
    pub corner_shapes: Corners<Property<CornerShape>>,
}

impl SurfaceProperties {
    /// Registers canonical surface properties in `registry`.
    ///
    /// This uses stable property names prefixed with `Surface.`. Register the
    /// bundle once per registry; registering it twice in the same registry will
    /// panic through [`PropertyRegistry::register`]'s duplicate-name check.
    #[must_use]
    pub fn register(registry: &mut PropertyRegistry, channels: SurfacePropertyChannels) -> Self {
        let paint = channels.paint();
        let geometry_and_paint = channels.geometry_and_paint();

        Self {
            background: register_optional_brush(registry, "Surface.Background", paint),
            border_brushes: Edges::new(
                register_optional_brush(registry, "Surface.BorderTopBrush", paint),
                register_optional_brush(registry, "Surface.BorderRightBrush", paint),
                register_optional_brush(registry, "Surface.BorderBottomBrush", paint),
                register_optional_brush(registry, "Surface.BorderLeftBrush", paint),
            ),
            border_widths: Edges::new(
                register_length(registry, "Surface.BorderTopWidth", geometry_and_paint),
                register_length(registry, "Surface.BorderRightWidth", geometry_and_paint),
                register_length(registry, "Surface.BorderBottomWidth", geometry_and_paint),
                register_length(registry, "Surface.BorderLeftWidth", geometry_and_paint),
            ),
            padding_widths: Edges::new(
                register_length(registry, "Surface.PaddingTop", geometry_and_paint),
                register_length(registry, "Surface.PaddingRight", geometry_and_paint),
                register_length(registry, "Surface.PaddingBottom", geometry_and_paint),
                register_length(registry, "Surface.PaddingLeft", geometry_and_paint),
            ),
            corner_radii: Corners::new(
                register_corner_radius(registry, "Surface.CornerTopLeftRadius", geometry_and_paint),
                register_corner_radius(
                    registry,
                    "Surface.CornerTopRightRadius",
                    geometry_and_paint,
                ),
                register_corner_radius(
                    registry,
                    "Surface.CornerBottomRightRadius",
                    geometry_and_paint,
                ),
                register_corner_radius(
                    registry,
                    "Surface.CornerBottomLeftRadius",
                    geometry_and_paint,
                ),
            ),
            corner_shapes: Corners::new(
                register_corner_shape(registry, "Surface.CornerTopLeftShape", geometry_and_paint),
                register_corner_shape(registry, "Surface.CornerTopRightShape", geometry_and_paint),
                register_corner_shape(
                    registry,
                    "Surface.CornerBottomRightShape",
                    geometry_and_paint,
                ),
                register_corner_shape(
                    registry,
                    "Surface.CornerBottomLeftShape",
                    geometry_and_paint,
                ),
            ),
        }
    }

    /// Resolves the registered properties into value types.
    ///
    /// Use this when a caller wants to inspect or transform the resolved
    /// decoration before building a [`SurfacePrimitive`]. Passing `None` for
    /// `style` resolves without style-layer values.
    #[must_use]
    pub fn resolve_values<'a, K, F, O>(
        self,
        cx: &ResolveCx<'a, K, F>,
        object: &O,
        style: Option<StyleMatch<'_>>,
    ) -> SurfacePropertyValues
    where
        K: Copy + Eq + 'a,
        F: ResolveParentLookup<'a, K>,
        O: DependencyObject<K>,
    {
        let style = style.map(StyleMatch::as_resolve_pair);
        SurfacePropertyValues {
            background: cx.get_value(object, self.background, style),
            border_brushes: Edges {
                top: cx.get_value(object, self.border_brushes.top, style),
                right: cx.get_value(object, self.border_brushes.right, style),
                bottom: cx.get_value(object, self.border_brushes.bottom, style),
                left: cx.get_value(object, self.border_brushes.left, style),
            },
            border_widths: Edges::new(
                finite_non_negative(cx.get_value(object, self.border_widths.top, style)),
                finite_non_negative(cx.get_value(object, self.border_widths.right, style)),
                finite_non_negative(cx.get_value(object, self.border_widths.bottom, style)),
                finite_non_negative(cx.get_value(object, self.border_widths.left, style)),
            ),
            padding_widths: Edges::new(
                finite_non_negative(cx.get_value(object, self.padding_widths.top, style)),
                finite_non_negative(cx.get_value(object, self.padding_widths.right, style)),
                finite_non_negative(cx.get_value(object, self.padding_widths.bottom, style)),
                finite_non_negative(cx.get_value(object, self.padding_widths.left, style)),
            ),
            corner_radii: CornerRadii::new(
                cx.get_value(object, self.corner_radii.top_left, style)
                    .to_size(),
                cx.get_value(object, self.corner_radii.top_right, style)
                    .to_size(),
                cx.get_value(object, self.corner_radii.bottom_right, style)
                    .to_size(),
                cx.get_value(object, self.corner_radii.bottom_left, style)
                    .to_size(),
            ),
            corner_shapes: CornerShapes::new(
                cx.get_value(object, self.corner_shapes.top_left, style),
                cx.get_value(object, self.corner_shapes.top_right, style),
                cx.get_value(object, self.corner_shapes.bottom_right, style),
                cx.get_value(object, self.corner_shapes.bottom_left, style),
            ),
        }
    }

    /// Resolves the registered properties into a presentation surface.
    ///
    /// Passing `None` for `style` resolves without style-layer values.
    #[must_use]
    pub fn resolve_surface<'a, K, F, O>(
        self,
        cx: &ResolveCx<'a, K, F>,
        object: &O,
        style: Option<StyleMatch<'_>>,
    ) -> SurfacePrimitive
    where
        K: Copy + Eq + 'a,
        F: ResolveParentLookup<'a, K>,
        O: DependencyObject<K>,
    {
        self.resolve_values(cx, object, style).into_surface()
    }
}

/// Resolved surface property values before they are packed into presentation.
///
/// This type is useful when a caller needs the property-level values for
/// diagnostics, template decisions, or custom lowering. Use
/// [`SurfacePropertyValues::into_surface`] for the standard presentation path.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SurfacePropertyValues {
    /// Optional background brush for the first background layer.
    pub background: Option<Brush>,
    /// Resolved per-side border brushes.
    pub border_brushes: Edges<Option<Brush>>,
    /// Resolved per-side border widths in logical pixels.
    pub border_widths: Edges<f64>,
    /// Resolved physical padding widths in logical pixels.
    pub padding_widths: Edges<f64>,
    /// Resolved per-corner radii in logical pixels.
    pub corner_radii: CornerRadii,
    /// Resolved per-corner shapes.
    pub corner_shapes: CornerShapes,
}

impl SurfacePropertyValues {
    /// Converts resolved values into a presentation surface primitive.
    #[must_use]
    pub fn into_surface(self) -> SurfacePrimitive {
        let mut surface = SurfacePrimitive {
            border: Border {
                top: BorderSide {
                    brush: self.border_brushes.top,
                    width: self.border_widths.top,
                },
                right: BorderSide {
                    brush: self.border_brushes.right,
                    width: self.border_widths.right,
                },
                bottom: BorderSide {
                    brush: self.border_brushes.bottom,
                    width: self.border_widths.bottom,
                },
                left: BorderSide {
                    brush: self.border_brushes.left,
                    width: self.border_widths.left,
                },
            },
            padding_widths: self.padding_widths,
            corner_radii: self.corner_radii,
            corner_shapes: self.corner_shapes,
            ..SurfacePrimitive::default()
        };

        if let Some(background) = self.background {
            surface.backgrounds.push(BackgroundLayer::new(background));
        }

        surface
    }
}

fn register_optional_brush(
    registry: &mut PropertyRegistry,
    name: &'static str,
    channels: ChannelSet,
) -> Property<Option<Brush>> {
    registry.register(
        name,
        PropertyMetadataBuilder::new(None::<Brush>)
            .affects_channels(channels)
            .build(),
    )
}

fn register_length(
    registry: &mut PropertyRegistry,
    name: &'static str,
    channels: ChannelSet,
) -> Property<f64> {
    registry.register(
        name,
        PropertyMetadataBuilder::new(0.0_f64)
            .affects_channels(channels)
            .coerce(finite_non_negative)
            .build(),
    )
}

fn register_corner_radius(
    registry: &mut PropertyRegistry,
    name: &'static str,
    channels: ChannelSet,
) -> Property<CornerRadius> {
    registry.register(
        name,
        PropertyMetadataBuilder::new(CornerRadius::ZERO)
            .affects_channels(channels)
            .coerce(CornerRadius::clamped_non_negative)
            .build(),
    )
}

fn register_corner_shape(
    registry: &mut PropertyRegistry,
    name: &'static str,
    channels: ChannelSet,
) -> Property<CornerShape> {
    registry.register(
        name,
        PropertyMetadataBuilder::new(CornerShape::Round)
            .affects_channels(channels)
            .build(),
    )
}

fn finite_non_negative(value: f64) -> f64 {
    if value.is_finite() && value > 0.0 {
        value
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use invalidation::Channel;
    use understory_presentation::Color;
    use understory_property::{DependencyObject, PropertyRegistry, PropertyStore};
    use understory_style::{
        NoResolveParentLookup, ResolveCx, StyleBuilder, StyleCascadeBuilder, StyleOrigin,
        ThemeBuilder,
    };

    use super::*;

    const GEOMETRY: Channel = Channel::new(0);
    const PAINT: Channel = Channel::new(1);

    #[derive(Debug)]
    struct Element {
        key: u32,
        parent: Option<u32>,
        store: PropertyStore<u32>,
    }

    impl Element {
        fn new(key: u32) -> Self {
            Self {
                key,
                parent: None,
                store: PropertyStore::new(key),
            }
        }
    }

    impl DependencyObject<u32> for Element {
        fn property_store(&self) -> &PropertyStore<u32> {
            &self.store
        }

        fn property_store_mut(&mut self) -> &mut PropertyStore<u32> {
            &mut self.store
        }

        fn key(&self) -> u32 {
            self.key
        }

        fn parent_key(&self) -> Option<u32> {
            self.parent
        }
    }

    fn register_surface(registry: &mut PropertyRegistry) -> SurfaceProperties {
        SurfaceProperties::register(
            registry,
            SurfacePropertyChannels::new(GEOMETRY.into_set(), PAINT.into_set()),
        )
    }

    #[test]
    fn register_assigns_invalidation_channels_by_property_kind() {
        let mut registry = PropertyRegistry::new();
        let surface = register_surface(&mut registry);

        let background_channels = registry.affects_channels(surface.background.id());
        assert!(!background_channels.contains(GEOMETRY));
        assert!(background_channels.contains(PAINT));

        let brush_channels = registry.affects_channels(surface.border_brushes.top.id());
        assert!(!brush_channels.contains(GEOMETRY));
        assert!(brush_channels.contains(PAINT));

        let width_channels = registry.affects_channels(surface.border_widths.top.id());
        assert!(width_channels.contains(GEOMETRY));
        assert!(width_channels.contains(PAINT));

        let padding_channels = registry.affects_channels(surface.padding_widths.top.id());
        assert!(padding_channels.contains(GEOMETRY));
        assert!(padding_channels.contains(PAINT));

        let radius_channels = registry.affects_channels(surface.corner_radii.top_left.id());
        assert!(radius_channels.contains(GEOMETRY));
        assert!(radius_channels.contains(PAINT));

        let shape_channels = registry.affects_channels(surface.corner_shapes.top_left.id());
        assert!(shape_channels.contains(GEOMETRY));
        assert!(shape_channels.contains(PAINT));
    }

    #[test]
    fn defaults_resolve_to_empty_surface() {
        let mut registry = PropertyRegistry::new();
        let surface = register_surface(&mut registry);
        let theme = ThemeBuilder::new().build();
        let cx = ResolveCx::new(&registry, &theme, NoResolveParentLookup);
        let element = Element::new(1);

        let primitive = surface.resolve_surface(&cx, &element, None);

        assert!(primitive.is_empty());
        assert_eq!(primitive.border.visible_widths(), Edges::ZERO);
        assert_eq!(primitive.padding_widths, Edges::ZERO);
        assert_eq!(primitive.corner_radii, CornerRadii::ZERO);
        assert_eq!(primitive.corner_shapes, CornerShapes::ROUND);
    }

    #[test]
    fn style_values_resolve_into_surface_primitive() {
        let mut registry = PropertyRegistry::new();
        let surface = register_surface(&mut registry);
        let theme = ThemeBuilder::new().build();
        let cx = ResolveCx::new(&registry, &theme, NoResolveParentLookup);
        let element = Element::new(1);

        let style = StyleBuilder::new()
            .set(surface.background, Some(Brush::from(Color::WHITE)))
            .set(surface.border_widths.top, 2.0)
            .set(surface.border_widths.right, 4.0)
            .set(surface.border_widths.bottom, -8.0)
            .set(surface.border_widths.left, f64::NAN)
            .set(surface.border_brushes.top, Some(Brush::from(Color::BLACK)))
            .set(
                surface.border_brushes.right,
                Some(Brush::from(Color::BLACK)),
            )
            .set(surface.padding_widths.top, 4.0)
            .set(surface.padding_widths.right, f64::INFINITY)
            .set(surface.padding_widths.bottom, 6.0)
            .set(surface.padding_widths.left, -2.0)
            .set(surface.corner_radii.top_left, CornerRadius::new(8.0, 4.0))
            .set(
                surface.corner_radii.bottom_right,
                CornerRadius::new(f64::INFINITY, 10.0),
            )
            .set(surface.corner_shapes.top_left, CornerShape::squircle())
            .set(surface.corner_shapes.bottom_right, CornerShape::scoop())
            .build();
        let cascade = StyleCascadeBuilder::new()
            .push_style(StyleOrigin::Base, style)
            .build();

        let primitive = surface.resolve_surface(
            &cx,
            &element,
            Some(StyleMatch::new(&cascade, cascade.root_state())),
        );

        assert_eq!(primitive.backgrounds.len(), 1);
        assert_eq!(primitive.backgrounds[0].brush, Brush::from(Color::WHITE));
        assert_eq!(
            primitive.border.visible_widths(),
            Edges::new(2.0, 4.0, 0.0, 0.0)
        );
        assert_eq!(primitive.padding_widths, Edges::new(4.0, 0.0, 6.0, 0.0));
        assert_eq!(primitive.corner_radii.top_left, Size::new(8.0, 4.0));
        assert_eq!(primitive.corner_radii.bottom_right, Size::new(0.0, 10.0));
        assert_eq!(primitive.corner_shapes.top_left, CornerShape::squircle());
        assert_eq!(primitive.corner_shapes.bottom_right, CornerShape::scoop());
    }

    #[test]
    fn local_values_win_over_style_values() {
        let mut registry = PropertyRegistry::new();
        let surface = register_surface(&mut registry);
        let theme = ThemeBuilder::new().build();
        let mut element = Element::new(1);
        element.store.set_local(surface.border_widths.top, 6.0);

        let style = StyleBuilder::new()
            .set(surface.border_widths.top, 2.0)
            .build();
        let cascade = StyleCascadeBuilder::new()
            .push_style(StyleOrigin::Base, style)
            .build();
        let cx = ResolveCx::new(&registry, &theme, NoResolveParentLookup);

        let primitive = surface.resolve_surface(
            &cx,
            &element,
            Some(StyleMatch::new(&cascade, cascade.root_state())),
        );

        assert_eq!(primitive.border.top.width, 6.0);
    }
}
