// Copyright 2025 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Benchmarks for path-aware style matching.

use criterion::{
    BatchSize, BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main,
};
use understory_property::{Property, PropertyMetadataBuilder, PropertyRegistry};
use understory_style::{
    ClassId, MatchState, PartTag, PseudoClassId, Selector, SelectorCombinator, SelectorInputs,
    SelectorStep, StyleBuilder, StyleCascade, StyleCascadeBuilder, StyleOrigin, TypeTag,
};

const BUTTON: TypeTag = TypeTag(1);
const TOGGLE: TypeTag = TypeTag(2);
const ROW: TypeTag = TypeTag(3);

const CHROME: PartTag = PartTag(10);
const CONTENT: PartTag = PartTag(11);
const TRACK: PartTag = PartTag(12);
const THUMB: PartTag = PartTag(13);
const TEXT: PartTag = PartTag(14);
const BADGE: PartTag = PartTag(15);
const DETAIL: PartTag = PartTag(16);
const META: PartTag = PartTag(17);

const PRIMARY: ClassId = ClassId(20);
const ODD: ClassId = ClassId(21);
const EVEN: ClassId = ClassId(22);

const HOVER: PseudoClassId = PseudoClassId(30);
const CHECKED: PseudoClassId = PseudoClassId(31);

#[derive(Clone, Debug)]
struct Subject {
    parent: Option<usize>,
    type_tag: Option<TypeTag>,
    part_tag: Option<PartTag>,
    classes: Vec<ClassId>,
    pseudos: Vec<PseudoClassId>,
}

impl Subject {
    fn root(type_tag: TypeTag) -> Self {
        Self {
            parent: None,
            type_tag: Some(type_tag),
            part_tag: None,
            classes: Vec::new(),
            pseudos: Vec::new(),
        }
    }

    fn part(parent: usize, part_tag: PartTag) -> Self {
        Self {
            parent: Some(parent),
            type_tag: None,
            part_tag: Some(part_tag),
            classes: Vec::new(),
            pseudos: Vec::new(),
        }
    }

    fn inputs(&self) -> SelectorInputs<'_> {
        SelectorInputs::with_part(self.type_tag, self.part_tag, &self.classes, &self.pseudos)
    }
}

fn type_selector(type_tag: TypeTag) -> SelectorStep {
    SelectorStep::type_tag(type_tag)
}

fn part_selector(part_tag: PartTag) -> SelectorStep {
    SelectorStep::part_tag(part_tag)
}

fn path(steps: impl IntoIterator<Item = SelectorStep>) -> Selector {
    Selector::from_steps(steps)
}

fn make_cascade(color: Property<u32>) -> StyleCascade {
    StyleCascadeBuilder::new()
        .push_rules(
            StyleOrigin::Sheet,
            [
                (
                    Selector::single(type_selector(BUTTON)),
                    StyleBuilder::new().set(color, 10_u32).build(),
                ),
                (
                    path([type_selector(BUTTON), part_selector(CHROME)]),
                    StyleBuilder::new().set(color, 11_u32).build(),
                ),
                (
                    path([
                        type_selector(BUTTON),
                        part_selector(CHROME),
                        part_selector(CONTENT),
                    ]),
                    StyleBuilder::new().set(color, 12_u32).build(),
                ),
                (
                    Selector::from(SelectorStep::type_tag(BUTTON).with_class(PRIMARY)),
                    StyleBuilder::new().set(color, 13_u32).build(),
                ),
                (
                    Selector::from(SelectorStep::type_tag(BUTTON).with_pseudo(HOVER)),
                    StyleBuilder::new().set(color, 14_u32).build(),
                ),
                (
                    Selector::single(type_selector(TOGGLE)),
                    StyleBuilder::new().set(color, 20_u32).build(),
                ),
                (
                    path([type_selector(TOGGLE), part_selector(TRACK)]),
                    StyleBuilder::new().set(color, 21_u32).build(),
                ),
                (
                    path([
                        type_selector(TOGGLE),
                        part_selector(TRACK),
                        part_selector(THUMB),
                    ]),
                    StyleBuilder::new().set(color, 22_u32).build(),
                ),
                (
                    path([
                        SelectorStep::type_tag(TOGGLE).with_pseudo(CHECKED),
                        part_selector(TRACK),
                    ]),
                    StyleBuilder::new().set(color, 23_u32).build(),
                ),
                (
                    Selector::from(SelectorStep::type_tag(ROW).with_class(ODD)),
                    StyleBuilder::new().set(color, 30_u32).build(),
                ),
                (
                    Selector::from(SelectorStep::type_tag(ROW).with_class(EVEN)),
                    StyleBuilder::new().set(color, 31_u32).build(),
                ),
                (
                    path([
                        type_selector(ROW),
                        part_selector(CONTENT),
                        part_selector(TEXT),
                    ]),
                    StyleBuilder::new().set(color, 32_u32).build(),
                ),
            ],
        )
        .build()
}

fn make_descendant_cascade(color: Property<u32>) -> StyleCascade {
    StyleCascadeBuilder::new()
        .push_rules(
            StyleOrigin::Sheet,
            [
                (
                    Selector::from_segments(
                        SelectorStep::type_tag(ROW),
                        [(SelectorCombinator::Descendant, SelectorStep::part_tag(TEXT))],
                    ),
                    StyleBuilder::new().set(color, 40_u32).build(),
                ),
                (
                    Selector::from_segments(
                        SelectorStep::type_tag(TOGGLE).with_pseudo(CHECKED),
                        [(
                            SelectorCombinator::Descendant,
                            SelectorStep::part_tag(THUMB),
                        )],
                    ),
                    StyleBuilder::new().set(color, 41_u32).build(),
                ),
            ],
        )
        .build()
}

fn make_heavy_descendant_cascade(color: Property<u32>) -> StyleCascade {
    StyleCascadeBuilder::new()
        .push_rules(
            StyleOrigin::Sheet,
            [
                (
                    Selector::from_segments(
                        SelectorStep::type_tag(ROW),
                        [(
                            SelectorCombinator::Descendant,
                            SelectorStep::part_tag(CONTENT),
                        )],
                    ),
                    StyleBuilder::new().set(color, 50_u32).build(),
                ),
                (
                    Selector::from_segments(
                        SelectorStep::type_tag(ROW),
                        [(SelectorCombinator::Descendant, SelectorStep::part_tag(TEXT))],
                    ),
                    StyleBuilder::new().set(color, 51_u32).build(),
                ),
                (
                    Selector::from_segments(
                        SelectorStep::type_tag(ROW),
                        [(
                            SelectorCombinator::Descendant,
                            SelectorStep::part_tag(BADGE),
                        )],
                    ),
                    StyleBuilder::new().set(color, 52_u32).build(),
                ),
                (
                    Selector::from_segments(
                        SelectorStep::type_tag(ROW),
                        [(
                            SelectorCombinator::Descendant,
                            SelectorStep::part_tag(DETAIL),
                        )],
                    ),
                    StyleBuilder::new().set(color, 53_u32).build(),
                ),
                (
                    Selector::from_segments(
                        SelectorStep::type_tag(ROW),
                        [(SelectorCombinator::Descendant, SelectorStep::part_tag(META))],
                    ),
                    StyleBuilder::new().set(color, 54_u32).build(),
                ),
                (
                    Selector::from_segments(
                        SelectorStep::type_tag(ROW).with_class(EVEN),
                        [
                            (
                                SelectorCombinator::Descendant,
                                SelectorStep::part_tag(CONTENT),
                            ),
                            (SelectorCombinator::Child, SelectorStep::part_tag(TEXT)),
                        ],
                    ),
                    StyleBuilder::new().set(color, 55_u32).build(),
                ),
                (
                    Selector::from_segments(
                        SelectorStep::type_tag(ROW).with_class(ODD),
                        [
                            (
                                SelectorCombinator::Descendant,
                                SelectorStep::part_tag(DETAIL),
                            ),
                            (SelectorCombinator::Descendant, SelectorStep::part_tag(META)),
                        ],
                    ),
                    StyleBuilder::new().set(color, 56_u32).build(),
                ),
            ],
        )
        .build()
}

fn make_rule_pressure_cascade(color: Property<u32>, rule_count: usize) -> StyleCascade {
    let rules = (0..rule_count)
        .map(|index| {
            let part = PartTag(1_000 + u32::try_from(index).unwrap_or(u32::MAX));
            (
                Selector::descendant(SelectorStep::type_tag(ROW), SelectorStep::part_tag(part)),
                StyleBuilder::new()
                    .set(color, 100_u32 + u32::try_from(index % 1_000).unwrap_or(0))
                    .build(),
            )
        })
        .collect::<Vec<_>>();

    StyleCascadeBuilder::new()
        .push_rules(StyleOrigin::Sheet, rules)
        .build()
}

fn make_buttons(count: usize, hovered: Option<usize>) -> Vec<Subject> {
    let mut subjects = Vec::with_capacity(count * 3);
    for i in 0..count {
        let root = subjects.len();
        let mut button = Subject::root(BUTTON);
        if i % 4 == 0 {
            button.classes.push(PRIMARY);
        }
        if hovered == Some(i) {
            button.pseudos.push(HOVER);
        }
        subjects.push(button);

        let chrome = subjects.len();
        subjects.push(Subject::part(root, CHROME));
        subjects.push(Subject::part(chrome, CONTENT));
    }
    subjects
}

fn make_toggles(count: usize, checked: Option<usize>) -> Vec<Subject> {
    let mut subjects = Vec::with_capacity(count * 4);
    for i in 0..count {
        let root = subjects.len();
        let mut toggle = Subject::root(TOGGLE);
        if checked == Some(i) {
            toggle.pseudos.push(CHECKED);
        }
        subjects.push(toggle);

        let track = subjects.len();
        subjects.push(Subject::part(root, TRACK));
        subjects.push(Subject::part(track, THUMB));
        subjects.push(Subject::part(root, CONTENT));
    }
    subjects
}

fn make_rows(count: usize) -> Vec<Subject> {
    let mut subjects = Vec::with_capacity(count * 3);
    for i in 0..count {
        let root = subjects.len();
        let mut row = Subject::root(ROW);
        if i % 2 == 0 {
            row.classes.push(EVEN);
        } else {
            row.classes.push(ODD);
        }
        subjects.push(row);

        let content = subjects.len();
        subjects.push(Subject::part(root, CONTENT));
        subjects.push(Subject::part(content, TEXT));
    }
    subjects
}

fn make_deep_rows(count: usize) -> Vec<Subject> {
    let mut subjects = Vec::with_capacity(count * 6);
    for i in 0..count {
        let root = subjects.len();
        let mut row = Subject::root(ROW);
        if i % 2 == 0 {
            row.classes.push(EVEN);
        } else {
            row.classes.push(ODD);
        }
        subjects.push(row);

        let content = subjects.len();
        subjects.push(Subject::part(root, CONTENT));
        let text = subjects.len();
        subjects.push(Subject::part(content, TEXT));
        let badge = subjects.len();
        subjects.push(Subject::part(text, BADGE));
        let detail = subjects.len();
        subjects.push(Subject::part(badge, DETAIL));
        subjects.push(Subject::part(detail, META));
    }
    subjects
}

fn make_rule_pressure_rows(count: usize, depth: usize, part_count: usize) -> Vec<Subject> {
    let mut subjects = Vec::with_capacity(count * (depth + 1));
    let part_count = part_count.max(1);
    for row_index in 0..count {
        let root = subjects.len();
        subjects.push(Subject::root(ROW));

        let mut parent = root;
        for depth_index in 0..depth {
            let part_index = (row_index + depth_index) % part_count;
            let part = PartTag(1_000 + u32::try_from(part_index).unwrap_or(u32::MAX));
            let subject = subjects.len();
            subjects.push(Subject::part(parent, part));
            parent = subject;
        }
    }
    subjects
}

fn restyle(cascade: &StyleCascade, subjects: &[Subject], color: Property<u32>) -> u64 {
    let mut states = vec![MatchState::default(); subjects.len()];
    let mut sum = 0_u64;

    for (index, subject) in subjects.iter().enumerate() {
        let parent = subject
            .parent
            .map_or_else(|| cascade.root_state(), |parent| states[parent]);
        let state = cascade.enter_subject(parent, &subject.inputs());
        states[index] = state;

        if let Some(value) = cascade.get_value_ref(state, color) {
            sum = sum.wrapping_add(u64::from(*value));
        }
    }

    sum
}

fn match_states(cascade: &StyleCascade, subjects: &[Subject]) -> Vec<MatchState> {
    let mut states = vec![MatchState::default(); subjects.len()];

    for (index, subject) in subjects.iter().enumerate() {
        let parent = subject
            .parent
            .map_or_else(|| cascade.root_state(), |parent| states[parent]);
        states[index] = cascade.enter_subject(parent, &subject.inputs());
    }

    states
}

fn changed_property_count(cascade: &StyleCascade, old: MatchState, new: MatchState) -> usize {
    cascade.changed_properties(old, new).len()
}

fn bench_style_matching(c: &mut Criterion) {
    let mut registry = PropertyRegistry::new();
    let color = registry.register("Color", PropertyMetadataBuilder::new(0_u32).build());
    let cascade = make_cascade(color);

    let mut group = c.benchmark_group("style_matching/full_restyle");
    for (name, subjects) in [
        ("buttons_1000", make_buttons(1_000, Some(125))),
        ("toggles_1000", make_toggles(1_000, Some(125))),
        ("rows_10000", make_rows(10_000)),
    ] {
        group.throughput(Throughput::Elements(subjects.len() as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(name),
            &subjects,
            |b, subjects| {
                b.iter(|| black_box(restyle(&cascade, subjects, color)));
            },
        );
    }
    group.finish();

    let descendant_cascade = make_descendant_cascade(color);
    let mut group = c.benchmark_group("style_matching/descendant_restyle");
    for (name, subjects) in [
        ("toggles_1000", make_toggles(1_000, Some(125))),
        ("rows_10000", make_rows(10_000)),
    ] {
        group.throughput(Throughput::Elements(subjects.len() as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(name),
            &subjects,
            |b, subjects| {
                b.iter(|| black_box(restyle(&descendant_cascade, subjects, color)));
            },
        );
    }
    group.finish();

    let heavy_descendant_cascade = make_heavy_descendant_cascade(color);
    let mut group = c.benchmark_group("style_matching/heavy_descendant_restyle");
    for (name, subjects) in [
        ("deep_rows_5000", make_deep_rows(5_000)),
        ("deep_rows_10000", make_deep_rows(10_000)),
    ] {
        group.throughput(Throughput::Elements(subjects.len() as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(name),
            &subjects,
            |b, subjects| {
                b.iter(|| black_box(restyle(&heavy_descendant_cascade, subjects, color)));
            },
        );
    }
    group.finish();

    let rule_pressure_subjects = make_rule_pressure_rows(1_000, 8, 512);
    let mut group = c.benchmark_group("style_matching/cold_rule_pressure_restyle");
    for rule_count in [128, 512] {
        group.throughput(Throughput::Elements(rule_pressure_subjects.len() as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{rule_count}_descendant_rules")),
            &rule_count,
            |b, rule_count| {
                b.iter_batched(
                    || make_rule_pressure_cascade(color, *rule_count),
                    |cascade| black_box(restyle(&cascade, &rule_pressure_subjects, color)),
                    BatchSize::SmallInput,
                );
            },
        );
    }
    group.finish();

    let cache_subjects = make_rule_pressure_rows(1_000, 8, 512);
    let hot_cache_cascade = make_rule_pressure_cascade(color, 512);
    black_box(restyle(&hot_cache_cascade, &cache_subjects, color));
    let mut group = c.benchmark_group("style_matching/transition_cache");
    group.throughput(Throughput::Elements(cache_subjects.len() as u64));
    group.bench_function("hot_cache_512_rules", |b| {
        b.iter(|| black_box(restyle(&hot_cache_cascade, &cache_subjects, color)));
    });
    group.bench_function("cold_cache_512_rules", |b| {
        b.iter_batched(
            || make_rule_pressure_cascade(color, 512),
            |cascade| black_box(restyle(&cascade, &cache_subjects, color)),
            BatchSize::SmallInput,
        );
    });
    group.finish();

    let old_buttons = make_buttons(1_000, None);
    let new_buttons = make_buttons(1_000, Some(125));
    let old_button_states = match_states(&cascade, &old_buttons);
    let new_button_states = match_states(&cascade, &new_buttons);
    let button_root = 125 * 3;

    let old_toggles = make_toggles(1_000, None);
    let new_toggles = make_toggles(1_000, Some(125));
    let old_toggle_states = match_states(&cascade, &old_toggles);
    let new_toggle_states = match_states(&cascade, &new_toggles);
    let toggle_track = 125 * 4 + 1;

    let mut group = c.benchmark_group("style_matching/state_change");
    group.bench_function("button_hover_root", |b| {
        b.iter(|| {
            black_box(changed_property_count(
                &cascade,
                old_button_states[button_root],
                new_button_states[button_root],
            ));
        });
    });
    group.bench_function("toggle_checked_track", |b| {
        b.iter(|| {
            black_box(changed_property_count(
                &cascade,
                old_toggle_states[toggle_track],
                new_toggle_states[toggle_track],
            ));
        });
    });
    group.finish();
}

criterion_group!(benches, bench_style_matching);
criterion_main!(benches);
