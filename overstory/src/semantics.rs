// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Semantic roles and states for Overstory controls and surfaces.
//!
//! These types are the toolkit-facing semantic model that higher layers can
//! later adapt into accessibility bridges such as AccessKit. Overstory keeps
//! them renderer-agnostic and independent of any platform API.

use alloc::boxed::Box;

/// High-level semantic role for one resolved UI element.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum SemanticRole {
    /// No stronger semantic contract than "generic UI element" is known yet.
    #[default]
    Generic,
    /// Grouping/container surface.
    Group,
    /// Interactive push button.
    Button,
    /// Read-only text content.
    Text,
    /// Editable text field.
    TextInput,
    /// Scrollable region.
    ScrollArea,
    /// Draggable separator/splitter.
    Splitter,
    /// Decorative or semantic separator.
    Separator,
    /// Tooltip/popup hint surface.
    Tooltip,
    /// Busy or indeterminate progress indicator.
    ProgressIndicator,
}

/// Semantic state flags attached to one resolved element.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct SemanticState {
    /// The element is unavailable for interaction.
    pub disabled: bool,
    /// The element currently has focus.
    pub focused: bool,
    /// Focus should be visibly indicated for this element.
    pub focus_visible: bool,
    /// The element is currently hovered.
    pub hovered: bool,
    /// The element is currently pressed/active.
    pub pressed: bool,
    /// The element is currently busy.
    pub busy: bool,
}

/// Resolved semantic snapshot for one element.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SemanticInfo {
    /// High-level semantic role.
    pub role: SemanticRole,
    /// Human-readable accessible name, if available.
    pub name: Option<Box<str>>,
    /// Current semantic value, if available.
    pub value: Option<Box<str>>,
    /// Current semantic state flags.
    pub state: SemanticState,
}
