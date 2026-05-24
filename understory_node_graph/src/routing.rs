// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Edge routing primitives.

use alloc::vec::Vec;

use kurbo::{Point, Rect};

use crate::ids::EdgeId;

/// Context provided to edge routers.
#[derive(Copy, Clone, Debug)]
pub struct RouteContext {
    /// Source/output anchor.
    pub output_anchor: Point,
    /// Destination/input anchor.
    pub input_anchor: Point,
}

/// Realized routed edge geometry.
#[derive(Clone, Debug, Default)]
pub struct RoutedEdge {
    /// Polyline control points in world space.
    pub points: Vec<Point>,
    /// Bounding box of the routed edge.
    pub bounds: Rect,
}

impl RoutedEdge {
    /// Creates routed geometry from a point list.
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
pub trait EdgeRouter {
    /// Routes one edge from the provided anchors.
    fn route(&self, edge: EdgeId, cx: &RouteContext) -> RoutedEdge;
}

/// Straight-line router used by the default examples and tests.
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
