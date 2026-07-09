// Copyright 2025 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Theme resource lookup.
//!
//! This module provides [`Theme`], a collection of themed resources that can
//! be looked up by [`ResourceKey`], [`VariantTheme`], which layers per-variant
//! override tables over a base [`Theme`], and [`ThemeScope`], a subtree-scoped
//! chain of themes carried by the embedder.

use alloc::rc::Rc;
use alloc::vec::Vec;
use core::fmt;

use understory_property::ErasedValue;
use understory_property_expression::ExprResourceKey;

/// A key for looking up resources in a [`Theme`].
///
/// Resource keys are simple u16 identifiers, typically defined as constants
/// at the application level.
///
/// # Example
///
/// ```rust
/// use understory_style::ResourceKey;
///
/// // Define resource keys as constants
/// const ACCENT_COLOR: ResourceKey = ResourceKey::new(0);
/// const FONT_SIZE: ResourceKey = ResourceKey::new(1);
/// const CORNER_RADIUS: ResourceKey = ResourceKey::new(2);
/// ```
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ResourceKey(u16);

impl ResourceKey {
    /// Creates a new resource key with the given index.
    #[must_use]
    #[inline]
    pub const fn new(index: u16) -> Self {
        Self(index)
    }

    /// Returns the underlying index of this resource key.
    #[must_use]
    #[inline]
    pub const fn index(self) -> u16 {
        self.0
    }
}

impl fmt::Debug for ResourceKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("ResourceKey").field(&self.0).finish()
    }
}

impl fmt::Display for ResourceKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ResourceKey({})", self.0)
    }
}

impl From<ResourceKey> for ExprResourceKey {
    fn from(key: ResourceKey) -> Self {
        Self::new(key.index())
    }
}

impl From<ExprResourceKey> for ResourceKey {
    fn from(key: ExprResourceKey) -> Self {
        Self::new(key.index())
    }
}

/// A collection of themed resources.
///
/// Themes provide resource lookup by [`ResourceKey`], enabling theming
/// (light/dark modes, brand colors, etc.). Unlike styles which set property
/// values directly, themes provide values that properties can reference.
///
/// Themes are immutable after creation. Use [`ThemeBuilder`] to construct them.
///
/// # Memory Layout
///
/// Internally, `Theme` wraps an `Rc<ThemeData>`, making cloning cheap.
/// Resources are stored in a sorted vector for O(log n) lookup.
///
/// # Example
///
/// ```rust
/// use understory_style::{Theme, ThemeBuilder, ResourceKey};
///
/// const ACCENT_COLOR: ResourceKey = ResourceKey::new(0);
///
/// let light_theme = ThemeBuilder::new()
///     .set(ACCENT_COLOR, 0x0078D4_u32)
///     .build();
///
/// let dark_theme = ThemeBuilder::new()
///     .set(ACCENT_COLOR, 0x4CC2FF_u32)
///     .build();
///
/// assert_eq!(light_theme.get::<u32>(ACCENT_COLOR), Some(&0x0078D4));
/// assert_eq!(dark_theme.get::<u32>(ACCENT_COLOR), Some(&0x4CC2FF));
/// ```
#[derive(Clone, Debug)]
pub struct Theme {
    inner: Rc<ThemeData>,
}

/// Internal storage for theme resources.
#[derive(Debug, Default)]
struct ThemeData {
    /// Sorted by `ResourceKey` for binary search lookup.
    resources: Vec<(ResourceKey, ErasedValue)>,
}

impl Theme {
    /// Returns `true` if this theme has no resources.
    #[must_use]
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.inner.resources.is_empty()
    }

    /// Returns the number of resources in this theme.
    #[must_use]
    #[inline]
    pub fn len(&self) -> usize {
        self.inner.resources.len()
    }

    /// Gets the value for a resource key, if present.
    #[must_use]
    #[inline]
    pub fn get<T: Clone + 'static>(&self, key: ResourceKey) -> Option<&T> {
        self.inner
            .resources
            .binary_search_by_key(&key, |(k, _)| *k)
            .ok()
            .and_then(|idx| self.inner.resources[idx].1.downcast_ref())
    }

    /// Gets an erased owned copy of the value for a resource key, if present.
    ///
    /// This preserves the typed borrowed [`Self::get`] API while supporting
    /// expression evaluation contexts that need to resolve resources without
    /// knowing their concrete value type at compile time.
    #[must_use]
    #[inline]
    pub fn get_erased(&self, key: ResourceKey) -> Option<ErasedValue> {
        self.inner
            .resources
            .binary_search_by_key(&key, |(k, _)| *k)
            .ok()
            .map(|idx| self.inner.resources[idx].1.clone())
    }

    /// Returns `true` if this theme has a value for the resource key.
    #[must_use]
    #[inline]
    pub fn contains(&self, key: ResourceKey) -> bool {
        self.inner
            .resources
            .binary_search_by_key(&key, |(k, _)| *k)
            .is_ok()
    }

    /// Returns an iterator over the resource keys in this theme.
    pub fn keys(&self) -> impl Iterator<Item = ResourceKey> + '_ {
        self.inner.resources.iter().map(|(k, _)| *k)
    }
}

impl Default for Theme {
    fn default() -> Self {
        ThemeBuilder::new().build()
    }
}

/// Builder for constructing [`Theme`] instances.
///
/// # Example
///
/// ```rust
/// use understory_style::{ThemeBuilder, ResourceKey};
///
/// const PRIMARY: ResourceKey = ResourceKey::new(0);
/// const SECONDARY: ResourceKey = ResourceKey::new(1);
///
/// let theme = ThemeBuilder::new()
///     .set(PRIMARY, "#0078D4".to_string())
///     .set(SECONDARY, "#106EBE".to_string())
///     .build();
///
/// assert_eq!(theme.get::<String>(PRIMARY).map(|s| s.as_str()), Some("#0078D4"));
/// ```
#[derive(Debug, Default)]
pub struct ThemeBuilder {
    resources: Vec<(ResourceKey, ErasedValue)>,
}

impl ThemeBuilder {
    /// Creates a new empty theme builder.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets a resource value in the theme.
    ///
    /// If the resource was already set, the value is replaced.
    #[must_use]
    pub fn set<T: Clone + 'static>(mut self, key: ResourceKey, value: T) -> Self {
        let erased = ErasedValue::new(value);

        match self.resources.binary_search_by_key(&key, |(k, _)| *k) {
            Ok(idx) => {
                self.resources[idx].1 = erased;
            }
            Err(idx) => {
                self.resources.insert(idx, (key, erased));
            }
        }
        self
    }

    /// Builds the theme.
    #[must_use]
    pub fn build(self) -> Theme {
        Theme {
            inner: Rc::new(ThemeData {
                resources: self.resources,
            }),
        }
    }
}

/// An identifier for a theme variant, such as light or dark.
///
/// Theme variants are simple u16 identifiers with well-known constants for
/// [`Self::LIGHT`], [`Self::DARK`], and [`Self::HIGH_CONTRAST`]. Applications
/// can define custom variants as constants at other indices, the same way
/// [`ResourceKey`] constants are defined.
///
/// # Example
///
/// ```rust
/// use understory_style::ThemeVariant;
///
/// // Custom variants use indices outside the well-known range.
/// const SEPIA: ThemeVariant = ThemeVariant::new(16);
///
/// assert_ne!(SEPIA, ThemeVariant::DARK);
/// ```
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ThemeVariant(u16);

impl ThemeVariant {
    /// The well-known light variant.
    pub const LIGHT: Self = Self(0);

    /// The well-known dark variant.
    pub const DARK: Self = Self(1);

    /// The well-known high-contrast variant.
    pub const HIGH_CONTRAST: Self = Self(2);

    /// Creates a new theme variant with the given index.
    ///
    /// Indices `0..=2` are the well-known [`Self::LIGHT`], [`Self::DARK`], and
    /// [`Self::HIGH_CONTRAST`] constants. Custom variants should use other
    /// indices.
    #[must_use]
    #[inline]
    pub const fn new(index: u16) -> Self {
        Self(index)
    }

    /// Returns the underlying index of this theme variant.
    #[must_use]
    #[inline]
    pub const fn index(self) -> u16 {
        self.0
    }

    fn well_known_name(self) -> Option<&'static str> {
        match self {
            Self::LIGHT => Some("Light"),
            Self::DARK => Some("Dark"),
            Self::HIGH_CONTRAST => Some("HighContrast"),
            _ => None,
        }
    }
}

impl fmt::Debug for ThemeVariant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.well_known_name() {
            Some(name) => write!(f, "ThemeVariant({name})"),
            None => write!(f, "ThemeVariant({})", self.0),
        }
    }
}

impl fmt::Display for ThemeVariant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

/// A theme with per-variant override tables over a base [`Theme`].
///
/// `VariantTheme` makes light/dark/high-contrast (and custom) variants a
/// first-class dimension instead of requiring the host to swap whole theme
/// tables. Lookup takes the active [`ThemeVariant`] and falls back
/// variant → base: a resource present in the active variant's table shadows
/// the base entry; everything else resolves from the base table. An unknown
/// variant (one with no override table) falls back cleanly to the base table.
///
/// Shadowing is by key presence: when the active variant's table contains a
/// key, that table supplies the entry, even for typed lookups that then fail
/// to downcast.
///
/// Like [`Theme`], variant themes are immutable after creation and cheap to
/// clone. Use [`VariantThemeBuilder`] to construct them.
///
/// # Example
///
/// ```rust
/// use understory_style::{ResourceKey, ThemeBuilder, ThemeVariant, VariantThemeBuilder};
///
/// const ACCENT_COLOR: ResourceKey = ResourceKey::new(0);
/// const FONT_SIZE: ResourceKey = ResourceKey::new(1);
///
/// let theme = VariantThemeBuilder::new()
///     .base(
///         ThemeBuilder::new()
///             .set(ACCENT_COLOR, 0x0078D4_u32)
///             .set(FONT_SIZE, 14.0_f64)
///             .build(),
///     )
///     .variant(
///         ThemeVariant::DARK,
///         ThemeBuilder::new().set(ACCENT_COLOR, 0x4CC2FF_u32).build(),
///     )
///     .build();
///
/// // The dark variant overrides the accent color and falls back for the rest.
/// let dark = Some(ThemeVariant::DARK);
/// assert_eq!(theme.get::<u32>(dark, ACCENT_COLOR), Some(&0x4CC2FF));
/// assert_eq!(theme.get::<f64>(dark, FONT_SIZE), Some(&14.0));
///
/// // No active variant resolves from the base table only.
/// assert_eq!(theme.get::<u32>(None, ACCENT_COLOR), Some(&0x0078D4));
/// ```
#[derive(Clone, Debug, Default)]
pub struct VariantTheme {
    inner: Rc<VariantThemeData>,
}

/// Internal storage for variant theme tables.
#[derive(Debug, Default)]
struct VariantThemeData {
    base: Theme,
    /// Sorted by `ThemeVariant` for binary search lookup.
    variants: Vec<(ThemeVariant, Theme)>,
}

impl VariantTheme {
    /// Returns the base theme consulted when the active variant has no entry.
    #[must_use]
    #[inline]
    pub fn base(&self) -> &Theme {
        &self.inner.base
    }

    /// Returns the override table for a variant, if one was provided.
    #[must_use]
    pub fn variant(&self, variant: ThemeVariant) -> Option<&Theme> {
        self.inner
            .variants
            .binary_search_by_key(&variant, |(v, _)| *v)
            .ok()
            .map(|idx| &self.inner.variants[idx].1)
    }

    /// Returns an iterator over the variant override tables in this theme.
    pub fn variants(&self) -> impl Iterator<Item = (ThemeVariant, &Theme)> {
        self.inner.variants.iter().map(|(v, theme)| (*v, theme))
    }

    /// Gets the value for a resource key under the active variant, if present.
    ///
    /// The active variant's table is consulted first; keys it does not contain
    /// fall back to the base table. `None` resolves from the base table only.
    #[must_use]
    pub fn get<T: Clone + 'static>(
        &self,
        variant: Option<ThemeVariant>,
        key: ResourceKey,
    ) -> Option<&T> {
        if let Some(table) = self.active_table(variant, key) {
            return table.get(key);
        }
        self.inner.base.get(key)
    }

    /// Gets an erased owned copy of the value for a resource key under the
    /// active variant, if present.
    #[must_use]
    pub fn get_erased(
        &self,
        variant: Option<ThemeVariant>,
        key: ResourceKey,
    ) -> Option<ErasedValue> {
        if let Some(table) = self.active_table(variant, key) {
            return table.get_erased(key);
        }
        self.inner.base.get_erased(key)
    }

    /// Returns `true` if the active variant or the base table has a value for
    /// the resource key.
    #[must_use]
    pub fn contains(&self, variant: Option<ThemeVariant>, key: ResourceKey) -> bool {
        self.active_table(variant, key).is_some() || self.inner.base.contains(key)
    }

    /// Returns the variant table that shadows `key`, if any.
    fn active_table(&self, variant: Option<ThemeVariant>, key: ResourceKey) -> Option<&Theme> {
        let table = self.variant(variant?)?;
        table.contains(key).then_some(table)
    }
}

impl From<Theme> for VariantTheme {
    fn from(base: Theme) -> Self {
        VariantThemeBuilder::new().base(base).build()
    }
}

/// Builder for constructing [`VariantTheme`] instances.
///
/// # Example
///
/// ```rust
/// use understory_style::{ResourceKey, ThemeBuilder, ThemeVariant, VariantThemeBuilder};
///
/// const ACCENT_COLOR: ResourceKey = ResourceKey::new(0);
///
/// let theme = VariantThemeBuilder::new()
///     .base(ThemeBuilder::new().set(ACCENT_COLOR, 0x0078D4_u32).build())
///     .variant(
///         ThemeVariant::DARK,
///         ThemeBuilder::new().set(ACCENT_COLOR, 0x4CC2FF_u32).build(),
///     )
///     .build();
///
/// assert!(theme.variant(ThemeVariant::DARK).is_some());
/// assert!(theme.variant(ThemeVariant::HIGH_CONTRAST).is_none());
/// ```
#[derive(Debug, Default)]
pub struct VariantThemeBuilder {
    base: Theme,
    variants: Vec<(ThemeVariant, Theme)>,
}

impl VariantThemeBuilder {
    /// Creates a new variant theme builder with an empty base theme.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the base theme.
    #[must_use]
    pub fn base(mut self, base: Theme) -> Self {
        self.base = base;
        self
    }

    /// Sets the override table for a variant.
    ///
    /// If the variant already had a table, it is replaced.
    #[must_use]
    pub fn variant(mut self, variant: ThemeVariant, theme: Theme) -> Self {
        match self.variants.binary_search_by_key(&variant, |(v, _)| *v) {
            Ok(idx) => {
                self.variants[idx].1 = theme;
            }
            Err(idx) => {
                self.variants.insert(idx, (variant, theme));
            }
        }
        self
    }

    /// Builds the variant theme.
    #[must_use]
    pub fn build(self) -> VariantTheme {
        VariantTheme {
            inner: Rc::new(VariantThemeData {
                base: self.base,
                variants: self.variants,
            }),
        }
    }
}

/// A subtree-scoped chain of themes.
///
/// `ThemeScope` follows the same embedding philosophy as
/// [`MatchState`][crate::MatchState]: this crate does not store the app tree.
/// The embedder walks its own tree and carries a scope handle from parent to
/// child. Most subjects share their parent's scope; a popup or themed island
/// pushes a [`Self::child`] scope that overrides some resources for its
/// subtree.
///
/// Lookup consults the innermost scope first and falls back outward to the
/// root scope, which normally holds the app theme. At each level the active
/// [`ThemeVariant`] fallback applies: variant table, then base table, then the
/// next outer scope. Shadowing is by key presence, so a scope that contains a
/// key supplies the entry even for typed lookups that then fail to downcast.
///
/// Scopes are immutable after creation and cheap to clone: a scope is one
/// `Rc` link to its theme and parent.
///
/// # Example
///
/// ```rust
/// use understory_style::{ResourceKey, ThemeBuilder, ThemeScope};
///
/// const ACCENT_COLOR: ResourceKey = ResourceKey::new(0);
/// const FONT_SIZE: ResourceKey = ResourceKey::new(1);
///
/// let app = ThemeScope::new(
///     ThemeBuilder::new()
///         .set(ACCENT_COLOR, 0x0078D4_u32)
///         .set(FONT_SIZE, 14.0_f64)
///         .build(),
/// );
/// let popup = app.child(ThemeBuilder::new().set(ACCENT_COLOR, 0xE81123_u32).build());
///
/// // The popup scope shadows the accent color and falls back outward.
/// assert_eq!(popup.get::<u32>(None, ACCENT_COLOR), Some(&0xE81123));
/// assert_eq!(popup.get::<f64>(None, FONT_SIZE), Some(&14.0));
/// assert_eq!(app.get::<u32>(None, ACCENT_COLOR), Some(&0x0078D4));
/// ```
#[derive(Clone, Debug)]
pub struct ThemeScope {
    inner: Rc<ThemeScopeNode>,
}

/// One link in a theme scope chain.
#[derive(Debug)]
struct ThemeScopeNode {
    theme: VariantTheme,
    parent: Option<ThemeScope>,
}

impl ThemeScope {
    /// Creates a root scope, normally holding the app theme.
    #[must_use]
    pub fn new(theme: impl Into<VariantTheme>) -> Self {
        Self {
            inner: Rc::new(ThemeScopeNode {
                theme: theme.into(),
                parent: None,
            }),
        }
    }

    /// Creates a child scope whose theme shadows this scope for a subtree.
    #[must_use]
    pub fn child(&self, theme: impl Into<VariantTheme>) -> Self {
        Self {
            inner: Rc::new(ThemeScopeNode {
                theme: theme.into(),
                parent: Some(self.clone()),
            }),
        }
    }

    /// Returns this scope's own theme, without outer-scope fallback.
    #[must_use]
    #[inline]
    pub fn theme(&self) -> &VariantTheme {
        &self.inner.theme
    }

    /// Returns the parent scope, if this is not the root scope.
    #[must_use]
    #[inline]
    pub fn parent(&self) -> Option<&Self> {
        self.inner.parent.as_ref()
    }

    /// Returns the number of parent hops to the root scope.
    #[must_use]
    pub fn depth(&self) -> usize {
        let mut depth = 0;
        let mut current = self;
        while let Some(parent) = current.parent() {
            depth += 1;
            current = parent;
        }
        depth
    }

    /// Gets the value for a resource key under the active variant, consulting
    /// this scope first and falling back outward to the root scope.
    #[must_use]
    pub fn get<T: Clone + 'static>(
        &self,
        variant: Option<ThemeVariant>,
        key: ResourceKey,
    ) -> Option<&T> {
        self.containing_scope(variant, key)?
            .theme()
            .get(variant, key)
    }

    /// Gets an erased owned copy of the value for a resource key under the
    /// active variant, consulting this scope first and falling back outward.
    #[must_use]
    pub fn get_erased(
        &self,
        variant: Option<ThemeVariant>,
        key: ResourceKey,
    ) -> Option<ErasedValue> {
        self.containing_scope(variant, key)?
            .theme()
            .get_erased(variant, key)
    }

    /// Returns `true` if any scope in the chain has a value for the resource
    /// key under the active variant.
    #[must_use]
    pub fn contains(&self, variant: Option<ThemeVariant>, key: ResourceKey) -> bool {
        self.containing_scope(variant, key).is_some()
    }

    /// Returns the innermost scope whose theme contains `key`, if any.
    fn containing_scope(&self, variant: Option<ThemeVariant>, key: ResourceKey) -> Option<&Self> {
        let mut current = self;
        loop {
            if current.theme().contains(variant, key) {
                return Some(current);
            }
            current = current.parent()?;
        }
    }
}

impl From<Theme> for ThemeScope {
    fn from(theme: Theme) -> Self {
        Self::new(theme)
    }
}

impl From<VariantTheme> for ThemeScope {
    fn from(theme: VariantTheme) -> Self {
        Self::new(theme)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::{String, ToString};
    use alloc::vec::Vec;

    const PRIMARY: ResourceKey = ResourceKey::new(0);
    const SECONDARY: ResourceKey = ResourceKey::new(1);
    const FONT_SIZE: ResourceKey = ResourceKey::new(2);

    #[test]
    fn resource_key_basics() {
        let key = ResourceKey::new(42);
        assert_eq!(key.index(), 42);

        let key2 = ResourceKey::new(42);
        assert_eq!(key, key2);

        let key3 = ResourceKey::new(43);
        assert_ne!(key, key3);
    }

    #[test]
    fn resource_key_debug() {
        use alloc::format;
        let key = ResourceKey::new(42);
        assert_eq!(format!("{:?}", key), "ResourceKey(42)");
    }

    #[test]
    fn resource_key_display() {
        use alloc::format;
        let key = ResourceKey::new(42);
        assert_eq!(format!("{}", key), "ResourceKey(42)");
    }

    #[test]
    fn theme_empty() {
        let theme = ThemeBuilder::new().build();
        assert!(theme.is_empty());
        assert_eq!(theme.len(), 0);
    }

    #[test]
    fn theme_single_resource() {
        let theme = ThemeBuilder::new().set(PRIMARY, 0x0078D4_u32).build();

        assert!(!theme.is_empty());
        assert_eq!(theme.len(), 1);
        assert_eq!(theme.get::<u32>(PRIMARY), Some(&0x0078D4));
    }

    #[test]
    fn theme_multiple_resources() {
        let theme = ThemeBuilder::new()
            .set(PRIMARY, 0x0078D4_u32)
            .set(FONT_SIZE, 14.0_f64)
            .build();

        assert_eq!(theme.len(), 2);
        assert_eq!(theme.get::<u32>(PRIMARY), Some(&0x0078D4));
        assert_eq!(theme.get::<f64>(FONT_SIZE), Some(&14.0));
    }

    #[test]
    fn theme_string_resources() {
        let theme = ThemeBuilder::new()
            .set(PRIMARY, "#0078D4".to_string())
            .set(SECONDARY, "#106EBE".to_string())
            .build();

        assert_eq!(
            theme.get::<String>(PRIMARY).map(|s| s.as_str()),
            Some("#0078D4")
        );
        assert_eq!(
            theme.get::<String>(SECONDARY).map(|s| s.as_str()),
            Some("#106EBE")
        );
    }

    #[test]
    fn theme_replace_value() {
        let theme = ThemeBuilder::new()
            .set(PRIMARY, 0x0078D4_u32)
            .set(PRIMARY, 0x4CC2FF_u32)
            .build();

        assert_eq!(theme.len(), 1);
        assert_eq!(theme.get::<u32>(PRIMARY), Some(&0x4CC2FF));
    }

    #[test]
    fn theme_contains() {
        let theme = ThemeBuilder::new().set(PRIMARY, 0x0078D4_u32).build();

        assert!(theme.contains(PRIMARY));
        assert!(!theme.contains(SECONDARY));
    }

    #[test]
    fn theme_clone_is_cheap() {
        let theme = ThemeBuilder::new().set(PRIMARY, 0x0078D4_u32).build();
        let theme2 = theme.clone();

        // Both reference the same data
        assert_eq!(theme.get::<u32>(PRIMARY), Some(&0x0078D4));
        assert_eq!(theme2.get::<u32>(PRIMARY), Some(&0x0078D4));

        // Rc makes this cheap
        assert!(Rc::ptr_eq(&theme.inner, &theme2.inner));
    }

    #[test]
    fn theme_keys() {
        let theme = ThemeBuilder::new()
            .set(SECONDARY, 0_u32)
            .set(PRIMARY, 0_u32)
            .set(FONT_SIZE, 0.0_f64)
            .build();

        let keys: Vec<_> = theme.keys().collect();
        assert_eq!(keys.len(), 3);
        // Should be sorted by ResourceKey
        assert!(keys[0].index() < keys[1].index());
        assert!(keys[1].index() < keys[2].index());
    }

    #[test]
    fn theme_default() {
        let theme = Theme::default();
        assert!(theme.is_empty());
    }

    #[test]
    fn theme_get_wrong_type_returns_none() {
        let theme = ThemeBuilder::new().set(PRIMARY, 0x0078D4_u32).build();

        // PRIMARY is u32, trying to get as f64 fails
        assert!(theme.get::<f64>(PRIMARY).is_none());
    }

    #[test]
    fn theme_get_erased_returns_owned_value() {
        let theme = ThemeBuilder::new().set(FONT_SIZE, 14.0_f64).build();

        let value = theme.get_erased(FONT_SIZE).unwrap();
        assert_eq!(value.downcast_ref::<f64>(), Some(&14.0));
        assert!(theme.get_erased(PRIMARY).is_none());
    }

    #[test]
    fn theme_variant_basics() {
        const SEPIA: ThemeVariant = ThemeVariant::new(16);

        assert_eq!(ThemeVariant::LIGHT.index(), 0);
        assert_eq!(ThemeVariant::DARK.index(), 1);
        assert_eq!(ThemeVariant::HIGH_CONTRAST.index(), 2);
        assert_eq!(SEPIA.index(), 16);
        assert_ne!(SEPIA, ThemeVariant::DARK);
        assert_eq!(ThemeVariant::new(1), ThemeVariant::DARK);
    }

    #[test]
    fn theme_variant_debug_and_display() {
        use alloc::format;

        assert_eq!(format!("{:?}", ThemeVariant::LIGHT), "ThemeVariant(Light)");
        assert_eq!(format!("{:?}", ThemeVariant::DARK), "ThemeVariant(Dark)");
        assert_eq!(
            format!("{:?}", ThemeVariant::HIGH_CONTRAST),
            "ThemeVariant(HighContrast)"
        );
        assert_eq!(format!("{:?}", ThemeVariant::new(16)), "ThemeVariant(16)");
        assert_eq!(format!("{}", ThemeVariant::DARK), "ThemeVariant(Dark)");
    }

    fn light_dark_theme() -> VariantTheme {
        VariantThemeBuilder::new()
            .base(
                ThemeBuilder::new()
                    .set(PRIMARY, 0x0078D4_u32)
                    .set(FONT_SIZE, 14.0_f64)
                    .build(),
            )
            .variant(
                ThemeVariant::DARK,
                ThemeBuilder::new().set(PRIMARY, 0x4CC2FF_u32).build(),
            )
            .build()
    }

    #[test]
    fn variant_theme_variant_overrides_base() {
        let theme = light_dark_theme();
        let dark = Some(ThemeVariant::DARK);

        assert_eq!(theme.get::<u32>(dark, PRIMARY), Some(&0x4CC2FF));
        assert_eq!(
            theme
                .get_erased(dark, PRIMARY)
                .unwrap()
                .downcast_ref::<u32>(),
            Some(&0x4CC2FF)
        );
    }

    #[test]
    fn variant_theme_falls_back_to_base() {
        let theme = light_dark_theme();
        let dark = Some(ThemeVariant::DARK);

        // FONT_SIZE has no dark override, so the base entry wins.
        assert_eq!(theme.get::<f64>(dark, FONT_SIZE), Some(&14.0));
        assert!(theme.contains(dark, FONT_SIZE));
    }

    #[test]
    fn variant_theme_unknown_variant_falls_back_to_base() {
        let theme = light_dark_theme();
        let sepia = Some(ThemeVariant::new(16));

        assert!(theme.variant(ThemeVariant::new(16)).is_none());
        assert_eq!(theme.get::<u32>(sepia, PRIMARY), Some(&0x0078D4));
        assert_eq!(theme.get::<f64>(sepia, FONT_SIZE), Some(&14.0));
    }

    #[test]
    fn variant_theme_no_variant_uses_base_only() {
        let theme = light_dark_theme();

        assert_eq!(theme.get::<u32>(None, PRIMARY), Some(&0x0078D4));
        assert!(!theme.contains(None, SECONDARY));
    }

    #[test]
    fn variant_theme_shadows_by_key_presence() {
        // The dark table stores PRIMARY as a string, shadowing the base u32.
        let theme = VariantThemeBuilder::new()
            .base(ThemeBuilder::new().set(PRIMARY, 0x0078D4_u32).build())
            .variant(
                ThemeVariant::DARK,
                ThemeBuilder::new()
                    .set(PRIMARY, "#4CC2FF".to_string())
                    .build(),
            )
            .build();
        let dark = Some(ThemeVariant::DARK);

        // Presence shadows: a typed lookup does not fall through to the base
        // entry of a different type.
        assert_eq!(theme.get::<u32>(dark, PRIMARY), None);
        assert_eq!(
            theme
                .get_erased(dark, PRIMARY)
                .unwrap()
                .downcast_ref::<String>()
                .map(|s| s.as_str()),
            Some("#4CC2FF")
        );
    }

    #[test]
    fn variant_theme_builder_replaces_variant_table() {
        let theme = VariantThemeBuilder::new()
            .variant(
                ThemeVariant::DARK,
                ThemeBuilder::new().set(PRIMARY, 1_u32).build(),
            )
            .variant(
                ThemeVariant::DARK,
                ThemeBuilder::new().set(PRIMARY, 2_u32).build(),
            )
            .build();

        assert_eq!(theme.variants().count(), 1);
        assert_eq!(
            theme.get::<u32>(Some(ThemeVariant::DARK), PRIMARY),
            Some(&2)
        );
    }

    #[test]
    fn variant_theme_from_theme_is_base_only() {
        let theme = VariantTheme::from(ThemeBuilder::new().set(PRIMARY, 7_u32).build());

        assert_eq!(
            theme.get::<u32>(Some(ThemeVariant::DARK), PRIMARY),
            Some(&7)
        );
        assert_eq!(theme.variants().count(), 0);
    }

    #[test]
    fn theme_scope_child_shadows_outer_scope() {
        let root = ThemeScope::new(ThemeBuilder::new().set(PRIMARY, 1_u32).build());
        let popup = root.child(ThemeBuilder::new().set(PRIMARY, 2_u32).build());

        assert_eq!(popup.get::<u32>(None, PRIMARY), Some(&2));
        assert_eq!(root.get::<u32>(None, PRIMARY), Some(&1));
    }

    #[test]
    fn theme_scope_falls_back_to_root() {
        let root = ThemeScope::new(
            ThemeBuilder::new()
                .set(PRIMARY, 1_u32)
                .set(FONT_SIZE, 14.0_f64)
                .build(),
        );
        let inner = root
            .child(ThemeBuilder::new().set(PRIMARY, 2_u32).build())
            .child(ThemeBuilder::new().set(SECONDARY, 3_u32).build());

        assert_eq!(inner.depth(), 2);
        assert_eq!(inner.get::<u32>(None, SECONDARY), Some(&3));
        assert_eq!(inner.get::<u32>(None, PRIMARY), Some(&2));
        assert_eq!(inner.get::<f64>(None, FONT_SIZE), Some(&14.0));
        assert!(!inner.contains(None, ResourceKey::new(99)));
    }

    #[test]
    fn theme_scope_applies_variant_fallback_at_each_level() {
        // Root theme has base and dark tables; the child scope has only a base
        // entry for PRIMARY.
        let root = ThemeScope::new(
            VariantThemeBuilder::new()
                .base(
                    ThemeBuilder::new()
                        .set(PRIMARY, 1_u32)
                        .set(SECONDARY, 10_u32)
                        .build(),
                )
                .variant(
                    ThemeVariant::DARK,
                    ThemeBuilder::new()
                        .set(PRIMARY, 2_u32)
                        .set(SECONDARY, 20_u32)
                        .build(),
                )
                .build(),
        );
        let island = root.child(ThemeBuilder::new().set(PRIMARY, 3_u32).build());
        let dark = Some(ThemeVariant::DARK);

        // The inner scope wins entirely for keys it contains: its base entry
        // shadows the outer scope's dark entry.
        assert_eq!(island.get::<u32>(dark, PRIMARY), Some(&3));
        // Keys the inner scope lacks fall back outward with variant fallback.
        assert_eq!(island.get::<u32>(dark, SECONDARY), Some(&20));
        assert_eq!(island.get::<u32>(None, SECONDARY), Some(&10));
    }

    #[test]
    fn theme_scope_clone_is_cheap() {
        let root = ThemeScope::new(ThemeBuilder::new().set(PRIMARY, 1_u32).build());
        let clone = root.clone();

        assert!(Rc::ptr_eq(&root.inner, &clone.inner));
        assert!(root.parent().is_none());
        assert_eq!(root.depth(), 0);
    }

    #[test]
    fn theme_scope_get_erased_walks_chain() {
        let root = ThemeScope::new(ThemeBuilder::new().set(FONT_SIZE, 14.0_f64).build());
        let inner = root.child(Theme::default());

        let value = inner.get_erased(None, FONT_SIZE).unwrap();
        assert_eq!(value.downcast_ref::<f64>(), Some(&14.0));
        assert!(inner.get_erased(None, PRIMARY).is_none());
    }
}
