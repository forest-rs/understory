// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

macro_rules! id_type {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
        #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
        pub struct $name(pub u64);
    };
}

id_type! {
    /// Caller-owned identity for an overlay in an [`OverlayStack`](crate::OverlayStack).
    ///
    /// The crate never allocates these ids. Hosts usually derive them from
    /// widget state, menu instances, or dialog controllers and reuse them while
    /// the same logical overlay remains open.
    OverlayId
}

id_type! {
    /// Caller-owned identity for an anchor that can dismiss attached overlays.
    ///
    /// Store one in [`OverlayEntry::anchor`](crate::OverlayEntry::anchor) when
    /// the host wants anchor blur or focus transitions to affect an overlay.
    AnchorId
}

id_type! {
    /// Identity for a focus scope described by an [`OverlayFrame`](crate::OverlayFrame).
    ///
    /// You get these from [`FocusScopeFrame::id`](crate::FocusScopeFrame::id).
    /// The first version derives the value from the owning [`OverlayId`].
    FocusScopeId
}

/// Monotonic revision for a changed [`OverlayStack`](crate::OverlayStack).
///
/// You get this from [`OverlayStack::revision`](crate::OverlayStack::revision).
/// It advances after every successful [`OverlayOp`](crate::OverlayOp)
/// application.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Revision(pub u64);

impl Revision {
    pub(crate) fn advance(&mut self) {
        self.0 = self.0.saturating_add(1);
    }
}
