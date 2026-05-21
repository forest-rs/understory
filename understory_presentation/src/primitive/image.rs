// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use peniko::{ImageBrush, ImageSampler, kurbo::Insets};

/// Resolved image drawing intent.
///
/// `ImageKey` is chosen by the host toolkit. Presentation stores image intent,
/// not decoded image bytes or GPU resources. The lowerer maps this key through
/// the host's image registry.
///
/// Geometry such as the destination bounds lives outside this crate. The image
/// primitive stores the image resource, sampling hints, fitting behavior, and
/// optional nine-slice metadata for the lowerer to apply to those bounds.
#[derive(Clone, Debug, PartialEq)]
pub struct ImagePrimitive<ImageKey = u64> {
    /// Image resource and sampling parameters.
    pub brush: ImageBrush<ImageKey>,

    /// How the image maps into the node bounds.
    pub fit: ImageFit,

    /// Optional image slicing mode.
    pub slice: ImageSlice,
}

impl<ImageKey> ImagePrimitive<ImageKey> {
    /// Creates an image primitive with default sampling and stretch fitting.
    #[must_use]
    pub fn new(image: ImageKey) -> Self {
        Self {
            brush: ImageBrush {
                image,
                sampler: ImageSampler::default(),
            },
            fit: ImageFit::default(),
            slice: ImageSlice::default(),
        }
    }
}

/// How an image is fitted into destination bounds.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[non_exhaustive]
pub enum ImageFit {
    /// Stretch independently in x and y to fill the bounds.
    #[default]
    Stretch,

    /// Preserve aspect ratio and fit entirely within the bounds.
    Contain,

    /// Preserve aspect ratio and cover the bounds, allowing crop.
    Cover,

    /// Use the image's intrinsic size, positioned by the lowerer.
    None,

    /// Use intrinsic size unless the image must shrink to fit.
    ScaleDown,
}

/// Image slicing mode.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[non_exhaustive]
pub enum ImageSlice {
    /// Draw the image as a single fitted rectangle.
    #[default]
    None,

    /// Draw the image using nine-slice scaling.
    Nine(NineSlice),
}

/// Nine-slice image metadata.
///
/// Insets are measured in source image pixels. The lowerer maps the sliced
/// source regions into the node bounds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NineSlice {
    /// Source-image insets that define the nine regions.
    pub insets: Insets,

    /// Scaling behavior for the edge regions.
    pub edges: SliceMode,

    /// Scaling behavior for the center region.
    pub center: SliceMode,
}

impl NineSlice {
    /// Creates nine-slice metadata using stretched edges and center.
    #[must_use]
    pub const fn new(insets: Insets) -> Self {
        Self {
            insets,
            edges: SliceMode::Stretch,
            center: SliceMode::Stretch,
        }
    }
}

/// Scaling behavior for a sliced image region.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[non_exhaustive]
pub enum SliceMode {
    /// Stretch the source region to the destination region.
    #[default]
    Stretch,

    /// Tile the source region across the destination region.
    Repeat,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_primitive_defaults_to_stretching_the_image() {
        let image = ImagePrimitive::new("button-background");

        assert_eq!(image.brush.image, "button-background");
        assert_eq!(image.brush.sampler, ImageSampler::default());
        assert_eq!(image.fit, ImageFit::Stretch);
        assert_eq!(image.slice, ImageSlice::None);
    }

    #[test]
    fn nine_slice_defaults_to_stretching_regions() {
        let slice = NineSlice::new(Insets::uniform(6.0));

        assert_eq!(slice.insets.x0, 6.0);
        assert_eq!(slice.edges, SliceMode::Stretch);
        assert_eq!(slice.center, SliceMode::Stretch);
    }
}
