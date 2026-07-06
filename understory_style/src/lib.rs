// Copyright 2025 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

// After you edit the crate's doc comment, run this command, then check README.md for any missing links
// cargo rdme --workspace-project=understory_style --heading-base-level=0

//! Understory Style: presentation policy for dependency properties.
//!
//! This crate extends `understory_property` with selector matching, cascade
//! rules, theme resources, style expressions, and expression-aware value
//! resolution. It is the presentation policy layer: it decides which property
//! value wins for a styled subject, but it does not own the app's reactive
//! runtime.
//!
//! Resolution follows this precedence chain:
//!
//! **Animation → Local → Style → Resource fallback → Inherited → Expression default → Default**
//!
//! The resource fallback stage is present only when a configured
//! [`ResolveQuery`] supplies one. Theme values are also consulted when a winning
//! style entry or expression token points at a [`ResourceKey`].
//!
//! ## Start Here
//!
//! Use this crate when your application already owns a tree of UI, template, or
//! model nodes and needs style matching over that tree. `understory_style` does
//! not store the tree. The embedder walks its own style subjects and carries a
//! compact [`MatchState`] from parent to child. Selectors can start at any
//! entered subject, so a one-step selector such as `Button` or `SliderThumb`
//! matches nested subjects as well as roots; parent state is still used to
//! continue child and descendant selectors.
//!
//! The crate owns:
//!
//! - selector matching over child and descendant paths;
//! - optional style vocabulary interning from author-facing names to compact ids;
//! - style values, style expressions, and theme resources for dependency properties;
//! - expression-aware resolution through animation, local, style, inheritance,
//!   expression defaults, and registry defaults;
//! - conservative style invalidation facts through `invalidation` channels;
//! - inspection hooks for matched rules and winning style sources.
//!
//! The crate does not own widgets, templates, bindings, animation sampling,
//! layout, rendering, event dispatch, caching, dirty queues, frame scheduling,
//! sibling relationships, parent queries, or structural selectors such as
//! `nth-*`, `odd`, or `even`.
//!
//! ### Glossary
//!
//! - **Style subject**: one addressable item in the embedder's walk. It may be
//!   an element, generated template node, model row, or widget part.
//! - **`TypeTag`**: an application-defined subject kind, such as `Button`,
//!   `Toggle`, or `Row`.
//! - **`PartTag`**: an owner-local part label, such as `track`, `thumb`, or
//!   `icon`.
//! - **`StyleVocabulary`**: the name-to-id table used by parsers, tools, and
//!   embedders that want author-facing selector and resource names without
//!   coordinating integer id ranges by hand.
//! - **`SelectorInputs`**: the type, part, class, and pseudoclass snapshot for
//!   one subject.
//! - **`MatchState`**: matcher progress after entering a subject. It is valid
//!   only with the cascade that produced it.
//!
//! ### Vocabulary
//!
//! Selector matching and theme lookup use compact ids, but parsers and tools
//! usually start from names. [`StyleVocabulary`] is the shared name-to-id layer
//! for that boundary:
//!
//! ```rust
//! use understory_style::{ClassId, ResourceKey, StyleTokenSet, StyleVocabulary};
//!
//! struct AppStyleTokens;
//!
//! #[derive(Clone, Copy)]
//! struct AppTokens {
//!     primary: ClassId,
//!     accent_background: ResourceKey,
//! }
//!
//! impl StyleTokenSet for AppStyleTokens {
//!     type Resolved = AppTokens;
//!
//!     fn resolve(vocabulary: &mut StyleVocabulary) -> Self::Resolved {
//!         AppTokens {
//!             primary: vocabulary.class_id(".primary"),
//!             accent_background: vocabulary.resource_key("accent.background"),
//!         }
//!     }
//! }
//!
//! let mut vocabulary = StyleVocabulary::new();
//! let tokens = vocabulary.style_tokens::<AppStyleTokens>();
//! let hover = vocabulary.pseudo_class_id(":hover");
//!
//! assert_eq!(vocabulary.class_name(tokens.primary), Some(".primary"));
//! assert_eq!(
//!     vocabulary.resource_name(tokens.accent_background),
//!     Some("accent.background")
//! );
//! assert_eq!(vocabulary.pseudo_name(hover), Some(":hover"));
//! ```
//!
//! Names are exact author-facing spellings. The vocabulary does not add or
//! remove CSS-like sigils, so choose one canonical spelling for the language or
//! parser that owns the names.
//!
//! ### First Example: Owner State Styling A Part
//!
//! This styles a `Toggle` owner's `track` part when the owner has `:checked`.
//! The checked state stays on the owner; it is not copied into the part inputs.
//!
//! ```rust
//! use invalidation::Channel;
//! use understory_property::{PropertyMetadataBuilder, PropertyRegistry};
//! use understory_style::{
//!     PartTag, PseudoClassId, Selector, SelectorInputs, SelectorStep, StyleBuilder,
//!     StyleCascadeBuilder, StyleOrigin, TypeTag,
//! };
//!
//! const PAINT: Channel = Channel::new(1);
//! const TOGGLE: TypeTag = TypeTag(1);
//! const TRACK: PartTag = PartTag(1);
//! const CHECKED: PseudoClassId = PseudoClassId(1);
//!
//! let mut registry = PropertyRegistry::new();
//! let background = registry.register(
//!     "Background",
//!     PropertyMetadataBuilder::new(0_u32)
//!         .affects_channels(PAINT.into_set())
//!         .build(),
//! );
//!
//! let cascade = StyleCascadeBuilder::new()
//!     .push_rule(
//!         StyleOrigin::Sheet,
//!         Selector::child(
//!             SelectorStep::type_tag(TOGGLE).with_pseudo(CHECKED),
//!             SelectorStep::part_tag(TRACK),
//!         ),
//!         StyleBuilder::new().set(background, 0x00ff00_u32).build(),
//!     )
//!     .build();
//!
//! let unchecked_owner = cascade.enter_subject(
//!     cascade.root_state(),
//!     &SelectorInputs::typed(TOGGLE),
//! );
//! let unchecked_track = cascade.enter_subject(
//!     unchecked_owner,
//!     &SelectorInputs::part(TRACK),
//! );
//!
//! let checked = [CHECKED];
//! let checked_owner = cascade.enter_subject(
//!     cascade.root_state(),
//!     &SelectorInputs::typed_with_pseudos(TOGGLE, &checked),
//! );
//! let restyle = cascade.restyle_subject(
//!     &registry,
//!     unchecked_track,
//!     checked_owner,
//!     &SelectorInputs::part(TRACK),
//! );
//!
//! assert_eq!(cascade.get_value_ref(restyle.state(), background), Some(&0x00ff00));
//! assert!(restyle.changed_channels().contains(PAINT));
//! assert_eq!(cascade.matching_rules(restyle.state()).count(), 1);
//! assert!(cascade.winning_source(restyle.state(), background).unwrap().rule().is_some());
//! ```
//!
//! ### Long-Lived Rules Of Thumb
//!
//! Anchor part selectors under an owner [`TypeTag`]. [`PartTag`] values are
//! application-defined and may be reused by unrelated owners:
//!
//! ```rust
//! use understory_style::{PartTag, Selector, SelectorStep, TypeTag};
//!
//! const BUTTON: TypeTag = TypeTag(1);
//! const ROW: TypeTag = TypeTag(2);
//! const LOCAL_PART: PartTag = PartTag(1);
//!
//! let button_part = Selector::child(
//!     SelectorStep::type_tag(BUTTON),
//!     SelectorStep::part_tag(LOCAL_PART),
//! );
//! let row_part = Selector::child(
//!     SelectorStep::type_tag(ROW),
//!     SelectorStep::part_tag(LOCAL_PART),
//! );
//!
//! assert_ne!(button_part.steps()[0], row_part.steps()[0]);
//! ```
//!
//! Use [`SelectorInputsOwned`] when classes or pseudoclasses come from unsorted
//! application data. It sorts and deduplicates before exposing borrowed
//! [`SelectorInputs`].
//!
//! For integration debugging, use [`StyleCascade::matching_rules`],
//! [`StyleCascade::winning_source`], and [`Selector::diagnose_path`]. These are
//! deliberately small diagnostics for the current child / descendant grammar,
//! not a browser-CSS explanation engine.
//!
//! ## Reference Concepts
//!
//! ### Styles
//!
//! [`Style`] is a shared collection of property setters. Unlike per-element
//! storage, styles are immutable after creation and can be shared across
//! many elements—matching `WinUI`'s `OptimizedStyle` approach.
//!
//! ```rust
//! use understory_style::{Style, StyleBuilder};
//! use understory_property::{PropertyMetadataBuilder, PropertyRegistry};
//!
//! let mut registry = PropertyRegistry::new();
//! let width = registry.register("Width", PropertyMetadataBuilder::new(0.0_f64).build());
//! let height = registry.register("Height", PropertyMetadataBuilder::new(0.0_f64).build());
//!
//! // Create a shared style
//! let button_style = StyleBuilder::new()
//!     .set(width, 100.0)
//!     .set(height, 40.0)
//!     .build();
//!
//! // Multiple elements can reference the same style
//! assert_eq!(button_style.get(width), Some(&100.0));
//! ```
//!
//! ### Themes
//!
//! [`Theme`] provides resource lookup by key. Themes map resource keys to
//! typed values, enabling theming (light/dark modes, brand colors, etc.).
//!
//! ```rust
//! use understory_style::{Theme, ThemeBuilder, ResourceKey};
//!
//! // Define resource keys as constants
//! const ACCENT_COLOR: ResourceKey = ResourceKey::new(0);
//! const FONT_SIZE: ResourceKey = ResourceKey::new(1);
//!
//! let light_theme = ThemeBuilder::new()
//!     .set(ACCENT_COLOR, 0x0078D4_u32)  // Blue
//!     .set(FONT_SIZE, 14.0_f64)
//!     .build();
//!
//! let dark_theme = ThemeBuilder::new()
//!     .set(ACCENT_COLOR, 0x4CC2FF_u32)  // Light blue
//!     .set(FONT_SIZE, 14.0_f64)
//!     .build();
//!
//! assert_eq!(light_theme.get::<u32>(ACCENT_COLOR), Some(&0x0078D4));
//! ```
//!
//! ### Expression Defaults
//!
//! Expression defaults derive a property value from other resolved properties
//! and theme resources. App code normally keeps one [`ExpressionLayer`] and
//! passes it to [`ResolveCx::new`].
//!
//! ```rust
//! use understory_property::{
//!     DependencyObject, PropertyMetadataBuilder, PropertyRegistry, PropertyStore,
//! };
//! use understory_style::{
//!     ExpressionLayer, NoResolveParentLookup, ResolveCx, ResourceKey, ThemeBuilder, expr,
//! };
//!
//! const GAP_TOKEN: ResourceKey = ResourceKey::new(0);
//!
//! struct Element {
//!     key: u32,
//!     parent: Option<u32>,
//!     store: PropertyStore<u32>,
//! }
//!
//! impl DependencyObject<u32> for Element {
//!     fn property_store(&self) -> &PropertyStore<u32> { &self.store }
//!     fn property_store_mut(&mut self) -> &mut PropertyStore<u32> { &mut self.store }
//!     fn key(&self) -> u32 { self.key }
//!     fn parent_key(&self) -> Option<u32> { self.parent }
//! }
//!
//! let mut registry = PropertyRegistry::new();
//! let scale = registry.register("Scale", PropertyMetadataBuilder::new(2.0_f64).build());
//! let padding = registry.register("Padding", PropertyMetadataBuilder::new(0.0_f64).build());
//!
//! let mut expressions = ExpressionLayer::new();
//! expressions.set_default(padding, expr::prop(scale) * 2.0 + expr::token(GAP_TOKEN));
//!
//! let theme = ThemeBuilder::new().set(GAP_TOKEN, 4.0_f64).build();
//! let element = Element {
//!     key: 1,
//!     parent: None,
//!     store: PropertyStore::new(1),
//! };
//! let cx = ResolveCx::new(&registry, &theme, &expressions, NoResolveParentLookup);
//!
//! let value = cx.get_value(&element, padding, None);
//! assert_eq!(value, 8.0);
//! ```
//!
//! ### Style Expressions
//!
//! Style entries can also be expressions. [`ResolveCx`] evaluates them through
//! the expression layer it carries.
//!
//! ```rust
//! use understory_property::{
//!     DependencyObject, PropertyMetadataBuilder, PropertyRegistry, PropertyStore,
//! };
//! use understory_style::{
//!     ExpressionLayer, NoResolveParentLookup, ResolveCx, ResourceKey, StyleBuilder,
//!     StyleCascadeBuilder, StyleOrigin, ThemeBuilder, expr,
//! };
//!
//! const GAP_TOKEN: ResourceKey = ResourceKey::new(0);
//!
//! struct Element {
//!     key: u32,
//!     parent: Option<u32>,
//!     store: PropertyStore<u32>,
//! }
//!
//! impl DependencyObject<u32> for Element {
//!     fn property_store(&self) -> &PropertyStore<u32> { &self.store }
//!     fn property_store_mut(&mut self) -> &mut PropertyStore<u32> { &mut self.store }
//!     fn key(&self) -> u32 { self.key }
//!     fn parent_key(&self) -> Option<u32> { self.parent }
//! }
//!
//! let mut registry = PropertyRegistry::new();
//! let scale = registry.register("Scale", PropertyMetadataBuilder::new(2.0_f64).build());
//! let width = registry.register("Width", PropertyMetadataBuilder::new(0.0_f64).build());
//!
//! let style = StyleBuilder::new()
//!     .set(scale, 3.0)
//!     .set_expr(width, expr::prop(scale) * 2.0 + expr::token(GAP_TOKEN))
//!     .build();
//! let cascade = StyleCascadeBuilder::new()
//!     .push_style(StyleOrigin::Override, style)
//!     .build();
//!
//! let theme = ThemeBuilder::new().set(GAP_TOKEN, 4.0_f64).build();
//! let expressions = ExpressionLayer::new();
//! let element = Element {
//!     key: 1,
//!     parent: None,
//!     store: PropertyStore::new(1),
//! };
//! let cx = ResolveCx::new(&registry, &theme, &expressions, NoResolveParentLookup);
//!
//! let value = cx.get_value(&element, width, Some((&cascade, cascade.root_state())));
//! assert_eq!(value, 10.0);
//! ```
//!
//! ### Expression Dependency Inspection
//!
//! Expressions expose static dependency sets through
//! [`ExprDeps`]. Hosts can inspect
//! style expressions with [`Style::expression_entries`] and default expressions
//! with
//! [`ExpressionDefaults::expression_entries`][understory_property_expression::ExpressionDefaults::expression_entries]
//! to build an invalidation index without evaluating expression bodies.
//!
//! The usual host policy is:
//!
//! - When a dependency property in `entry.deps().properties` changes on a
//!   subject, invalidate the entry's target property on that subject.
//! - If the target property inherits, also invalidate descendants that can
//!   observe the inherited result.
//! - When a dependency resource in `entry.deps().resources` changes, invalidate
//!   target properties whose expressions mention that resource. A conservative
//!   whole-theme invalidation can instead invalidate every expression target.
//! - Convert each invalidated target property to work with
//!   [`PropertyRegistry::affects_channels`][understory_property::PropertyRegistry::affects_channels].
//!
//! `understory_style` reports the dependency facts; it deliberately does not
//! own caching, scheduling, dirty queues, or tree-wide invalidation walks.
//!
//! ### Host Frame Loop
//!
//! A typical host update loop keeps the orchestration outside this crate:
//!
//! 1. Apply app, input, or model changes to dependency-property stores.
//! 2. Drain `understory_property_binding` so template and app bindings write
//!    their target local-source slots.
//! 3. Walk style subjects with [`StyleCascade::enter_subject`] or
//!    [`StyleCascade::restyle_subject`] when selector inputs change.
//! 4. Expand expression dependencies with [`Style::expression_entries`] and
//!    [`ExpressionLayer::defaults`] so property and resource changes invalidate
//!    derived targets.
//! 5. Sample animation or motion systems into animation slots.
//! 6. Resolve values through [`ResolveCx`] for rendering,
//!    layout, accessibility, or diagnostics.
//!
//! The `style_reactive_loop` example in the workspace demonstrates this full
//! shape end to end. The `style_motion_loop` example focuses on the
//! style-to-motion handoff: resolve the styled target, sample a
//! `understory_motion` transition, write each sample to the animation slot, and
//! clear that slot when the transition completes so the styled target is visible
//! again.
//!
//! ### Configured Resolution Queries
//!
//! [`ResolveCx::get_value`] and [`ResolveCx::explain_value`] are the normal
//! APIs. Use [`ResolveCx::query`] when the lookup policy itself is part of the
//! question, such as combining style state with a property-level resource
//! fallback.
//!
//! ```rust
//! use understory_property::{
//!     DependencyObject, PropertyMetadataBuilder, PropertyRegistry, PropertyStore,
//! };
//! use understory_style::{
//!     ExpressionLayer, NoResolveParentLookup, ResolveCx, ResourceKey, ThemeBuilder, expr,
//! };
//!
//! const WIDTH_TOKEN: ResourceKey = ResourceKey::new(1);
//! const DEFAULT_WIDTH: ResourceKey = ResourceKey::new(2);
//!
//! struct Element {
//!     key: u32,
//!     parent: Option<u32>,
//!     store: PropertyStore<u32>,
//! }
//!
//! impl DependencyObject<u32> for Element {
//!     fn property_store(&self) -> &PropertyStore<u32> { &self.store }
//!     fn property_store_mut(&mut self) -> &mut PropertyStore<u32> { &mut self.store }
//!     fn key(&self) -> u32 { self.key }
//!     fn parent_key(&self) -> Option<u32> { self.parent }
//! }
//!
//! let mut registry = PropertyRegistry::new();
//! let width = registry.register("Width", PropertyMetadataBuilder::new(0.0_f64).build());
//!
//! let mut expressions = ExpressionLayer::new();
//! expressions.set_default(width, expr::token(DEFAULT_WIDTH));
//!
//! let theme = ThemeBuilder::new()
//!     .set(WIDTH_TOKEN, 80.0_f64)
//!     .set(DEFAULT_WIDTH, 40.0_f64)
//!     .build();
//! let element = Element {
//!     key: 1,
//!     parent: None,
//!     store: PropertyStore::new(1),
//! };
//! let resolve = ResolveCx::new(&registry, &theme, &expressions, NoResolveParentLookup);
//!
//! let value = resolve
//!     .query(&element, width)
//!     .resource_fallback(WIDTH_TOKEN)
//!     .try_value()
//!     .unwrap();
//!
//! assert_eq!(value, 80.0);
//! ```
//!
//! ### Resolution Context
//!
//! [`ResolveCx`] bundles everything needed to resolve property values through
//! animation, local, style, inheritance, and default stages. This avoids passing
//! many parameters to resolution functions.
//!
//! ```rust
//! use understory_style::{
//!     ClassId, ExpressionLayer, NoResolveParentLookup, SelectorStep, PseudoClassId, ResolveCx,
//!     SelectorInputs, StyleCascade, StyleCascadeBuilder, StyleBuilder, StyleOrigin, PartTag,
//!     ThemeBuilder,
//! };
//! use understory_property::{
//!     DependencyObject, PropertyMetadataBuilder, PropertyRegistry, PropertyStore,
//! };
//!
//! let mut registry = PropertyRegistry::new();
//! let width = registry.register("Width", PropertyMetadataBuilder::new(0.0_f64).build());
//!
//! let theme = ThemeBuilder::new().build();
//! let expressions = ExpressionLayer::new();
//!
//! const PRIMARY: ClassId = ClassId(1);
//! const HOVER: PseudoClassId = PseudoClassId(1);
//! const ICON: PartTag = PartTag(1);
//!
//! // Base style for a "button"
//! let base = StyleBuilder::new().set(width, 100.0).build();
//! // Hover style when PRIMARY + HOVER
//! let hover = StyleBuilder::new().set(width, 120.0).build();
//!
//! let style: StyleCascade = StyleCascadeBuilder::new()
//!     .push_style(StyleOrigin::Base, base)
//!     .push_rule(
//!         StyleOrigin::Sheet,
//!         SelectorStep::class(PRIMARY).with_pseudo(HOVER),
//!         hover,
//!     )
//!     .build();
//!
//! struct Element {
//!     key: u32,
//!     parent: Option<u32>,
//!     store: PropertyStore<u32>,
//! }
//!
//! impl DependencyObject<u32> for Element {
//!     fn property_store(&self) -> &PropertyStore<u32> { &self.store }
//!     fn property_store_mut(&mut self) -> &mut PropertyStore<u32> { &mut self.store }
//!     fn key(&self) -> u32 { self.key }
//!     fn parent_key(&self) -> Option<u32> { self.parent }
//! }
//!
//! let element = Element {
//!     key: 1,
//!     parent: None,
//!     store: PropertyStore::new(1),
//! };
//!
//! // Create resolution context. Flat doctests use NoResolveParentLookup; apps
//! // normally pass their tree's parent/style-state lookup here.
//! let cx = ResolveCx::new(&registry, &theme, &expressions, NoResolveParentLookup);
//!
//! // Resolve with style (no hover)
//! let inputs = SelectorInputs::new(None, &[PRIMARY], &[]);
//! let state = style.enter_subject(style.root_state(), &inputs);
//! let value = cx.get_value(&element, width, Some((&style, state)));
//! assert_eq!(value, 100.0);
//!
//! // Resolve with style (hovered)
//! let hovered = SelectorInputs::new(None, &[PRIMARY], &[HOVER]);
//! let hovered_state = style.enter_subject(style.root_state(), &hovered);
//! let value = cx.get_value(&element, width, Some((&style, hovered_state)));
//! assert_eq!(value, 120.0);
//!
//! // Parts are owner-local style addresses supplied by the embedder.
//! let icon_inputs = SelectorInputs::with_part(None, Some(ICON), &[PRIMARY], &[]);
//! assert_eq!(icon_inputs.part_tag, Some(ICON));
//! ```
//!
//! [`PartTag`] values are application-defined. In UI code, prefer anchoring
//! part selectors under an owner [`TypeTag`] (for example, `Button > icon`) so
//! unrelated widgets can reuse local part IDs without colliding.
//!
//! ### Path Matching And Style Changes
//!
//! [`StyleCascade`] is path-aware. Embedders walk their own style subject tree
//! and carry a compact [`MatchState`] from parent to child. A `MatchState` is
//! valid only with the cascade that produced it.
//!
//! ```rust
//! use invalidation::Channel;
//! use understory_property::{PropertyMetadataBuilder, PropertyRegistry};
//! use understory_style::{
//!     PseudoClassId, Selector, SelectorInputs, SelectorStep, StyleBuilder,
//!     StyleCascadeBuilder, StyleOrigin, PartTag, TypeTag,
//! };
//!
//! const PAINT: Channel = Channel::new(1);
//! const TOGGLE: TypeTag = TypeTag(1);
//! const TRACK: PartTag = PartTag(2);
//! const CHECKED: PseudoClassId = PseudoClassId(3);
//!
//! let mut registry = PropertyRegistry::new();
//! let background = registry.register(
//!     "Background",
//!     PropertyMetadataBuilder::new(0_u32)
//!         .affects_channels(PAINT.into_set())
//!         .build(),
//! );
//!
//! let cascade = StyleCascadeBuilder::new()
//!     .push_rule(
//!         StyleOrigin::Sheet,
//!         [
//!             SelectorStep::type_tag(TOGGLE).with_pseudo(CHECKED),
//!             SelectorStep::part_tag(TRACK),
//!         ],
//!         StyleBuilder::new().set(background, 0x00ff00_u32).build(),
//!     )
//!     .build();
//!
//! let checked = [CHECKED];
//! let unchecked_root = cascade.enter_subject(
//!     cascade.root_state(),
//!     &SelectorInputs::typed(TOGGLE),
//! );
//! let checked_root = cascade.enter_subject(
//!     cascade.root_state(),
//!     &SelectorInputs::typed_with_pseudos(TOGGLE, &checked),
//! );
//! let unchecked_track = cascade.enter_subject(
//!     unchecked_root,
//!     &SelectorInputs::part(TRACK),
//! );
//! let checked_track = cascade.enter_subject(
//!     checked_root,
//!     &SelectorInputs::part(TRACK),
//! );
//!
//! let changed = cascade.changed_properties(unchecked_track, checked_track);
//! assert_eq!(changed.property_ids(), &[background.id()]);
//! assert!(changed.affected_channels(&registry).contains(PAINT));
//!
//! let descendant = Selector::descendant(
//!     SelectorStep::type_tag(TOGGLE).with_pseudo(CHECKED),
//!     SelectorStep::part_tag(TRACK),
//! );
//! assert!(descendant.matches_path(&[
//!     SelectorInputs::typed_with_pseudos(TOGGLE, &checked),
//!     SelectorInputs::with_part(None, Some(PartTag(99)), &[], &[]),
//!     SelectorInputs::part(TRACK),
//! ]));
//! ```
//!
//! Plain selector arrays are exact child paths. For fallback relationships where
//! a step may appear deeper in the subject tree, use [`SelectorCombinator::Descendant`].
//! The current grammar is intentionally limited to child and descendant
//! relationships. It does not include sibling selectors, `nth-*` selectors,
//! parent queries, or structural `odd`/`even` selectors. Embedders that need
//! structural state today should compute that state themselves and expose it as
//! classes or pseudoclasses:
//!
//! ```rust
//! use understory_style::{ClassId, PartTag, Selector, SelectorStep, TypeTag};
//!
//! const ROW: TypeTag = TypeTag(1);
//! const TEXT: PartTag = PartTag(2);
//! const ODD: ClassId = ClassId(3);
//!
//! let odd_row_text = Selector::from([
//!     SelectorStep::type_tag(ROW).with_class(ODD),
//!     SelectorStep::part_tag(TEXT),
//! ]);
//! assert_eq!(odd_row_text.len(), 2);
//! ```
//!
//! [`StyleCascade::changed_properties`] is conservative and reports properties
//! whose winning style source changes; it does not compare concrete typed
//! values for equality.
//!
//! For inspection and update loops, [`StyleCascade`] also exposes
//! [`StyleCascade::matching_rules`], [`StyleCascade::winning_source`], and
//! [`StyleCascade::restyle_subject`]. For selector authoring diagnostics, use
//! [`Selector::diagnose_path`] to get the first path mismatch.
//!
//! ## `no_std` Support
//!
//! This crate is `no_std` and uses `alloc`. It does not depend on `std`.

#![no_std]

extern crate alloc;

mod matcher;
mod resolve;
mod selector;
pub mod selectors;
mod style;
mod stylesheet;
mod theme;
mod vocabulary;

pub use matcher::{
    MatchRule, MatchState, Matcher, MatcherBuilder, RuleCursor, StyleCascade, StyleCascadeBuilder,
    StyleChangeSet, SubjectRestyle, WinningStyleSource,
};
pub use resolve::{
    ExprResolveOptions, NoResolveParentLookup, PropertyParentLookup, ResolveCx, ResolveParent,
    ResolveParentLookup, ResolveQuery, Resolved, ResolvedSource,
};
pub use selector::{
    ClassId, IdSet, PartTag, PseudoClassId, Selector, SelectorBuilder, SelectorCombinator,
    SelectorInputs, SelectorInputsOwned, SelectorMismatch, SelectorStep, Specificity, TypeTag,
};
pub use style::{
    ExprRef, Style, StyleBuilder, StyleExpressionId, StyleExpressionRef, StyleValueKind,
    StyleValueRef,
};
pub use stylesheet::StyleOrigin;
pub use theme::{ResourceKey, Theme, ThemeBuilder};
pub use understory_property_expression::{
    ExprDeps, ExprError, ExprResourceKey, ExpressionLayer, expr,
};
pub use vocabulary::{
    StylePartName, StyleTokenName, StyleTokenSet, StyleVocabulary, StyleVocabularyIdBindings,
};
