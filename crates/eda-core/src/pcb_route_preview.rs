//! Validated session prefixes and reusable immutable route preview snapshots.
use super::*;
use std::{
    collections::{BTreeSet, HashSet},
    sync::Arc,
};
#[cfg(test)]
#[path = "pcb_route_preview_tests.rs"]
mod tests;

#[derive(Clone)]
pub struct RoutePreview {
    pub board: Arc<Board>,
    pub source_len: usize,
    pub source_changes: BTreeSet<Uuid>,
    /// Changed source primitives plus placed/active route copper, in stable slots.
    pub changed_items: BTreeSet<Uuid>,
}
#[derive(Clone, PartialEq, Eq)]
struct Request {
    end: Point,
    anchor: Point,
    layer: u16,
    net: Option<u32>,
    width: i64,
    clearance: i64,
    corner: Corner,
    diagonal_first: bool,
    conflict: Conflict,
}
impl Request {
    fn new(route: &RouteSession, end: Point) -> Self {
        Self {
            end,
            anchor: route.anchor,
            layer: route.layer,
            net: route.net,
            width: route.width,
            clearance: route.clearance,
            corner: route.corner,
            diagonal_first: route.diagonal_first,
            conflict: route.conflict,
        }
    }
}
#[derive(Clone, Default)]
pub(super) struct Cache {
    source_len: usize,
    source_changes: BTreeSet<Uuid>,
    board: Option<Arc<Board>>,
    ids: Vec<Uuid>,
    reserved: HashSet<Uuid>,
    request: Option<Request>,
    changed_items: BTreeSet<Uuid>,
}
impl Cache {
    pub fn new(source: &Board, draft: &Board) -> Self {
        Self {
            source_len: source.items.len(),
            source_changes: source
                .items
                .iter()
                .zip(&draft.items)
                .filter(|(a, b)| a != b)
                .map(|(_, b)| b.id)
                .collect(),
            ..Default::default()
        }
    }
    pub fn invalidate(&mut self, checkpoint: &Checkpoint) {
        self.source_changes.extend(
            checkpoint
                .restored
                .iter()
                .filter(|(index, _)| *index < self.source_len)
                .map(|(_, item)| item.id),
        );
        self.board = None;
        self.request = None;
        self.ids.clear();
        self.reserved.clear();
        self.changed_items.clear();
    }
    fn output(&self) -> RoutePreview {
        RoutePreview {
            board: self.board.as_ref().unwrap().clone(),
            changed_items: self.changed_items.clone(),
            source_len: self.source_len,
            source_changes: self.source_changes.clone(),
        }
    }
}
impl RouteSession {
    pub fn preview_current(&self, end: Point) -> bool {
        self.preview.request.as_ref() == Some(&Request::new(self, end))
    }
    /// The private draft prefix is validated at begin/place/via/backtrack and
    /// cannot be changed by callers. Validate only new geometry/references here;
    /// placement and History still fully validate the completed transaction.
    pub fn preview_shared(&mut self, end: Point) -> Result<RoutePreview, String> {
        let profile = std::env::var_os("PCBCRAFT_ROUTE_PROFILE").is_some();
        let started = std::time::Instant::now();
        let request = Request::new(self, end);
        if self.preview.request.as_ref() == Some(&request) {
            return Ok(self.preview.output());
        }
        if !(1..=100_000_000_000).contains(&self.width)
            || !(0..=100_000_000_000).contains(&self.clearance)
        {
            return Err("Width must be positive and clearance non-negative".into());
        }
        crate::pcb_rules::route_check(
            &self.draft,
            &crate::pcb_rules::probe(self.layer, self.net, self.anchor, self.width),
        )?;
        self.obstacles.error()?;
        let mut items: Vec<_> = self
            .points(end)
            .windows(2)
            .filter(|p| p[0] != p[1])
            .map(|p| {
                Item::new(
                    self.layer,
                    self.net,
                    Shape::Track {
                        start: p[0],
                        end: p[1],
                        width: self.width,
                    },
                )
            })
            .collect();
        if self.draft.items.len() + items.len() > MAX_ITEMS {
            return Err("PCB entity limit exceeded".into());
        }
        if !items.is_empty()
            && self
                .net
                .is_some_and(|net| !self.draft.nets.iter().any(|n| n.index == net))
        {
            return Err("Track refers to missing net".into());
        }
        for item in &items {
            item.validate_geometry()?;
        }
        let solved = started.elapsed();
        let cache = &mut self.preview;
        let owners = cache.board.as_ref().map_or(0, Arc::strong_count);
        if cache.board.is_none() {
            cache.reserved = self
                .draft
                .items
                .iter()
                .map(|i| i.id)
                .chain(self.draft.components.iter().map(|c| c.id))
                .chain(self.draft.polygon_pours.iter().map(|p| p.id))
                .collect();
            cache.board = Some(Arc::new(self.draft.clone()));
        }
        while cache.ids.len() < items.len() {
            let id = Uuid::new_v4();
            if cache.reserved.insert(id) {
                cache.ids.push(id);
            }
        }
        for (item, id) in items.iter_mut().zip(&cache.ids) {
            item.id = *id;
        }
        let board = Arc::make_mut(cache.board.as_mut().unwrap());
        board.items.truncate(self.draft.items.len());
        board.pours_stale =
            self.draft.pours_stale || !items.is_empty() && !board.polygon_pours.is_empty();
        board.items.extend(items);
        cache.changed_items = cache
            .source_changes
            .iter()
            .copied()
            .chain(board.items[cache.source_len..].iter().map(|i| i.id))
            .collect();
        cache.request = Some(request);
        if profile {
            eprintln!(
                "route_preview owners={owners} solve_ms={:.3} update_ms={:.3}",
                solved.as_secs_f64() * 1000.,
                (started.elapsed() - solved).as_secs_f64() * 1000.
            );
        }
        Ok(cache.output())
    }
}
