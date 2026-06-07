// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Author-facing style vocabulary for style token ids.
//!
//! This module owns the mapping between style token ids defined by
//! `understory_style` and the names used by authors, parsers, inspectors, logs,
//! and traces. Selector matching, cascade resolution, theme lookup, hashing,
//! and equality remain based on the raw ids.

use alloc::string::String;
use alloc::vec::Vec;

use crate::{ClassId, PartTag, PseudoClassId, ResourceKey, TypeTag};

/// An author-facing name for one style token id.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StyleTokenName<T> {
    /// Token identifier.
    pub id: T,
    /// Author-facing token name.
    pub name: String,
}

/// An author-facing name for one owner-local part token.
///
/// The vocabulary records part names together with the owning [`TypeTag`] so
/// embedders can reuse local names and ids across unrelated owners. Selector
/// matching still compares raw [`PartTag`] values; callers must anchor part
/// selectors under an owner type when reused part ids should not match each
/// other.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StylePartName {
    /// Selector identity of the owner that defines the part.
    pub owner_type_tag: TypeTag,
    /// Owner-local part identifier.
    pub part_tag: PartTag,
    /// Author-facing part name.
    pub name: String,
}

/// Author-facing names and ids for style vocabulary used by one embedder.
///
/// Names are the stable authoring surface. Parsers, applications, inspectors,
/// and tools should call methods such as [`Self::class_id`] with an
/// author-facing name and treat the returned id as the compiled style handle.
/// Ids can still be stored and matched directly, but ordinary callers should
/// not coordinate raw integer ranges.
///
/// Names are exact. The vocabulary does not normalize, add, or strip language
/// sigils; a CSS-like parser that wants `.primary` and `:hover` in diagnostics
/// should pass those complete spellings.
///
/// Fresh ids start at raw value `1` in each token space. Raw value `0` is not
/// auto-allocated, but embedders may bind externally defined id `0` through
/// [`Self::id_bindings`] when they already expose such an id.
///
/// Embedders with pre-existing built-in ids can bind them through
/// [`Self::id_bindings`]. That path is for hydrating externally defined handles;
/// it is not the normal app/parser allocation API.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct StyleVocabulary {
    classes: Vec<StyleTokenName<ClassId>>,
    pseudos: Vec<StyleTokenName<PseudoClassId>>,
    type_tags: Vec<StyleTokenName<TypeTag>>,
    resources: Vec<StyleTokenName<ResourceKey>>,
    parts: Vec<StylePartName>,
}

impl StyleVocabulary {
    /// Creates an empty style vocabulary.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            classes: Vec::new(),
            pseudos: Vec::new(),
            type_tags: Vec::new(),
            resources: Vec::new(),
            parts: Vec::new(),
        }
    }

    /// Returns the class id for `name`, registering a fresh id if needed.
    ///
    /// This is the parser/app-authoring path for class names that are not
    /// already represented by fixed constants.
    ///
    /// `name` is stored exactly as supplied.
    ///
    /// # Panics
    ///
    /// Panics if all `u32` class ids are already registered.
    pub fn class_id(&mut self, name: impl Into<String>) -> ClassId {
        intern_token_name(&mut self.classes, name, ClassId, ClassId::raw, "class")
    }

    /// Returns the pseudoclass id for `name`, registering a fresh id if needed.
    ///
    /// `name` is stored exactly as supplied.
    ///
    /// # Panics
    ///
    /// Panics if all `u32` pseudoclass ids are already registered.
    pub fn pseudo_class_id(&mut self, name: impl Into<String>) -> PseudoClassId {
        intern_token_name(
            &mut self.pseudos,
            name,
            PseudoClassId,
            PseudoClassId::raw,
            "pseudoclass",
        )
    }

    /// Returns the type tag for `name`, registering a fresh tag if needed.
    ///
    /// `name` is stored exactly as supplied.
    ///
    /// # Panics
    ///
    /// Panics if all `u32` type tags are already registered.
    pub fn type_tag(&mut self, name: impl Into<String>) -> TypeTag {
        intern_token_name(&mut self.type_tags, name, TypeTag, TypeTag::raw, "type tag")
    }

    /// Returns the resource key for `name`, registering a fresh key if needed.
    ///
    /// This is the parser/app-authoring path for theme resources that are not
    /// already represented by fixed constants.
    ///
    /// `name` is stored exactly as supplied.
    ///
    /// # Panics
    ///
    /// Panics if all `u16` resource keys are already registered.
    pub fn resource_key(&mut self, name: impl Into<String>) -> ResourceKey {
        intern_token_name(
            &mut self.resources,
            name,
            resource_key_from_raw,
            |key| u32::from(key.index()),
            "resource",
        )
    }

    /// Returns the owner-local part tag for `name`, registering a fresh tag if needed.
    ///
    /// Part names are recorded with `owner_type_tag`: the same part name under
    /// two different owners can resolve to different [`PartTag`] values. The
    /// matcher still compares raw part tags, so selectors should include the
    /// owner type when owner-local ids may be reused.
    ///
    /// `name` is stored exactly as supplied.
    ///
    /// # Panics
    ///
    /// Panics if all `u32` part tags are already registered for this owner.
    pub fn part_tag(&mut self, owner_type_tag: TypeTag, name: impl Into<String>) -> PartTag {
        let name = name.into();
        if let Some(existing) = self.parts.iter().find(|candidate| {
            candidate.owner_type_tag == owner_type_tag && candidate.name.as_str() == name
        }) {
            return existing.part_tag;
        }

        let part_tag = next_part_tag(&self.parts, owner_type_tag);
        self.parts.push(StylePartName {
            owner_type_tag,
            part_tag,
            name,
        });
        part_tag
    }

    /// Returns a binding view for externally assigned ids.
    ///
    /// Most callers should use [`Self::class_id`], [`Self::pseudo_class_id`],
    /// [`Self::type_tag`], [`Self::resource_key`], and [`Self::part_tag`] so the
    /// vocabulary allocates ids from names. This view exists for embedders that
    /// already expose stable built-in ids and need the vocabulary to know their
    /// author-facing names.
    pub fn id_bindings(&mut self) -> StyleVocabularyIdBindings<'_> {
        StyleVocabularyIdBindings { vocabulary: self }
    }

    /// Resolves a reusable set of style tokens against this vocabulary.
    ///
    /// Token-set marker types let crates publish one canonical registration
    /// path for the classes, pseudoclasses, type tags, resources, and parts
    /// they own. The implementation may intern names through methods such as
    /// [`Self::class_id`] or bind pre-existing ids through [`Self::id_bindings`].
    pub fn style_tokens<T>(&mut self) -> T::Resolved
    where
        T: StyleTokenSet,
    {
        T::resolve(self)
    }

    /// Returns the registered id for a class name.
    #[must_use]
    pub fn class_id_by_name(&self, name: &str) -> Option<ClassId> {
        token_id_by_name(&self.classes, name)
    }

    /// Returns the registered id for a pseudoclass name.
    #[must_use]
    pub fn pseudo_class_id_by_name(&self, name: &str) -> Option<PseudoClassId> {
        token_id_by_name(&self.pseudos, name)
    }

    /// Returns the registered id for a type tag name.
    #[must_use]
    pub fn type_tag_by_name(&self, name: &str) -> Option<TypeTag> {
        token_id_by_name(&self.type_tags, name)
    }

    /// Returns the registered key for a resource name.
    #[must_use]
    pub fn resource_key_by_name(&self, name: &str) -> Option<ResourceKey> {
        token_id_by_name(&self.resources, name)
    }

    /// Returns the registered owner-local part id for a part name.
    #[must_use]
    pub fn part_tag_by_name(&self, owner_type_tag: TypeTag, name: &str) -> Option<PartTag> {
        self.parts
            .iter()
            .find(|candidate| {
                candidate.owner_type_tag == owner_type_tag && candidate.name.as_str() == name
            })
            .map(|candidate| candidate.part_tag)
    }

    /// Returns the registered name for a class id.
    #[must_use]
    pub fn class_name(&self, id: ClassId) -> Option<&str> {
        token_name(&self.classes, id)
    }

    /// Returns the registered name for a pseudoclass id.
    #[must_use]
    pub fn pseudo_name(&self, id: PseudoClassId) -> Option<&str> {
        token_name(&self.pseudos, id)
    }

    /// Returns the registered name for a type tag.
    #[must_use]
    pub fn type_name(&self, id: TypeTag) -> Option<&str> {
        token_name(&self.type_tags, id)
    }

    /// Returns the registered name for a resource key.
    #[must_use]
    pub fn resource_name(&self, id: ResourceKey) -> Option<&str> {
        token_name(&self.resources, id)
    }

    /// Returns the registered name for an owner-local part tag.
    #[must_use]
    pub fn part_name(&self, owner_type_tag: TypeTag, part_tag: PartTag) -> Option<&str> {
        self.parts
            .iter()
            .find(|candidate| {
                candidate.owner_type_tag == owner_type_tag && candidate.part_tag == part_tag
            })
            .map(|candidate| candidate.name.as_str())
    }

    /// Returns all registered class names in registration order.
    #[must_use]
    pub fn classes(&self) -> &[StyleTokenName<ClassId>] {
        &self.classes
    }

    /// Returns all registered pseudoclass names in registration order.
    #[must_use]
    pub fn pseudos(&self) -> &[StyleTokenName<PseudoClassId>] {
        &self.pseudos
    }

    /// Returns all registered type tag names in registration order.
    #[must_use]
    pub fn type_tags(&self) -> &[StyleTokenName<TypeTag>] {
        &self.type_tags
    }

    /// Returns all registered resource names in registration order.
    #[must_use]
    pub fn resources(&self) -> &[StyleTokenName<ResourceKey>] {
        &self.resources
    }

    /// Returns all registered owner-local part names in registration order.
    #[must_use]
    pub fn parts(&self) -> &[StylePartName] {
        &self.parts
    }
}

/// Reusable declaration of style tokens owned by a crate or subsystem.
///
/// Implement this trait on a zero-sized marker type, then resolve it through
/// [`StyleVocabulary::style_tokens`]. Implementations should register every
/// token they own and return the compiled handles that downstream selector
/// construction needs.
///
/// ```rust
/// use understory_style::{ClassId, ResourceKey, StyleTokenSet, StyleVocabulary};
///
/// struct AppStyleTokens;
///
/// #[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// struct AppTokens {
///     card: ClassId,
///     card_background: ResourceKey,
/// }
///
/// impl StyleTokenSet for AppStyleTokens {
///     type Resolved = AppTokens;
///
///     fn resolve(vocabulary: &mut StyleVocabulary) -> Self::Resolved {
///         AppTokens {
///             card: vocabulary.class_id(".card"),
///             card_background: vocabulary.resource_key("card.background"),
///         }
///     }
/// }
///
/// let mut vocabulary = StyleVocabulary::new();
/// let tokens = vocabulary.style_tokens::<AppStyleTokens>();
/// assert_eq!(vocabulary.class_name(tokens.card), Some(".card"));
/// assert_eq!(
///     vocabulary.resource_name(tokens.card_background),
///     Some("card.background")
/// );
/// ```
pub trait StyleTokenSet {
    /// The resolved handles returned after registration.
    type Resolved;

    /// Registers or resolves this token set against `vocabulary`.
    fn resolve(vocabulary: &mut StyleVocabulary) -> Self::Resolved;
}

/// Binding view for externally assigned style ids.
///
/// This is the advanced path for crates that already have fixed ids in their
/// public API. App code and parsers should use the name-interning methods on
/// [`StyleVocabulary`] instead.
#[derive(Debug)]
pub struct StyleVocabularyIdBindings<'a> {
    vocabulary: &'a mut StyleVocabulary,
}

impl StyleVocabularyIdBindings<'_> {
    /// Binds an author-facing name to an externally assigned class id.
    ///
    /// Rebinding the same id to the same name is a no-op.
    ///
    /// # Panics
    ///
    /// Panics if the id already has a different name or the name already maps
    /// to a different id.
    pub fn class(&mut self, id: ClassId, name: impl Into<String>) -> &mut Self {
        bind_token_name(&mut self.vocabulary.classes, id, name, "class");
        self
    }

    /// Binds an author-facing name to an externally assigned pseudoclass id.
    ///
    /// Rebinding the same id to the same name is a no-op.
    ///
    /// # Panics
    ///
    /// Panics if the id already has a different name or the name already maps
    /// to a different id.
    pub fn pseudo_class(&mut self, id: PseudoClassId, name: impl Into<String>) -> &mut Self {
        bind_token_name(&mut self.vocabulary.pseudos, id, name, "pseudoclass");
        self
    }

    /// Binds an author-facing name to an externally assigned type tag.
    ///
    /// Rebinding the same id to the same name is a no-op.
    ///
    /// # Panics
    ///
    /// Panics if the id already has a different name or the name already maps
    /// to a different id.
    pub fn type_tag(&mut self, id: TypeTag, name: impl Into<String>) -> &mut Self {
        bind_token_name(&mut self.vocabulary.type_tags, id, name, "type tag");
        self
    }

    /// Binds an author-facing name to an externally assigned resource key.
    ///
    /// Rebinding the same key to the same name is a no-op.
    ///
    /// # Panics
    ///
    /// Panics if the key already has a different name or the name already maps
    /// to a different key.
    pub fn resource(&mut self, id: ResourceKey, name: impl Into<String>) -> &mut Self {
        bind_token_name(&mut self.vocabulary.resources, id, name, "resource");
        self
    }

    /// Binds an author-facing name to an externally assigned owner-local part id.
    ///
    /// Rebinding the same `(owner_type_tag, part_tag)` pair to the same name is
    /// a no-op.
    ///
    /// # Panics
    ///
    /// Panics if the owner-local part already has a different name or the name
    /// already maps to a different part id under the same owner.
    pub fn part(
        &mut self,
        owner_type_tag: TypeTag,
        part_tag: PartTag,
        name: impl Into<String>,
    ) -> &mut Self {
        let name = name.into();
        if let Some(existing) = self.vocabulary.parts.iter().find(|existing| {
            existing.owner_type_tag == owner_type_tag && existing.part_tag == part_tag
        }) {
            assert!(
                existing.name == name,
                "style part id already has a different name"
            );
            return self;
        }
        if let Some(existing) = self.vocabulary.parts.iter().find(|existing| {
            existing.owner_type_tag == owner_type_tag && existing.name.as_str() == name
        }) {
            assert!(
                existing.part_tag == part_tag,
                "style part name already has a different id"
            );
        }
        self.vocabulary.parts.push(StylePartName {
            owner_type_tag,
            part_tag,
            name,
        });
        self
    }
}

fn bind_token_name<T>(
    names: &mut Vec<StyleTokenName<T>>,
    id: T,
    name: impl Into<String>,
    token_kind: &str,
) where
    T: Copy + Eq,
{
    let name = name.into();
    if let Some(existing) = names.iter().find(|existing| existing.id == id) {
        assert!(
            existing.name == name,
            "style {token_kind} id already has a different name"
        );
        return;
    }
    if let Some(existing) = names.iter().find(|existing| existing.name == name) {
        assert!(
            existing.id == id,
            "style {token_kind} name already has a different id"
        );
    }
    names.push(StyleTokenName { id, name });
}

fn intern_token_name<T>(
    names: &mut Vec<StyleTokenName<T>>,
    name: impl Into<String>,
    from_raw: impl FnOnce(u32) -> T,
    to_raw: impl Fn(T) -> u32,
    token_kind: &str,
) -> T
where
    T: Copy + Eq,
{
    let name = name.into();
    if let Some(existing) = names
        .iter()
        .find(|candidate| candidate.name.as_str() == name)
    {
        return existing.id;
    }

    let id = from_raw(next_token_raw(names, to_raw, token_kind));
    names.push(StyleTokenName { id, name });
    id
}

fn token_id_by_name<T>(names: &[StyleTokenName<T>], name: &str) -> Option<T>
where
    T: Copy,
{
    names
        .iter()
        .find(|candidate| candidate.name.as_str() == name)
        .map(|candidate| candidate.id)
}

fn token_name<T>(names: &[StyleTokenName<T>], id: T) -> Option<&str>
where
    T: Copy + Eq,
{
    names
        .iter()
        .find(|candidate| candidate.id == id)
        .map(|candidate| candidate.name.as_str())
}

fn next_token_raw<T>(
    names: &[StyleTokenName<T>],
    to_raw: impl Fn(T) -> u32,
    token_kind: &str,
) -> u32
where
    T: Copy,
{
    names
        .iter()
        .map(|candidate| to_raw(candidate.id))
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .unwrap_or_else(|| panic!("too many style {token_kind} ids registered"))
}

fn next_part_tag(parts: &[StylePartName], owner_type_tag: TypeTag) -> PartTag {
    parts
        .iter()
        .filter(|candidate| candidate.owner_type_tag == owner_type_tag)
        .map(|candidate| candidate.part_tag.raw())
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .map(PartTag)
        .unwrap_or_else(|| panic!("too many style part ids registered for owner"))
}

fn resource_key_from_raw(raw: u32) -> ResourceKey {
    let index =
        u16::try_from(raw).unwrap_or_else(|_| panic!("too many style resource ids registered"));
    ResourceKey::new(index)
}

impl ClassId {
    const fn raw(self) -> u32 {
        self.0
    }
}

impl PseudoClassId {
    const fn raw(self) -> u32 {
        self.0
    }
}

impl TypeTag {
    const fn raw(self) -> u32 {
        self.0
    }
}

impl PartTag {
    const fn raw(self) -> u32 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_style_tokens() {
        let mut vocabulary = StyleVocabulary::new();

        vocabulary
            .id_bindings()
            .class(ClassId(1), ".primary")
            .pseudo_class(PseudoClassId(2), ":hover")
            .type_tag(TypeTag(3), "Button")
            .resource(ResourceKey::new(4), "accent.bg");

        assert_eq!(vocabulary.class_name(ClassId(1)), Some(".primary"));
        assert_eq!(vocabulary.pseudo_name(PseudoClassId(2)), Some(":hover"));
        assert_eq!(vocabulary.type_name(TypeTag(3)), Some("Button"));
        assert_eq!(
            vocabulary.resource_name(ResourceKey::new(4)),
            Some("accent.bg")
        );
        assert_eq!(vocabulary.class_name(ClassId(99)), None);
    }

    #[test]
    fn interns_style_tokens_by_name() {
        let mut vocabulary = StyleVocabulary::new();

        let class = vocabulary.class_id(".primary");
        let pseudo = vocabulary.pseudo_class_id(":hover");
        let type_tag = vocabulary.type_tag("Button");
        let resource = vocabulary.resource_key("accent.bg");

        assert_eq!(vocabulary.class_id(".primary"), class);
        assert_eq!(vocabulary.pseudo_class_id(":hover"), pseudo);
        assert_eq!(vocabulary.type_tag("Button"), type_tag);
        assert_eq!(vocabulary.resource_key("accent.bg"), resource);
        assert_eq!(vocabulary.class_name(class), Some(".primary"));
        assert_eq!(vocabulary.class_id_by_name(".primary"), Some(class));
        assert_eq!(vocabulary.pseudo_class_id_by_name(":hover"), Some(pseudo));
        assert_eq!(vocabulary.type_tag_by_name("Button"), Some(type_tag));
        assert_eq!(vocabulary.resource_key_by_name("accent.bg"), Some(resource));
    }

    #[test]
    fn style_token_sets_resolve_named_handles() {
        struct ExampleTokens;

        #[derive(Debug, Eq, PartialEq)]
        struct ExampleClasses {
            primary: ClassId,
            accent: ResourceKey,
        }

        impl StyleTokenSet for ExampleTokens {
            type Resolved = ExampleClasses;

            fn resolve(vocabulary: &mut StyleVocabulary) -> Self::Resolved {
                ExampleClasses {
                    primary: vocabulary.class_id(".primary"),
                    accent: vocabulary.resource_key("accent.bg"),
                }
            }
        }

        let mut vocabulary = StyleVocabulary::new();

        let classes = vocabulary.style_tokens::<ExampleTokens>();

        assert_eq!(classes.primary, ClassId(1));
        assert_eq!(classes.accent, ResourceKey::new(1));
        assert_eq!(vocabulary.class_name(classes.primary), Some(".primary"));
        assert_eq!(vocabulary.resource_name(classes.accent), Some("accent.bg"));
        assert_eq!(
            vocabulary.style_tokens::<ExampleTokens>(),
            ExampleClasses {
                primary: ClassId(1),
                accent: ResourceKey::new(1)
            }
        );
    }

    #[test]
    fn style_token_sets_can_bind_fixed_ids() {
        struct FixedTokens;

        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        struct FixedHandles {
            card: ClassId,
            hover: PseudoClassId,
            button: TypeTag,
            accent: ResourceKey,
            icon: PartTag,
        }

        impl StyleTokenSet for FixedTokens {
            type Resolved = FixedHandles;

            fn resolve(vocabulary: &mut StyleVocabulary) -> Self::Resolved {
                let handles = FixedHandles {
                    card: ClassId(40),
                    hover: PseudoClassId(41),
                    button: TypeTag(42),
                    accent: ResourceKey::new(44),
                    icon: PartTag(43),
                };

                vocabulary
                    .id_bindings()
                    .class(handles.card, ".card")
                    .pseudo_class(handles.hover, ":hover")
                    .type_tag(handles.button, "Button")
                    .resource(handles.accent, "accent.bg")
                    .part(handles.button, handles.icon, "Button.Icon");

                handles
            }
        }

        let mut vocabulary = StyleVocabulary::new();

        let handles = vocabulary.style_tokens::<FixedTokens>();

        assert_eq!(handles.card, ClassId(40));
        assert_eq!(vocabulary.class_name(handles.card), Some(".card"));
        assert_eq!(vocabulary.pseudo_name(handles.hover), Some(":hover"));
        assert_eq!(vocabulary.type_name(handles.button), Some("Button"));
        assert_eq!(vocabulary.resource_name(handles.accent), Some("accent.bg"));
        assert_eq!(
            vocabulary.part_name(handles.button, handles.icon),
            Some("Button.Icon")
        );
        assert_eq!(vocabulary.class_id_by_name(".card"), Some(handles.card));
        assert_eq!(
            vocabulary.pseudo_class_id_by_name(":hover"),
            Some(handles.hover)
        );
        assert_eq!(vocabulary.type_tag_by_name("Button"), Some(handles.button));
        assert_eq!(
            vocabulary.resource_key_by_name("accent.bg"),
            Some(handles.accent)
        );
        assert_eq!(
            vocabulary.part_tag_by_name(handles.button, "Button.Icon"),
            Some(handles.icon)
        );
        assert_eq!(vocabulary.style_tokens::<FixedTokens>(), handles);
    }

    #[test]
    fn auto_allocated_ids_start_at_one() {
        let mut vocabulary = StyleVocabulary::new();

        assert_eq!(vocabulary.class_id(".primary"), ClassId(1));
        assert_eq!(vocabulary.pseudo_class_id(":hover"), PseudoClassId(1));
        assert_eq!(vocabulary.type_tag("Button"), TypeTag(1));
        assert_eq!(vocabulary.resource_key("accent.bg"), ResourceKey::new(1));
        assert_eq!(vocabulary.part_tag(TypeTag(1), "Label"), PartTag(1));
    }

    #[test]
    fn externally_bound_zero_ids_are_valid() {
        let mut vocabulary = StyleVocabulary::new();

        vocabulary
            .id_bindings()
            .class(ClassId(0), ".root")
            .pseudo_class(PseudoClassId(0), ":initial")
            .type_tag(TypeTag(0), "Root")
            .resource(ResourceKey::new(0), "root.bg")
            .part(TypeTag(0), PartTag(0), "Root.Content");

        assert_eq!(vocabulary.class_name(ClassId(0)), Some(".root"));
        assert_eq!(vocabulary.pseudo_name(PseudoClassId(0)), Some(":initial"));
        assert_eq!(vocabulary.type_name(TypeTag(0)), Some("Root"));
        assert_eq!(
            vocabulary.resource_name(ResourceKey::new(0)),
            Some("root.bg")
        );
        assert_eq!(
            vocabulary.part_name(TypeTag(0), PartTag(0)),
            Some("Root.Content")
        );
        assert_eq!(vocabulary.class_id(".primary"), ClassId(1));
        assert_eq!(vocabulary.resource_key("accent.bg"), ResourceKey::new(1));
    }

    #[test]
    fn names_are_exact_author_facing_spellings() {
        let mut vocabulary = StyleVocabulary::new();

        let sigiled = vocabulary.class_id(".primary");
        let bare = vocabulary.class_id("primary");

        assert_ne!(sigiled, bare);
        assert_eq!(vocabulary.class_name(sigiled), Some(".primary"));
        assert_eq!(vocabulary.class_name(bare), Some("primary"));
    }

    #[test]
    fn scopes_part_names_by_owner_type() {
        let mut vocabulary = StyleVocabulary::new();

        vocabulary
            .id_bindings()
            .part(TypeTag(1), PartTag(1), "Button.Label")
            .part(TypeTag(2), PartTag(1), "Toggle.Thumb");

        assert_eq!(
            vocabulary.part_name(TypeTag(1), PartTag(1)),
            Some("Button.Label")
        );
        assert_eq!(
            vocabulary.part_name(TypeTag(2), PartTag(1)),
            Some("Toggle.Thumb")
        );
    }

    #[test]
    fn interns_part_names_by_owner_type() {
        let mut vocabulary = StyleVocabulary::new();

        let button_label = vocabulary.part_tag(TypeTag(1), "Label");
        let toggle_label = vocabulary.part_tag(TypeTag(2), "Label");

        assert_eq!(button_label, PartTag(1));
        assert_eq!(toggle_label, PartTag(1));
        assert_eq!(vocabulary.part_tag(TypeTag(1), "Label"), button_label);
        assert_eq!(vocabulary.part_tag(TypeTag(2), "Label"), toggle_label);
        assert_eq!(
            vocabulary.part_tag_by_name(TypeTag(1), "Label"),
            Some(button_label)
        );
        assert_eq!(
            vocabulary.part_tag_by_name(TypeTag(2), "Label"),
            Some(toggle_label)
        );
        assert_eq!(
            vocabulary.part_name(TypeTag(1), button_label),
            Some("Label")
        );
        assert_eq!(
            vocabulary.part_name(TypeTag(2), toggle_label),
            Some("Label")
        );
    }

    #[test]
    fn duplicate_same_name_is_idempotent() {
        let mut vocabulary = StyleVocabulary::new();

        vocabulary
            .id_bindings()
            .class(ClassId(1), ".primary")
            .class(ClassId(1), ".primary");

        assert_eq!(vocabulary.classes().len(), 1);
    }

    #[test]
    #[should_panic(expected = "style class id already has a different name")]
    fn duplicate_different_token_name_panics() {
        let mut vocabulary = StyleVocabulary::new();

        vocabulary
            .id_bindings()
            .class(ClassId(1), ".primary")
            .class(ClassId(1), ".secondary");
    }

    #[test]
    #[should_panic(expected = "style class name already has a different id")]
    fn duplicate_different_token_id_panics() {
        let mut vocabulary = StyleVocabulary::new();

        vocabulary
            .id_bindings()
            .class(ClassId(1), ".primary")
            .class(ClassId(2), ".primary");
    }

    #[test]
    #[should_panic(expected = "style resource name already has a different id")]
    fn duplicate_different_resource_id_panics() {
        let mut vocabulary = StyleVocabulary::new();

        vocabulary
            .id_bindings()
            .resource(ResourceKey::new(1), "accent.bg")
            .resource(ResourceKey::new(2), "accent.bg");
    }

    #[test]
    #[should_panic(expected = "style part id already has a different name")]
    fn duplicate_different_part_name_panics() {
        let mut vocabulary = StyleVocabulary::new();

        vocabulary
            .id_bindings()
            .part(TypeTag(1), PartTag(1), "Button.Label")
            .part(TypeTag(1), PartTag(1), "Button.Icon");
    }

    #[test]
    #[should_panic(expected = "style part name already has a different id")]
    fn duplicate_different_part_id_panics() {
        let mut vocabulary = StyleVocabulary::new();

        vocabulary
            .id_bindings()
            .part(TypeTag(1), PartTag(1), "Button.Label")
            .part(TypeTag(1), PartTag(2), "Button.Label");
    }
}
