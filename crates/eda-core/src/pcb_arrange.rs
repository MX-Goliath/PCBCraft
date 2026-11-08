//! Atomic component arrangement in document coordinates.
use crate::pcb::*;
use eda_geom::Point;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    Left,
    Right,
    Top,
    Bottom,
    CentreX,
    CentreY,
    DistributeX,
    DistributeY,
    ExpandX,
    ContractX,
    ExpandY,
    ContractY,
    Grid,
}

pub fn components(
    board: &Board,
    selection: &[Uuid],
    alignment: Align,
    grid: i64,
) -> Result<Board, String> {
    let ids: Vec<_> = board
        .components
        .iter()
        .filter(|c| selection.contains(&c.id))
        .map(|c| c.id)
        .collect();
    objects(board, &ids, alignment, grid)
}

pub fn objects(
    board: &Board,
    selection: &[Uuid],
    alignment: Align,
    grid: i64,
) -> Result<Board, String> {
    objects_impl(board, selection, alignment, grid, None)
}

pub fn objects_at_reference(
    board: &Board,
    selection: &[Uuid],
    alignment: Align,
    grid: i64,
    reference: Uuid,
) -> Result<Board, String> {
    objects_impl(board, selection, alignment, grid, Some(reference))
}

/// Resolve a hit primitive back to a selected component/pour, without changing
/// selection filters or accidentally accepting an unselected object.
pub fn reference_object(board: &Board, selection: &[Uuid], hit: Uuid) -> Option<Uuid> {
    if selection.contains(&hit) {
        return Some(hit);
    }
    let item = board.item(hit)?;
    board
        .components
        .iter()
        .find(|c| Some(c.index) == item.component && selection.contains(&c.id))
        .map(|c| c.id)
        .or_else(|| {
            board
                .polygon_pours
                .iter()
                .find(|p| Some(p.index) == item.polygon && selection.contains(&p.id))
                .map(|p| p.id)
        })
}

/// Unlike ordinary Align To Grid, this command excludes components whose pad
/// centers are off the current grid. A locked component remains stationary.
pub fn all_component_origins_to_grid(board: &Board, grid: i64) -> Result<Board, String> {
    if grid <= 0 {
        return Err("Snap grid must be positive".into());
    }
    let off_grid: std::collections::BTreeSet<_> = board
        .items
        .iter()
        .filter(|i| {
            matches!(i.shape, Shape::Pad { .. })
                && (i.anchor().x.rem_euclid(grid) != 0 || i.anchor().y.rem_euclid(grid) != 0)
        })
        .filter_map(|i| i.component)
        .collect();
    let ids: Vec<_> = board
        .components
        .iter()
        .filter(|c| !off_grid.contains(&c.index))
        .map(|c| c.id)
        .collect();
    if ids.is_empty() {
        return Ok(board.clone());
    }
    components(board, &ids, Align::Grid, grid)
}

fn objects_impl(
    board: &Board,
    selection: &[Uuid],
    alignment: Align,
    grid: i64,
    reference: Option<Uuid>,
) -> Result<Board, String> {
    struct Object {
        id: Uuid,
        index: usize,
        position: Point,
        locked: bool,
    }
    let owners: std::collections::BTreeSet<_> = board
        .components
        .iter()
        .filter(|c| selection.contains(&c.id))
        .map(|c| c.index)
        .collect();
    let pours: std::collections::BTreeSet<_> = board
        .polygon_pours
        .iter()
        .filter(|p| selection.contains(&p.id))
        .map(|p| p.index)
        .collect();
    let mut entries = vec![];
    for (index, id) in selection
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .enumerate()
    {
        if let Some(c) = board.components.iter().find(|c| c.id == id) {
            entries.push((
                Object {
                    id,
                    index,
                    position: c.position,
                    locked: c.locked,
                },
                board.selected_bounds(&[id]).unwrap(),
            ));
        } else if let Some(i) = board.item(id) {
            if i.component.is_some_and(|n| owners.contains(&n))
                || i.polygon.is_some_and(|n| pours.contains(&n))
            {
                continue;
            }
            entries.push((
                Object {
                    id,
                    index,
                    position: i.anchor(),
                    locked: i.locked
                        || i.component.is_some_and(|n| {
                            board.components.iter().any(|c| c.index == n && c.locked)
                        }),
                },
                i.bounds(),
            ));
        } else if let Some(p) = board.polygon_pours.iter().find(|p| p.id == id) {
            let bounds = Bounds::points(&p.points());
            entries.push((
                Object {
                    id,
                    index,
                    position: bounds.centre(),
                    locked: p.locked,
                },
                bounds,
            ));
        }
    }
    if entries.is_empty() {
        return Err("Select PCB objects first".into());
    }
    let spacing = matches!(
        alignment,
        Align::ExpandX | Align::ContractX | Align::ExpandY | Align::ContractY
    );
    if (alignment == Align::Grid || spacing) && grid <= 0 {
        return Err("Snap grid must be positive".into());
    }
    let mut total = Bounds::empty();
    for (_, b) in &entries {
        total.union(*b);
    }
    let vertical = matches!(
        alignment,
        Align::DistributeY | Align::ExpandY | Align::ContractY
    );
    entries.sort_by_key(|(c, b)| {
        (
            if spacing {
                if vertical { c.position.y } else { c.position.x }
            } else if vertical {
                b.min.y
            } else {
                b.min.x
            },
            c.index,
        )
    });
    let reference_centre = if matches!(alignment, Align::CentreX | Align::CentreY) {
        let reference = reference
            .and_then(|id| reference_object(board, selection, id))
            .ok_or("Choose a selected reference object for center alignment")?;
        Some(
            entries
                .iter()
                .find(|(c, _)| c.id == reference)
                .ok_or("Reference object is not part of the arrangement")?
                .1
                .centre(),
        )
    } else {
        None
    };
    let mut next = board.clone();
    let occupied: i64 = entries
        .iter()
        .map(|(_, b)| {
            if vertical {
                b.max.y - b.min.y
            } else {
                b.max.x - b.min.x
            }
        })
        .sum();
    let span = if vertical {
        total.max.y - total.min.y
    } else {
        total.max.x - total.min.x
    };
    let gap = if entries.len() > 1 {
        (span - occupied) as f64 / (entries.len() - 1) as f64
    } else {
        0.
    };
    let mut cursor = if vertical {
        total.min.y as f64
    } else {
        total.min.x as f64
    };
    for (rank, (c, b)) in entries.iter().enumerate() {
        let mut delta = Point::default();
        match alignment {
            Align::Left => delta.x = total.min.x - b.min.x,
            Align::Right => delta.x = total.max.x - b.max.x,
            Align::Top => delta.y = total.max.y - b.max.y,
            Align::Bottom => delta.y = total.min.y - b.min.y,
            Align::CentreX => delta.x = reference_centre.unwrap().x - b.centre().x,
            Align::CentreY => delta.y = reference_centre.unwrap().y - b.centre().y,
            Align::DistributeX if entries.len() > 2 => delta.x = cursor.round() as i64 - b.min.x,
            Align::DistributeY if entries.len() > 2 => delta.y = cursor.round() as i64 - b.min.y,
            Align::Grid => {
                delta.x = snap_coord(c.position.x, grid) - c.position.x;
                delta.y = snap_coord(c.position.y, grid) - c.position.y;
            }
            Align::ExpandX | Align::ContractX | Align::ExpandY | Align::ContractY => {
                let amount = grid
                    .checked_mul(rank as i64)
                    .ok_or("Spacing exceeds document limits")?;
                let amount = if matches!(alignment, Align::ContractX | Align::ContractY) {
                    -amount
                } else {
                    amount
                };
                if vertical {
                    delta.y = amount;
                } else {
                    delta.x = amount;
                }
            }
            _ => {}
        }
        cursor += if vertical {
            (b.max.y - b.min.y) as f64 + gap
        } else {
            (b.max.x - b.min.x) as f64 + gap
        };
        if !c.locked && delta != Point::default() {
            next.transform_unvalidated(&[c.id], c.position, delta, 0.)?;
        }
    }
    next.validate()?;
    Ok(next)
}

/// Four real stroked tracks, inserted together as one history operation.
pub fn rectangle(layer: u16, a: Point, b: Point, width: i64) -> Vec<Item> {
    let p = [a, Point { x: b.x, y: a.y }, b, Point { x: a.x, y: b.y }];
    (0..4)
        .map(|n| {
            Item::new(
                layer,
                None,
                Shape::Track {
                    start: p[n],
                    end: p[(n + 1) % 4],
                    width,
                },
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Board {
        let mut b = Board::default();
        for (n, x) in [1, 5, 14].into_iter().enumerate() {
            let id = Uuid::new_v4();
            b.components.push(Component {
                id,
                index: n as u32,
                designator: format!("U{n}"),
                comment: String::new(),
                footprint: String::new(),
                footprint_parameters: None,
                parameters: None,
                position: Point::mm(x, 0),
                rotation: 0.,
                layer: TOP,
                locked: false,
                properties: Fields::new(),
            });
            let mut item = Item::new(
                TOP,
                None,
                Shape::Track {
                    start: Point::mm(x, 0),
                    end: Point::mm(x + 1, 0),
                    width: 200_000,
                },
            );
            item.component = Some(n as u32);
            item.locked = true;
            b.items.push(item);
        }
        b
    }
    #[test]
    fn arrangement_moves_owned_locked_primitives_but_keeps_locked_components() {
        let b = fixture();
        let ids: Vec<_> = b.components.iter().map(|c| c.id).collect();
        let next = components(&b, &ids, Align::Left, 1_000_000).unwrap();
        assert!(next.components.iter().all(|c| c.position.x == 1_000_000));
        assert!(next.items.iter().all(|i| i.anchor().x == 1_000_000));
        let mut locked = b.clone();
        locked.components[2].locked = true;
        let arranged = components(&locked, &ids, Align::Left, 1_000_000).unwrap();
        assert_eq!(arranged.components[2], locked.components[2]);
        assert_eq!(arranged.items[2], locked.items[2]);
        assert_eq!(arranged.components[1].position, Point::mm(1, 0));
        assert_eq!(locked.components[0].position, b.components[0].position);
    }
    #[test]
    fn centers_require_a_selected_reference_and_spacing_uses_one_grid_per_interval() {
        let b = fixture();
        let ids: Vec<_> = b.components.iter().map(|c| c.id).collect();
        assert!(objects(&b, &ids, Align::CentreX, 1_000_000).is_err());
        let centered = objects_at_reference(&b, &ids, Align::CentreX, 1_000_000, ids[1]).unwrap();
        assert!(
            centered
                .components
                .iter()
                .all(|c| c.position.x == 5_000_000)
        );
        assert!(objects_at_reference(&b, &ids, Align::CentreX, 1_000_000, Uuid::new_v4()).is_err());
        let wider = objects(&b, &ids, Align::ExpandX, 1_000_000).unwrap();
        assert_eq!(
            wider
                .components
                .iter()
                .map(|c| c.position.x)
                .collect::<Vec<_>>(),
            vec![1_000_000, 6_000_000, 16_000_000]
        );
        let narrower = objects(&wider, &ids, Align::ContractX, 1_000_000).unwrap();
        assert_eq!(narrower, b);
        assert!(objects(&b, &ids, Align::ExpandX, 100_000_000_000).is_err());
        assert_eq!(b.components[1].position, Point::mm(5, 0));
    }
    #[test]
    fn all_origins_to_grid_skips_off_grid_pads_and_does_not_move_locked_roots() {
        let mut b = fixture();
        for c in &mut b.components {
            c.position.x += 250_000;
        }
        for (n, x) in [2, 5, 14].into_iter().enumerate() {
            let mut pad = Item::new(
                TOP,
                None,
                Shape::Pad {
                    position: Point::mm(x, 1),
                    size: Point::mm(1, 1),
                    rotation: 0.,
                    shape: PadShape::Round,
                    hole: 0,
                    plated: true,
                    name: String::new(),
                    bottom_size: None,
                    middle_size: None,
                },
            );
            pad.component = Some(n as u32);
            if n == 1
                && let Shape::Pad { position, .. } = &mut pad.shape
            {
                position.x += 100_000;
            }
            b.items.push(pad);
        }
        b.components[2].locked = true;
        let next = all_component_origins_to_grid(&b, 1_000_000).unwrap();
        assert_eq!(next.components[0].position.x, 1_000_000);
        assert_eq!(next.components[1], b.components[1]);
        assert_eq!(next.components[2], b.components[2]);
        assert!(all_component_origins_to_grid(&b, 0).is_err());
    }
}
