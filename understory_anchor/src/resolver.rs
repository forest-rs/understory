// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::vec::Vec;

use kurbo::{Insets, Point, Rect, Size, Vec2};

use crate::anchor::reference_rect;
use crate::{
    Align, AnchorCandidate, AnchorConstraint, AnchorFrame, AnchorInput, AnchorOptionKey,
    AnchorPolicy, AnchorPositionOption, AnchorRejectReason, ArrowFrame, CandidateDiagnostics,
    CandidateMetrics, CollisionReport, HysteresisPolicy, Placement, PositionTryOrder,
    PreviousAnchorFrame, ScoringPolicy, Side,
};

/// Resolve a floating surface frame from scene facts and policy.
///
/// Call this once per overlay layout/update frame. The returned
/// [`AnchorFrame`] contains the chosen geometry and a full
/// [`CollisionReport`]. Store a
/// [`PreviousAnchorFrame`] derived from it and pass
/// that back through [`AnchorInput::previous`](crate::AnchorInput::previous) on
/// the next frame to enable placement hysteresis.
///
/// Caller-authored geometry and numeric policy values must be finite. Values
/// documented as non-negative must also be non-negative. Debug builds assert
/// these contracts at entry.
#[must_use]
pub fn resolve_anchor(input: AnchorInput<'_>, policy: AnchorPolicy<'_>) -> AnchorFrame {
    debug_assert_input_and_policy(input, policy);
    let reference = reference_rect(input.anchor);
    let candidates = generate_candidates(input, policy, reference);
    let reference = reference.unwrap_or(Rect::ZERO);

    let (chosen, hysteresis_applied) = choose_candidate_with_report(
        &candidates,
        input.previous,
        policy.scoring,
        policy.hysteresis,
    );
    let chosen_candidate = &candidates[chosen];
    let collision_bounds = nonempty_rect(input.viewport)
        .zip(nonempty_rect(input.boundary))
        .map(|(viewport, boundary)| viewport.intersect(boundary))
        .unwrap_or(Rect::ZERO);
    let detached = !reference.overlaps(collision_bounds);
    let visible = !chosen_candidate.rejected && chosen_candidate.metrics.visible_area > 0.0;
    let clipped = has_overflow(chosen_candidate.metrics.overflow)
        || chosen_candidate.metrics.visible_area < chosen_candidate.rect.area();
    let transform_origin = transform_origin(
        chosen_candidate.rect,
        reference,
        chosen_candidate.placement,
        chosen_candidate.arrow,
    );

    AnchorFrame {
        option_index: chosen_candidate.option_index,
        option_key: chosen_candidate.option_key,
        rect: chosen_candidate.rect,
        placement: chosen_candidate.placement,
        reference_rect: reference,
        floating_size: chosen_candidate.floating_size,
        arrow: chosen_candidate.arrow,
        transform_origin,
        visible,
        clipped,
        detached,
        collision: CollisionReport {
            candidates,
            chosen,
            previous_placement: input.previous.map(PreviousAnchorFrame::placement),
            hysteresis_applied,
        },
    }
}

fn generate_candidates(
    input: AnchorInput<'_>,
    policy: AnchorPolicy<'_>,
    reference: Option<Rect>,
) -> Vec<AnchorCandidate> {
    let viewport = nonempty_rect(input.viewport);
    let boundary = nonempty_rect(input.boundary);
    let bounds = viewport
        .zip(boundary)
        .map(|(viewport, boundary)| viewport.intersect(boundary));
    let options = position_options(
        policy.preferred,
        policy.fallbacks,
        policy.order,
        reference,
        bounds,
    );
    let desired_size = input.floating_size;
    let detached = reference
        .zip(bounds)
        .is_none_or(|(reference, bounds)| !reference.overlaps(bounds));

    let mut candidates = Vec::with_capacity(options.len());
    for option in options {
        let placement = option.option.placement;
        let constraints = option.option.constraints;
        let (main_offset, cross_offset) = offsets(constraints);
        let arrow_constraint = arrow_constraint(constraints);
        let hide_when_detached = constraints
            .iter()
            .any(|constraint| matches!(constraint, AnchorConstraint::HideWhenDetached));
        let mut reject_reason = if reference.is_none() {
            Some(AnchorRejectReason::EmptyAnchor)
        } else if viewport.is_none() {
            Some(AnchorRejectReason::EmptyViewport)
        } else if boundary.is_none() {
            Some(AnchorRejectReason::EmptyBoundary)
        } else {
            None
        };

        let reference = reference.unwrap_or(Rect::ZERO);
        let bounds = bounds.unwrap_or(Rect::ZERO);
        let mut size = desired_size;

        for constraint in constraints {
            if let AnchorConstraint::Size {
                min,
                max,
                allow_shrink,
            } = *constraint
            {
                match constrain_size(
                    size,
                    min,
                    max,
                    allow_shrink,
                    reference,
                    placement,
                    bounds,
                    main_offset,
                ) {
                    Some(next_size) => size = next_size,
                    None => reject_reason = Some(AnchorRejectReason::CannotSatisfyMinSize),
                }
            }
        }

        let base_rect = apply_offset(
            placement_rect(reference, size, placement),
            placement.side,
            main_offset,
            cross_offset,
        );
        let mut rect = base_rect;
        for constraint in constraints {
            match *constraint {
                AnchorConstraint::Shift { padding }
                | AnchorConstraint::KeepInBounds { padding } => {
                    rect = clamp_rect_to_bounds(rect, bounds.inset(-padding));
                }
                _ => {}
            }
        }

        if reject_reason.is_none() && detached && hide_when_detached {
            reject_reason = Some(AnchorRejectReason::Detached);
        }

        let arrow = arrow_constraint.map(|(arrow_size, padding)| {
            arrow_frame(rect, reference, placement.side, arrow_size, padding)
        });
        let metrics = candidate_metrics(
            rect,
            base_rect,
            reference,
            desired_size,
            size,
            bounds,
            option.option_index == 0,
            input.previous.is_some_and(|previous| {
                previous_matches_candidate(previous, option.option_index, option.option.key)
            }),
        );

        if reject_reason.is_none() && metrics.visible_area <= 0.0 {
            reject_reason = Some(AnchorRejectReason::NoVisibleArea);
        }

        let mut candidate = AnchorCandidate {
            option_index: option.option_index,
            option_key: option.option.key,
            placement,
            rect,
            floating_size: size,
            arrow,
            metrics,
            diagnostics: CandidateDiagnostics::default(),
            rejected: reject_reason.is_some(),
            reject_reason,
        };
        candidate.diagnostics.default_score = default_candidate_score(&candidate, policy.scoring);
        candidates.push(candidate);
    }

    candidates
}

#[derive(Clone, Copy, Debug)]
struct ResolvedPositionOption<'a> {
    option_index: usize,
    option: AnchorPositionOption<'a>,
}

fn default_candidate_score(candidate: &AnchorCandidate, scoring: ScoringPolicy) -> f64 {
    let metrics = candidate.metrics;
    let size_delta = metrics.size_delta.width + metrics.size_delta.height;
    let mut score = metrics.visible_area * scoring.visible_area_weight;
    score -= metrics.overflow_area * scoring.overflow_area_weight;
    score -= metrics.anchor_distance * scoring.anchor_distance_weight;
    score -= metrics.shifted_distance * scoring.shifted_distance_weight;
    score -= size_delta * scoring.size_delta_weight;
    if metrics.is_preferred {
        score += scoring.preferred_bonus;
    }
    if metrics.is_incumbent {
        score += scoring.incumbent_bonus;
    }
    if candidate.rejected {
        score -= 1_000_000.0;
    }
    score
}

fn debug_assert_input_and_policy(input: AnchorInput<'_>, policy: AnchorPolicy<'_>) {
    debug_assert!(
        input.floating_size.is_finite()
            && input.floating_size.width >= 0.0
            && input.floating_size.height >= 0.0,
        "AnchorInput::floating_size must be finite and non-negative",
    );
    debug_assert!(
        input.viewport.is_finite(),
        "AnchorInput::viewport must be finite",
    );
    debug_assert!(
        input.boundary.is_finite(),
        "AnchorInput::boundary must be finite",
    );
    debug_assert!(
        policy.scoring.is_finite(),
        "AnchorPolicy::scoring values must be finite",
    );
    debug_assert!(
        policy.hysteresis.is_finite(),
        "AnchorPolicy::hysteresis values must be finite",
    );
    debug_assert!(
        policy.hysteresis.switch_threshold >= 0.0,
        "AnchorPolicy::hysteresis switch_threshold must be non-negative",
    );
    debug_assert!(
        policy_constraints(policy).all(AnchorConstraint::is_finite),
        "AnchorPositionOption::constraints numeric values must be finite",
    );
    debug_assert!(
        policy_constraints(policy).all(|constraint| match *constraint {
            AnchorConstraint::Offset { .. } | AnchorConstraint::HideWhenDetached => true,
            AnchorConstraint::Shift { padding } | AnchorConstraint::KeepInBounds { padding } => {
                padding.x0 >= 0.0 && padding.y0 >= 0.0 && padding.x1 >= 0.0 && padding.y1 >= 0.0
            }
            AnchorConstraint::Size {
                min,
                max,
                allow_shrink: _,
            } => {
                min.width >= 0.0
                    && min.height >= 0.0
                    && max.is_none_or(|max| max.width >= 0.0 && max.height >= 0.0)
            }
            AnchorConstraint::Arrow { size, padding } => {
                size.width >= 0.0 && size.height >= 0.0 && padding >= 0.0
            }
        }),
        "AnchorPositionOption::constraints must contain non-negative insets, sizes, and padding where documented",
    );
}

fn policy_constraints(policy: AnchorPolicy<'_>) -> impl Iterator<Item = &AnchorConstraint> {
    policy.preferred.constraints.iter().chain(
        policy
            .fallbacks
            .iter()
            .flat_map(|option| option.constraints),
    )
}

fn position_options<'a>(
    preferred: AnchorPositionOption<'a>,
    fallbacks: &'a [AnchorPositionOption<'a>],
    order: PositionTryOrder,
    reference: Option<Rect>,
    bounds: Option<Rect>,
) -> Vec<ResolvedPositionOption<'a>> {
    let mut options = Vec::with_capacity(fallbacks.len() + 1);
    options.push(ResolvedPositionOption {
        option_index: 0,
        option: preferred,
    });
    options.extend(
        fallbacks
            .iter()
            .copied()
            .enumerate()
            .map(|(index, option)| ResolvedPositionOption {
                option_index: index + 1,
                option,
            }),
    );

    match (order, reference, bounds) {
        (PositionTryOrder::Normal, _, _) | (_, None, _) | (_, _, None) => {}
        (PositionTryOrder::MostWidth, Some(reference), Some(bounds)) => {
            options.sort_by(|a, b| {
                option_available_size(*b, reference, bounds)
                    .width
                    .total_cmp(&option_available_size(*a, reference, bounds).width)
            });
        }
        (PositionTryOrder::MostHeight, Some(reference), Some(bounds)) => {
            options.sort_by(|a, b| {
                option_available_size(*b, reference, bounds)
                    .height
                    .total_cmp(&option_available_size(*a, reference, bounds).height)
            });
        }
    }

    options
}

fn option_available_size(
    option: ResolvedPositionOption<'_>,
    reference: Rect,
    bounds: Rect,
) -> Size {
    let (main_offset, _) = offsets(option.option.constraints);
    available_size(reference, option.option.placement.side, bounds, main_offset)
}

fn offsets(constraints: &[AnchorConstraint]) -> (f64, f64) {
    let mut main_axis = 0.0;
    let mut cross_axis = 0.0;
    for constraint in constraints {
        if let AnchorConstraint::Offset {
            main_axis: main,
            cross_axis: cross,
        } = *constraint
        {
            main_axis += main;
            cross_axis += cross;
        }
    }
    (main_axis, cross_axis)
}

fn arrow_constraint(constraints: &[AnchorConstraint]) -> Option<(Size, f64)> {
    constraints.iter().find_map(|constraint| {
        if let AnchorConstraint::Arrow { size, padding } = *constraint {
            Some((size, padding))
        } else {
            None
        }
    })
}

fn placement_rect(reference: Rect, size: Size, placement: Placement) -> Rect {
    match placement.side {
        Side::Top => {
            let x = aligned_cross_origin(reference.x0, reference.x1, size.width, placement.align);
            Rect::new(x, reference.y0 - size.height, x + size.width, reference.y0)
        }
        Side::Right => {
            let y = aligned_cross_origin(reference.y0, reference.y1, size.height, placement.align);
            Rect::new(reference.x1, y, reference.x1 + size.width, y + size.height)
        }
        Side::Bottom => {
            let x = aligned_cross_origin(reference.x0, reference.x1, size.width, placement.align);
            Rect::new(x, reference.y1, x + size.width, reference.y1 + size.height)
        }
        Side::Left => {
            let y = aligned_cross_origin(reference.y0, reference.y1, size.height, placement.align);
            Rect::new(reference.x0 - size.width, y, reference.x0, y + size.height)
        }
    }
}

fn aligned_cross_origin(start: f64, end: f64, floating: f64, align: Align) -> f64 {
    match align {
        Align::Start => start,
        Align::Center => 0.5 * (start + end) - floating * 0.5,
        Align::End => end - floating,
    }
}

fn apply_offset(rect: Rect, side: Side, main_axis: f64, cross_axis: f64) -> Rect {
    rect + match side {
        Side::Top => Vec2::new(cross_axis, -main_axis),
        Side::Right => Vec2::new(main_axis, cross_axis),
        Side::Bottom => Vec2::new(cross_axis, main_axis),
        Side::Left => Vec2::new(-main_axis, cross_axis),
    }
}

fn constrain_size(
    current: Size,
    min: Size,
    max: Option<Size>,
    allow_shrink: bool,
    reference: Rect,
    placement: Placement,
    bounds: Rect,
    main_offset: f64,
) -> Option<Size> {
    let max = max.unwrap_or(Size::INFINITY);
    if max.width < min.width || max.height < min.height {
        return None;
    }

    let mut size = current.max(min).min(max);
    if allow_shrink {
        let available = available_size(reference, placement.side, bounds, main_offset);
        size = size.min(available);
        if size.width < min.width || size.height < min.height {
            return None;
        }
    }
    Some(size)
}

fn available_size(reference: Rect, side: Side, bounds: Rect, main_offset: f64) -> Size {
    let bounds_width = bounds.width();
    let bounds_height = bounds.height();
    match side {
        Side::Top => Size::new(
            bounds_width,
            (reference.y0 - main_offset - bounds.y0).max(0.0),
        ),
        Side::Bottom => Size::new(
            bounds_width,
            (bounds.y1 - (reference.y1 + main_offset)).max(0.0),
        ),
        Side::Left => Size::new(
            (reference.x0 - main_offset - bounds.x0).max(0.0),
            bounds_height,
        ),
        Side::Right => Size::new(
            (bounds.x1 - (reference.x1 + main_offset)).max(0.0),
            bounds_height,
        ),
    }
}

fn clamp_rect_to_bounds(rect: Rect, bounds: Rect) -> Rect {
    let width = rect.width();
    let height = rect.height();
    let x0 = if width <= bounds.width() {
        rect.x0.clamp(bounds.x0, bounds.x1 - width)
    } else {
        bounds.x0
    };
    let y0 = if height <= bounds.height() {
        rect.y0.clamp(bounds.y0, bounds.y1 - height)
    } else {
        bounds.y0
    };
    Rect::new(x0, y0, x0 + width, y0 + height)
}

fn arrow_frame(
    floating: Rect,
    reference: Rect,
    side: Side,
    arrow_size: Size,
    padding: f64,
) -> ArrowFrame {
    let reference_center = reference.center();
    match side {
        Side::Top | Side::Bottom => {
            let half_width = arrow_size.width * 0.5;
            let min_center = floating.x0 + padding + half_width;
            let max_center = floating.x1 - padding - half_width;
            let unclamped = reference_center.x;
            let center_x = clamp_or_center(unclamped, min_center, max_center, floating.center().x);
            let clamped = center_x != unclamped;
            let (y0, y1, tip_y) = match side {
                Side::Top => (
                    floating.y1,
                    floating.y1 + arrow_size.height,
                    floating.y1 + arrow_size.height,
                ),
                Side::Bottom => (
                    floating.y0 - arrow_size.height,
                    floating.y0,
                    floating.y0 - arrow_size.height,
                ),
                Side::Right | Side::Left => unreachable!(),
            };
            let rect = Rect::new(center_x - half_width, y0, center_x + half_width, y1);
            ArrowFrame {
                rect,
                tip: Point::new(center_x, tip_y),
                side,
                center: rect.center(),
                clamped,
            }
        }
        Side::Left | Side::Right => {
            let half_height = arrow_size.height * 0.5;
            let min_center = floating.y0 + padding + half_height;
            let max_center = floating.y1 - padding - half_height;
            let unclamped = reference_center.y;
            let center_y = clamp_or_center(unclamped, min_center, max_center, floating.center().y);
            let clamped = center_y != unclamped;
            let (x0, x1, tip_x) = match side {
                Side::Left => (
                    floating.x1,
                    floating.x1 + arrow_size.width,
                    floating.x1 + arrow_size.width,
                ),
                Side::Right => (
                    floating.x0 - arrow_size.width,
                    floating.x0,
                    floating.x0 - arrow_size.width,
                ),
                Side::Top | Side::Bottom => unreachable!(),
            };
            let rect = Rect::new(x0, center_y - half_height, x1, center_y + half_height);
            ArrowFrame {
                rect,
                tip: Point::new(tip_x, center_y),
                side,
                center: rect.center(),
                clamped,
            }
        }
    }
}

fn transform_origin(
    rect: Rect,
    reference: Rect,
    placement: Placement,
    arrow: Option<ArrowFrame>,
) -> Point {
    let reference_center = reference.center();
    match placement.side {
        Side::Top => Point::new(
            arrow
                .map(|arrow| arrow.center.x)
                .unwrap_or_else(|| reference_center.x.clamp(rect.x0, rect.x1))
                - rect.x0,
            rect.height(),
        ),
        Side::Right => Point::new(
            0.0,
            arrow
                .map(|arrow| arrow.center.y)
                .unwrap_or_else(|| reference_center.y.clamp(rect.y0, rect.y1))
                - rect.y0,
        ),
        Side::Bottom => Point::new(
            arrow
                .map(|arrow| arrow.center.x)
                .unwrap_or_else(|| reference_center.x.clamp(rect.x0, rect.x1))
                - rect.x0,
            0.0,
        ),
        Side::Left => Point::new(
            rect.width(),
            arrow
                .map(|arrow| arrow.center.y)
                .unwrap_or_else(|| reference_center.y.clamp(rect.y0, rect.y1))
                - rect.y0,
        ),
    }
}

fn candidate_metrics(
    rect: Rect,
    base_rect: Rect,
    reference: Rect,
    desired_size: Size,
    resolved_size: Size,
    bounds: Rect,
    is_preferred: bool,
    is_incumbent: bool,
) -> CandidateMetrics {
    let visible = rect.intersect(bounds);
    let visible_area = visible.area();
    let rect_area = rect.area();
    let overflow = overflow_insets(rect, bounds);
    CandidateMetrics {
        visible_area,
        overflow,
        overflow_area: rect_area - visible_area,
        anchor_distance: point_to_rect_distance(reference.center(), rect),
        shifted_distance: base_rect.origin().distance(rect.origin()),
        size_delta: Size::new(
            (desired_size.width - resolved_size.width).abs(),
            (desired_size.height - resolved_size.height).abs(),
        ),
        is_preferred,
        is_incumbent,
    }
}

fn overflow_insets(rect: Rect, bounds: Rect) -> Insets {
    Insets::new(
        (bounds.x0 - rect.x0).max(0.0),
        (bounds.y0 - rect.y0).max(0.0),
        (rect.x1 - bounds.x1).max(0.0),
        (rect.y1 - bounds.y1).max(0.0),
    )
}

fn choose_candidate_with_report(
    candidates: &[AnchorCandidate],
    previous: Option<&PreviousAnchorFrame>,
    scoring: ScoringPolicy,
    hysteresis: HysteresisPolicy,
) -> (usize, bool) {
    let raw_best = best_candidate(candidates, scoring, HysteresisPolicy::disabled());
    let Some(previous) = previous else {
        return (raw_best, false);
    };

    if !hysteresis.enabled {
        return (raw_best, false);
    }

    let incumbent = candidates.iter().position(|candidate| {
        !candidate.rejected
            && previous_matches_candidate(previous, candidate.option_index, candidate.option_key)
    });
    let Some(incumbent) = incumbent else {
        return (raw_best, false);
    };
    let hysteresis_best = best_candidate(candidates, scoring, hysteresis);
    if incumbent == hysteresis_best {
        return (hysteresis_best, raw_best != hysteresis_best);
    }

    let best_score = choice_score(&candidates[hysteresis_best], scoring, hysteresis);
    let incumbent_score = choice_score(&candidates[incumbent], scoring, hysteresis);
    let threshold = hysteresis.switch_threshold;
    if best_score < incumbent_score + threshold {
        (incumbent, raw_best != incumbent)
    } else {
        (hysteresis_best, raw_best != hysteresis_best)
    }
}

fn best_candidate(
    candidates: &[AnchorCandidate],
    scoring: ScoringPolicy,
    hysteresis: HysteresisPolicy,
) -> usize {
    let mut best: Option<(usize, f64)> = None;
    for (index, candidate) in candidates.iter().enumerate() {
        if candidate.rejected {
            continue;
        }
        let score = choice_score(candidate, scoring, hysteresis);
        match best {
            Some((_, best_score)) if score <= best_score => {}
            _ => best = Some((index, score)),
        }
    }
    best.map(|(index, _)| index).unwrap_or(0)
}

fn choice_score(
    candidate: &AnchorCandidate,
    mut scoring: ScoringPolicy,
    hysteresis: HysteresisPolicy,
) -> f64 {
    scoring.incumbent_bonus = if hysteresis.enabled {
        hysteresis.incumbent_bonus
    } else {
        0.0
    };
    default_candidate_score(candidate, scoring)
}

fn previous_matches_candidate(
    previous: &PreviousAnchorFrame,
    option_index: usize,
    option_key: AnchorOptionKey,
) -> bool {
    match (previous.option_key(), option_key) {
        (AnchorOptionKey::Id(previous), AnchorOptionKey::Id(current)) => previous == current,
        _ => previous.option_index() == option_index,
    }
}

fn nonempty_rect(rect: Rect) -> Option<Rect> {
    let rect = rect.abs();
    if rect.width() > 0.0 && rect.height() > 0.0 {
        Some(rect)
    } else {
        None
    }
}

fn has_overflow(insets: Insets) -> bool {
    insets.x0 > 0.0 || insets.y0 > 0.0 || insets.x1 > 0.0 || insets.y1 > 0.0
}

fn point_to_rect_distance(point: Point, rect: Rect) -> f64 {
    let closest = Point::new(
        point.x.clamp(rect.x0, rect.x1),
        point.y.clamp(rect.y0, rect.y1),
    );
    point.distance(closest)
}

fn clamp_or_center(value: f64, min: f64, max: f64, center: f64) -> f64 {
    if min > max {
        center
    } else {
        value.clamp(min, max)
    }
}

#[cfg(test)]
mod tests {
    use kurbo::{Insets, Point, Rect, Size};

    use super::*;
    use crate::{Anchor, AnchorRects, RectReference};

    fn scene(anchor: Rect, size: Size) -> AnchorInput<'static> {
        AnchorInput {
            anchor: Anchor::Rect(anchor),
            floating_size: size,
            viewport: Rect::new(0.0, 0.0, 300.0, 200.0),
            boundary: Rect::new(0.0, 0.0, 300.0, 200.0),
            previous: None,
        }
    }

    fn assert_close(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() < 0.000_001,
            "expected {expected}, got {actual}",
        );
    }

    fn option(placement: Placement) -> AnchorPositionOption<'static> {
        AnchorPositionOption::new(placement)
    }

    fn option_with_constraints<'a>(
        placement: Placement,
        constraints: &'a [AnchorConstraint],
    ) -> AnchorPositionOption<'a> {
        AnchorPositionOption::new(placement).with_constraints(constraints)
    }

    fn policy_with_constraints<'a>(
        placement: Placement,
        constraints: &'a [AnchorConstraint],
    ) -> AnchorPolicy<'a> {
        AnchorPolicy::new(option_with_constraints(placement, constraints), &[])
    }

    #[test]
    fn bottom_center_places_rect_below_anchor() {
        let frame = resolve_anchor(
            scene(Rect::new(100.0, 80.0, 140.0, 100.0), Size::new(80.0, 40.0)),
            AnchorPolicy::placement(Placement::BOTTOM),
        );

        assert_eq!(frame.rect, Rect::new(80.0, 100.0, 160.0, 140.0));
        assert_eq!(frame.transform_origin, Point::new(40.0, 0.0));
        assert!(frame.visible);
    }

    #[test]
    fn alignments_work_for_top_start_and_right_end() {
        let top = resolve_anchor(
            scene(Rect::new(100.0, 80.0, 140.0, 100.0), Size::new(80.0, 40.0)),
            AnchorPolicy::placement(Placement::TOP_START),
        );
        assert_eq!(top.rect, Rect::new(100.0, 40.0, 180.0, 80.0));

        let right = resolve_anchor(
            scene(Rect::new(100.0, 80.0, 140.0, 100.0), Size::new(80.0, 40.0)),
            AnchorPolicy::placement(Placement::RIGHT_END),
        );
        assert_eq!(right.rect, Rect::new(140.0, 60.0, 220.0, 100.0));
    }

    #[test]
    fn point_anchor_and_zero_rect_anchor_are_valid() {
        let input = AnchorInput {
            anchor: Anchor::Point(Point::new(120.0, 90.0)),
            floating_size: Size::new(40.0, 20.0),
            viewport: Rect::new(0.0, 0.0, 300.0, 200.0),
            boundary: Rect::new(0.0, 0.0, 300.0, 200.0),
            previous: None,
        };
        let point = resolve_anchor(input, AnchorPolicy::placement(Placement::BOTTOM));
        assert_eq!(point.reference_rect, Rect::new(120.0, 90.0, 120.0, 90.0),);
        assert_eq!(point.rect, Rect::new(100.0, 90.0, 140.0, 110.0));

        let zero = resolve_anchor(
            scene(Rect::new(120.0, 90.0, 120.0, 90.0), Size::new(40.0, 20.0)),
            AnchorPolicy::placement(Placement::BOTTOM),
        );
        assert!(zero.visible);
    }

    #[test]
    fn multi_rect_reference_policies_are_respected() {
        let rects = [
            Rect::new(100.0, 100.0, 240.0, 118.0),
            Rect::new(80.0, 120.0, 260.0, 138.0),
            Rect::new(80.0, 140.0, 140.0, 158.0),
        ];
        let anchor = Anchor::Rects {
            rects: AnchorRects {
                rects: &rects,
                primary: Some(0),
                focus: Some(2),
            },
            reference: RectReference::Focus,
        };
        assert_eq!(reference_rect(anchor), Some(rects[2]));

        let bbox = Anchor::Rects {
            rects: AnchorRects {
                rects: &rects,
                primary: Some(0),
                focus: Some(2),
            },
            reference: RectReference::BoundingBox,
        };
        assert_eq!(
            reference_rect(bbox),
            Some(Rect::new(80.0, 100.0, 260.0, 158.0)),
        );
    }

    #[test]
    fn empty_rect_slice_rejects_cleanly() {
        let input = AnchorInput {
            anchor: Anchor::Rects {
                rects: AnchorRects {
                    rects: &[],
                    primary: None,
                    focus: None,
                },
                reference: RectReference::BoundingBox,
            },
            floating_size: Size::new(80.0, 40.0),
            viewport: Rect::new(0.0, 0.0, 300.0, 200.0),
            boundary: Rect::new(0.0, 0.0, 300.0, 200.0),
            previous: None,
        };
        let frame = resolve_anchor(input, AnchorPolicy::placement(Placement::BOTTOM));
        assert!(!frame.visible);
        assert_eq!(
            frame.collision.candidates[0].reject_reason,
            Some(AnchorRejectReason::EmptyAnchor),
        );
    }

    #[test]
    #[should_panic(expected = "AnchorInput::viewport must be finite")]
    fn resolve_anchor_debug_asserts_non_finite_viewport() {
        let input = AnchorInput {
            anchor: Anchor::Rect(Rect::new(100.0, 80.0, 140.0, 100.0)),
            floating_size: Size::new(80.0, 40.0),
            viewport: Rect::new(0.0, 0.0, f64::NAN, 200.0),
            boundary: Rect::new(0.0, 0.0, 300.0, 200.0),
            previous: None,
        };

        let _ = resolve_anchor(input, AnchorPolicy::placement(Placement::BOTTOM));
    }

    #[test]
    #[should_panic(expected = "anchor point must be finite")]
    fn reference_rect_debug_asserts_non_finite_point() {
        let _ = reference_rect(Anchor::Point(Point::new(f64::NAN, 10.0)));
    }

    #[test]
    #[should_panic(expected = "AnchorPositionOption::constraints numeric values must be finite")]
    fn resolve_anchor_debug_asserts_non_finite_constraint() {
        let constraints = [AnchorConstraint::Arrow {
            size: Size::new(12.0, 6.0),
            padding: f64::NAN,
        }];
        let input = scene(Rect::new(100.0, 80.0, 140.0, 100.0), Size::new(80.0, 40.0));

        let _ = resolve_anchor(
            input,
            policy_with_constraints(Placement::BOTTOM, &constraints),
        );
    }

    #[test]
    #[should_panic(
        expected = "AnchorPositionOption::constraints must contain non-negative insets, sizes, and padding where documented"
    )]
    fn resolve_anchor_debug_asserts_negative_constraint_value() {
        let constraints = [AnchorConstraint::Arrow {
            size: Size::new(12.0, 6.0),
            padding: -1.0,
        }];
        let input = scene(Rect::new(100.0, 80.0, 140.0, 100.0), Size::new(80.0, 40.0));

        let _ = resolve_anchor(
            input,
            policy_with_constraints(Placement::BOTTOM, &constraints),
        );
    }

    #[test]
    fn offset_moves_on_main_and_cross_axes() {
        let constraints = [AnchorConstraint::Offset {
            main_axis: 8.0,
            cross_axis: 3.0,
        }];
        let policy = policy_with_constraints(Placement::BOTTOM_START, &constraints);
        let frame = resolve_anchor(
            scene(Rect::new(100.0, 80.0, 140.0, 100.0), Size::new(80.0, 40.0)),
            policy,
        );
        assert_eq!(frame.rect, Rect::new(103.0, 108.0, 183.0, 148.0));
    }

    #[test]
    fn shift_keeps_candidate_inside_boundary_with_padding() {
        let constraints = [AnchorConstraint::Shift {
            padding: Insets::uniform(8.0),
        }];
        let input = AnchorInput {
            anchor: Anchor::Rect(Rect::new(260.0, 80.0, 290.0, 100.0)),
            floating_size: Size::new(80.0, 40.0),
            viewport: Rect::new(0.0, 0.0, 300.0, 200.0),
            boundary: Rect::new(0.0, 0.0, 300.0, 200.0),
            previous: None,
        };
        let frame = resolve_anchor(
            input,
            policy_with_constraints(Placement::BOTTOM, &constraints),
        );

        assert_eq!(frame.rect, Rect::new(212.0, 100.0, 292.0, 140.0));
        assert!(frame.collision.candidates[0].metrics.shifted_distance > 0.0);
    }

    #[test]
    fn keep_in_bounds_applies_as_final_translation_clamp() {
        let constraints = [
            AnchorConstraint::Shift {
                padding: Insets::ZERO,
            },
            AnchorConstraint::KeepInBounds {
                padding: Insets::uniform(20.0),
            },
        ];
        let input = AnchorInput {
            anchor: Anchor::Rect(Rect::new(260.0, 80.0, 290.0, 100.0)),
            floating_size: Size::new(80.0, 40.0),
            viewport: Rect::new(0.0, 0.0, 300.0, 200.0),
            boundary: Rect::new(0.0, 0.0, 300.0, 200.0),
            previous: None,
        };
        let frame = resolve_anchor(
            input,
            policy_with_constraints(Placement::BOTTOM, &constraints),
        );

        assert_eq!(frame.rect, Rect::new(200.0, 100.0, 280.0, 140.0));
        assert!(
            frame.collision.candidates[0].metrics.shifted_distance > 0.0,
            "final clamp should be reflected in movement diagnostics",
        );
    }

    #[test]
    fn fallback_option_can_carry_distinct_constraints() {
        let fallback_constraints = [AnchorConstraint::Offset {
            main_axis: 6.0,
            cross_axis: 0.0,
        }];
        let fallbacks = [option_with_constraints(
            Placement::TOP,
            &fallback_constraints,
        )];
        let input = AnchorInput {
            anchor: Anchor::Rect(Rect::new(140.0, 196.0, 160.0, 200.0)),
            floating_size: Size::new(80.0, 40.0),
            viewport: Rect::new(0.0, 0.0, 300.0, 200.0),
            boundary: Rect::new(0.0, 0.0, 300.0, 200.0),
            previous: None,
        };

        let frame = resolve_anchor(
            input,
            AnchorPolicy::new(option(Placement::BOTTOM), &fallbacks),
        );

        assert_eq!(frame.placement, Placement::TOP);
        assert_eq!(frame.option_index, 1);
        assert_eq!(frame.rect, Rect::new(110.0, 150.0, 190.0, 190.0));
    }

    #[test]
    fn most_height_order_reorders_candidates_before_scoring() {
        let fallbacks = [option(Placement::RIGHT)];
        let frame = resolve_anchor(
            scene(Rect::new(100.0, 80.0, 140.0, 100.0), Size::new(80.0, 40.0)),
            AnchorPolicy::new(option(Placement::BOTTOM), &fallbacks)
                .with_order(PositionTryOrder::MostHeight),
        );

        assert_eq!(frame.collision.candidates[0].placement, Placement::RIGHT);
        assert_eq!(frame.collision.candidates[0].option_index, 1);
        assert_eq!(frame.collision.candidates[1].placement, Placement::BOTTOM);
        assert_eq!(frame.collision.candidates[1].option_index, 0);
    }

    #[test]
    fn fallback_wins_when_preferred_overflows_badly() {
        let fallbacks = [option(Placement::TOP)];
        let input = AnchorInput {
            anchor: Anchor::Rect(Rect::new(140.0, 170.0, 160.0, 190.0)),
            floating_size: Size::new(120.0, 80.0),
            viewport: Rect::new(0.0, 0.0, 300.0, 200.0),
            boundary: Rect::new(0.0, 0.0, 300.0, 200.0),
            previous: None,
        };
        let frame = resolve_anchor(
            input,
            AnchorPolicy::new(option(Placement::BOTTOM), &fallbacks),
        );
        assert_eq!(frame.placement, Placement::TOP);
    }

    #[test]
    fn default_scoring_keeps_viable_preference_at_small_and_large_scales() {
        let fallbacks = [option(Placement::TOP)];
        for size in [Size::new(20.0, 10.0), Size::new(220.0, 120.0)] {
            let frame = resolve_anchor(
                scene(Rect::new(130.0, 80.0, 170.0, 100.0), size),
                AnchorPolicy::new(option(Placement::BOTTOM), &fallbacks),
            );

            assert_eq!(frame.placement, Placement::BOTTOM);
        }
    }

    #[test]
    fn default_scoring_rejects_fully_invisible_preference_at_small_and_large_scales() {
        let fallbacks = [option(Placement::TOP)];
        for size in [Size::new(20.0, 10.0), Size::new(220.0, 120.0)] {
            let input = AnchorInput {
                anchor: Anchor::Rect(Rect::new(130.0, 196.0, 170.0, 200.0)),
                floating_size: size,
                viewport: Rect::new(0.0, 0.0, 300.0, 200.0),
                boundary: Rect::new(0.0, 0.0, 300.0, 200.0),
                previous: None,
            };
            let frame = resolve_anchor(
                input,
                AnchorPolicy::new(option(Placement::BOTTOM), &fallbacks),
            );

            assert_eq!(frame.placement, Placement::TOP);
        }
    }

    #[test]
    fn collision_first_scoring_removes_preferred_bonus() {
        let scoring = ScoringPolicy::collision_first();

        assert_eq!(scoring.preferred_bonus, 0.0);
        assert_eq!(scoring.incumbent_bonus, 0.0);
        assert_eq!(
            scoring.visible_area_weight,
            ScoringPolicy::default().visible_area_weight,
        );
    }

    #[test]
    fn size_constraint_shrinks_to_available_space() {
        let constraints = [AnchorConstraint::Size {
            min: Size::new(60.0, 20.0),
            max: None,
            allow_shrink: true,
        }];
        let input = AnchorInput {
            anchor: Anchor::Rect(Rect::new(100.0, 150.0, 140.0, 170.0)),
            floating_size: Size::new(100.0, 80.0),
            viewport: Rect::new(0.0, 0.0, 300.0, 200.0),
            boundary: Rect::new(0.0, 0.0, 300.0, 200.0),
            previous: None,
        };
        let frame = resolve_anchor(
            input,
            policy_with_constraints(Placement::BOTTOM, &constraints),
        );
        assert_eq!(frame.floating_size, Size::new(100.0, 30.0));
        assert!(frame.visible);
    }

    #[test]
    fn duplicate_constraint_behavior_is_deterministic() {
        let constraints = [
            AnchorConstraint::Offset {
                main_axis: 4.0,
                cross_axis: 1.0,
            },
            AnchorConstraint::Offset {
                main_axis: 6.0,
                cross_axis: 2.0,
            },
            AnchorConstraint::Size {
                min: Size::new(40.0, 20.0),
                max: Some(Size::new(70.0, 30.0)),
                allow_shrink: false,
            },
            AnchorConstraint::Size {
                min: Size::new(50.0, 10.0),
                max: Some(Size::new(60.0, 25.0)),
                allow_shrink: false,
            },
            AnchorConstraint::Arrow {
                size: Size::new(10.0, 4.0),
                padding: 0.0,
            },
            AnchorConstraint::Arrow {
                size: Size::new(30.0, 12.0),
                padding: 0.0,
            },
        ];
        let frame = resolve_anchor(
            scene(Rect::new(100.0, 80.0, 140.0, 100.0), Size::new(80.0, 40.0)),
            policy_with_constraints(Placement::BOTTOM_START, &constraints),
        );

        assert_eq!(frame.rect, Rect::new(103.0, 110.0, 163.0, 135.0));
        assert_eq!(frame.floating_size, Size::new(60.0, 25.0));
        assert_eq!(
            frame
                .arrow
                .expect("first arrow constraint is used")
                .rect
                .size(),
            Size::new(10.0, 4.0),
        );
    }

    #[test]
    fn size_constraint_rejects_when_min_cannot_fit() {
        let constraints = [AnchorConstraint::Size {
            min: Size::new(60.0, 40.0),
            max: None,
            allow_shrink: true,
        }];
        let input = AnchorInput {
            anchor: Anchor::Rect(Rect::new(100.0, 180.0, 140.0, 190.0)),
            floating_size: Size::new(100.0, 80.0),
            viewport: Rect::new(0.0, 0.0, 300.0, 200.0),
            boundary: Rect::new(0.0, 0.0, 300.0, 200.0),
            previous: None,
        };
        let frame = resolve_anchor(
            input,
            policy_with_constraints(Placement::BOTTOM, &constraints),
        );
        assert_eq!(
            frame.collision.candidates[0].reject_reason,
            Some(AnchorRejectReason::CannotSatisfyMinSize),
        );
    }

    #[test]
    fn arrow_points_toward_anchor_and_clamps_near_edge() {
        let constraints = [
            AnchorConstraint::Shift {
                padding: Insets::ZERO,
            },
            AnchorConstraint::Arrow {
                size: Size::new(12.0, 6.0),
                padding: 8.0,
            },
        ];
        let input = AnchorInput {
            anchor: Anchor::Rect(Rect::new(0.0, 80.0, 10.0, 100.0)),
            floating_size: Size::new(40.0, 30.0),
            viewport: Rect::new(0.0, 0.0, 300.0, 200.0),
            boundary: Rect::new(0.0, 0.0, 300.0, 200.0),
            previous: None,
        };
        let frame = resolve_anchor(
            input,
            policy_with_constraints(Placement::BOTTOM, &constraints),
        );
        let arrow = frame.arrow.expect("arrow should be emitted");
        assert_eq!(arrow.side, Side::Bottom);
        assert_close(arrow.tip.y, frame.rect.y0 - 6.0);
        assert!(arrow.clamped);
        assert_eq!(frame.transform_origin.y, 0.0);
    }

    #[test]
    fn hide_when_detached_marks_frame_invisible() {
        let constraints = [AnchorConstraint::HideWhenDetached];
        let input = AnchorInput {
            anchor: Anchor::Rect(Rect::new(100.0, 240.0, 140.0, 260.0)),
            floating_size: Size::new(80.0, 40.0),
            viewport: Rect::new(0.0, 0.0, 300.0, 200.0),
            boundary: Rect::new(0.0, 0.0, 300.0, 200.0),
            previous: None,
        };
        let frame = resolve_anchor(
            input,
            policy_with_constraints(Placement::BOTTOM, &constraints),
        );
        assert!(!frame.visible);
        assert!(frame.detached);
        assert_eq!(
            frame.collision.candidates[0].reject_reason,
            Some(AnchorRejectReason::Detached),
        );
    }

    #[test]
    fn hysteresis_keeps_previous_when_score_difference_is_small() {
        let fallbacks = [option(Placement::BOTTOM).with_key(AnchorOptionKey::id(20))];
        let input = AnchorInput {
            anchor: Anchor::Rect(Rect::new(100.0, 92.0, 140.0, 108.0)),
            floating_size: Size::new(80.0, 80.0),
            viewport: Rect::new(0.0, 0.0, 300.0, 200.0),
            boundary: Rect::new(0.0, 0.0, 300.0, 200.0),
            previous: None,
        };
        let previous = resolve_anchor(
            input,
            AnchorPolicy::new(
                option(Placement::TOP).with_key(AnchorOptionKey::id(10)),
                &fallbacks,
            ),
        );
        let previous = PreviousAnchorFrame::from(&previous);

        let current_input = AnchorInput {
            previous: Some(&previous),
            ..input
        };
        let fallbacks = [option(Placement::TOP).with_key(AnchorOptionKey::id(10))];
        let policy = AnchorPolicy {
            preferred: option(Placement::BOTTOM).with_key(AnchorOptionKey::id(20)),
            fallbacks: &fallbacks,
            order: PositionTryOrder::Normal,
            scoring: ScoringPolicy {
                preferred_bonus: 0.0,
                incumbent_bonus: 0.0,
                ..ScoringPolicy::default()
            },
            hysteresis: HysteresisPolicy {
                enabled: true,
                incumbent_bonus: 0.0,
                switch_threshold: 1000.0,
            },
        };
        let frame = resolve_anchor(current_input, policy);
        assert_eq!(frame.placement, Placement::TOP);
        assert!(frame.collision.hysteresis_applied);
    }

    #[test]
    fn hysteresis_applied_when_incumbent_bonus_keeps_previous_option() {
        let input = AnchorInput {
            anchor: Anchor::Rect(Rect::new(100.0, 92.0, 140.0, 108.0)),
            floating_size: Size::new(80.0, 80.0),
            viewport: Rect::new(0.0, 0.0, 300.0, 200.0),
            boundary: Rect::new(0.0, 0.0, 300.0, 200.0),
            previous: None,
        };
        let previous = resolve_anchor(
            input,
            AnchorPolicy::new(
                option(Placement::TOP).with_key(AnchorOptionKey::id(10)),
                &[option(Placement::BOTTOM).with_key(AnchorOptionKey::id(20))],
            ),
        );
        let previous = PreviousAnchorFrame::from(&previous);

        let input = AnchorInput {
            previous: Some(&previous),
            ..input
        };
        let fallbacks = [option(Placement::TOP).with_key(AnchorOptionKey::id(10))];
        let policy = AnchorPolicy {
            preferred: option(Placement::BOTTOM).with_key(AnchorOptionKey::id(20)),
            fallbacks: &fallbacks,
            order: PositionTryOrder::Normal,
            scoring: ScoringPolicy {
                preferred_bonus: 100.0,
                incumbent_bonus: 0.0,
                ..ScoringPolicy::default()
            },
            hysteresis: HysteresisPolicy {
                enabled: true,
                incumbent_bonus: 150.0,
                switch_threshold: 0.0,
            },
        };
        let frame = resolve_anchor(input, policy);

        assert_eq!(frame.placement, Placement::TOP);
        assert_eq!(frame.option_key, AnchorOptionKey::id(10));
        assert!(frame.collision.hysteresis_applied);
    }

    #[test]
    fn hysteresis_disabled_allows_switch() {
        let input = AnchorInput {
            anchor: Anchor::Rect(Rect::new(100.0, 92.0, 140.0, 108.0)),
            floating_size: Size::new(80.0, 80.0),
            viewport: Rect::new(0.0, 0.0, 300.0, 200.0),
            boundary: Rect::new(0.0, 0.0, 300.0, 200.0),
            previous: None,
        };
        let previous = resolve_anchor(input, AnchorPolicy::placement(Placement::TOP));
        let previous = PreviousAnchorFrame::from(&previous);
        let input = AnchorInput {
            previous: Some(&previous),
            ..input
        };
        let fallbacks = [option(Placement::TOP)];
        let policy = AnchorPolicy {
            preferred: option(Placement::BOTTOM),
            fallbacks: &fallbacks,
            order: PositionTryOrder::Normal,
            scoring: ScoringPolicy {
                preferred_bonus: 500.0,
                incumbent_bonus: 0.0,
                ..ScoringPolicy::default()
            },
            hysteresis: HysteresisPolicy {
                enabled: false,
                incumbent_bonus: 0.0,
                switch_threshold: 1000.0,
            },
        };
        let frame = resolve_anchor(input, policy);
        assert_eq!(frame.placement, Placement::BOTTOM);
        assert!(!frame.collision.hysteresis_applied);
    }

    #[test]
    fn previous_anchor_frame_keeps_only_hysteresis_state() {
        let frame = resolve_anchor(
            scene(Rect::new(100.0, 80.0, 140.0, 100.0), Size::new(80.0, 40.0)),
            AnchorPolicy::placement(Placement::BOTTOM),
        );

        let previous = PreviousAnchorFrame::from(&frame);

        assert_eq!(previous.option_index(), frame.option_index);
        assert_eq!(previous.option_key(), frame.option_key);
        assert_eq!(previous.placement(), frame.placement);
        assert_eq!(previous.rect(), frame.rect);
        assert_eq!(previous.reference_rect(), frame.reference_rect);
        assert_eq!(previous.floating_size(), frame.floating_size);
    }
}
