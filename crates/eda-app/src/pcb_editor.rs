//! PCB controller: commands, previews, atomic history, tools and Properties.
use super::*;
use eda_core::pcb::{self, Board, Bounds, ContourVertex, Edit, Item, PadShape, Shape};
use eda_core::{
    pcb_polygon::{self, Connection, Polygon},
    pcb_route::{self, Corner, RouteSession},
    pcb_tools::{self, Handle},
};
use eda_geom::Point;
#[path = "pcb_properties_v2.rs"]
mod properties_v2;
use eda_render::pcb::Input;
use std::collections::{BTreeMap, BTreeSet};
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Tool {
    #[default]
    Select,
    Route,
    Pad,
    Via,
    Text,
    Fill,
    Region,
    Polygon,
    PolygonCutout,
    Arc,
    Outline,
    Move,
    Paste,
    CopyReference,
}
#[derive(Clone, Copy)]
pub(super) enum Action {
    Tool(Tool),
    Delete,
    Copy,
    Cut,
    Paste,
    Rotate,
    RotateBack,
    SelectAll,
    Layers,
    Grid,
    Report,
    Repour,
    Shelve,
    Restore,
    RouteVia,
    RouteLayer(u16),
    Backtrack,
    FinishRoute,
    Flip,
    GroupComponent,
}
#[derive(Clone)]
pub(super) struct Clipboard {
    fragment: Board,
    anchor: Point,
}
#[derive(Clone)]
enum Gesture {
    Move {
        base: Board,
        revision: u64,
        origin: Point,
        selection: Vec<Uuid>,
        connected: bool,
    },
    Box {
        origin: Point,
        add: bool,
    },
    Handle {
        base: Board,
        revision: u64,
        handle: Handle,
    },
}
#[derive(Clone)]
struct Draft {
    id: Uuid,
    revision: u64,
    item: Item,
}
#[derive(Clone)]
struct ComponentDraft {
    revision: u64,
    value: pcb::Component,
}
#[derive(Clone)]
struct PolygonDraft {
    revision: u64,
    value: Polygon,
}
pub(super) struct Editor {
    pub tool: Tool,
    start: Option<Point>,
    points: Vec<Point>,
    pub width: i64,
    pub diameter: i64,
    pub drill: i64,
    pub pad_size: Point,
    pub pad_shape: PadShape,
    pub pad_name: String,
    pub pad_hole: i64,
    pub pad_plated: bool,
    pub via_from: u16,
    pub via_to: u16,
    pub text: String,
    pub text_height: i64,
    pub net: Option<u32>,
    pub rotation: f64,
    corner_mode: u8,
    diagonal_first: bool,
    paused: bool,
    gesture: Option<Gesture>,
    tool_base: Option<(Board, u64)>,
    draft: Option<Draft>,
    sequence: Option<(Key, f64)>,
    cut_after_copy: bool,
    last_hits: Vec<Uuid>,
    overlap: usize,
    route_session: Option<RouteSession>,
    last_point: Option<Point>,
    component_draft: Option<ComponentDraft>,
    polygon_draft: Option<PolygonDraft>,
    properties_panel: properties_v2::Panel,
    last_error: Option<String>,
}
impl Default for Editor {
    fn default() -> Self {
        Self {
            tool: Tool::Select,
            start: None,
            points: vec![],
            width: 250_000,
            diameter: 600_000,
            drill: 300_000,
            pad_size: Point {
                x: 1_500_000,
                y: 1_000_000,
            },
            pad_shape: PadShape::Round,
            pad_name: "1".into(),
            pad_hole: 0,
            pad_plated: true,
            via_from: pcb::TOP,
            via_to: pcb::BOTTOM,
            text: "Text".into(),
            text_height: 1_000_000,
            net: None,
            rotation: 0.0,
            corner_mode: 0,
            diagonal_first: true,
            paused: false,
            gesture: None,
            tool_base: None,
            draft: None,
            sequence: None,
            cut_after_copy: false,
            last_hits: vec![],
            overlap: 0,
            route_session: None,
            last_point: None,
            component_draft: None,
            polygon_draft: None,
            properties_panel: Default::default(),
            last_error: None,
        }
    }
}
impl Editor {
    pub(super) fn cancel(&mut self, view: &mut eda_render::View) {
        self.route_session = None;
        self.last_error = None;
        self.tool = Tool::Select;
        self.start = None;
        self.points.clear();
        self.gesture = None;
        self.tool_base = None;
        self.paused = false;
        self.sequence = None;
        view.pcb.clear_gesture();
    }
    fn set_tool(&mut self, tool: Tool, view: &mut eda_render::View) {
        self.cancel(view);
        self.tool = tool;
        view.pcb.tool_active = tool != Tool::Select;
    }
    pub(super) fn properties_dirty(&self, board: &Board, revision: u64) -> bool {
        self.draft.as_ref().is_some_and(|d| {
            d.revision == revision && board.item(d.id).is_some_and(|old| old != &d.item)
        }) || self.component_draft.as_ref().is_some_and(|d| {
            d.revision == revision
                && board
                    .components
                    .iter()
                    .any(|old| old.id == d.value.id && old != &d.value)
        }) || self.polygon_draft.as_ref().is_some_and(|d| {
            d.revision == revision
                && board
                    .polygon_pours
                    .iter()
                    .any(|old| old.id == d.value.id && old != &d.value)
        })
    }
    pub(super) fn has_pending(&self) -> bool {
        self.route_session.is_some()
            || self.gesture.is_some()
            || self.tool_base.is_some()
            || self.start.is_some()
            || !self.points.is_empty()
    }
}
fn commit(doc: &mut OpenDocument, next: Board) -> Result<bool, String> {
    let next = if next.pours_stale {
        pcb_polygon::repour(&next)?
    } else {
        next
    };
    let mut model = doc.model.clone();
    model.pcb = Some(next);
    doc.history.commit(&mut doc.model, model)
}
fn picked(board: &Board, view: &eda_render::pcb::BoardView, id: Uuid) -> Uuid {
    if let Some(poly) = board
        .item(id)
        .and_then(|i| i.polygon)
        .and_then(|n| board.polygon_pours.iter().find(|p| p.index == n))
    {
        return poly.id;
    }
    if view.component_selection
        && let Some(component) = board
            .item(id)
            .and_then(|i| i.component)
            .and_then(|n| board.components.iter().find(|c| c.index == n))
    {
        return component.id;
    }
    id
}
fn clipboard(board: &Board, selection: &[Uuid], anchor: Point) -> Clipboard {
    let ids = board.resolve_selection(selection);
    let mut fragment = Board {
        components: board
            .components
            .iter()
            .filter(|c| selection.contains(&c.id))
            .cloned()
            .collect(),
        ..Board::default()
    };
    let owners: BTreeSet<_> = fragment.components.iter().map(|c| c.index).collect();
    fragment.polygon_pours = board
        .polygon_pours
        .iter()
        .filter(|p| selection.contains(&p.id))
        .cloned()
        .collect();
    let pours: BTreeSet<_> = fragment.polygon_pours.iter().map(|p| p.index).collect();
    fragment.items = board
        .items
        .iter()
        .filter(|i| ids.contains(&i.id) && i.polygon.is_none_or(|n| !pours.contains(&n)))
        .cloned()
        .map(|mut i| {
            if i.component.is_some_and(|c| !owners.contains(&c)) {
                i.component = None;
            }
            i.polygon = None;
            i.source = None;
            i.locked = false;
            i
        })
        .collect();
    for c in &mut fragment.components {
        c.locked = false;
    }
    for p in &mut fragment.polygon_pours {
        p.locked = false;
    }
    let nets: BTreeSet<_> = fragment
        .items
        .iter()
        .filter_map(|i| i.net)
        .chain(fragment.polygon_pours.iter().filter_map(|p| p.net))
        .collect();
    fragment.nets = board
        .nets
        .iter()
        .filter(|n| nets.contains(&n.index))
        .cloned()
        .collect();
    Clipboard { fragment, anchor }
}
fn pasted(board: &Board, clip: &Clipboard, at: Point) -> Result<(Board, Vec<Uuid>), String> {
    let mut next = board.clone();
    let mut net_map = BTreeMap::new();
    let mut component_map = BTreeMap::new();
    let mut selection = vec![];
    let mut next_net = next
        .nets
        .iter()
        .map(|n| n.index)
        .max()
        .map_or(Some(0), |n| n.checked_add(1))
        .ok_or("Net index overflow")?;
    for n in &clip.fragment.nets {
        let idx = if let Some(old) = next.nets.iter().find(|p| p.name == n.name) {
            old.index
        } else {
            let mut n = n.clone();
            let id = next_net;
            next_net = next_net.checked_add(1).ok_or("Net index overflow")?;
            n.index = id;
            next.nets.push(n);
            id
        };
        net_map.insert(n.index, idx);
    }
    let mut next_component = next
        .components
        .iter()
        .map(|c| c.index)
        .max()
        .map_or(Some(0), |n| n.checked_add(1))
        .ok_or("Component index overflow")?;
    for c in &clip.fragment.components {
        let mut new = c.clone();
        new.id = Uuid::new_v4();
        new.index = next_component;
        next_component = next_component
            .checked_add(1)
            .ok_or("Component index overflow")?;
        new.locked = false;
        component_map.insert(c.index, new.index);
        selection.push(new.id);
        let original = new.designator.clone();
        if next
            .components
            .iter()
            .any(|c| c.designator == new.designator)
        {
            let prefix = original.trim_end_matches(|c: char| c.is_ascii_digit());
            let prefix = if prefix.is_empty() { "U" } else { prefix };
            for n in 1u32..=1_000_000 {
                let candidate = format!("{prefix}{n}");
                if !next.components.iter().any(|c| c.designator == candidate) {
                    new.designator = candidate;
                    break;
                }
            }
        }
        next.components.push(new);
    }
    for i in &clip.fragment.items {
        let mut new = i.clone();
        new.id = Uuid::new_v4();
        new.net = new.net.and_then(|n| net_map.get(&n).copied());
        new.component = new.component.and_then(|c| component_map.get(&c).copied());
        new.locked = false;
        new.source = None;
        if new
            .properties
            .get("DESIGNATOR")
            .is_some_and(|v| v.eq_ignore_ascii_case("TRUE"))
            && let Some(c) = new
                .component
                .and_then(|id| next.components.iter().find(|c| c.index == id))
            && let Shape::Text { text, .. } = &mut new.shape
        {
            *text = c.designator.clone();
        }
        if new.component.is_none() {
            selection.push(new.id);
        }
        next.items.push(new);
    }
    let mut poly_index = next
        .polygon_pours
        .iter()
        .map(|p| p.index)
        .max()
        .map_or(Some(0), |n| n.checked_add(1))
        .ok_or("Polygon index overflow")?;
    for p in &clip.fragment.polygon_pours {
        let mut p = p.clone();
        p.id = Uuid::new_v4();
        p.index = poly_index;
        poly_index = poly_index.checked_add(1).ok_or("Polygon index overflow")?;
        p.net = p.net.and_then(|n| net_map.get(&n).copied());
        p.locked = false;
        selection.push(p.id);
        next.polygon_pours.push(p);
    }
    let delta = Point {
        x: at.x - clip.anchor.x,
        y: at.y - clip.anchor.y,
    };
    next = next.edited(Edit::Transform {
        selection: selection.clone(),
        pivot: clip.anchor,
        translation: delta,
        angle: 0.0,
    })?;
    Ok((next, selection))
}
fn box_selection(
    board: &Board,
    view: &eda_render::pcb::BoardView,
    a: Point,
    z: Point,
) -> Vec<Uuid> {
    let box_ = Bounds::points(&[a, z]);
    let crossing = z.x < a.x;
    let test = |b: Bounds| {
        if crossing {
            box_.intersects(b)
        } else {
            box_.contains(b.min) && box_.contains(b.max)
        }
    };
    let mut ids = BTreeSet::new();
    let mut components: BTreeMap<u32, Bounds> = BTreeMap::new();
    for i in &board.items {
        if !view.selectable(i) {
            continue;
        }
        if view.component_selection
            && let Some(c) = i.component
        {
            components
                .entry(c)
                .or_insert_with(Bounds::empty)
                .union(i.bounds());
            continue;
        }
        if test(i.bounds()) {
            ids.insert(picked(board, view, i.id));
        }
    }
    for (index, bounds) in components {
        if test(bounds)
            && let Some(c) = board.components.iter().find(|c| c.index == index)
            && (view.select_locked || !c.locked)
        {
            ids.insert(c.id);
        }
    }
    for p in &board.polygon_pours {
        if view.filters[7]
            && view.visible.contains(&p.layer)
            && (view.select_locked || !p.locked)
            && test(Bounds::points(&p.points()))
        {
            ids.insert(p.id);
        }
    }
    ids.into_iter().collect()
}
fn finish_route(doc: &mut OpenDocument, ed: &mut Editor) -> Result<(), String> {
    if let Some(route) = &ed.route_session {
        if route.has_segments() {
            commit(doc, route.finish(doc.history.revision)?)?;
        }
        ed.route_session = None;
        ed.start = None;
        doc.view.pcb.preview = None;
    }
    Ok(())
}
pub(super) fn prepare_save(doc: &mut OpenDocument) -> Result<(), String> {
    let mut ed = std::mem::take(&mut doc.pcb_editor);
    let result = (|| -> Result<(), String> {
        properties_v2::flush_pending(doc, &mut ed, true)?;
        finish_route(doc, &mut ed)?;
        if ed.has_pending() {
            return Err("Finish or cancel the current PCB gesture before saving".into());
        }
        Ok(())
    })();
    doc.pcb_editor = ed;
    result
}
fn route_layer(doc: &mut OpenDocument, ed: &mut Editor, layer: u16) -> Result<(), String> {
    if let Some(route) = &mut ed.route_session {
        if route.revision != doc.history.revision {
            return Err("Document changed during routing".into());
        }
        if route.layer != layer {
            route.via(
                ed.last_point.unwrap_or(route.anchor),
                layer,
                ed.diameter,
                ed.drill,
            )?;
            doc.view.pcb.preview = Some(route.draft.clone());
        }
    }
    doc.view.pcb.active_layer = layer;
    Ok(())
}
fn placement(ed: &Editor, point: Point, layer: u16) -> Option<Item> {
    let shape = match ed.tool {
        Tool::Pad => Shape::Pad {
            position: point,
            size: ed.pad_size,
            rotation: ed.rotation,
            shape: ed.pad_shape,
            hole: ed.pad_hole,
            plated: ed.pad_plated,
            name: ed.pad_name.clone(),
            bottom_size: None,
            middle_size: None,
        },
        Tool::Via => Shape::Via {
            position: point,
            diameter: ed.diameter,
            hole: ed.drill,
            from_layer: ed.via_from,
            to_layer: ed.via_to,
        },
        Tool::Text => Shape::Text {
            position: point,
            text: ed.text.clone(),
            height: ed.text_height,
            stroke: 150_000,
            rotation: ed.rotation,
            mirrored: false,
            font: String::new(),
        },
        Tool::Fill => Shape::Fill {
            start: ed.start?,
            end: point,
            rotation: ed.rotation,
        },
        Tool::Arc => {
            if ed.points.len() < 2 {
                return None;
            }
            let c = ed.points[0];
            let a = ed.points[1];
            let radius = pcb::distance_sq(c, a).sqrt().round() as i64;
            if radius < 1 {
                return None;
            }
            Shape::Arc {
                centre: c,
                radius,
                start_angle: ((a.y - c.y) as f64).atan2((a.x - c.x) as f64).to_degrees(),
                end_angle: ((point.y - c.y) as f64)
                    .atan2((point.x - c.x) as f64)
                    .to_degrees(),
                width: ed.width,
            }
        }
        Tool::Region => {
            let mut points = ed.points.clone();
            if points.last() != Some(&point) {
                points.push(point);
            }
            if points.len() < 3 {
                return None;
            }
            Shape::Region {
                contour: points,
                holes: vec![],
                cutout: false,
            }
        }
        _ => return None,
    };
    Some(Item::new(
        if ed.tool == Tool::Via || (ed.tool == Tool::Pad && ed.pad_hole > 0) {
            pcb::MULTI_LAYER
        } else {
            layer
        },
        ed.net,
        shape,
    ))
}
fn finish_polygon(doc: &mut OpenDocument, ed: &mut Editor) -> Result<(), String> {
    pcb_polygon::validate_ring(&ed.points)?;
    let mut next = doc.model.pcb.as_ref().ok_or("Not a PCB document")?.clone();
    let mut selected = None;
    match ed.tool {
        Tool::Outline => next.outline = pcb_polygon::straight_contour(&ed.points),
        Tool::Polygon => {
            let index = next
                .polygon_pours
                .iter()
                .map(|p| p.index)
                .max()
                .map_or(Some(0), |n| n.checked_add(1))
                .ok_or("Polygon index overflow")?;
            let p = Polygon {
                index,
                priority: index,
                name: format!("Polygon {}", index + 1),
                layer: doc.view.pcb.active_layer,
                net: ed.net,
                contour: pcb_polygon::straight_contour(&ed.points),
                ..Default::default()
            };
            selected = Some(p.id);
            next = pcb_tools::replace_polygon(&next, p)?;
        }
        Tool::PolygonCutout => {
            let id = doc
                .view
                .pcb
                .selected
                .first()
                .copied()
                .ok_or("Select the polygon before drawing its cutout")?;
            let mut p = next
                .polygon_pours
                .iter()
                .find(|p| p.id == id)
                .ok_or("Select a polygon definition")?
                .clone();
            p.holes.push(ed.points.clone());
            next = pcb_tools::replace_polygon(&next, p)?;
            selected = Some(id);
        }
        _ => {
            next = next.edited(Edit::Insert(vec![Item::new(
                doc.view.pcb.active_layer,
                ed.net,
                Shape::Region {
                    contour: ed.points.clone(),
                    holes: vec![],
                    cutout: false,
                },
            )]))?
        }
    }
    next.pours_stale |= !next.polygon_pours.is_empty();
    commit(doc, next)?;
    if let Some(id) = selected {
        doc.view.pcb.selected = vec![id];
    }
    ed.points.clear();
    doc.view.pcb.preview = None;
    doc.view.pcb.draft_outline.clear();
    Ok(())
}
fn input(
    doc: &mut OpenDocument,
    ed: &mut Editor,
    clip: &mut Option<Clipboard>,
    i: Input,
    ctx: &egui::Context,
) -> Result<(), String> {
    ed.last_hits = i.hits.clone();
    if let Some(p) = i.point {
        ed.last_point = Some(p);
    }
    let modifiers = ctx.input(|s| s.modifiers);
    if ed.paused {
        if i.clicked {
            ed.paused = false;
        }
        return Ok(());
    }
    if i.secondary_clicked && ed.tool != Tool::Select {
        if ed.route_session.is_some() {
            return finish_route(doc, ed);
        }
        if matches!(
            ed.tool,
            Tool::Region | Tool::Outline | Tool::Polygon | Tool::PolygonCutout
        ) && ed.points.len() >= 3
        {
            return finish_polygon(doc, ed);
        }
        if ed.start.is_some() || !ed.points.is_empty() {
            ed.start = None;
            ed.points.clear();
            ed.tool_base = None;
            doc.view.pcb.preview = None;
        } else {
            ed.cancel(&mut doc.view);
        }
        return Ok(());
    }
    if let Some(gesture) = &ed.gesture {
        if i.drag_stopped && !i.hovered {
            ed.gesture = None;
            doc.view.pcb.preview = None;
            doc.view.pcb.selection_box = None;
            return Ok(());
        }
        if let Some(p) = i.point {
            match gesture {
                Gesture::Move {
                    base,
                    revision,
                    origin,
                    selection,
                    connected,
                } => {
                    if *revision != doc.history.revision {
                        ed.gesture = None;
                        doc.view.pcb.preview = None;
                        return Err("Document changed during drag; gesture cancelled".into());
                    }
                    let translation = Point {
                        x: p.x - origin.x,
                        y: p.y - origin.y,
                    };
                    let next = if *connected {
                        pcb_tools::drag_connected(base, selection, *origin, translation, 0.0)
                    } else {
                        base.edited(Edit::Transform {
                            selection: selection.clone(),
                            pivot: *origin,
                            translation,
                            angle: 0.0,
                        })
                    };
                    match next {
                        Ok(next) => {
                            if i.drag_stopped {
                                ed.gesture = None;
                                doc.view.pcb.preview = None;
                                commit(doc, next)?;
                            } else {
                                doc.view.pcb.preview = Some(next);
                            }
                        }
                        Err(error) => {
                            doc.view.pcb.preview = None;
                            if i.drag_stopped {
                                ed.gesture = None;
                            }
                            return Err(error);
                        }
                    }
                }
                Gesture::Handle {
                    base,
                    revision,
                    handle,
                } => {
                    if *revision != doc.history.revision {
                        return Err("Document changed during handle drag".into());
                    }
                    match pcb_tools::move_handle(base, *handle, p, true) {
                        Ok(next) => {
                            if i.drag_stopped {
                                ed.gesture = None;
                                doc.view.pcb.preview = None;
                                commit(doc, next)?;
                            } else {
                                doc.view.pcb.preview = Some(next);
                            }
                        }
                        Err(error) => {
                            if i.drag_stopped {
                                ed.gesture = None;
                                doc.view.pcb.preview = None;
                                return Err(error);
                            }
                        }
                    }
                }
                Gesture::Box { origin, add } => {
                    let a = *origin;
                    let add = *add;
                    doc.view.pcb.selection_box = Some((a, p));
                    if i.drag_stopped {
                        let board = doc.model.pcb.as_ref().ok_or("Not a PCB document")?;
                        let selection = box_selection(board, &doc.view.pcb, a, p);
                        if !add {
                            doc.view.pcb.selected.clear();
                        }
                        for id in selection {
                            if !doc.view.pcb.selected.contains(&id) {
                                doc.view.pcb.selected.push(id);
                            }
                        }
                        doc.view.pcb.selection_box = None;
                        ed.gesture = None;
                    }
                }
            }
        }
        return Ok(());
    }
    let Some(p) = i.point else {
        return Ok(());
    };
    if ed.tool == Tool::Select {
        let board = doc.model.pcb.as_ref().ok_or("Not a PCB document")?;
        if i.drag_started {
            let origin = i.press_origin.unwrap_or(p);
            let tol = 7e6 / doc.view.camera.pixels_per_mm;
            if let Some((handle, _)) = pcb_tools::handles(board, &doc.view.pcb.selected)
                .into_iter()
                .filter(|(_, at)| pcb::distance_sq(*at, origin) <= tol * tol)
                .min_by(|a, b| {
                    pcb::distance_sq(a.1, origin).total_cmp(&pcb::distance_sq(b.1, origin))
                })
            {
                ed.gesture = Some(Gesture::Handle {
                    base: board.clone(),
                    revision: doc.history.revision,
                    handle,
                });
                return Ok(());
            }
            let hit = board
                .items
                .iter()
                .filter(|v| doc.view.pcb.selectable(v))
                .map(|v| (v, pcb::hit_distance(v, origin)))
                .filter(|(_, d)| *d <= tol)
                .min_by(|a, b| {
                    let rank = |i: &Item| match i.shape {
                        Shape::Pad { .. } | Shape::Via { .. } => 0,
                        Shape::Track { .. } | Shape::Arc { .. } => 1,
                        Shape::Text { .. } => 2,
                        _ => 3,
                    };
                    rank(a.0).cmp(&rank(b.0)).then_with(|| a.1.total_cmp(&b.1))
                })
                .map(|(v, _)| picked(board, &doc.view.pcb, v.id))
                .or_else(|| {
                    board
                        .polygon_pours
                        .iter()
                        .rev()
                        .find(|poly| {
                            doc.view.pcb.visible.contains(&poly.layer)
                                && pcb::point_in_polygon(origin, &poly.points())
                        })
                        .map(|p| p.id)
                });
            if let Some(id) = hit {
                if !doc.view.pcb.selected.contains(&id) {
                    doc.view.pcb.selected = vec![id];
                }
                ed.gesture = Some(Gesture::Move {
                    base: board.clone(),
                    revision: doc.history.revision,
                    origin: board.snap(
                        origin,
                        doc.view.grid_nm,
                        0,
                        doc.view.pcb.active_layer,
                        &doc.view.pcb.visible,
                    ),
                    selection: doc.view.pcb.selected.clone(),
                    connected: modifiers.ctrl || modifiers.command,
                });
            } else {
                ed.gesture = Some(Gesture::Box {
                    origin,
                    add: modifiers.shift,
                });
            }
            return Ok(());
        }
        if i.clicked || i.double_clicked {
            if modifiers.ctrl || modifiers.command {
                doc.view.pcb.highlight_net =
                    i.hit.and_then(|id| board.item(id)).and_then(|v| v.net);
                return Ok(());
            }
            let id = i.hit.map(|id| picked(board, &doc.view.pcb, id));
            if modifiers.shift {
                if let Some(id) = id {
                    if let Some(n) = doc.view.pcb.selected.iter().position(|v| *v == id) {
                        doc.view.pcb.selected.remove(n);
                    } else {
                        doc.view.pcb.selected.push(id);
                    }
                }
            } else {
                doc.view.pcb.selected = id.into_iter().collect();
            }
            ed.draft = None;
        }
        return Ok(());
    }
    let board = doc.model.pcb.as_ref().ok_or("Not a PCB document")?;
    let layer = doc.view.pcb.active_layer;
    match ed.tool {
        Tool::CopyReference => {
            if i.clicked {
                let c = clipboard(board, &doc.view.pcb.selected, p);
                if c.fragment.items.is_empty() && c.fragment.polygon_pours.is_empty() {
                    return Err("No objects selected".into());
                }
                *clip = Some(c);
                if ed.cut_after_copy {
                    let next = board.edited(Edit::Delete(doc.view.pcb.selected.clone()))?;
                    commit(doc, next)?;
                    doc.view.pcb.selected.clear();
                }
                ed.cancel(&mut doc.view);
            }
        }
        Tool::Paste => {
            if let Some(c) = clip {
                let (next, selection) = pasted(board, c, p)?;
                if i.clicked {
                    commit(doc, next)?;
                    doc.view.pcb.selected = selection;
                    doc.view.pcb.preview = None;
                } else {
                    doc.view.pcb.preview = Some(next);
                }
            }
        }
        Tool::Move => {
            if let Some((base, revision)) = &ed.tool_base {
                if *revision != doc.history.revision {
                    return Err(
                        "Document changed during move; cancel and restart the command".into(),
                    );
                }
                let origin = ed.start.unwrap_or(p);
                let next = base.edited(Edit::Transform {
                    selection: doc.view.pcb.selected.clone(),
                    pivot: origin,
                    translation: Point {
                        x: p.x - origin.x,
                        y: p.y - origin.y,
                    },
                    angle: ed.rotation,
                })?;
                if i.clicked {
                    commit(doc, next)?;
                    ed.cancel(&mut doc.view);
                } else {
                    doc.view.pcb.preview = Some(next);
                }
            } else if i.clicked {
                ed.start = Some(p);
                ed.rotation = 0.0;
                ed.tool_base = Some((board.clone(), doc.history.revision));
            }
        }
        Tool::Route => {
            let tol = (7e6 / doc.view.camera.pixels_per_mm).round() as i64;
            if let Some(route) = &mut ed.route_session {
                if route.revision != doc.history.revision {
                    return Err("Document changed during routing; cancel and restart".into());
                }
                route.width = ed.width;
                route.corner = match ed.corner_mode {
                    0 => Corner::FortyFive,
                    1 => Corner::Orthogonal,
                    _ => Corner::AnyAngle,
                };
                route.diagonal_first = ed.diagonal_first;
                let hit = pcb_route::target(board, i.raw_point.unwrap_or(p), route.layer, tol);
                let end = hit.map_or(p, |h| h.point);
                ed.last_point = Some(end);
                if i.clicked {
                    let finishes = hit.is_some() && end != route.anchor;
                    route.place(end, hit)?;
                    doc.view.pcb.preview = Some(route.draft.clone());
                    if finishes {
                        finish_route(doc, ed)?;
                    }
                } else {
                    doc.view.pcb.preview = Some(route.preview(end)?);
                }
            } else if i.clicked {
                let route = RouteSession::begin(
                    board,
                    doc.history.revision,
                    i.raw_point.unwrap_or(p),
                    layer,
                    ed.net,
                    ed.width,
                    tol,
                )?;
                // Free-space starts use the grid; existing pads/tracks use exact geometry.
                let route = if pcb_route::target(board, i.raw_point.unwrap_or(p), layer, tol)
                    .is_none()
                {
                    RouteSession::begin(board, doc.history.revision, p, layer, ed.net, ed.width, 0)?
                } else {
                    route
                };
                ed.net = route.net;
                doc.view.pcb.preview = Some(route.draft.clone());
                ed.route_session = Some(route);
            }
        }
        Tool::Region | Tool::Outline | Tool::Polygon | Tool::PolygonCutout => {
            if i.double_clicked && ed.points.len() >= 3 {
                return finish_polygon(doc, ed);
            }
            if i.clicked && ed.points.last() != Some(&p) {
                ed.points.push(p);
            }
            doc.view.pcb.draft_outline = ed
                .points
                .iter()
                .copied()
                .chain(std::iter::once(p))
                .collect();
            if ed.tool == Tool::Region {
                if let Some(item) = placement(ed, p, layer) {
                    doc.view.pcb.preview = Some(board.edited(Edit::Insert(vec![item]))?);
                }
            } else if ed.tool == Tool::Outline && ed.points.len() >= 2 {
                let mut next = board.clone();
                let mut points = ed.points.clone();
                if points.last() != Some(&p) {
                    points.push(p);
                }
                next.outline = points
                    .into_iter()
                    .map(|position| ContourVertex {
                        position,
                        arc: None,
                    })
                    .collect();
                doc.view.pcb.preview = Some(next);
            }
        }
        Tool::Arc => {
            if ed.points.len() < 2 {
                if i.clicked {
                    ed.points.push(p);
                }
            } else if let Some(item) = placement(ed, p, layer) {
                let next = board.edited(Edit::Insert(vec![item]))?;
                if i.clicked {
                    commit(doc, next)?;
                    ed.points.clear();
                    doc.view.pcb.preview = None;
                } else {
                    doc.view.pcb.preview = Some(next);
                }
            }
        }
        Tool::Fill if ed.start.is_none() => {
            if i.clicked {
                ed.start = Some(p);
            }
        }
        _ => {
            if let Some(item) = placement(ed, p, layer) {
                let next = board.edited(Edit::Insert(vec![item]))?;
                if i.clicked {
                    commit(doc, next)?;
                    doc.view.pcb.preview = None;
                    if ed.tool == Tool::Fill {
                        ed.start = None;
                    }
                } else {
                    doc.view.pcb.preview = Some(next);
                }
            }
        }
    }
    Ok(())
}
impl PcbCraft {
    pub(super) fn pcb_input(&mut self, index: usize, input_: Input, ctx: &egui::Context) {
        if input_.double_clicked {
            self.settings.properties = true;
            self.settings.right_components = false;
            self.settings.right_messages = false;
        }
        let doc = &mut self.documents[index];
        let mut editor = std::mem::take(&mut doc.pcb_editor);
        let result = input(doc, &mut editor, &mut self.pcb_clipboard, input_, ctx);
        let error = result.err();
        let changed = error
            .as_ref()
            .is_some_and(|e| editor.last_error.as_ref() != Some(e));
        editor.last_error = error.clone();
        doc.pcb_editor = editor;
        if changed {
            self.message(true, error.unwrap());
        }
    }
    pub(super) fn pcb_action(&mut self, action: Action) {
        let Some(index) = self.active else {
            return;
        };
        if self.documents[index].model.pcb.is_none() {
            return;
        }
        let doc = &mut self.documents[index];
        let mut ed = std::mem::take(&mut doc.pcb_editor);
        let result = (|| -> Result<(), String> {
            // Switching to another edit must not discard a route already entered.
            if ed.route_session.is_some()
                && matches!(
                    action,
                    Action::Tool(_)
                        | Action::Copy
                        | Action::Cut
                        | Action::Paste
                        | Action::Delete
                        | Action::Rotate
                        | Action::RotateBack
                        | Action::Repour
                        | Action::Shelve
                        | Action::Restore
                        | Action::Flip
                        | Action::GroupComponent
                )
            {
                finish_route(doc, &mut ed)?;
            }
            if ed.gesture.is_some()
                && !matches!(action, Action::Report | Action::Layers | Action::Grid)
            {
                return Err("Finish or cancel the active drag before running another edit".into());
            }
            match action {
                Action::Tool(tool) => {
                    finish_route(doc, &mut ed)?;
                    ed.set_tool(tool, &mut doc.view);
                }
                Action::Layers => doc.view.pcb.show_layer_dialog = true,
                Action::Grid => doc.view.pcb.show_grid_dialog = true,
                Action::Copy | Action::Cut => {
                    if doc.view.pcb.selected.is_empty() {
                        return Err("Select PCB objects first".into());
                    }
                    ed.set_tool(Tool::CopyReference, &mut doc.view);
                    ed.cut_after_copy = matches!(action, Action::Cut);
                }
                Action::Paste => {
                    if self.pcb_clipboard.is_none() {
                        return Err("PCB clipboard is empty".into());
                    }
                    ed.set_tool(Tool::Paste, &mut doc.view);
                }
                Action::Delete => {
                    let next = doc
                        .model
                        .pcb
                        .as_ref()
                        .unwrap()
                        .edited(Edit::Delete(doc.view.pcb.selected.clone()))?;
                    commit(doc, next)?;
                    doc.view.pcb.selected.clear();
                    ed.draft = None;
                }
                Action::Rotate | Action::RotateBack => {
                    let board = doc.model.pcb.as_ref().unwrap();
                    if let Some(bounds) = board.selected_bounds(&doc.view.pcb.selected) {
                        let next = board.edited(Edit::Transform {
                            selection: doc.view.pcb.selected.clone(),
                            pivot: bounds.centre(),
                            translation: Point::default(),
                            angle: if matches!(action, Action::RotateBack) {
                                -90.0
                            } else {
                                90.0
                            },
                        })?;
                        commit(doc, next)?;
                        ed.draft = None;
                    }
                }
                Action::SelectAll => {
                    let board = doc.model.pcb.as_ref().unwrap();
                    let mut selected: BTreeSet<_> = board
                        .items
                        .iter()
                        .filter(|i| doc.view.pcb.selectable(i))
                        .map(|i| picked(board, &doc.view.pcb, i.id))
                        .collect();
                    if doc.view.pcb.filters[7] {
                        selected.extend(
                            board
                                .polygon_pours
                                .iter()
                                .filter(|p| doc.view.pcb.visible.contains(&p.layer))
                                .map(|p| p.id),
                        );
                    }
                    doc.view.pcb.selected = selected.into_iter().collect();
                }
                Action::Repour => {
                    let next = pcb_polygon::repour(doc.model.pcb.as_ref().unwrap())?;
                    commit(doc, next)?;
                }
                Action::Shelve | Action::Restore => {
                    let mut next = doc.model.pcb.as_ref().unwrap().clone();
                    let selected = &doc.view.pcb.selected;
                    for p in &mut next.polygon_pours {
                        if selected.is_empty() || selected.contains(&p.id) {
                            if p.locked {
                                return Err("Unlock the polygon first".into());
                            }
                            p.shelved = matches!(action, Action::Shelve);
                        }
                    }
                    next.pours_stale = true;
                    commit(doc, next)?;
                }
                Action::FinishRoute => finish_route(doc, &mut ed)?,
                Action::Backtrack => {
                    if let Some(route) = &mut ed.route_session {
                        if route.backtrack() {
                            doc.view.pcb.active_layer = route.layer;
                            doc.view.pcb.preview = Some(route.draft.clone());
                        } else {
                            ed.route_session = None;
                            doc.view.pcb.preview = None;
                        }
                    } else {
                        ed.points.pop();
                        doc.view.pcb.draft_outline = ed.points.clone();
                    }
                }
                Action::RouteLayer(layer) => route_layer(doc, &mut ed, layer)?,
                Action::RouteVia => {
                    let board = doc.model.pcb.as_ref().unwrap();
                    let mut layers: Vec<_> = board.layers.iter().filter(|l| l.copper).collect();
                    layers.sort_by_key(|l| l.order);
                    let n = layers
                        .iter()
                        .position(|l| l.id == doc.view.pcb.active_layer)
                        .unwrap_or(0);
                    if layers.len() < 2 {
                        return Err("Routing via needs at least two copper layers".into());
                    }
                    let layer = layers[(n + 1) % layers.len()].id;
                    if ed.route_session.is_none() {
                        return Err(
                            "Start an interactive route before adding its layer-change via".into(),
                        );
                    }
                    route_layer(doc, &mut ed, layer)?;
                }
                Action::Flip => {
                    let id = doc
                        .view
                        .pcb
                        .selected
                        .first()
                        .copied()
                        .ok_or("Select one component")?;
                    let next = pcb_tools::flip_component(doc.model.pcb.as_ref().unwrap(), id)?;
                    commit(doc, next)?;
                }
                Action::GroupComponent => {
                    let board = doc.model.pcb.as_ref().unwrap();
                    let ids = board.resolve_selection(&doc.view.pcb.selected);
                    if ids.is_empty() {
                        return Err("Select footprint primitives first".into());
                    }
                    if board.items.iter().any(|i| {
                        ids.contains(&i.id)
                            && (i.locked || i.component.is_some() || i.polygon.is_some())
                    }) {
                        return Err(
                            "Only unlocked standalone primitives can be grouped into a component"
                                .into(),
                        );
                    }
                    let mut next = board.clone();
                    let index = next
                        .components
                        .iter()
                        .map(|c| c.index)
                        .max()
                        .map_or(Some(0), |n| n.checked_add(1))
                        .ok_or("Component index overflow")?;
                    if !matches!(doc.view.pcb.active_layer, pcb::TOP | pcb::BOTTOM) {
                        return Err(
                            "Choose Top or Bottom Layer before creating a footprint component"
                                .into(),
                        );
                    }
                    let position = board
                        .selected_bounds(&doc.view.pcb.selected)
                        .ok_or("No selection bounds")?
                        .centre();
                    let mut designator = String::new();
                    for n in 1..=1_000_000 {
                        let v = format!("U{n}");
                        if !board.components.iter().any(|c| c.designator == v) {
                            designator = v;
                            break;
                        }
                    }
                    let id = Uuid::new_v4();
                    next.components.push(pcb::Component {
                        id,
                        index,
                        designator,
                        comment: String::new(),
                        footprint: "Local footprint".into(),
                        position,
                        rotation: 0.0,
                        layer: doc.view.pcb.active_layer,
                        locked: false,
                        properties: pcb::Fields::new(),
                    });
                    for i in &mut next.items {
                        if ids.contains(&i.id) {
                            i.component = Some(index);
                        }
                    }
                    commit(doc, next)?;
                    doc.view.pcb.selected = vec![id];
                }
                Action::Report => {}
            }
            Ok(())
        })();
        doc.pcb_editor = ed;
        if let Err(e) = result {
            self.message(true, e);
        }
        if matches!(action, Action::Report) {
            let b = self.documents[index].model.pcb.as_ref().unwrap();
            let summary = format!(
                "PCB: {} primitives, {} components, {} nets, {} layers. Native PcbDoc writing has conservative unsupported-dependency guards. No DRC/shove/3D; Altium parity is not verified.",
                b.items.len(),
                b.components.len(),
                b.nets.len(),
                b.layers.len()
            );
            let warnings = b.warnings.clone();
            self.message(false, summary);
            for w in warnings {
                self.message(false, w);
            }
            self.settings.messages = true;
            self.settings.right_messages = true;
        }
    }
    pub(super) fn pcb_shortcuts(&mut self, ctx: &egui::Context) -> bool {
        let Some(index) = self.active else {
            return false;
        };
        if self.documents[index].model.pcb.is_none() {
            return false;
        }
        let mut action = None;
        let mut handled = false;
        let doc = &mut self.documents[index];
        let now = ctx.input(|i| i.time);
        if doc
            .pcb_editor
            .sequence
            .is_some_and(|(_, time)| now - time > 2.0)
        {
            doc.pcb_editor.sequence = None;
        }
        ctx.input_mut(|input| {
            if let Some((prefix, _)) = doc.pcb_editor.sequence {
                for (key, tool) in [
                    (Key::T, Tool::Route),
                    (Key::P, Tool::Pad),
                    (Key::V, Tool::Via),
                    (Key::S, Tool::Text),
                    (Key::F, Tool::Fill),
                    (Key::G, Tool::Polygon),
                    (Key::A, Tool::Arc),
                    (Key::M, Tool::Move),
                ] {
                    if input.consume_key(egui::Modifiers::NONE, key) {
                        if prefix == Key::P || (prefix == Key::M && key == Key::M) {
                            action = Some(Action::Tool(tool));
                        }
                        doc.pcb_editor.sequence = None;
                        handled = true;
                        return;
                    }
                }
            }
            let ctrl = egui::Modifiers::CTRL;
            let shift = egui::Modifiers::SHIFT;
            if doc.pcb_editor.route_session.is_some() && input.consume_key(ctrl, Key::Z) {
                action = Some(Action::Backtrack);
                handled = true;
                return;
            }
            for (mods, key, command) in [
                (ctrl, Key::W, Action::Tool(Tool::Route)),
                (ctrl, Key::C, Action::Copy),
                (ctrl, Key::X, Action::Cut),
                (ctrl, Key::V, Action::Paste),
                (ctrl, Key::A, Action::SelectAll),
                (egui::Modifiers::NONE, Key::Delete, Action::Delete),
                (egui::Modifiers::NONE, Key::L, Action::Layers),
                (egui::Modifiers::NONE, Key::G, Action::Grid),
            ] {
                if input.consume_key(mods, key) {
                    action = Some(command);
                    handled = true;
                    return;
                }
            }
            if input.consume_key(shift, Key::E) {
                doc.view.pcb.snap.mode = match doc.view.pcb.snap.mode {
                    pcb::SnapLayerMode::All => pcb::SnapLayerMode::Current,
                    pcb::SnapLayerMode::Current => pcb::SnapLayerMode::Off,
                    pcb::SnapLayerMode::Off => pcb::SnapLayerMode::All,
                };
                handled = true;
                return;
            }
            if input.consume_key(shift, Key::S) {
                doc.view.pcb.single_layer = (doc.view.pcb.single_layer + 1) % 3;
                handled = true;
                return;
            }
            if input.consume_key(shift, Key::C) {
                doc.view.pcb.highlight_net = None;
                handled = true;
                return;
            }
            if input.consume_key(egui::Modifiers::NONE, Key::Escape) {
                if doc.pcb_editor.route_session.is_some() {
                    action = Some(Action::FinishRoute);
                } else {
                    doc.pcb_editor.cancel(&mut doc.view);
                }
                handled = true;
                return;
            }
            if input.consume_key(egui::Modifiers::NONE, Key::Backspace)
                && doc.pcb_editor.tool != Tool::Select
            {
                action = Some(Action::Backtrack);
                handled = true;
                return;
            }
            if doc.pcb_editor.route_session.is_some()
                && input.consume_key(egui::Modifiers::NONE, Key::V)
            {
                action = Some(Action::RouteVia);
                handled = true;
                return;
            }
            if input.consume_key(shift, Key::Tab) {
                if !doc.pcb_editor.last_hits.is_empty() {
                    doc.pcb_editor.overlap =
                        (doc.pcb_editor.overlap + 1) % doc.pcb_editor.last_hits.len();
                    doc.view.pcb.selected = vec![doc.pcb_editor.last_hits[doc.pcb_editor.overlap]];
                }
                handled = true;
                return;
            }
            if input.consume_key(egui::Modifiers::NONE, Key::Tab) {
                if doc.pcb_editor.tool != Tool::Select {
                    doc.pcb_editor.paused = !doc.pcb_editor.paused;
                } else if let Some(id) = doc.view.pcb.selected.first().copied() {
                    let board = doc.model.pcb.as_ref().unwrap();
                    if let Some(item) = board.item(id) {
                        if let Some(owner) = item
                            .component
                            .and_then(|c| board.components.iter().find(|o| o.index == c))
                        {
                            doc.view.pcb.selected = vec![owner.id];
                        } else if let Some(net) = item.net {
                            doc.view.pcb.selected = board
                                .items
                                .iter()
                                .filter(|i| i.net == Some(net))
                                .map(|i| i.id)
                                .collect();
                        }
                    }
                }
                handled = true;
                return;
            }
            if input.consume_key(shift, Key::Space) {
                if doc.pcb_editor.tool == Tool::Route {
                    doc.pcb_editor.corner_mode = (doc.pcb_editor.corner_mode + 1) % 3;
                } else if doc.pcb_editor.tool == Tool::Select {
                    action = Some(Action::RotateBack);
                } else {
                    doc.pcb_editor.rotation -= 90.0;
                }
                handled = true;
                return;
            }
            if input.consume_key(egui::Modifiers::NONE, Key::Space) {
                if doc.pcb_editor.tool == Tool::Route {
                    doc.pcb_editor.diagonal_first = !doc.pcb_editor.diagonal_first;
                } else if doc.pcb_editor.tool == Tool::Select {
                    action = Some(Action::Rotate);
                } else {
                    doc.pcb_editor.rotation += 90.0;
                }
                handled = true;
                return;
            }
            for key in [Key::P, Key::M] {
                if input.consume_key(egui::Modifiers::NONE, key) {
                    doc.pcb_editor.sequence = Some((key, now));
                    handled = true;
                    return;
                }
            }
            // Home must reach the PCB camera, not the shell's older Fit binding.

            if input.key_pressed(Key::Home) {
                handled = true;
            }
        });
        if let Some(a) = action {
            self.pcb_action(a);
        }
        handled
    }
    pub(super) fn pcb_edit_menu(&mut self, ui: &mut egui::Ui) {
        let selected = self
            .active
            .is_some_and(|i| !self.documents[i].view.pcb.selected.is_empty());
        ui.separator();
        for (label, shortcut, a, enabled) in [
            ("Cut", "Ctrl+X", Action::Cut, selected),
            ("Copy", "Ctrl+C", Action::Copy, selected),
            (
                "Paste",
                "Ctrl+V",
                Action::Paste,
                self.pcb_clipboard.is_some(),
            ),
            ("Delete", "Del", Action::Delete, selected),
            ("Rotate Selection 90°", "Space", Action::Rotate, selected),
            ("Move Selection", "M, M", Action::Tool(Tool::Move), selected),
            ("Flip Component", "", Action::Flip, selected),
            (
                "Create Component from Selection",
                "",
                Action::GroupComponent,
                selected,
            ),
            ("Select All", "Ctrl+A", Action::SelectAll, true),
        ] {
            self.action(ui, label, shortcut, Command::Pcb(a), enabled);
        }
    }
    pub(super) fn pcb_menu(&mut self, ui: &mut egui::Ui, label: &str) -> bool {
        match label {
            "Place" => {
                for (name, shortcut, tool) in [
                    ("Track", "P, T", Tool::Route),
                    ("Pad", "P, P", Tool::Pad),
                    ("Via", "P, V", Tool::Via),
                    ("String", "P, S", Tool::Text),
                    ("Fill", "P, F", Tool::Fill),
                    ("Polygon Pour", "P, G", Tool::Polygon),
                    ("Polygon Cutout", "", Tool::PolygonCutout),
                    ("Solid Region", "", Tool::Region),
                    ("Arc (centre, start, end)", "P, A", Tool::Arc),
                ] {
                    self.action(ui, name, shortcut, Command::Pcb(Action::Tool(tool)), true);
                }
            }
            "Route" => {
                self.action(
                    ui,
                    "Interactive Routing",
                    "Ctrl+W",
                    Command::Pcb(Action::Tool(Tool::Route)),
                    true,
                );
                self.action(
                    ui,
                    "Add Via / Next Copper Layer",
                    "V",
                    Command::Pcb(Action::RouteVia),
                    true,
                );
                self.action(
                    ui,
                    "Backtrack",
                    "Backspace",
                    Command::Pcb(Action::Backtrack),
                    true,
                );
                self.action(
                    ui,
                    "Finish Route",
                    "Esc",
                    Command::Pcb(Action::FinishRoute),
                    true,
                );
                ui.separator();
                for name in [
                    "Interactive Differential Pair Routing",
                    "Interactive Length Tuning",
                    "Push and Shove",
                ] {
                    ui.add_enabled(false, egui::Button::new(name));
                }
            }
            "Design" => {
                self.action(
                    ui,
                    "Define Board Shape",
                    "",
                    Command::Pcb(Action::Tool(Tool::Outline)),
                    true,
                );
                self.action(
                    ui,
                    "Layers and Colors",
                    "L",
                    Command::Pcb(Action::Layers),
                    true,
                );
                for name in ["Layer Stack Manager", "Rules…"] {
                    ui.add_enabled(false, egui::Button::new(name));
                }
            }
            "Tools" => {
                ui.add_enabled(false, egui::Button::new("Design Rule Check…"));
                self.action(
                    ui,
                    "Repour All Polygons",
                    "",
                    Command::Pcb(Action::Repour),
                    true,
                );
                self.action(
                    ui,
                    "Shelve Selected / All Polygons",
                    "",
                    Command::Pcb(Action::Shelve),
                    true,
                );
                self.action(
                    ui,
                    "Restore Selected / All Polygons",
                    "",
                    Command::Pcb(Action::Restore),
                    true,
                );
            }
            "Reports" => {
                self.action(
                    ui,
                    "PCB Import and Compatibility Report",
                    "",
                    Command::Pcb(Action::Report),
                    true,
                );
            }
            _ => return false,
        }
        true
    }
    pub(super) fn pcb_layer_bar(&mut self, ui: &mut egui::Ui, index: usize) {
        egui::TopBottomPanel::bottom("pcb_layers_v1")
            .exact_height(25.0)
            .show_inside(ui, |ui| {
                egui::ScrollArea::horizontal().show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let doc = &mut self.documents[index];
                        let Some(board) = &doc.model.pcb else {
                            return;
                        };
                        doc.view.pcb.initialize(board);
                        if ui.small_button("Layers").clicked() {
                            doc.view.pcb.show_layer_dialog = true;
                        }
                        for layer in &board.layers {
                            if !doc.view.pcb.visible.contains(&layer.id) {
                                continue;
                            }
                            let active = doc.view.pcb.active_layer == layer.id;
                            if ui
                                .selectable_label(
                                    active,
                                    RichText::new(&layer.name)
                                        .color(eda_render::pcb::layer_color(layer.id)),
                                )
                                .clicked()
                                && doc.view.pcb.active_layer != layer.id
                            {
                                self.command = Some(Command::Pcb(Action::RouteLayer(layer.id)));
                            }
                        }
                    });
                });
            });
        egui::TopBottomPanel::top("pcb_tools_v1")
            .exact_height(28.0)
            .show_inside(ui, |ui| {
                ui.horizontal(|ui| {
                    let mut command = None;
                    let doc = &self.documents[index];
                    for (name, tool) in [
                        ("Select", Tool::Select),
                        ("Route", Tool::Route),
                        ("Pad", Tool::Pad),
                        ("Via", Tool::Via),
                        ("Text", Tool::Text),
                        ("Polygon", Tool::Polygon),
                        ("Region", Tool::Region),
                    ] {
                        if ui
                            .selectable_label(doc.pcb_editor.tool == tool, name)
                            .clicked()
                        {
                            command = Some(Action::Tool(tool));
                        }
                    }
                    ui.separator();
                    ui.label(if doc.pcb_editor.paused {
                        "Paused — click canvas to resume"
                    } else if doc.pcb_editor.tool == Tool::CopyReference {
                        "Click clipboard reference point"
                    } else if doc.pcb_editor.tool == Tool::Route {
                        "Space: elbow · Shift+Space: mode · V: via · Backspace: undo leg"
                    } else {
                        "Esc cancels · Shift+click adds"
                    });
                    if let Some(a) = command {
                        self.command = Some(Command::Pcb(a));
                    }
                });
            });
    }
    pub(super) fn pcb_dialogs(&mut self, ctx: &egui::Context) {
        let Some(i) = self.active else {
            return;
        };
        let doc = &mut self.documents[i];
        let Some(board) = &doc.model.pcb else {
            return;
        };
        let mut layers = doc.view.pcb.show_layer_dialog;
        if layers {
            egui::Window::new("View Configuration — Layers and Colors")
                .open(&mut layers)
                .default_width(380.0)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        if ui.button("All On").clicked() {
                            doc.view.pcb.visible = board.layers.iter().map(|l| l.id).collect();
                        }
                        if ui.button("All Off").clicked() {
                            doc.view.pcb.visible.clear();
                        }
                    });
                    for l in &board.layers {
                        ui.horizontal(|ui| {
                            let mut visible = doc.view.pcb.visible.contains(&l.id);
                            if ui.checkbox(&mut visible, "").changed() {
                                if visible {
                                    doc.view.pcb.visible.insert(l.id);
                                } else {
                                    doc.view.pcb.visible.remove(&l.id);
                                }
                            }
                            ui.colored_label(eda_render::pcb::layer_color(l.id), "■");
                            if ui
                                .selectable_label(doc.view.pcb.active_layer == l.id, &l.name)
                                .clicked()
                            {
                                self.command = Some(Command::Pcb(Action::RouteLayer(l.id)));
                            }
                        });
                    }
                    ui.separator();
                    ui.checkbox(&mut doc.view.pcb.polygons_visible, "Show polygon copper");
                    ui.horizontal(|ui| {
                        ui.label("Single layer");
                        for (value, label) in [(0, "Off"), (1, "Dim"), (2, "Hide")] {
                            ui.selectable_value(&mut doc.view.pcb.single_layer, value, label);
                        }
                    });
                });
        }
        doc.view.pcb.show_layer_dialog = layers;
        let mut grid = doc.view.pcb.show_grid_dialog;
        if grid {
            egui::Window::new("Snap Grid")
                .open(&mut grid)
                .resizable(false)
                .show(ctx, |ui| {
                    for nm in [
                        25_400, 50_800, 100_000, 127_000, 254_000, 500_000, 1_000_000,
                    ] {
                        ui.selectable_value(
                            &mut doc.view.grid_nm,
                            nm,
                            format!("{:.4} mm / {:.2} mil", nm as f64 / 1e6, nm as f64 / 25400.0),
                        );
                    }
                    ui.checkbox(&mut doc.view.grid, "Show grid");
                });
        }
        doc.view.pcb.show_grid_dialog = grid;
    }
    pub(super) fn pcb_properties(&mut self, ui: &mut egui::Ui) -> bool {
        properties_v2::show(self, ui)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn clipboard_remaps_nets_and_detaches_unselected_owner() {
        let mut b = Board::default();
        b.nets.push(pcb::Net {
            index: 7,
            name: "GND".into(),
            properties: pcb::Fields::new(),
        });
        let i = Item::new(
            1,
            Some(7),
            Shape::Track {
                start: Point::default(),
                end: Point { x: 1_000_000, y: 0 },
                width: 100_000,
            },
        );
        let id = i.id;
        b.items.push(i);
        let clip = clipboard(&b, &[id], Point::default());
        let (next, selection) =
            pasted(&Board::default(), &clip, Point { x: 2_000_000, y: 0 }).unwrap();
        assert_eq!(next.nets[0].name, "GND");
        assert_eq!(next.items[0].net, Some(0));
        assert_eq!(next.items[0].anchor().x, 2_000_000);
        assert_eq!(selection.len(), 1);
        assert_ne!(next.items[0].id, id);
    }
    #[test]
    fn copy_does_not_modify_source() {
        let mut b = Board::default();
        let i = Item::new(
            1,
            None,
            Shape::Track {
                start: Point::default(),
                end: Point { x: 1, y: 1 },
                width: 1,
            },
        );
        let id = i.id;
        b.items.push(i);
        let before = b.clone();
        let clip = clipboard(&b, &[id], Point::default());
        pasted(&b, &clip, Point { x: 20, y: 30 }).unwrap();
        assert_eq!(b, before);
    }
}

#[cfg(test)]
mod controller_tests {
    use super::*;
    fn doc() -> OpenDocument {
        let mut model = Document::new(DocumentKind::Board, "controller-test.PcbDoc");
        let mut board = Board::from_legacy(&model);
        board.nets.push(pcb::Net {
            index: 0,
            name: "GND".into(),
            properties: pcb::Fields::new(),
        });
        board.items.push(Item::new(
            pcb::TOP,
            Some(0),
            Shape::Pad {
                position: Point::mm(10, 0),
                size: Point::mm(1, 1),
                rotation: 0.0,
                shape: PadShape::Round,
                hole: 0,
                plated: true,
                name: "1".into(),
                bottom_size: None,
                middle_size: None,
            },
        ));
        model.pcb = Some(board);
        let mut d = OpenDocument::new(model, None);
        d.view.pcb.initialize(d.model.pcb.as_ref().unwrap());
        d.view.camera.pixels_per_mm = 100.0;
        d
    }
    fn click(x: i64, y: i64) -> Input {
        Input {
            point: Some(Point::mm(x, y)),
            raw_point: Some(Point::mm(x, y)),
            clicked: true,
            hovered: true,
            ..Default::default()
        }
    }
    #[test]
    fn routing_preview_is_not_committed_until_finished() {
        let mut d = doc();
        let original = d.model.clone();
        let mut ed = Editor::default();
        let mut clip = None;
        ed.net = Some(0);
        ed.set_tool(Tool::Route, &mut d.view);
        let ctx = egui::Context::default();
        input(&mut d, &mut ed, &mut clip, click(0, 0), &ctx).unwrap();
        input(&mut d, &mut ed, &mut clip, click(5, 2), &ctx).unwrap();
        assert_eq!(d.model, original);
        assert!(d.view.pcb.preview.is_some());
        input(&mut d, &mut ed, &mut clip, click(10, 0), &ctx).unwrap();
        assert!(ed.route_session.is_none());
        assert!(d.model.pcb.as_ref().unwrap().items.len() > 1);
        assert_eq!(d.history.revision, 1);
        d.history.undo(&mut d.model);
        assert_eq!(d.model, original);
    }
    #[test]
    fn cancel_route_does_not_leave_splits_or_new_tracks() {
        let mut d = doc();
        let original = d.model.clone();
        let mut ed = Editor::default();
        ed.set_tool(Tool::Route, &mut d.view);
        let ctx = egui::Context::default();
        input(&mut d, &mut ed, &mut None, click(0, 0), &ctx).unwrap();
        input(&mut d, &mut ed, &mut None, click(4, 3), &ctx).unwrap();
        ed.cancel(&mut d.view);
        assert_eq!(d.model, original);
        assert!(!d.history.can_undo());
        assert!(d.view.pcb.preview.is_none());
    }
    #[test]
    fn save_finishes_a_pending_route_in_one_transaction() {
        let mut d = doc();
        let mut ed = Editor::default();
        ed.set_tool(Tool::Route, &mut d.view);
        let ctx = egui::Context::default();
        input(&mut d, &mut ed, &mut None, click(0, 0), &ctx).unwrap();
        input(&mut d, &mut ed, &mut None, click(2, 1), &ctx).unwrap();
        d.pcb_editor = ed;
        assert!(d.pcb_editor.has_pending());
        prepare_save(&mut d).unwrap();
        assert!(!d.pcb_editor.has_pending());
        assert_eq!(d.history.revision, 1);
    }
    #[test]
    fn polygon_creation_copy_paste_and_delete_are_definition_operations() {
        let mut d = doc();
        let mut ed = Editor {
            tool: Tool::Polygon,
            net: Some(0),
            points: vec![
                Point::mm(1, 1),
                Point::mm(8, 1),
                Point::mm(8, 8),
                Point::mm(1, 8),
            ],
            ..Editor::default()
        };
        finish_polygon(&mut d, &mut ed).unwrap();
        let b = d.model.pcb.as_ref().unwrap();
        assert_eq!(b.polygon_pours.len(), 1);
        let id = b.polygon_pours[0].id;
        let clip = clipboard(b, &[id], Point::mm(1, 1));
        assert!(clip.fragment.items.is_empty());
        let (n, ids) = pasted(b, &clip, Point::mm(12, 12)).unwrap();
        assert_eq!(n.polygon_pours.len(), 2);
        assert_eq!(ids.len(), 1);
        let n = n.edited(Edit::Delete(ids)).unwrap();
        assert_eq!(n.polygon_pours.len(), 1);
    }
    #[test]
    fn invalid_polygon_is_atomic() {
        let mut d = doc();
        let original = d.model.clone();
        let mut ed = Editor {
            tool: Tool::Polygon,
            points: vec![
                Point::mm(0, 0),
                Point::mm(4, 4),
                Point::mm(0, 4),
                Point::mm(4, 0),
            ],
            ..Editor::default()
        };
        assert!(finish_polygon(&mut d, &mut ed).is_err());
        assert_eq!(d.model, original);
    }
    #[test]
    fn save_does_not_discard_an_unfinished_fill() {
        let mut d = doc();
        d.pcb_editor.tool = Tool::Fill;
        d.pcb_editor.start = Some(Point::mm(3, 4));
        assert!(prepare_save(&mut d).is_err());
        assert_eq!(d.pcb_editor.start, Some(Point::mm(3, 4)));
        assert_eq!(d.history.revision, 0);
    }
}
