//! Pure PCB editing operations. Each function returns a validated copy or leaves
//! its input unchanged. The application commits that copy through History.
use crate::{
    pcb::*,
    pcb_polygon::{Polygon, validate_ring},
};
use eda_geom::Point;
use uuid::Uuid;

pub fn replace_polygon(board: &Board, polygon: Polygon) -> Result<Board, String> {
    polygon.validate()?;
    let mut next = board.clone();
    if let Some(old) = next.polygon_pours.iter_mut().find(|p| p.id == polygon.id) {
        if old.locked && old != &polygon {
            let mut unlocked = old.clone();
            unlocked.locked = false;
            if polygon != unlocked {
                return Err("Unlock the polygon before editing it".into());
            }
        }
        if old.index != polygon.index {
            return Err("Polygon index cannot be changed".into());
        }
        *old = polygon;
    } else {
        next.polygon_pours.push(polygon);
    }
    next.pours_stale = true;
    next.validate()?;
    Ok(next)
}
pub fn edit_component(board: &Board, edited: Component) -> Result<Board, String> {
    let old = board
        .components
        .iter()
        .find(|c| c.id == edited.id)
        .ok_or("Component no longer exists")?;
    if old.index != edited.index {
        return Err("Component index cannot be changed".into());
    }
    if old.locked {
        let mut unlocked = old.clone();
        unlocked.locked = false;
        if edited != unlocked && &edited != old {
            return Err("Unlock the component before editing it".into());
        }
    }
    let mut next = board.clone();
    let changed_pose = edited.position != old.position || edited.rotation != old.rotation;
    if changed_pose {
        next = next.edited(Edit::Transform {
            selection: vec![old.id],
            pivot: old.position,
            translation: Point {
                x: edited.position.x - old.position.x,
                y: edited.position.y - old.position.y,
            },
            angle: edited.rotation - old.rotation,
        })?;
    }
    if edited.layer != old.layer {
        return Err("Use Flip Component to move to the other board side".into());
    }
    for i in next
        .items
        .iter_mut()
        .filter(|i| i.component == Some(old.index))
    {
        if let Shape::Text { text, .. } = &mut i.shape {
            if i.properties
                .get("DESIGNATOR")
                .is_some_and(|v| v.eq_ignore_ascii_case("TRUE"))
            {
                *text = edited.designator.clone();
            }
            if i.properties
                .get("COMMENT")
                .is_some_and(|v| v.eq_ignore_ascii_case("TRUE"))
            {
                *text = edited.comment.clone();
            }
        }
    }
    let component = next
        .components
        .iter_mut()
        .find(|c| c.id == edited.id)
        .unwrap();
    *component = edited;
    next.pours_stale |= !next.polygon_pours.is_empty();
    next.validate()?;
    Ok(next)
}
fn opposite_layer(board: &Board, id: u16) -> u16 {
    match id {
        TOP_OVERLAY => BOTTOM_OVERLAY,
        BOTTOM_OVERLAY => TOP_OVERLAY,
        35 => 36,
        36 => 35,
        37 => 38,
        38 => 37,
        _ if copper_layer(id) => {
            let mut stack: Vec<_> = board.layers.iter().filter(|l| l.copper).collect();
            stack.sort_by_key(|l| l.order);
            stack
                .iter()
                .position(|l| l.id == id)
                .map_or(id, |n| stack[stack.len() - 1 - n].id)
        }
        _ => id,
    }
}
/// Mirror around the footprint origin on X; transform all owned copper and graphics.
pub fn flip_component(board: &Board, id: Uuid) -> Result<Board, String> {
    let c = board
        .components
        .iter()
        .find(|c| c.id == id)
        .ok_or("Select a component to flip")?;
    if c.locked
        || board
            .items
            .iter()
            .any(|i| i.component == Some(c.index) && i.locked)
    {
        return Err("Selection contains locked objects".into());
    }
    let mirror = |p: Point| Point {
        x: 2 * c.position.x - p.x,
        y: p.y,
    };
    let mut next = board.clone();
    for item in next
        .items
        .iter_mut()
        .filter(|i| i.component == Some(c.index))
    {
        item.layer = opposite_layer(board, item.layer);
        match &mut item.shape {
            Shape::Track { start, end, .. } => {
                *start = mirror(*start);
                *end = mirror(*end);
            }
            Shape::Arc {
                centre,
                start_angle,
                end_angle,
                ..
            } => {
                *centre = mirror(*centre);
                let a = *start_angle;
                *start_angle = 180.0 - *end_angle;
                *end_angle = 180.0 - a;
            }
            Shape::Pad {
                position,
                rotation,
                bottom_size,
                middle_size: _,
                size,
                ..
            } => {
                *position = mirror(*position);
                *rotation = (180.0 - *rotation).rem_euclid(360.0);
                if item.layer == MULTI_LAYER
                    && let Some(bottom) = bottom_size
                {
                    std::mem::swap(size, bottom);
                }
            }
            Shape::Via {
                position,
                from_layer,
                to_layer,
                ..
            } => {
                *position = mirror(*position);
                let a = opposite_layer(board, *from_layer);
                *from_layer = opposite_layer(board, *to_layer);
                *to_layer = a;
            }
            Shape::Text {
                position,
                rotation,
                mirrored,
                ..
            } => {
                *position = mirror(*position);
                *rotation = (-*rotation).rem_euclid(360.0);
                *mirrored = !*mirrored;
            }
            Shape::Fill {
                start,
                end,
                rotation,
            } => {
                let a = mirror(*start);
                let b = mirror(*end);
                *start = Point {
                    x: a.x.min(b.x),
                    y: a.y.min(b.y),
                };
                *end = Point {
                    x: a.x.max(b.x),
                    y: a.y.max(b.y),
                };
                *rotation = (-*rotation).rem_euclid(360.0);
            }
            Shape::Region { contour, holes, .. } => {
                for p in contour.iter_mut() {
                    *p = mirror(*p);
                }
                contour.reverse();
                for h in holes {
                    for p in h.iter_mut() {
                        *p = mirror(*p);
                    }
                    h.reverse();
                }
            }
        }
        if let Some(stack) = &mut item.pad_stack {
            let mode = stack.mode;
            stack.layers = std::mem::take(&mut stack.layers)
                .into_iter()
                .map(|(layer, mut g)| {
                    g.offset.y = -g.offset.y;
                    let key = if mode == StackMode::Simple
                        || mode == StackMode::TopMiddleBottom && layer == 2
                    {
                        layer
                    } else {
                        opposite_layer(board, layer)
                    };
                    (key, g)
                })
                .collect();
            stack.drill_rotation = -stack.drill_rotation;
        }
        if let Some(g) = item.pad_geometry(TOP)
            && item.pad_stack.is_some()
        {
            item.set_pad_geometry(TOP, g);
        }
        if let Some(m) = &mut item.masks {
            m.paste.swap(0, 1);
            m.solder.swap(0, 1);
        }
    }
    let target = next.components.iter_mut().find(|x| x.id == id).unwrap();
    target.layer = opposite_layer(board, c.layer);
    target.rotation = (180.0 - c.rotation).rem_euclid(360.0);
    next.pours_stale |= !next.polygon_pours.is_empty();
    next.validate()?;
    Ok(next)
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Handle {
    TrackStart(Uuid),
    TrackEnd(Uuid),
    RegionVertex(Uuid, usize),
    PolygonVertex(Uuid, usize),
}
pub fn handles(board: &Board, selected: &[Uuid]) -> Vec<(Handle, Point)> {
    let mut out = Vec::new();
    for id in selected {
        if let Some(item) = board.item(*id) {
            match &item.shape {
                Shape::Track { start, end, .. } => {
                    out.push((Handle::TrackStart(*id), *start));
                    out.push((Handle::TrackEnd(*id), *end));
                }
                Shape::Region { contour, .. } if item.polygon.is_none() => out.extend(
                    contour
                        .iter()
                        .copied()
                        .enumerate()
                        .map(|(n, p)| (Handle::RegionVertex(*id, n), p)),
                ),
                _ => {}
            }
        }
        if let Some(poly) = board.polygon_pours.iter().find(|p| p.id == *id) {
            out.extend(
                poly.contour
                    .iter()
                    .enumerate()
                    .map(|(n, v)| (Handle::PolygonVertex(*id, n), v.position)),
            );
        }
    }
    out
}
pub fn move_handle(
    board: &Board,
    handle: Handle,
    at: Point,
    connected: bool,
) -> Result<Board, String> {
    let mut next = board.clone();
    match handle {
        Handle::PolygonVertex(id, n) => {
            let p = next
                .polygon_pours
                .iter_mut()
                .find(|p| p.id == id)
                .ok_or("Polygon not found")?;
            if p.locked {
                return Err("Polygon is locked".into());
            }
            let v = p.contour.get_mut(n).ok_or("Polygon vertex not found")?;
            v.position = at;
            v.arc = None;
            if n > 0 {
                p.contour[n - 1].arc = None;
            } else if let Some(v) = p.contour.last_mut() {
                v.arc = None;
            }
            p.validate()?;
        }
        Handle::RegionVertex(id, n) => {
            let mut i = board.item(id).ok_or("Region not found")?.clone();
            if let Shape::Region { contour, .. } = &mut i.shape {
                *contour.get_mut(n).ok_or("Vertex not found")? = at;
                validate_ring(contour)?;
            }
            next = board.edited(Edit::Replace(i))?;
        }
        Handle::TrackStart(id) | Handle::TrackEnd(id) => {
            let item = board.item(id).ok_or("Track not found")?;
            let Shape::Track { start, end, .. } = item.shape else {
                return Err("Not a track".into());
            };
            let old = if matches!(handle, Handle::TrackStart(_)) {
                start
            } else {
                end
            };
            let mut changed = Vec::new();
            for candidate in &board.items {
                if candidate.id != id
                    && !(connected
                        && candidate.net == item.net
                        && candidate.layer == item.layer
                        && candidate.polygon.is_none())
                {
                    continue;
                }
                let mut i = candidate.clone();
                if let Shape::Track { start, end, .. } = &mut i.shape {
                    let mut touched = false;
                    if *start == old {
                        *start = at;
                        touched = true;
                    }
                    if *end == old {
                        *end = at;
                        touched = true;
                    }
                    if touched {
                        changed.push(i);
                    }
                }
            }
            for item in changed {
                next = next.edited(Edit::Replace(item))?;
            }
        }
    }
    next.pours_stale |= !next.polygon_pours.is_empty();
    next.validate()?;
    Ok(next)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shared_track_endpoint_moves_atomically() {
        let mut b = Board::default();
        let a = Item::new(
            TOP,
            None,
            Shape::Track {
                start: Point::mm(0, 0),
                end: Point::mm(5, 0),
                width: 250_000,
            },
        );
        let id = a.id;
        b.items.push(a);
        b.items.push(Item::new(
            TOP,
            None,
            Shape::Track {
                start: Point::mm(5, 0),
                end: Point::mm(10, 0),
                width: 250_000,
            },
        ));
        let n = move_handle(&b, Handle::TrackEnd(id), Point::mm(5, 2), true).unwrap();
        assert_eq!(n.items[1].anchor(), Point::mm(5, 2));
        assert_eq!(b.items[1].anchor(), Point::mm(5, 0));
    }
    #[test]
    fn flip_component_twice_restores_geometry() {
        let mut b = Board::default();
        let c = Component {
            id: Uuid::new_v4(),
            index: 0,
            designator: "R1".into(),
            comment: "1k".into(),
            footprint: "0805".into(),
            position: Point::mm(5, 5),
            rotation: 0.0,
            layer: TOP,
            locked: false,
            properties: Fields::new(),
        };
        let id = c.id;
        b.components.push(c);
        let mut i = Item::new(
            TOP,
            None,
            Shape::Track {
                start: Point::mm(4, 5),
                end: Point::mm(6, 5),
                width: 250_000,
            },
        );
        i.component = Some(0);
        b.items.push(i);
        let twice = flip_component(&flip_component(&b, id).unwrap(), id).unwrap();
        assert_eq!(b, twice);
    }
}

/// Move a selected group and stretch attached track endpoints, without shove or
/// obstacle checking. Entire operation is atomic, including locked neighbours.
pub fn drag_connected(
    board: &Board,
    selection: &[Uuid],
    pivot: Point,
    translation: Point,
    angle: f64,
) -> Result<Board, String> {
    let ids = board.resolve_selection(selection);
    let mut next = board.edited(Edit::Transform {
        selection: selection.to_vec(),
        pivot,
        translation,
        angle,
    })?;
    for candidate in &board.items {
        if ids.contains(&candidate.id)
            || candidate.component.is_some()
            || candidate.polygon.is_some()
        {
            continue;
        }
        let Shape::Track { start, end, width } = candidate.shape else {
            continue;
        };
        let attached = |point: Point| {
            board
                .items
                .iter()
                .filter(|i| {
                    ids.contains(&i.id)
                        && i.net == candidate.net
                        && crate::pcb_polygon::on_layer(board, i, candidate.layer)
                })
                .any(|i| match i.shape {
                    Shape::Pad { .. } | Shape::Via { .. } => hit_distance(i, point) <= 2.0,
                    Shape::Track { start, end, .. } => {
                        distance_sq(point, start) <= 4.0 || distance_sq(point, end) <= 4.0
                    }
                    _ => false,
                })
        };
        let a = attached(start);
        let b = attached(end);
        if a || b {
            let mut updated = candidate.clone();
            updated.shape = Shape::Track {
                start: if a {
                    transform_point(start, pivot, translation, angle)?
                } else {
                    start
                },
                end: if b {
                    transform_point(end, pivot, translation, angle)?
                } else {
                    end
                },
                width,
            };
            next = next.edited(Edit::Replace(updated))?;
        }
    }
    Ok(next)
}

#[cfg(test)]
mod connected_drag_tests {
    use super::*;
    #[test]
    fn neighbour_endpoint_stretches_and_locked_neighbour_aborts() {
        let mut b = Board::default();
        let pad = Item::new(
            TOP,
            None,
            Shape::Pad {
                position: Point::mm(1, 1),
                size: Point::mm(1, 1),
                rotation: 0.0,
                shape: PadShape::Round,
                hole: 0,
                plated: true,
                name: "1".into(),
                bottom_size: None,
                middle_size: None,
            },
        );
        let id = pad.id;
        b.items.push(pad);
        b.items.push(Item::new(
            TOP,
            None,
            Shape::Track {
                start: Point::mm(1, 1),
                end: Point::mm(5, 1),
                width: 250_000,
            },
        ));
        let next = drag_connected(&b, &[id], Point::mm(1, 1), Point::mm(2, 0), 0.0).unwrap();
        assert_eq!(next.items[1].anchor(), Point::mm(3, 1));
        b.items[1].locked = true;
        let before = b.clone();
        assert!(drag_connected(&b, &[id], Point::mm(1, 1), Point::mm(2, 0), 0.0).is_err());
        assert_eq!(b, before);
    }
}
