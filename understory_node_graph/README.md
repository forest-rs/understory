<div align="center">

# Understory Node Graph

**Headless node-graph document, projection, session, and derived-state primitives**

[![Latest published version.](https://img.shields.io/crates/v/understory_node_graph.svg)](https://crates.io/crates/understory_node_graph)
[![Documentation build status.](https://img.shields.io/docsrs/understory_node_graph.svg)](https://docs.rs/understory_node_graph)
[![Apache 2.0 license.](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](#license)
\
[![GitHub Actions CI status.](https://img.shields.io/github/actions/workflow/status/forest-rs/understory/ci.yml?logo=github&label=CI)](https://github.com/forest-rs/understory/actions)

</div>

<!-- We use cargo-rdme to update the README with the contents of lib.rs.
To edit the following section, update it in lib.rs, then run:
cargo rdme --workspace-project=understory_node_graph
Full documentation at https://github.com/orium/cargo-rdme -->

<!-- Intra-doc links used in lib.rs may be evaluated here. -->

<!-- cargo-rdme start -->

Understory Node Graph: headless node-graph document and projection primitives.

This crate provides the model behind a node graph editor/viewer without
collapsing everything into one monolithic editor instance.

The current v0 shape is built around four explicit layers:
- [`GraphDoc`]: durable semantic graph content.
- [`GraphProjection`]: one 2D presentation of that graph.
- [`GraphSession`]: ephemeral interaction state for one projection user.
- [`GraphComputed`]: derived geometry, visibility, and edge-preview state.

[`GraphInvalidation`] provides explicit coarse invalidation channels for the
derived phases. Scoped [`GraphDoc::edit`] and [`GraphProjection::edit`] calls
return [`GraphChanges`] receipts covering complete source revision transitions.
Record them in each view's invalidation state to derive only affected geometry.
Missing, reordered or foreign receipts fall back to a full geometry rebuild;
targeted hints alone cannot hide source mutations. Sources retain no journal.
Cached sources have instance identities: replacing or cloning a source cannot
silently reuse equal revision counters. Each source allocates one shared identity
token, using `Arc` on pointer-atomic targets and `Rc` otherwise; mutations do not
allocate identity tokens. Clones have independent identities and state.
Hosts can also provide a [`PortCompatibility`] policy to drive edge-creation
previews and optional connection validation without pushing a type system
into the crate itself. Policies receive a [`ConnectionContext`] so they can
inspect endpoint metadata and existing topology.
Observers report live [`GraphSize`] separately from phase-local entity visits
and actual router calls. Preview routing has its own phase; visibility reports
its full scan even when anchor and route derivation were targeted.

The crate intentionally does **not** own rendering, widget policy, graph
execution, or application-specific node semantics.

## Architectural split

Existing node-graph systems often tangle together:
- document data,
- positions and layout,
- interaction state,
- rendering structures,
- and runtime/execution concerns.

`understory_node_graph` instead keeps those concerns separate so the same
graph document can support:
- a full editor,
- a read-only viewer,
- a minimap,
- tests and headless tools,
- and alternate projections over the same graph.

## Minimal example

```rust
use kurbo::{Point, Rect, Size};
use understory_node_graph::{
    GraphComputed, GraphDoc, GraphInvalidation, GraphProjection, GraphSession, NodeData,
    NodeView, NoopGraphDeriveObserver, PortDirection, StraightEdgeRouter,
};

let mut doc = GraphDoc::<&'static str, &'static str, ()>::new();
let node = doc.add_node(NodeData { meta: "Add" });
let input = doc.add_port(node, PortDirection::Input, "lhs").unwrap();
let output = doc.add_port(node, PortDirection::Output, "sum").unwrap();
assert_ne!(input, output);

let mut projection = GraphProjection::<(), (), ()>::new();
projection.set_node_view(
    node,
    NodeView::new(Point::new(100.0, 80.0), Size::new(180.0, 96.0), ()),
);

let session = GraphSession::new(Rect::new(0.0, 0.0, 800.0, 600.0));

let mut computed = GraphComputed::new();
let mut invalidation = GraphInvalidation::new();
let mut observer = NoopGraphDeriveObserver;

assert!(computed.rebuild(
    &doc,
    &projection,
    &session,
    &mut invalidation,
    &StraightEdgeRouter,
    &mut observer,
));
assert_eq!(computed.visible_nodes(), &[node]);
assert!(computed.hit_test_world(&doc, &projection, Point::new(110.0, 90.0)).is_some());
```

## Scoped edits and cache affinity

A normal setter is safe but does not provide coverage for a targeted derive.
Use scoped edits on hot paths and record every receipt, in order, in each
consuming cache's invalidation state:

```rust
use kurbo::{Point, Rect, Size, Vec2};
use understory_node_graph::*;
let mut doc = GraphDoc::<(), (), ()>::new();
let node = doc.add_node(NodeData::default());
let mut projection = GraphProjection::<(), (), ()>::new();
projection.set_node_view(node, NodeView::new(Point::ZERO, Size::new(80.0, 40.0), ()));
let session = GraphSession::new(Rect::new(0.0, 0.0, 800.0, 600.0));
let mut computed = GraphComputed::new();
let mut invalidation = GraphInvalidation::new();
computed.rebuild(&doc, &projection, &session, &mut invalidation,
    &StraightEdgeRouter, &mut NoopGraphDeriveObserver);
let (_, changes) = projection.edit(|edit| edit.translate_node(node, Vec2::new(20.0, 0.0)));
invalidation.record(&changes);
computed.rebuild(&doc, &projection, &session, &mut invalidation,
    &StraightEdgeRouter, &mut NoopGraphDeriveObserver);
assert_eq!(computed.node_bounds(node).unwrap().x0, 20.0);
```

Receipts record applied mutations; they do not make a transaction atomic or
roll back rejected application edits. Validate application policy first.
Sources retain no change history. A receipt can be cloned for independent
consumers; a missing receipt, foreign instance or incomplete revision range
causes full derivation. Cloning a source gives it a new cache identity while
preserving its document-local entity handles. Those handles are not durable
application identities. Use [`GraphComputed::reset`] to clear realized state
explicitly without discarding allocated table capacity.

## Computed result changes

`rebuild_with_changes` and `rebuild_with_compatibility_and_changes` return an
owned `ComputedChanges` report. The existing boolean rebuild methods remain
available. Source receipts certify the input transition; result reports list
actual changed/removed node bounds, socket anchors and routes, visible-set
entries/exits, and preview changes. A phase can run and produce an empty report.
Targeted derivation compares only the affected geometry; it does not copy or
compare a complete geometry snapshot. Visibility still scans its inputs, then
merges the old and new membership lists.

Bind each retained host mapping to one computed cache and apply its reports in
order, consuming updated values before another rebuild. If `before` does not
match the mapping's last applied `after`, resynchronize from the current
snapshot. A `reset` report instead establishes a complete new baseline; discard
the old mapping first. Initialization, cache reset, document replacement and
recovery from an interrupted rebuild use this path. There is no result journal.

Results use world coordinates. Metadata, selection styling and changes to the
world-to-view transform remain host responsibilities. Panning can require new
screen positions even when world geometry and visible membership are unchanged.

## Socket and floating-surface anchors

[`PortAnchor::Automatic`] retains distributed side-of-node placement.
[`PortAnchor::NodeLocal`] directly specifies a socket point, including points
outside the node rectangle; sibling insertion does not reposition it.
[`PortView::anchor_offset`] nudges either resolved anchor.

`understory_anchor` has a separate responsibility: placing floating surfaces
around a point or rectangle, with collision and fallback policy. A host can
convert [`GraphComputed::node_bounds`] or [`GraphComputed::port_anchor`] through
its viewport and pass the result to that resolver for an inspector or tooltip.
No graph-specific hook or dependency is needed for this composition.

Routers receive [`RouteRequest::Edge`] for committed edges and
[`RouteRequest::Preview`] for gestures, whose source and optional target ports
are explicit. Preview requests never carry a fabricated edge identity.

## Intentional v0 limits

This crate starts with nodes, ports, and edges only. It deliberately defers:
- groups and comments,
- spatial acceleration backends,
- text/label modeling,
- and execution/runtime semantics.

It also keeps the top-level pieces separate instead of forcing everything
into one giant editor object. A host can reuse one [`GraphDoc`] with
multiple projections, sessions, and computed caches.

Those can be added later as clear layers above or beside this one.

This crate is `no_std` and uses `alloc`. The default `std` feature enables
`kurbo/std` through the viewport stack. `no_std` consumers should disable
default features and enable `libm`:

```text
understory_node_graph = { version = "...", default-features = false, features = ["libm"] }
```

<!-- cargo-rdme end -->

## License

Licensed under either of

 * Apache License, Version 2.0
   ([LICENSE-APACHE](LICENSE-APACHE) or https://www.apache.org/licenses/LICENSE-2.0)
 * MIT license
   ([LICENSE-MIT](LICENSE-MIT) or https://opensource.org/licenses/MIT)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.
