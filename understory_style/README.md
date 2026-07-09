<div align="center">

# Understory Style

**Style and theme resolution for Understory dependency properties**

[![Latest published version.](https://img.shields.io/crates/v/understory_style.svg)](https://crates.io/crates/understory_style)
[![Documentation build status.](https://img.shields.io/docsrs/understory_style.svg)](https://docs.rs/understory_style)
[![Apache 2.0 or MIT license.](https://img.shields.io/badge/license-Apache--2.0_OR_MIT-blue.svg)](#license)
\
[![GitHub Actions CI status.](https://img.shields.io/github/actions/workflow/status/forest-rs/understory/ci.yml?logo=github&label=CI)](https://github.com/forest-rs/understory/actions)

</div>

<!-- We use cargo-rdme to update the README with the contents of lib.rs.
To edit the following section, update it in lib.rs, then run:
cargo rdme --workspace-project=understory_style --heading-base-level=0
Full documentation at https://github.com/orium/cargo-rdme -->

<!-- Intra-doc links used in lib.rs may be evaluated here. -->

<!-- cargo-rdme start -->

Understory Style: presentation policy for dependency properties.

This crate extends `understory_property` with selector matching, cascade
rules, theme resources, style expressions, and expression-aware value
resolution. It is the presentation policy layer: it decides which property
value wins for a styled subject, but it does not own the app's reactive
runtime.

Resolution follows this precedence chain:

**Animation → Local → Style → Resource fallback → Inherited → Expression default → Default**

The resource fallback stage is present only when a configured
[`ResolveQuery`] supplies one. Theme values are also consulted when a winning
style entry or expression token points at a [`ResourceKey`]. Every theme
lookup flows through the same [`ThemeResolveOptions`] entry point, so an
active [`ThemeVariant`] (variant table → base table) and a subtree
[`ThemeScope`] chain (innermost scope → outward to the root app theme) apply
uniformly across all of those stages.

## Start Here

Use this crate when your application already owns a tree of UI, template, or
model nodes and needs style matching over that tree. `understory_style` does
not store the tree. The embedder walks its own style subjects and carries a
compact [`MatchState`] from parent to child. Selectors can start at any
entered subject, so a one-step selector such as `Button` or `SliderThumb`
matches nested subjects as well as roots; parent state is still used to
continue child and descendant selectors.

The crate owns:

- selector matching over child and descendant paths;
- optional style vocabulary interning from author-facing names to compact ids;
- style values, style expressions, and theme resources for dependency properties;
- theme variants (light, dark, high-contrast, custom) and subtree-scoped
  theme overrides carried by the embedder;
- expression-aware resolution through animation, local, style, resource
  fallback, inheritance, expression defaults, and registry defaults;
- conservative style invalidation facts through `invalidation` channels;
- inspection hooks for matched rules and style source provenance.

The crate does not own widgets, templates, bindings, animation sampling,
layout, rendering, event dispatch, caching, dirty queues, frame scheduling,
theme persistence or serialization, color math, sibling relationships,
parent queries, or structural selectors such as `nth-*`, `odd`, or `even`.

### Glossary

- **Style subject**: one addressable item in the embedder's walk. It may be
  an element, generated template node, model row, or widget part.
- **`TypeTag`**: an application-defined subject kind, such as `Button`,
  `Toggle`, or `Row`.
- **`PartTag`**: an owner-local part label, such as `track`, `thumb`, or
  `icon`.
- **`StyleVocabulary`**: the name-to-id table used by parsers, tools, and
  embedders that want author-facing selector and resource names without
  coordinating integer id ranges by hand.
- **`SelectorInputs`**: the type, part, class, and pseudoclass snapshot for
  one subject.
- **`MatchState`**: matcher progress after entering a subject. It is valid
  only with the cascade that produced it.

### Vocabulary

Selector matching and theme lookup use compact ids, but parsers and tools
usually start from names. [`StyleVocabulary`] is the shared name-to-id layer
for that boundary:

```rust
use understory_style::{ClassId, ResourceKey, StyleTokenSet, StyleVocabulary};

struct AppStyleTokens;

#[derive(Clone, Copy)]
struct AppTokens {
    primary: ClassId,
    accent_background: ResourceKey,
}

impl StyleTokenSet for AppStyleTokens {
    type Resolved = AppTokens;

    fn resolve(vocabulary: &mut StyleVocabulary) -> Self::Resolved {
        AppTokens {
            primary: vocabulary.class_id(".primary"),
            accent_background: vocabulary.resource_key("accent.background"),
        }
    }
}

let mut vocabulary = StyleVocabulary::new();
let tokens = vocabulary.style_tokens::<AppStyleTokens>();
let hover = vocabulary.pseudo_class_id(":hover");

assert_eq!(vocabulary.class_name(tokens.primary), Some(".primary"));
assert_eq!(
    vocabulary.resource_name(tokens.accent_background),
    Some("accent.background")
);
assert_eq!(vocabulary.pseudo_name(hover), Some(":hover"));
```

Names are exact author-facing spellings. The vocabulary does not add or
remove CSS-like sigils, so choose one canonical spelling for the language or
parser that owns the names.

### First Example: Owner State Styling A Part

This styles a `Toggle` owner's `track` part when the owner has `:checked`.
The checked state stays on the owner; it is not copied into the part inputs.

```rust
use invalidation::Channel;
use understory_property::{PropertyMetadataBuilder, PropertyRegistry};
use understory_style::{
    PartTag, PseudoClassId, Selector, SelectorInputs, SelectorStep, StyleBuilder,
    StyleCascadeBuilder, StyleOrigin, TypeTag,
};

const PAINT: Channel = Channel::new(1);
const TOGGLE: TypeTag = TypeTag(1);
const TRACK: PartTag = PartTag(1);
const CHECKED: PseudoClassId = PseudoClassId(1);

let mut registry = PropertyRegistry::new();
let background = registry.register(
    "Background",
    PropertyMetadataBuilder::new(0_u32)
        .affects_channels(PAINT.into_set())
        .build(),
);

let cascade = StyleCascadeBuilder::new()
    .push_rule(
        StyleOrigin::Sheet,
        Selector::child(
            SelectorStep::type_tag(TOGGLE).with_pseudo(CHECKED),
            SelectorStep::part_tag(TRACK),
        ),
        StyleBuilder::new().set(background, 0x00ff00_u32).build(),
    )
    .build();

let unchecked_owner = cascade.enter_subject(
    cascade.root_state(),
    &SelectorInputs::typed(TOGGLE),
);
let unchecked_track = cascade.enter_subject(
    unchecked_owner,
    &SelectorInputs::part(TRACK),
);

let checked = [CHECKED];
let checked_owner = cascade.enter_subject(
    cascade.root_state(),
    &SelectorInputs::typed_with_pseudos(TOGGLE, &checked),
);
let restyle = cascade.restyle_subject(
    &registry,
    unchecked_track,
    checked_owner,
    &SelectorInputs::part(TRACK),
);

assert_eq!(cascade.get_value_ref(restyle.state(), background), Some(&0x00ff00));
assert!(restyle.changed_channels().contains(PAINT));
assert_eq!(cascade.matching_rules(restyle.state()).count(), 1);
assert!(cascade.winning_source(restyle.state(), background).unwrap().rule().is_some());
```

### Long-Lived Rules Of Thumb

Anchor part selectors under an owner [`TypeTag`]. [`PartTag`] values are
application-defined and may be reused by unrelated owners:

```rust
use understory_style::{PartTag, Selector, SelectorStep, TypeTag};

const BUTTON: TypeTag = TypeTag(1);
const ROW: TypeTag = TypeTag(2);
const LOCAL_PART: PartTag = PartTag(1);

let button_part = Selector::child(
    SelectorStep::type_tag(BUTTON),
    SelectorStep::part_tag(LOCAL_PART),
);
let row_part = Selector::child(
    SelectorStep::type_tag(ROW),
    SelectorStep::part_tag(LOCAL_PART),
);

assert_ne!(button_part.steps()[0], row_part.steps()[0]);
```

Use [`SelectorInputsOwned`] when classes or pseudoclasses come from unsorted
application data. It sorts and deduplicates before exposing borrowed
[`SelectorInputs`].

For integration debugging, use [`StyleCascade::matching_rules`],
[`StyleCascade::winning_source`], and [`Selector::diagnose_path`]. These are
deliberately small diagnostics for the current child / descendant grammar,
not a browser-CSS explanation engine.

## Reference Concepts

### Styles

[`Style`] is a shared collection of property setters. Unlike per-element
storage, styles are immutable after creation and can be shared across
many elements without duplicating setter storage.

```rust
use understory_style::{Style, StyleBuilder};
use understory_property::{PropertyMetadataBuilder, PropertyRegistry};

let mut registry = PropertyRegistry::new();
let width = registry.register("Width", PropertyMetadataBuilder::new(0.0_f64).build());
let height = registry.register("Height", PropertyMetadataBuilder::new(0.0_f64).build());

// Create a shared style
let button_style = StyleBuilder::new()
    .set(width, 100.0)
    .set(height, 40.0)
    .build();

// Multiple elements can reference the same style
assert_eq!(button_style.get(width), Some(&100.0));
```

### Themes

[`Theme`] provides resource lookup by key. Themes map resource keys to
typed values, enabling theming (light/dark modes, brand colors, etc.).

```rust
use understory_style::{Theme, ThemeBuilder, ResourceKey};

// Define resource keys as constants
const ACCENT_COLOR: ResourceKey = ResourceKey::new(0);
const FONT_SIZE: ResourceKey = ResourceKey::new(1);

let light_theme = ThemeBuilder::new()
    .set(ACCENT_COLOR, 0x0078D4_u32)  // Blue
    .set(FONT_SIZE, 14.0_f64)
    .build();

let dark_theme = ThemeBuilder::new()
    .set(ACCENT_COLOR, 0x4CC2FF_u32)  // Light blue
    .set(FONT_SIZE, 14.0_f64)
    .build();

assert_eq!(light_theme.get::<u32>(ACCENT_COLOR), Some(&0x0078D4));
```

### Theme Variants

[`VariantTheme`] makes light/dark/high-contrast (and custom) a first-class
dimension instead of swapping whole theme tables. A variant theme holds a
base [`Theme`] plus per-[`ThemeVariant`] override tables; lookup takes the
active variant and falls back variant → base, so an unknown variant falls
back cleanly to the base table.

```rust
use understory_style::{ResourceKey, ThemeBuilder, ThemeVariant, VariantThemeBuilder};

const ACCENT_COLOR: ResourceKey = ResourceKey::new(0);
const FONT_SIZE: ResourceKey = ResourceKey::new(1);

let theme = VariantThemeBuilder::new()
    .base(
        ThemeBuilder::new()
            .set(ACCENT_COLOR, 0x0078D4_u32)
            .set(FONT_SIZE, 14.0_f64)
            .build(),
    )
    .variant(
        ThemeVariant::DARK,
        ThemeBuilder::new().set(ACCENT_COLOR, 0x4CC2FF_u32).build(),
    )
    .build();

let dark = Some(ThemeVariant::DARK);
assert_eq!(theme.get::<u32>(dark, ACCENT_COLOR), Some(&0x4CC2FF));
// Resources without a dark override fall back to the base table.
assert_eq!(theme.get::<f64>(dark, FONT_SIZE), Some(&14.0));
```

### Theme Scopes

[`ThemeScope`] scopes theme overrides to a subtree, following the same
embedding philosophy as [`MatchState`]: this crate does not store the app
tree. The embedder walks its own tree and carries a cheap scope handle from
parent to child; a popup or themed island pushes a child scope that
overrides some resources. Lookup consults the innermost scope first and
falls back outward to the root scope (normally the app theme), applying the
same variant fallback at each level.

Resolution picks scoped values up automatically: pass the scope (and active
variant) to [`ResolveCx::new`] through [`ThemeResolveOptions`], and style
resource entries, resource fallbacks, and [`expr::token`] references all
observe the scoped, variant-aware values.

```rust
use understory_property::{
    DependencyObject, PropertyMetadataBuilder, PropertyRegistry, PropertyStore,
};
use understory_style::{
    ExpressionLayer, NoResolveParentLookup, ResolveCx, ResourceKey, StyleBuilder,
    StyleCascadeBuilder, StyleOrigin, ThemeBuilder, ThemeResolveOptions, ThemeScope,
    ThemeVariant, VariantThemeBuilder,
};

const ACCENT_COLOR: ResourceKey = ResourceKey::new(0);

struct Element {
    key: u32,
    parent: Option<u32>,
    store: PropertyStore<u32>,
}

impl DependencyObject<u32> for Element {
    fn property_store(&self) -> &PropertyStore<u32> { &self.store }
    fn property_store_mut(&mut self) -> &mut PropertyStore<u32> { &mut self.store }
    fn key(&self) -> u32 { self.key }
    fn parent_key(&self) -> Option<u32> { self.parent }
}

let mut registry = PropertyRegistry::new();
let background = registry.register("Background", PropertyMetadataBuilder::new(0_u32).build());

// The style dereferences a theme resource.
let style = StyleBuilder::new().set_resource(background, ACCENT_COLOR).build();
let cascade = StyleCascadeBuilder::new()
    .push_style(StyleOrigin::Base, style)
    .build();

// App theme with a dark variant; the popup shadows the accent color.
let app = ThemeScope::new(
    VariantThemeBuilder::new()
        .base(ThemeBuilder::new().set(ACCENT_COLOR, 0x0078D4_u32).build())
        .variant(
            ThemeVariant::DARK,
            ThemeBuilder::new().set(ACCENT_COLOR, 0x4CC2FF_u32).build(),
        )
        .build(),
);
let popup = app.child(ThemeBuilder::new().set(ACCENT_COLOR, 0xE81123_u32).build());

let expressions = ExpressionLayer::new();
let element = Element { key: 1, parent: None, store: PropertyStore::new(1) };

// Dark app subtree: the variant table wins.
let dark_app = ThemeResolveOptions::from(&app).with_variant(ThemeVariant::DARK);
let cx = ResolveCx::new(&registry, dark_app, &expressions, NoResolveParentLookup);
let value = cx.get_value(&element, background, Some((&cascade, cascade.root_state())));
assert_eq!(value, 0x4CC2FF);

// Popup subtree: the inner scope shadows the app theme.
let dark_popup = ThemeResolveOptions::from(&popup).with_variant(ThemeVariant::DARK);
let cx = ResolveCx::new(&registry, dark_popup, &expressions, NoResolveParentLookup);
let value = cx.get_value(&element, background, Some((&cascade, cascade.root_state())));
assert_eq!(value, 0xE81123);
```

#### Theme Change Invalidation

Themes, variant themes, and scopes are immutable; the host switches themes
by carrying a different scope, theme, or active variant into the next
resolution pass. When that happens, resource-derived values may have
changed. The conservative fact for normal style resolution is
[`StyleCascade::resource_dependent_properties_with_defaults`]: it reports
every property that any cascade source or expression default can derive from
theme resources, and [`StyleChangeSet::affected_channels`] converts that to
`invalidation` channels. Hosts that build finer-grained resource indexes can
use [`StyleCascade::resource_dependent_properties`],
[`Style::resource_entries`], [`StyleCascade::expression_entries`], and
[`ExpressionLayer::resource_dependent_properties`] directly. Property-level
resource fallbacks supplied through [`ResolveQuery::resource_fallback`] are
caller-owned. The crate reports these facts; scheduling, dirty queues, and
subtree walks stay with the host, which knows which subtrees carried the
changed scope.

### Expression Defaults

Expression defaults derive a property value from other resolved properties
and theme resources. App code normally keeps one [`ExpressionLayer`] and
passes it to [`ResolveCx::new`].

```rust
use understory_property::{
    DependencyObject, PropertyMetadataBuilder, PropertyRegistry, PropertyStore,
};
use understory_style::{
    ExpressionLayer, NoResolveParentLookup, ResolveCx, ResourceKey, ThemeBuilder, expr,
};

const GAP_TOKEN: ResourceKey = ResourceKey::new(0);

struct Element {
    key: u32,
    parent: Option<u32>,
    store: PropertyStore<u32>,
}

impl DependencyObject<u32> for Element {
    fn property_store(&self) -> &PropertyStore<u32> { &self.store }
    fn property_store_mut(&mut self) -> &mut PropertyStore<u32> { &mut self.store }
    fn key(&self) -> u32 { self.key }
    fn parent_key(&self) -> Option<u32> { self.parent }
}

let mut registry = PropertyRegistry::new();
let scale = registry.register("Scale", PropertyMetadataBuilder::new(2.0_f64).build());
let padding = registry.register("Padding", PropertyMetadataBuilder::new(0.0_f64).build());

let mut expressions = ExpressionLayer::new();
expressions.set_default(padding, expr::prop(scale) * 2.0 + expr::token(GAP_TOKEN));

let theme = ThemeBuilder::new().set(GAP_TOKEN, 4.0_f64).build();
let element = Element {
    key: 1,
    parent: None,
    store: PropertyStore::new(1),
};
let cx = ResolveCx::new(&registry, &theme, &expressions, NoResolveParentLookup);

let value = cx.get_value(&element, padding, None);
assert_eq!(value, 8.0);
```

### Style Expressions

Style entries can also be expressions. [`ResolveCx`] evaluates them through
the expression layer it carries.

```rust
use understory_property::{
    DependencyObject, PropertyMetadataBuilder, PropertyRegistry, PropertyStore,
};
use understory_style::{
    ExpressionLayer, NoResolveParentLookup, ResolveCx, ResourceKey, StyleBuilder,
    StyleCascadeBuilder, StyleOrigin, ThemeBuilder, expr,
};

const GAP_TOKEN: ResourceKey = ResourceKey::new(0);

struct Element {
    key: u32,
    parent: Option<u32>,
    store: PropertyStore<u32>,
}

impl DependencyObject<u32> for Element {
    fn property_store(&self) -> &PropertyStore<u32> { &self.store }
    fn property_store_mut(&mut self) -> &mut PropertyStore<u32> { &mut self.store }
    fn key(&self) -> u32 { self.key }
    fn parent_key(&self) -> Option<u32> { self.parent }
}

let mut registry = PropertyRegistry::new();
let scale = registry.register("Scale", PropertyMetadataBuilder::new(2.0_f64).build());
let width = registry.register("Width", PropertyMetadataBuilder::new(0.0_f64).build());

let style = StyleBuilder::new()
    .set(scale, 3.0)
    .set_expr(width, expr::prop(scale) * 2.0 + expr::token(GAP_TOKEN))
    .build();
let cascade = StyleCascadeBuilder::new()
    .push_style(StyleOrigin::Override, style)
    .build();

let theme = ThemeBuilder::new().set(GAP_TOKEN, 4.0_f64).build();
let expressions = ExpressionLayer::new();
let element = Element {
    key: 1,
    parent: None,
    store: PropertyStore::new(1),
};
let cx = ResolveCx::new(&registry, &theme, &expressions, NoResolveParentLookup);

let value = cx.get_value(&element, width, Some((&cascade, cascade.root_state())));
assert_eq!(value, 10.0);
```

### Expression Dependency Inspection

Expressions expose static dependency sets through
[`ExprDeps`]. Hosts can inspect
cascade style expressions for a matched subject with
[`StyleCascade::expression_entries`], standalone style payloads with
[`Style::expression_entries`], and default expressions with
[`ExpressionDefaults::expression_entries`][understory_property_expression::ExpressionDefaults::expression_entries]
to build an invalidation index without evaluating expression bodies.

The usual host policy is:

- When a dependency property in `entry.deps().properties` changes on a
  subject, invalidate the entry's target property on that subject.
- If the target property inherits, also invalidate descendants that can
  observe the inherited result.
- When a dependency resource in `entry.deps().resources` changes, invalidate
  target properties whose expressions mention that resource. A conservative
  whole-theme invalidation can instead invalidate every expression target.
- Convert each invalidated target property to work with
  [`PropertyRegistry::affects_channels`][understory_property::PropertyRegistry::affects_channels].

`understory_style` reports the dependency facts; it deliberately does not
own caching, scheduling, dirty queues, or tree-wide invalidation walks.

### Host Frame Loop

A typical host update loop keeps the orchestration outside this crate:

1. Apply app, input, or model changes to dependency-property stores.
2. Drain `understory_property_binding` so template and app bindings write
   their target local-source slots.
3. Walk style subjects with [`StyleCascade::enter_subject`] or
   [`StyleCascade::restyle_subject`] when selector inputs change.
4. Expand expression dependencies with [`StyleCascade::expression_entries`]
   and [`ExpressionLayer::defaults`] so property and resource changes
   invalidate derived targets.
5. Sample animation or motion systems into animation slots.
6. Resolve values through [`ResolveCx`] for rendering,
   layout, accessibility, or diagnostics.

The `style_reactive_loop` example in the workspace demonstrates this full
shape end to end. The `style_motion_loop` example focuses on the
style-to-motion handoff: resolve the styled target, sample a
`understory_motion` transition, write each sample to the animation slot, and
clear that slot when the transition completes so the styled target is visible
again.

### Configured Resolution Queries

[`ResolveCx::get_value`] and [`ResolveCx::explain_value`] are the normal
APIs. Use [`ResolveCx::query`] when the lookup policy itself is part of the
question, such as combining style state with a property-level resource
fallback.

```rust
use understory_property::{
    DependencyObject, PropertyMetadataBuilder, PropertyRegistry, PropertyStore,
};
use understory_style::{
    ExpressionLayer, NoResolveParentLookup, ResolveCx, ResourceKey, ThemeBuilder, expr,
};

const WIDTH_TOKEN: ResourceKey = ResourceKey::new(1);
const DEFAULT_WIDTH: ResourceKey = ResourceKey::new(2);

struct Element {
    key: u32,
    parent: Option<u32>,
    store: PropertyStore<u32>,
}

impl DependencyObject<u32> for Element {
    fn property_store(&self) -> &PropertyStore<u32> { &self.store }
    fn property_store_mut(&mut self) -> &mut PropertyStore<u32> { &mut self.store }
    fn key(&self) -> u32 { self.key }
    fn parent_key(&self) -> Option<u32> { self.parent }
}

let mut registry = PropertyRegistry::new();
let width = registry.register("Width", PropertyMetadataBuilder::new(0.0_f64).build());

let mut expressions = ExpressionLayer::new();
expressions.set_default(width, expr::token(DEFAULT_WIDTH));

let theme = ThemeBuilder::new()
    .set(WIDTH_TOKEN, 80.0_f64)
    .set(DEFAULT_WIDTH, 40.0_f64)
    .build();
let element = Element {
    key: 1,
    parent: None,
    store: PropertyStore::new(1),
};
let resolve = ResolveCx::new(&registry, &theme, &expressions, NoResolveParentLookup);

let value = resolve
    .query(&element, width)
    .resource_fallback(WIDTH_TOKEN)
    .try_value()
    .unwrap();

assert_eq!(value, 80.0);
```

### Resolution Context

[`ResolveCx`] bundles everything needed to resolve property values through
animation, local, style, resource fallback, inheritance, and default stages.
This avoids passing many parameters to resolution functions.

```rust
use understory_style::{
    ClassId, ExpressionLayer, NoResolveParentLookup, SelectorStep, PseudoClassId, ResolveCx,
    SelectorInputs, StyleCascade, StyleCascadeBuilder, StyleBuilder, StyleOrigin, PartTag,
    ThemeBuilder,
};
use understory_property::{
    DependencyObject, PropertyMetadataBuilder, PropertyRegistry, PropertyStore,
};

let mut registry = PropertyRegistry::new();
let width = registry.register("Width", PropertyMetadataBuilder::new(0.0_f64).build());

let theme = ThemeBuilder::new().build();
let expressions = ExpressionLayer::new();

const PRIMARY: ClassId = ClassId(1);
const HOVER: PseudoClassId = PseudoClassId(1);
const ICON: PartTag = PartTag(1);

// Base style for a "button"
let base = StyleBuilder::new().set(width, 100.0).build();
// Hover style when PRIMARY + HOVER
let hover = StyleBuilder::new().set(width, 120.0).build();

let style: StyleCascade = StyleCascadeBuilder::new()
    .push_style(StyleOrigin::Base, base)
    .push_rule(
        StyleOrigin::Sheet,
        SelectorStep::class(PRIMARY).with_pseudo(HOVER),
        hover,
    )
    .build();

struct Element {
    key: u32,
    parent: Option<u32>,
    store: PropertyStore<u32>,
}

impl DependencyObject<u32> for Element {
    fn property_store(&self) -> &PropertyStore<u32> { &self.store }
    fn property_store_mut(&mut self) -> &mut PropertyStore<u32> { &mut self.store }
    fn key(&self) -> u32 { self.key }
    fn parent_key(&self) -> Option<u32> { self.parent }
}

let element = Element {
    key: 1,
    parent: None,
    store: PropertyStore::new(1),
};

// Create resolution context. Flat doctests use NoResolveParentLookup; apps
// normally pass their tree's parent/style-state lookup here.
let cx = ResolveCx::new(&registry, &theme, &expressions, NoResolveParentLookup);

// Resolve with style (no hover)
let inputs = SelectorInputs::new(None, &[PRIMARY], &[]);
let state = style.enter_subject(style.root_state(), &inputs);
let value = cx.get_value(&element, width, Some((&style, state)));
assert_eq!(value, 100.0);

// Resolve with style (hovered)
let hovered = SelectorInputs::new(None, &[PRIMARY], &[HOVER]);
let hovered_state = style.enter_subject(style.root_state(), &hovered);
let value = cx.get_value(&element, width, Some((&style, hovered_state)));
assert_eq!(value, 120.0);

// Parts are owner-local style addresses supplied by the embedder.
let icon_inputs = SelectorInputs::with_part(None, Some(ICON), &[PRIMARY], &[]);
assert_eq!(icon_inputs.part_tag, Some(ICON));
```

[`PartTag`] values are application-defined. In UI code, prefer anchoring
part selectors under an owner [`TypeTag`] (for example, `Button > icon`) so
unrelated widgets can reuse local part IDs without colliding.

### Path Matching And Style Changes

[`StyleCascade`] is path-aware. Embedders walk their own style subject tree
and carry a compact [`MatchState`] from parent to child. A `MatchState` is
valid only with the cascade that produced it.

```rust
use invalidation::Channel;
use understory_property::{PropertyMetadataBuilder, PropertyRegistry};
use understory_style::{
    PseudoClassId, Selector, SelectorInputs, SelectorStep, StyleBuilder,
    StyleCascadeBuilder, StyleOrigin, PartTag, TypeTag,
};

const PAINT: Channel = Channel::new(1);
const TOGGLE: TypeTag = TypeTag(1);
const TRACK: PartTag = PartTag(2);
const CHECKED: PseudoClassId = PseudoClassId(3);

let mut registry = PropertyRegistry::new();
let background = registry.register(
    "Background",
    PropertyMetadataBuilder::new(0_u32)
        .affects_channels(PAINT.into_set())
        .build(),
);

let cascade = StyleCascadeBuilder::new()
    .push_rule(
        StyleOrigin::Sheet,
        [
            SelectorStep::type_tag(TOGGLE).with_pseudo(CHECKED),
            SelectorStep::part_tag(TRACK),
        ],
        StyleBuilder::new().set(background, 0x00ff00_u32).build(),
    )
    .build();

let checked = [CHECKED];
let unchecked_root = cascade.enter_subject(
    cascade.root_state(),
    &SelectorInputs::typed(TOGGLE),
);
let checked_root = cascade.enter_subject(
    cascade.root_state(),
    &SelectorInputs::typed_with_pseudos(TOGGLE, &checked),
);
let unchecked_track = cascade.enter_subject(
    unchecked_root,
    &SelectorInputs::part(TRACK),
);
let checked_track = cascade.enter_subject(
    checked_root,
    &SelectorInputs::part(TRACK),
);

let changed = cascade.changed_properties(unchecked_track, checked_track);
assert_eq!(changed.property_ids(), &[background.id()]);
assert!(changed.affected_channels(&registry).contains(PAINT));

let descendant = Selector::descendant(
    SelectorStep::type_tag(TOGGLE).with_pseudo(CHECKED),
    SelectorStep::part_tag(TRACK),
);
assert!(descendant.matches_path(&[
    SelectorInputs::typed_with_pseudos(TOGGLE, &checked),
    SelectorInputs::with_part(None, Some(PartTag(99)), &[], &[]),
    SelectorInputs::part(TRACK),
]));
```

Plain selector arrays are exact child paths. For fallback relationships where
a step may appear deeper in the subject tree, use [`SelectorCombinator::Descendant`].
The current grammar is intentionally limited to child and descendant
relationships. It does not include sibling selectors, `nth-*` selectors,
parent queries, or structural `odd`/`even` selectors. Embedders that need
structural state today should compute that state themselves and expose it as
classes or pseudoclasses:

```rust
use understory_style::{ClassId, PartTag, Selector, SelectorStep, TypeTag};

const ROW: TypeTag = TypeTag(1);
const TEXT: PartTag = PartTag(2);
const ODD: ClassId = ClassId(3);

let odd_row_text = Selector::from([
    SelectorStep::type_tag(ROW).with_class(ODD),
    SelectorStep::part_tag(TEXT),
]);
assert_eq!(odd_row_text.len(), 2);
```

[`StyleCascade::changed_properties`] is conservative and reports properties
whose winning style source changes; it does not compare concrete typed
values for equality.

For inspection and update loops, [`StyleCascade`] also exposes
[`StyleCascade::matching_rules`], [`StyleCascade::winning_source`], and
[`StyleCascade::restyle_subject`]. For selector authoring diagnostics, use
[`Selector::diagnose_path`] to get the first path mismatch.

## `no_std` Support

This crate is `no_std` and uses `alloc`. It does not depend on `std`.

<!-- cargo-rdme end -->

[`ResolveCx`]: https://docs.rs/understory_style/latest/understory_style/struct.ResolveCx.html
[`Style`]: https://docs.rs/understory_style/latest/understory_style/struct.Style.html
[`Theme`]: https://docs.rs/understory_style/latest/understory_style/struct.Theme.html
[`ThemeResolveOptions`]: https://docs.rs/understory_style/latest/understory_style/struct.ThemeResolveOptions.html
[`ThemeScope`]: https://docs.rs/understory_style/latest/understory_style/struct.ThemeScope.html
[`ThemeVariant`]: https://docs.rs/understory_style/latest/understory_style/struct.ThemeVariant.html
[`VariantTheme`]: https://docs.rs/understory_style/latest/understory_style/struct.VariantTheme.html

## Minimum supported Rust Version (MSRV)

This crate has been verified to compile with **Rust 1.88** and later.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE] or <http://www.apache.org/licenses/LICENSE-2.0>), or
- MIT license ([LICENSE-MIT] or <http://opensource.org/licenses/MIT>),

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you,
as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.

## Contribution

Contributions are welcome by pull request. The [Rust code of conduct] applies.
Please feel free to add your name to the [AUTHORS] file in any substantive pull request.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you,
as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.

[LICENSE-APACHE]: https://github.com/forest-rs/understory/blob/main/LICENSE-APACHE
[LICENSE-MIT]: https://github.com/forest-rs/understory/blob/main/LICENSE-MIT
[Rust code of conduct]: https://www.rust-lang.org/policies/code-of-conduct
[AUTHORS]: https://github.com/forest-rs/understory/blob/main/AUTHORS
