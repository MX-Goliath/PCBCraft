//! Reuse complex pour edges and evaluate only segments within the rule gap.
//! Containment is tested separately so holes and separate islands stay exact.
use geo::{BoundingRect, Distance, Euclidean, Intersects, Line, LinesIter, MultiPolygon};
use rstar::{AABB, RTree, primitives::CachedEnvelope};
use std::cell::OnceCell;

pub(super) struct Geometry {
    pub shape: MultiPolygon<f64>,
    edges: Option<OnceCell<RTree<CachedEnvelope<Line<f64>>>>>,
    vertices: usize,
}
impl Geometry {
    pub fn new(shape: MultiPolygon<f64>) -> Self {
        let count: usize = shape
            .0
            .iter()
            .map(|p| p.exterior().0.len() + p.interiors().iter().map(|r| r.0.len()).sum::<usize>())
            .sum();
        Self {
            shape,
            edges: (count > 128).then(OnceCell::new),
            vertices: count,
        }
    }
    pub fn intersects(&self, other: &Self) -> bool {
        if self.edges.is_some() || other.edges.is_some() {
            self.distance_within(other, 0.).is_some()
        } else {
            self.shape.intersects(&other.shape)
        }
    }
    pub fn distance_within(&self, other: &Self, gap: f64) -> Option<f64> {
        let (indexed, small, cache) = if let Some(cache) = &self.edges
            && self.vertices >= other.vertices
        {
            (&self.shape, &other.shape, cache)
        } else if let Some(cache) = &other.edges {
            (&other.shape, &self.shape, cache)
        } else {
            let distance = Euclidean.distance(&self.shape, &other.shape);
            return (distance <= gap).then_some(distance);
        };
        // Boundary distances alone would miss containment, including separate
        // islands. The same representative points are used by geo::Intersects.
        for a in &indexed.0 {
            for b in &small.0 {
                if a.exterior().0.first().is_some_and(|p| b.intersects(p))
                    || b.exterior().0.first().is_some_and(|p| a.intersects(p))
                {
                    return Some(0.);
                }
            }
        }
        let tree = cache.get_or_init(|| {
            RTree::bulk_load(indexed.lines_iter().map(CachedEnvelope::new).collect())
        });
        let mut distance = f64::INFINITY;
        for line in small.lines_iter() {
            let b = line.bounding_rect();
            let envelope = AABB::from_corners(
                geo::Point::new(b.min().x - gap, b.min().y - gap),
                geo::Point::new(b.max().x + gap, b.max().y + gap),
            );
            for edge in tree.locate_in_envelope_intersecting(&envelope) {
                distance = distance.min(Euclidean.distance(edge as &Line<f64>, &line));
                if distance == 0. {
                    return Some(0.);
                }
            }
        }
        (distance <= gap).then_some(distance)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use geo::{BooleanOps, Coord, Polygon};
    fn disk(x: f64, y: f64, r: f64) -> Polygon<f64> {
        Polygon::new(
            (0..=256)
                .map(|n| {
                    let angle = n as f64 * std::f64::consts::TAU / 256.;
                    Coord {
                        x: x + r * angle.cos(),
                        y: y + r * angle.sin(),
                    }
                })
                .collect::<Vec<_>>()
                .into(),
            vec![],
        )
    }
    #[test]
    fn indexed_rejection_never_skips_intersections_or_clearance_in_holes_and_islands() {
        let outer = MultiPolygon(vec![disk(0., 0., 5.), disk(12., 1., 2.)]);
        let material = outer.difference(&MultiPolygon(vec![disk(1., 0., 2.)]));
        let indexed = Geometry::new(material);
        for n in 0..100 {
            let other = Geometry::new(MultiPolygon(vec![disk(
                (n * 7919 % 173) as f64 / 10. - 4.,
                (n * 1543 % 101) as f64 / 10. - 5.,
                0.2,
            )]));
            let exact = Euclidean.distance(&indexed.shape, &other.shape);
            assert_eq!(
                indexed.intersects(&other),
                indexed.shape.intersects(&other.shape)
            );
            for gap in [0., 0.01, 0.2, 1.] {
                if exact <= gap {
                    let actual = indexed
                        .distance_within(&other, gap)
                        .expect("Nearby pair must survive pruning");
                    assert!(
                        (actual - exact).abs() < 1e-12,
                        "actual={actual}, exact={exact}"
                    );
                    assert!((other.distance_within(&indexed, gap).unwrap() - exact).abs() < 1e-12);
                } else {
                    assert!(indexed.distance_within(&other, gap).is_none());
                }
            }
        }
        let in_hole = Geometry::new(MultiPolygon(vec![disk(1., 0., 0.2)]));
        assert!(indexed.distance_within(&in_hole, 0.1).is_none());
    }
}
