<div align="center">

# Understory Timeline Model

**Validated headless timeline document and edit primitives**

[![Latest published version.](https://img.shields.io/crates/v/understory_timeline_model.svg)](https://crates.io/crates/understory_timeline_model)
[![Documentation build status.](https://img.shields.io/docsrs/understory_timeline_model.svg)](https://docs.rs/understory_timeline_model)
[![Apache 2.0 or MIT license.](https://img.shields.io/badge/license-Apache--2.0_OR_MIT-blue.svg)](#license)
\
[![GitHub Actions CI status.](https://img.shields.io/github/actions/workflow/status/forest-rs/understory/ci.yml?logo=github&label=CI)](https://github.com/forest-rs/understory/actions)

</div>

<!-- We use cargo-rdme to update the README with the contents of lib.rs.
To edit the following section, update it in lib.rs, then run:
cargo rdme --workspace-project=understory_timeline_model --heading-base-level=0
Full documentation at https://github.com/orium/cargo-rdme -->

<!-- Intra-doc links used in lib.rs may be evaluated here. -->

<!-- cargo-rdme start -->

Understory Timeline Model: validated headless timeline document primitives.

This crate owns the non-visual state for time-domain views such as
profiler timelines, transport lanes, or editor-like span collections.
It focuses on:
- Typed document-local lane, span, marker, and flow identifiers.
- Optional stable item keys for diffing and selection across snapshots.
- Generic lane, span, and marker labels with `String` defaults.
- Validated span, marker, and flow records.
- Playhead and optional time-range or item selection state.
- Plain edit operations such as moving or resizing spans and selections.
- Structural queries such as content bounds and time-range filtering.

It does **not** own:
- Rendering or text layout.
- Hit testing policy.
- Snap policy or anchor prioritization.
- Viewport math.
- Event routing or gesture interpretation.
- Persistence or application-specific timeline schemas.

A [`TimelineDoc`] keeps these invariants:
- Span times, marker times, playhead times, and selection endpoints are finite.
- Span start times are not greater than end times.
- Span and marker lane references point at existing lanes.
- Flow endpoints point at existing spans.
- Non-anonymous item keys are unique across lanes, spans, markers, and flows.

## Minimal example

```rust
use understory_timeline_model::{
    LaneId, SpanId, TimelineDoc, TimelineLane, TimelineSpan,
};

let mut doc = TimelineDoc::try_new(
    [TimelineLane::new("Main")],
    [TimelineSpan::new("Layout", 10.0, 20.0, LaneId::new(0))],
)
.unwrap();

doc.set_playhead(12.5).unwrap();
doc.set_selection(10.0, 18.0).unwrap();
doc.move_span_by(SpanId::new(0), 5.0).unwrap();

let span = &doc.spans()[0];
assert_eq!(span.start, 15.0);
assert_eq!(span.end, 25.0);
```

## Large timelines

The default constructors own `String` labels for convenience. For large
profiler traces or devtools captures with many repeated names, use
[`TimelineLane::from_label`], [`TimelineSpan::from_label`], and
[`TimelineMarker::from_label`] with compact application-defined label ids or
interned handles:

```rust
use understory_timeline_model::{
    LaneId, TimelineDoc, TimelineLane, TimelineSpan,
};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
struct LabelId(u32);

let doc = TimelineDoc::try_new(
    [TimelineLane::from_label(LabelId(1))],
    [TimelineSpan::from_label(LabelId(2), 0.0, 8.0, LaneId::new(0))],
)
.unwrap();

assert_eq!(doc.spans()[0].label, LabelId(2));
```

Time-range queries return iterators over borrowed records and do not allocate.
They intentionally use a linear scan in this crate; callers that need
repeated indexed viewport queries over very large traces can layer an index
beside the model without changing the document representation.

## Time units

Times are caller-defined `f64` scalar units. Use one unit consistently within
a document. For profiler and devtools timelines, microseconds are a practical
default: they preserve sub-millisecond detail while keeping common frame
durations readable. Hosts with very long captures can also choose
milliseconds or seconds before building the document.

## Ids and stable keys

[`LaneId`], [`SpanId`], [`MarkerId`], and [`FlowId`] are document-local vector
indexes. They are cheap and stable while a document's content arrays are not
replaced, but callers should not use them as durable identity across rebuilt
profiler frames, rolling buffers, or diffed view models.

Use [`TimelineItemKey`] for identity that should survive content replacement.
Anonymous keys are allowed for simple static documents. Non-anonymous keys
are validated as unique across all content records.

This crate is `no_std` and uses `alloc`.

<!-- cargo-rdme end -->

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
