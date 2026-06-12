# Understory: state of the workspace and strategy (June 2026)

A whole-workspace review of all 22 crates: what is good, bad, and ugly in each;
what should be improved; strategic next steps; and opportunities for crates
that do not exist yet.

The framing for this review is the repo's stated goal: take things many people
hand-roll in UIs, turn each into a crate, and push each further than most
would. Several crates are already in use in Floem and Overstory.

---

## 1. Executive summary

The workspace is in better shape than most pre-1.0 multi-crate projects:
exemplary CI, no unsafe code anywhere, a consistent no_std + alloc philosophy,
clear "what this crate does NOT own" fences, and a healthy test culture
(several hundred tests across the workspace). The architectural separation —
three-tree model, pluggable index backends, explicit commit/damage,
generational handles — is sound and consistently applied.

The three recurring weaknesses, in order of strategic importance:

1. **Missing connective tissue.** Each crate is individually clean, but a host
   wiring the input stack (responder + event_state + focus + selection +
   timing) or the property stack (property + binding + style +
   presentation_properties) writes 70–300 lines of repetitive glue. The crates
   compose in principle; nothing demonstrates or packages the composition.
   There is no graphical example anywhere — every example is a println
   walkthrough.

2. **Designed-but-stubbed features.** Several features exist in the API
   surface but are no-ops: responder tie-breaking (`router.rs` `id_cmp`/
   `id_is_newer` are stubs), focus groups (`FocusProps::group` is ignored by
   `DefaultPolicy`), `EnterScope`/`ExitScope` (return `None`), the responder
   `Localizer` (empty struct), `FocusEntry::scope_depth` (never read).
   Single-instance pointer capture (`capture: Option<K>`) and single-pointer
   drag block real multi-touch.

3. **Uneven incrementality.** node_graph has explicit invalidation channels;
   outline does full-projection rebuilds on any change; presentation dirty
   tracking is per-node, not per-primitive; transcript chunk appends are
   O(N²) for large streamed bodies. The "predictable updates" principle is
   stated but applied unevenly.

Plus a release-hygiene layer: version skew (0.0.1 / 0.1.0 / 0.1.2), 18 crates
without CHANGELOGs, README out of sync with the examples directory, two crates
whose default features contradict the no_std-first philosophy.

---

## 2. Scorecard

| Crate | Maturity | Headline issue |
|---|---|---|
| index | Production-ready | Visitor API not exposed publicly; SAH/backends under-validated |
| box_tree | Production-ready | Rounded-clip hit precision approximate; no `world_clip()` getter |
| responder | Production-ready | Single-instance pointer capture; tie-break stubs; empty Localizer |
| selection | Production-ready | O(n) membership scan (fine at UI scale); no `IntoIterator` |
| timing | Production-ready | No `pop_all_expired` / `time_until_next` conveniences |
| virtual_list | Production-ready | No sticky headers; no batched extent mutation for streaming |
| box_decoration | Production-ready | No shadow-spread contour; absolute-only radii (no length-%) |
| presentation_properties | Production-ready | No shadow/opacity properties; single background layer |
| view2d | Production-ready (debt) | viewport1d/viewport2d ~95% duplicated; no aspect-ratio fit |
| event_state | Promising-but-thin | hover.rs/focus.rs near line-for-line duplicates; single-pointer drag |
| focus | Promising-but-thin | Groups, RTL, EnterScope/ExitScope designed but unimplemented |
| property | Promising-but-thin | No animation timeline (storage only); host boilerplate |
| property_binding | Promising-but-thin | One-way only; clone-per-bind cycle detection |
| style | Promising-but-thin | No sibling/nth selectors; no transitions; release-mode cross-matcher state unchecked |
| presentation | Promising-but-thin | No opacity, gradients-via-properties, rich text; paint-order semantics undefined |
| outline | Promising-but-thin | Full rebuild per change; no cycle detection on bad models |
| inspector | Promising-but-thin | `FixedExtentModel<f64>` hardcoded; selection invariants escapable via `selection_mut()` |
| transcript | Promising-but-thin | O(N²) streamed chunk appends; chunking only for `EntryBody`; no persistence |
| node_graph | Promising-but-thin | No undo/redo, multi-select drag, groups; hand-rolled arena |
| axis | Promising-but-thin | 1365-line single file; log scale incomplete; libm unconditionally required |
| precise_hit | Sketch | RoundedRect distance is bbox-approximate; stroke story is one helper; 8 tests |
| guide | Sketch | 2 tests, no examples, no integration; unclear why it exists |

---

## 3. Per-crate findings

### Spatial / geometry layer

**understory_index** — Good: the `Scalar` abstraction with widened accumulators
(f32→f64, i64→i128) for robust SAH metrics; minimal backend trait; generational
keys; coarse damage with `union()`. Bad: `visit_point`/`visit_rect` exist on
the backend trait but are not exposed on `Index`, so early-exit queries are
impossible and the default collecting paths allocate per query; SAH correctness
across R-tree/BVH is inferred from unit tests, not property-tested;
`Index::reserve()` is a no-op that gives backends no hint. Ugly: zero-area AABB
semantics are defined (`is_zero_area`) but untested through indexing/queries;
backend-selection guidance is thin in-crate.

**understory_box_tree** — Good: clean delegation to index via generic backend;
explicit commit with damage; deterministic z/depth/generation tie-breaking;
flags (VISIBLE/PICKABLE/FOCUSABLE) take effect without commit, correctly
separated from geometry. Bad: rounded-rect clip hit testing is approximate at
corners and admits it only in a comment; `world_clip()` is private so callers
cannot reason about effective clipping without re-walking ancestors;
reparenting dirties the whole subtree even for sibling reorders. Ugly: internal
`node()`/`node_mut()` panic on stale `NodeId` behind public entry points and
stale-handle paths are untested; depth saturates at u16 and generation
overflow is "unspecified" per a comment; four separate dirty bools instead of
a bitmask.

**understory_precise_hit** — Good: decoupled from broad phase by design; small
trait (`PreciseHitTest`) that callers can implement; `HitScore` is small and
sortable. Bad: RoundedRect hit distance is computed against the bounding box,
not the corner curves, so near-corner scores don't rank properly when
tolerance > 0; BezPath is fill-only while the `stroke` module offers exactly
one helper (`StrokedLine`) — the stroke story is mixed messaging. Ugly: 8 tests
total; no NaN/degenerate/tolerance-boundary coverage; no in-crate example of
composing with box_tree (only the responder example shows it).

**understory_view2d** — Good: thorough input validation (non-finite rejection,
zoom clamping); explicit world↔view conversion; 1D and 2D kept separate so
timelines don't pay for 2D. Bad: viewport1d.rs and viewport2d.rs are ~95%
duplicated — same fields, setters, validation, clamp/fit logic; no
aspect-ratio-preserving fit; FitMode×ClampMode interaction undocumented. Ugly:
the private validation module can't be reused by hosts; `*DebugInfo` types
exist but nothing consumes them.

**understory_axis** — Good: genuinely further-than-most tick machinery — 1-2-5
decimal, binary power-of-two, and time-like ladders; major/medium/minor
classification with label eligibility; immutable `AxisRuler1D` snapshots;
spacing metrics exported for layout. Bad: log mapping only supports positive
domains and exposes fewer metrics than linear; subdivision policy is just
Auto/Fixed; no sub-millisecond time ladder; libm is a hard dependency with no
feature gate. Ugly: 1365 lines in one file with complex private functions
(`build_linear_ticks`, `choose_step`) tested only through the public API;
14 tests don't cover the ladder × subdivision matrix.

**understory_guide** — Good: the `AxisGuide2D::from_ruler()` lift from scalar
ruler marks to 2D geometry is a nice composition with axis. Bad: it is the
least-finished crate in the workspace — 2 tests, no examples, no integration
with box_tree or responder, and no stated story for what UI feature it serves.
Ugly: `from_endpoints()` returns `None` for coincident endpoints while the
constructor clamps negative length to zero — inconsistent degenerate handling.

### Input / interaction layer

**understory_responder** — The standout crate. Good: routing consumes
pre-resolved hits (the boundary is exactly right); capture/target/bubble
ordering verified correct against DOM semantics; path borrowing via the `Hit`
trait avoids rebuilds; `DepthKey` handles Z and ray-distance ordering with NaN
safety; strong test suite. Bad: pointer capture is `capture: Option<K>`
(router.rs:61) — a second pointer silently clobbers the first, blocking
multi-touch and mouse+stylus; `id_is_newer`/`id_cmp` (router.rs:320–334) are
stubbed no-ops with a TODO; `Localizer` (types.rs:100–111) is an empty struct
cloned through every dispatch; no pointer-id or event-kind (pointer vs
keyboard vs IME) threading in `ResolvedHit`. Ugly: meta is cloned per phase;
the root→target path invariant on caller-provided hits is unchecked.

**understory_event_state** — Good: hover/focus LCA transitions are correct with
the right leave-inner-first/enter-outer-first ordering; click recognition is
thoughtful (configurable spatial+temporal thresholds, per-pointer press
tracking, 50+ tests); drag is transparent and caller-driven. Bad: hover.rs and
focus.rs are near line-for-line duplicates including tests — a bug fixed in one
won't propagate; `DragState` holds a single start/last position pair
(drag.rs:40–44) so a second touch overwrites the first; no double-click or
long-press recognizers, and no glue to understory_focus (host must compute the
next focus target, build a path, and remember to feed it back into
`FocusState`). Ugly: duplicated module docs and doctests; the
`distance_exceeded` once-set-per-press invariant is subtle and undocumented.

**understory_focus** — Good: `FocusPolicy` is a clean one-method trait;
`FocusSpace` as an immutable snapshot is the right call; directional scoring
(hemiplane + weighted Manhattan) with linear fallback matches the design docs.
Bad: `FocusProps::group` is documented in the README but `DefaultPolicy`
ignores it — groups are a no-op; `EnterScope`/`ExitScope` return `None` with no
documented host obligation; no LTR/RTL despite
docs/issue_focus_direction_and_reading_order.md sketching `ReadingDirection`;
the directional weight `W = 4.0` is a hardcoded literal so tuning feel requires
a fork; `FocusEntry::scope_depth` is carried but never read. Ugly: the useful
linear-navigation helpers are private; all tests sit at the bottom of lib.rs.

**understory_selection** — Good: minimal, only `PartialEq` required, revision
counter for cheap change detection, API mirrors actual gestures (click /
ctrl+click / shift+click), ~50 tests. Bad: O(n) membership scans (fine for UI
scale; should be documented as such); no extend-from-anchor convenience —
hosts compute ranges themselves (defensible, since selection is
order-agnostic by design). Ugly: no `IntoIterator`; `primary()` can refer to a
deleted node and that contract is undocumented.

**understory_timing** — Good: genuinely host-agnostic (caller-supplied ticks,
no clocks/threads/callbacks); explicit rearm gives hosts batching control;
both repeat policies (drain-time vs scheduled-deadline) have real use cases;
`retain_pending` for bulk widget-teardown cancellation; ~40 tests. Bad: hosts
must loop `pop_expired` (no `pop_all_expired`) and compute `deadline - now`
themselves (no `time_until_next`); O(n) insertion is fine under ~100 timers
but the limit is undocumented.

### Property / style layer

**understory_property + understory_property_binding + understory_style** —
Good: a WPF-grade precedence model done in Rust without the WPF weight; sparse
binary-search storage appropriate for typical per-node property counts;
channel-aware invalidation metadata (LAYOUT/GEOMETRY/PAINT) integrated with the
external `invalidation` crate; binding drains are transactional with
deterministic reports and real cycle detection; the style cascade's
`enter_subject` walk state is a clean incremental-matching design; resource
tokens/theme indirection landed recently (#197/#198). Bad: the three crates
have three resolution paths that the host must coordinate by hand — a minimal
app writes 70–100 lines of glue (DependencyObject impl, BindingHost erased
get/set dispatch, registration, drain loop), and a full property+binding+style
example is 200–300 lines; binding is one-way only; the animation layer is pure
storage (no duration, easing, or completion — every host rebuilds an
animator); selectors lack sibling/nth combinators and negation; cycle
detection clones the dependency tracker per bind (O(edges)). Correctness
notes: cross-matcher `MatchState` misuse is only caught by debug assertions
(matcher.rs:217–220) — release builds silently corrupt; thread-safety
expectations are undocumented. Ugly: no selector validation or "why didn't
this match" diagnostics, which an inspector/devtools story will need.

### Presentation layer

**understory_presentation** — Good: property-blind retained cache with
deduped first-dirty-order draining; flat storage with caller-owned tree truth;
generic node/source/image keys without type erasure. Bad: no opacity or blend
modes anywhere (`BackgroundLayer` is brush-only) — the first thing a real UI
needs that isn't here; paint-order semantics are undefined (background vs
border vs shadows vs text — lowerers must guess; CSS pins this down);
`PlainTextPrimitive` is single-run only; dirty tracking is per-node, so one
changed primitive repaints all primitives on the node; shadows lack
inset/outer/text kinds and layer-order rules. Ugly: asymmetric primitive
accessors (`surface_mut` auto-creates, `image_mut` returns `Option`,
`set_path` replaces) on what is actually a bag (`SmallVec`) — the API implies
one-of-each; several `unreachable!` branches in store.rs; primitive tests are
sparse next to the strong store tests.

**understory_presentation_properties** — Good: canonical names
("Surface.Background", …) registered once with correct GEOMETRY/PAINT channel
splits; coercion at registration so the store guarantees clean values;
`SurfacePropertyValues` as an inspectable intermediate before
`into_surface()`. Bad: no shadow or opacity properties even though
`SurfacePrimitive` models shadows — hosts mutate the resolved primitive by
hand, breaking the properties→primitives contract; single background layer; no
length-percentage values (roadmapped, but hit immediately). Ugly: 5 tests; 15+
registrations of hand-rolled boilerplate.

**understory_box_decoration** — Good: the strongest "pushed further than most"
crate — elliptical per-corner radii, the CSS smallest-factor overflow rule,
four corner shapes including parameterized superellipse (scoop/squircle/notch,
tracking CSS Borders L4), derived border/padding/content contours, on-demand
path writing into caller-owned `BezPath`, hardened against
negative/NaN/infinite input. Bad: no shadow-spread contour helper (lowerers
expand boxes manually); no per-corner-region path extraction; radii are
absolute-only. Ugly: arc-kappa and 12-segment superellipse approximations have
no cited references or error bounds; the even-odd fill requirement of the
border ring lives only in a comment; no test that concave (scoop/notch)
contours stay non-self-intersecting — notable since #196 just fixed a concave
border bug.

### Data-projection layer

**understory_virtual_list** — Good: `ExtentModel` is minimal with an explicit
immutable-query vs `&mut self`-caching split; lazy prefix-sum maintenance;
`GridTrackModel` projects 1D→2D tracks without contaminating the core;
tail-anchoring (the chat-log case) is production-quality with epsilon
stickiness; excellent docs and tests. Bad: no sticky-header support (the
single most-requested virtualization feature); no batched extent mutation —
measuring 100 scattered items costs repeated total-extent recalculation; FP
rounding can make before+covered+after ≠ content extent and the behavior is
undocumented. Ugly: deprecated aliases already accumulating at v0.1.2 suggest
naming churn.

**understory_outline** — Good: projection over the host's own model via
`OutlineModel` (no parallel tree); `VisibleRow` carries everything a renderer
needs. Bad: any change rebuilds the entire visible-row list — toggling one
leaf in a 10K-row tree is O(10K); no cycle detection, so a malformed model
(sibling loop) hangs the projection. Ugly: `index_of_key` rebuilds even for
known-invisible keys.

**understory_inspector** — Good: a genuinely thin controller (~200 lines of
glue) over outline + virtual_list + selection with explicit `sync()`
semantics; focus fallback when the focused row collapses and selection pruning
are both correct. Bad: `VirtualList<FixedExtentModel<f64>>` is hardcoded
(inspector.rs:31) — variable-height inspector rows require a fork;
`selection_mut()` lets hosts break the invariants the controller maintains.
Ugly: 9 tests, no interaction-sequence coverage (expand → focus → select →
collapse).

**understory_transcript** — Good: generic payloads with `EntryBody` as a sane
default; parent vs cause as distinct link kinds; typed chunk appends that
reject text/bytes mismatches; children indexed by parent; revision counter.
Bad: streamed appends into a flat `String`/`Vec<u8>` are O(N²) for large
bodies — the exact agent/shell streaming case the crate targets; chunk appends
only work for `P = EntryBody`; parent/cause links are never validated (silent
dangling links); status transitions are unconstrained; no
persistence/serialization hook. The "re-measure extents after append" pattern
that makes transcript + virtual_list work is demonstrated only in an example,
not documented in the crate.

**understory_node_graph** — Good: the doc/projection/session/computed
four-layer split is exemplary and should be the template for future editor
crates; `GraphInvalidation` channels are the most sophisticated invalidation
in the workspace; `PortCompatibility` policy injection; 92 tests. Bad: no
undo/redo — disqualifying for a real node editor and entirely host-burden
today; interaction state models a single drag (no multi-select move); routing
is straight-line or basic Manhattan; no groups/comments; `default = ["std"]`
contradicts the workspace's no_std-first stance. Ugly: hand-rolled arena
(rather than reusing the workspace's own generational-key patterns or
slotmap); `GraphComputed::rebuild` takes six positional parameters.

---

## 4. Cross-cutting

### The good

- **CI is exemplary**: cargo-hack feature matrix per package, MSRV 1.88
  enforced, x86_64-unknown-none no_std builds, wasm32-unknown-unknown +
  wasip1, docs with `-D warnings` + private items, rustfmt/taplo/typos/
  copyright headers, cargo-rdme README sync checks.
- Linebender lint set v7, `unsafe_code = "deny"` honored everywhere.
- Fences ("does not own X, Y, Z") in every README keep scope creep visible.
- Commit hygiene: crate-prefixed conventional commits with PR links.
- MSRV consistent across Cargo.toml, CI, CHANGELOGs, README.

### The bad

- **Version skew**: box_tree and index at 0.0.1, timing and virtual_list at
  0.1.2, everything else 0.1.0 — with foundational crates at the *oldest*
  versions.
- **18 of 22 crates have no CHANGELOG** (only box_decoration,
  presentation_properties, timing, view2d, virtual_list do).
- **README drift**: 20 examples on disk, 15 documented; responder_precise_hit,
  responder_focus, style_subject_walk, property_binding_loop, focus_basics
  missing from Getting Started.
- **Feature-default inconsistency**: node_graph defaults to `std`,
  presentation defaults to `libm`; axis has a redundant empty `std` feature
  and a hard libm dep. The "no_std by default" claim is not uniformly true.
- **Benchmark coverage**: 7 bench targets, all index/box_tree/property/style/
  outline-centric; virtual_list, responder, selection, property_binding have
  none despite being hot paths.
- No Miri, no coverage, no bench regression tracking, no wasm-without-std job.

### The ugly (conventions that diverged crate-by-crate)

- **ID types**: generic `K`/`T` (selection, focus, event_state, responder) vs
  concrete newtypes (`TimerId(u64)`, node_graph's `NodeId`/`PortId`/`EdgeId`,
  box_tree's generational `NodeId`). Composition works but every boundary is a
  fresh decision.
- **Change-tracking vocabulary**: `revision()` (selection, transcript) vs
  `commit() -> Damage` (box_tree, index) vs `GraphInvalidation` channels
  (node_graph) vs `take_dirty()` (presentation) vs nothing (outline rebuilds).
  Four idioms for one concept.
- **Error handling**: `Option` (box_tree, timing) vs `Result<_, ConnectError>`
  (node_graph) vs internal panics on stale handles (box_tree accessors).
- **Parent/path lookup**: responder's `ParentLookup`, focus's adapter walking
  `Tree` directly, event_state consuming pre-built paths — three answers to
  "how do I walk up the tree."
- **Duplication hot spots**: hover.rs/focus.rs in event_state;
  viewport1d/viewport2d in view2d.

---

## 5. Strategic next steps

Ordered; the first two are the ones that change the project's trajectory.

### 1. Prove the stack end-to-end with one real, graphical example

Every example is a println walkthrough. Nothing in the repo demonstrates the
three-tree model actually rendering and responding to input. Build one
`examples/` app (winit + vello or even a tiny softbuffer renderer) that wires:
layout (hardcoded or Taffy) → box_tree → responder + event_state + focus →
property/style → presentation_properties → presentation → renderer.

This is the highest-leverage item because it (a) is the marketing artifact the
project lacks, (b) will surface every integration seam this review found
(empty Localizer, missing focus glue, undefined paint order, per-node dirty
granularity) as concrete blockers rather than review notes, and (c) produces
the reference host code that crate READMEs can point at instead of each
explaining its corner in isolation.

### 2. Build the lowerer / render-tree crate (the roadmap item)

The presentation layer stops one step short of a renderer, and the ambiguities
that blocks are piling up *in the existing crates*: paint order
(background/border/shadow/text), opacity scope, clipping of text to content
contours, shadow placement. A `understory_lowering` (or `understory_render_tree`)
crate that consumes `PresentationStore` + box_tree transforms/clips and emits a
peniko/vello scene would force those semantics to be defined once, in code,
and would make every presentation-layer gap (opacity, gradients via peniko's
existing brush support, per-primitive dirty bits) immediately testable.

### 3. Finish what's designed but stubbed (input stack credibility)

In rough order of pain-when-hit:
- Per-pointer capture in responder (`BTreeMap<PointerId, K>`) and per-pointer
  drag in event_state — multi-touch is binary: it works or it doesn't.
- Focus groups in `DefaultPolicy` (filter by `FocusSymbol`, fall back to all)
  — currently a documented feature that silently does nothing.
- ReadingDirection (LTR/RTL) per the existing issue doc; expose the
  directional weight as a policy parameter at the same time.
- Either implement responder tie-breaking via an injected comparator or
  delete the stubs; same for `scope_depth`.
- Give `Localizer` real fields (inverse transform + scroll offset, driven by
  what Floem actually needs) or replace it with a generic parameter.

### 4. Kill the duplication before it diverges

- Extract the shared LCA/path-transition core under hover.rs/focus.rs in
  event_state.
- Extract the shared 1D core under view2d's two viewports (viewport2d as two
  axes + the 2D-only extras).
- Define one parent/path-lookup protocol (responder's `ParentLookup` is the
  best candidate) and have focus adapters and event_state docs consume it.
  This may justify a tiny `understory_protocol`-style crate — but keep it to
  traits only; a fat "core" crate would undo the workspace's best property.

### 5. Make incrementality uniform

node_graph's invalidation channels are the house style at its best — apply it
downward: coarse invalidation hints for outline (subtree-scoped rebuilds),
per-primitive dirty bits in presentation, batched extent mutation in
virtual_list, chunked body storage in transcript (the O(N²) streamed-append
case is the crate's headline use case). Write the revision/damage/invalidation
vocabulary down in docs/ as a convention and name one pattern.

### 6. Release-hygiene pass (cheap, do alongside anything)

Align versions (or document why box_tree/index lag); add the 18 missing
CHANGELOGs; sync README's example list (CI already checks cargo-rdme — extend
the same discipline); make `no_std` the default everywhere (node_graph,
presentation, axis feature gates); add benches for virtual_list, responder,
selection, property_binding; decide guide's fate (see §6 — snapping gives it a
purpose, otherwise fold it into axis or remove it).

### 7. Hardening items worth scheduling (from the per-crate lists)

- box_tree: public `world_clip()`; non-panicking accessors; corner-accurate
  rounded-clip hits (or an explicit "approximate" contract); stale-handle
  tests.
- precise_hit: true distance-to-corner for RoundedRect; a real stroked-path
  story; triple the test count.
- index: expose visitor queries on `Index`; property-test the R-tree/BVH
  builds; wire `reserve()` through.
- style: make cross-matcher state misuse fail in release builds (cheap id
  check), document threading expectations.
- transcript: debug-assert link existence; an advancing-only status setter.

---

## 6. New crate opportunities

Ranked by (a) how universally hand-rolled the thing is, (b) how well it fits
the existing fences, (c) what this workspace already has that makes it easier
here than elsewhere.

### Tier 1 — natural next crates, clear demand from existing gaps

**understory_undo** — Generic command/transaction history: undo/redo stacks,
coalescing (typing, dragging), checkpoints, dirty-since-save. Everyone
hand-rolls it and most do it badly (no coalescing, no transaction grouping).
node_graph needs it today; any future text-editing or canvas work needs it;
selection/property mutations want to participate. The workspace's
revision-counter discipline is exactly the substrate an undo system composes
with. Headless, no_std-friendly, zero dependencies.

**understory_animation** — Timeline/easing/interpolation primitives:
`Animatable` trait (lerp), easing curves, transition tracking
(property X on node N is animating from A to B, t of duration), completion
events. The property review and the presentation review both flagged this
independently: property's animation layer is storage with no animator, and
style has no transitions. Pairs precisely with understory_timing (which owns
deadlines but deliberately not interpolation). This is the missing half of two
existing crates.

**understory_text_input** — Editing-core state: caret/selection-span model
(grapheme-aware via host-provided segmentation), IME composition state machine
(pre-edit, commit, cancel), kill-ring/word-motion intents. IME is entirely
absent from the input stack today and is the single most commonly botched
piece of hand-rolled UI code. Fits the house style perfectly: a state machine
that accepts pre-computed information and emits transitions, owning no text
layout or rendering (parlance adjacency helps here).

### Tier 2 — strong fits, slightly narrower audiences

**understory_gesture** — Recognizers above event_state: tap/double-tap/
long-press/pinch/rotate/fling with the disambiguation state machine
(tap-vs-drag threshold, double-tap windows). The event_state review found
double-click and long-press are already user-visible gaps; recognizers need
timing (have it), pointer state (have it), and thresholds (click.rs already
models them). Requires the multi-pointer fixes in §5.3 first.

**understory_drag_drop** — Drag-and-drop session model: drag sources, drop
targets, hover-with-intent (spring-loaded folders), drop effects
(copy/move/link), and the negotiation state machine. Distinct from
event_state's drag *tracking* (deltas) — this is the protocol layer. Composes
with responder paths and box_tree hit testing.

**understory_scroll** — Scroll physics: kinetic/fling decay, rubber-band
overscroll, snap points, smooth-scroll-to animations. virtual_list owns the
geometry math and deliberately not the physics; every host re-derives the
same exponential-decay curves. Pairs with timing and animation.

**understory_table** — 2D grid virtualization: frozen rows/columns, row+column
virtualization composed, cell spans. The virtual_list review hit this wall
explicitly (`GridTrackModel` can't do variable rows × variable columns).
Builds directly on `ExtentModel` — two axes plus a freeze policy.

**understory_snapping** — Snap-to-grid/guide/object resolution for editors:
candidate generation, tolerance ranking, snap-line reporting for rendering.
This is also the answer to "why does understory_guide exist" — guide provides
the geometry, snapping provides the behavior, node_graph and any canvas editor
are immediate consumers.

### Tier 3 — worth a design note, not a crate yet

**understory_shortcut** — Keymap/chord resolution with contexts and conflict
detection. Universal need, but design is dominated by platform conventions;
wait for the keyboard-routing story (§5.3 event-kind threading) to settle.

**understory_a11y adapter** — Mapping focus/selection/outline/box_tree state
into AccessKit. High value, but do it after the end-to-end example exists,
inside that example first, then extract.

**understory_dock** — Docking/split/tab-strip layout state (panel trees,
drag-to-dock hit zones, serializable layouts). Overstory-shaped; headless
state machine fits the house style, but it's a big crate — design doc first.

**Deliberately not**: a layout engine (Taffy exists and the box_tree fence
already says "not a layout engine" — integrate Taffy in the example instead);
a renderer (vello exists; the lowerer crate in §5.2 is the right boundary);
general reactivity/signals (that is the host framework's identity, and
property_binding already covers the dependency-graph slice).

---

## 7. Suggested sequencing

If the above were a six-month plan:

1. **Now**: §5.6 hygiene pass + §5.3 stub-finishing (small PRs, immediately
   shippable). Start the §5.1 end-to-end example in parallel — it will reorder
   the rest of this list by what it runs into.
2. **Next**: §5.2 lowerer crate, driven by the example's needs. Presentation
   gains opacity + paint-order semantics as part of it.
3. **Then**: understory_undo and understory_animation (Tier 1 crates with
   in-workspace consumers waiting: node_graph, property, style transitions).
4. **After**: incrementality pass (§5.5), then understory_text_input once the
   keyboard/IME routing decisions from the example are settled.
