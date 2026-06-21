// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

/// Playback state for a retained animation instance.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AnimationPlayState {
    /// The animation is not associated with active playback.
    Idle,
    /// The animation advances when its timeline advances.
    Running,
    /// The animation keeps its current time until resumed or explicitly
    /// sought.
    Paused,
    /// The animation has reached its natural end.
    Finished,
}

/// Direction used to map iteration progress into effect progress.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlaybackDirection {
    /// Every iteration plays from start to end.
    Normal,
    /// Every iteration plays from end to start.
    Reverse,
    /// Even iterations play forward; odd iterations play backward.
    Alternate,
    /// Even iterations play backward; odd iterations play forward.
    AlternateReverse,
}

/// Fill behavior outside an animation's active interval.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FillMode {
    /// The animation produces no value before or after its active interval.
    None,
    /// The animation produces its first active value before it starts.
    Backwards,
    /// The animation produces its final active value after it ends.
    Forwards,
    /// The animation fills both before and after its active interval.
    Both,
}

impl FillMode {
    pub(crate) fn fills_backwards(self) -> bool {
        matches!(self, Self::Backwards | Self::Both)
    }

    pub(crate) fn fills_forwards(self) -> bool {
        matches!(self, Self::Forwards | Self::Both)
    }
}
