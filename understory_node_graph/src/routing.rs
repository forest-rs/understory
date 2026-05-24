// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Edge routing primitives.

use alloc::vec::Vec;

use kurbo::{Point, Rect};

use crate::ids::EdgeId;

/// Context provided to edge routers.
///
/// Routes are computed from already-derived port anchors. A router does not
/// need access to the whole graph unless the host chooses to capture additional
/// state in the router value.
#[derive(Copy, Clone, Debug)]
pub struct RouteContext {
    /// Source/output anchor in world space.
    pub output_anchor: Point,
    /// Destination/input anchor in world space.
    pub input_anchor: Point,
}

/// Realized routed edge geometry.
///
/// Renderers can draw `points` as a polyline or use them as control data for a
/// richer path. The `bounds` field lets visibility and hit-test code avoid
/// recomputing extents for every frame.
#[derive(Clone, Debug, Default)]
pub struct RoutedEdge {
    /// Polyline control points in world space.
    pub points: Vec<Point>,
    /// Bounding box of the routed edge.
    pub bounds: Rect,
}

impl RoutedEdge {
    /// Creates routed geometry from a point list.
    ///
    /// Bounds are computed from the supplied points. An empty route receives
    /// [`Rect::ZERO`] bounds.
    #[must_use]
    pub fn from_points(points: Vec<Point>) -> Self {
        let bounds = if let Some(first) = points.first().copied() {
            let (min_x, min_y, max_x, max_y) = points.iter().copied().skip(1).fold(
                (first.x, first.y, first.x, first.y),
                |(min_x, min_y, max_x, max_y), point| {
                    (
                        min_x.min(point.x),
                        min_y.min(point.y),
                        max_x.max(point.x),
                        max_y.max(point.y),
                    )
                },
            );
            Rect::new(min_x, min_y, max_x, max_y)
        } else {
            Rect::ZERO
        };
        Self { points, bounds }
    }
}

/// Replaceable edge routing policy.
///
/// Implement this when straight or orthogonal routes are not enough. The router
/// is passed into [`GraphComputed::rebuild`](crate::GraphComputed::rebuild), so
/// routing strategy can be swapped per view or per rebuild without changing the
/// semantic graph.
pub trait EdgeRouter {
    /// Routes one edge from the provided anchors.
    ///
    /// The `edge` id is provided so routers can consult host-side route hints
    /// keyed by edge id.
    fn route(&self, edge: EdgeId, cx: &RouteContext) -> RoutedEdge;
}

/// Straight-line router used by the default examples and tests.
///
/// This is useful as a simple baseline and for tools where edge aesthetics are
/// handled by a renderer after the two endpoints are known.
#[derive(Copy, Clone, Debug, Default)]
pub struct StraightEdgeRouter;

impl EdgeRouter for StraightEdgeRouter {
    fn route(&self, _edge: EdgeId, cx: &RouteContext) -> RoutedEdge {
        RoutedEdge::from_points(alloc::vec![cx.output_anchor, cx.input_anchor])
    }
}

/// Orthogonal "dogleg" router for node-graph style connections.
///
/// The route runs horizontally out from the source anchor, bends at a shared
/// column, then runs horizontally into the destination anchor.
#[derive(Copy, Clone, Debug)]
pub struct OrthogonalEdgeRouter {
    /// Minimum horizontal run before the first bend.
    ///
    /// This keeps very close ports from producing cramped bends.
    pub min_horizontal_run: f64,
}

impl Default for OrthogonalEdgeRouter {
    fn default() -> Self {
        Self {
            min_horizontal_run: 24.0,
        }
    }
}

impl EdgeRouter for OrthogonalEdgeRouter {
    fn route(&self, _edge: EdgeId, cx: &RouteContext) -> RoutedEdge {
        let output = cx.output_anchor;
        let input = cx.input_anchor;

        if (output.x - input.x).abs() <= f64::EPSILON || (output.y - input.y).abs() <= f64::EPSILON
        {
            return RoutedEdge::from_points(alloc::vec![output, input]);
        }

        let half_gap = ((input.x - output.x).abs() * 0.5).max(self.min_horizontal_run);
        let bend_x = if input.x >= output.x {
            output.x + half_gap
        } else {
            output.x - half_gap
        };

        RoutedEdge::from_points(alloc::vec![
            output,
            Point::new(bend_x, output.y),
            Point::new(bend_x, input.y),
            input,
        ])
    }
}

#[cfg(test)]
mod tests {
    use kurbo::Point;

    use super::{EdgeRouter, OrthogonalEdgeRouter, RouteContext, StraightEdgeRouter};
    use crate::ids::EdgeId;

    #[test]
    fn straight_router_returns_two_points() {
        let route = StraightEdgeRouter.route(
            EdgeId::from_parts(0, 0),
            &RouteContext {
                output_anchor: Point::new(10.0, 20.0),
                input_anchor: Point::new(80.0, 48.0),
            },
        );
        assert_eq!(route.points.len(), 2);
    }

    #[test]
    fn orthogonal_router_inserts_bends() {
        let route = OrthogonalEdgeRouter::default().route(
            EdgeId::from_parts(0, 0),
            &RouteContext {
                output_anchor: Point::new(20.0, 30.0),
                input_anchor: Point::new(140.0, 90.0),
            },
        );
        assert_eq!(
            route.points,
            alloc::vec![
                Point::new(20.0, 30.0),
                Point::new(80.0, 30.0),
                Point::new(80.0, 90.0),
                Point::new(140.0, 90.0),
            ]
        );
    }
}
