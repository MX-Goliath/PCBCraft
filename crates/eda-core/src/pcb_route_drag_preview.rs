//! Reusable, revision-owned connected drag for a single assigned net.
use super::*;
use std::{collections::HashMap, sync::Arc};

#[derive(Clone, Copy, PartialEq, Eq)]
struct Request {
    pivot: Point,
    point: Point,
    angle: u64,
    options: DragOptions,
}
#[derive(Clone, Default)]
struct Saved {
    source: Vec<(usize, Item)>,
    tail: Vec<Item>,
    stale: bool,
}

#[derive(Clone)]
pub struct TrackDragPreview {
    source: Arc<Board>,
    selection: Vec<Uuid>,
    endpoint: Option<bool>,
    body_slide: Option<Slide>,
    indices: HashMap<Uuid, usize>,
    reserved: BTreeSet<Uuid>,
    tail_ids: Vec<Uuid>,
    environments: Environments,
    working: Option<Arc<Board>>,
    touched: BTreeSet<Uuid>,
    saved: Saved,
    last_valid: Option<Request>,
    last_request: Option<Request>,
    constrained: bool,
}

pub struct TrackDragUpdate {
    pub preview: RoutePreview,
    pub constrained: bool,
}

impl TrackDragPreview {
    /// Moving foreign nets or unassigned copper requires the complete solver.
    pub fn supports(board: &Board, selection: &[Uuid]) -> bool {
        track_selection(board, selection)
            && board
                .item(selection[0])
                .and_then(|i| i.net)
                .is_some_and(|net| {
                    selection
                        .iter()
                        .all(|id| board.item(*id).is_some_and(|i| i.net == Some(net)))
                })
    }
    pub fn new(board: &Board, selection: &[Uuid]) -> Result<Self, String> {
        if !Self::supports(board, selection) {
            return Err("Prepared track drag requires one assigned net".into());
        }
        board.validate()?;
        if selection.iter().any(|id| board.item(*id).unwrap().locked) {
            return Err("Selection contains locked objects".into());
        }
        Ok(Self {
            source: Arc::new(board.clone()),
            selection: selection.to_vec(),
            endpoint: None,
            body_slide: (selection.len() == 1).then(|| Slide::new(board, selection[0])),
            indices: board
                .items
                .iter()
                .enumerate()
                .map(|(n, i)| (i.id, n))
                .collect(),
            reserved: board
                .items
                .iter()
                .map(|i| i.id)
                .chain(board.components.iter().map(|c| c.id))
                .chain(board.polygon_pours.iter().map(|p| p.id))
                .collect(),
            tail_ids: vec![],
            environments: Environments::new(),
            working: None,
            touched: BTreeSet::new(),
            saved: Saved {
                stale: board.pours_stale,
                ..Default::default()
            },
            last_valid: None,
            last_request: None,
            constrained: false,
        })
    }
    pub fn new_endpoint(board: &Board, id: Uuid, at_start: bool) -> Result<Self, String> {
        let mut prepared = Self::new(board, &[id])?;
        prepared.endpoint = Some(at_start);
        prepared.body_slide = None;
        Ok(prepared)
    }
    fn original_request(&self, request: Request) -> Request {
        let point = if let Some(at_start) = self.endpoint {
            let Shape::Track { start, end, .. } =
                self.source.items[self.indices[&self.selection[0]]].shape
            else {
                unreachable!()
            };
            if at_start { start } else { end }
        } else {
            Point::default()
        };
        Request {
            point,
            angle: 0_f64.to_bits(),
            ..request
        }
    }
    fn reset_source(&mut self, next: &mut Board) {
        next.items.truncate(self.source.items.len());
        for id in &self.touched {
            if let Some(index) = self.indices.get(id) {
                next.items[*index] = self.source.items[*index].clone();
            }
        }
        self.touched.clear();
        next.pours_stale = self.source.pours_stale;
    }
    fn restore_saved(&mut self, next: &mut Board) {
        self.reset_source(next);
        for (index, item) in &self.saved.source {
            next.items[*index] = item.clone();
            self.touched.insert(item.id);
        }
        next.items.extend(self.saved.tail.iter().cloned());
        next.pours_stale = self.saved.stale;
    }
    fn attempt(&mut self, next: &mut Board, request: Request) -> Result<(), String> {
        self.reset_source(next);
        self.touched.extend(self.selection.iter().copied());
        if let Some(at_start) = self.endpoint {
            let index = self.indices[&self.selection[0]];
            let mut item = self.source.items[index].clone();
            let Shape::Track { start, end, .. } = &mut item.shape else {
                unreachable!()
            };
            if at_start {
                *start = request.point;
            } else {
                *end = request.point;
            }
            next.replace_item_at_unvalidated(index, item)?;
        } else if let Some(slide) = &self.body_slide
            && f64::from_bits(request.angle) == 0.
        {
            let index = self.indices[&self.selection[0]];
            next.replace_item_at_unvalidated(index, slide.at(request.point)?)?;
        } else if request.point != Point::default() || f64::from_bits(request.angle) != 0. {
            for id in &self.selection {
                crate::pcb::transform_item(
                    &mut next.items[self.indices[id]],
                    request.pivot,
                    request.point,
                    f64::from_bits(request.angle),
                    false,
                )?;
            }
        }
        reroute_into(
            &self.source,
            next,
            &self.selection,
            request.options,
            Some(&mut self.environments),
            &mut self.touched,
            Some(&self.indices),
        )?;
        if next.items.len() > MAX_ITEMS {
            return Err("PCB entity limit exceeded".into());
        }
        // Only track geometry changes; net/layer/component references and all
        // stationary model fields come from the fully validated immutable source.
        for id in &self.touched {
            if let Some(index) = self.indices.get(id) {
                next.items[*index].validate_geometry()?;
            }
        }
        for (n, item) in next
            .items
            .iter_mut()
            .skip(self.source.items.len())
            .enumerate()
        {
            item.validate_geometry()?;
            if n == self.tail_ids.len() {
                let mut id = Uuid::new_v4();
                while !self.reserved.insert(id) {
                    id = Uuid::new_v4();
                }
                self.tail_ids.push(id);
            }
            item.id = self.tail_ids[n];
        }
        Ok(())
    }
    fn save_valid(&mut self, next: &mut Board, request: Request) {
        self.saved.source = self
            .touched
            .iter()
            .filter_map(|id| self.indices.get(id))
            .filter(|&&index| next.items[index] != self.source.items[index])
            .map(|&index| (index, next.items[index].clone()))
            .collect();
        self.saved.tail = next.items[self.source.items.len()..].to_vec();
        if self.saved.source.is_empty() && self.saved.tail.is_empty() {
            next.pours_stale = self.source.pours_stale;
        }
        self.saved.stale = next.pours_stale;
        self.last_valid = Some(request);
    }
    fn output(&self) -> TrackDragUpdate {
        let source_changes: BTreeSet<_> = self.saved.source.iter().map(|(_, i)| i.id).collect();
        let changed_items = source_changes
            .iter()
            .copied()
            .chain(self.saved.tail.iter().map(|i| i.id))
            .collect();
        TrackDragUpdate {
            preview: RoutePreview {
                board: self.working.as_ref().unwrap().clone(),
                source_len: self.source.items.len(),
                source_changes,
                changed_items,
            },
            constrained: self.constrained,
        }
    }
    pub fn update(
        &mut self,
        pivot: Point,
        translation: Point,
        angle: f64,
        options: DragOptions,
    ) -> Result<TrackDragUpdate, String> {
        if !angle.is_finite() {
            return Err("Invalid rotation".into());
        }
        let request = Request {
            pivot,
            point: translation,
            angle: angle.to_bits(),
            options,
        };
        if self.last_request == Some(request) {
            return Ok(self.output());
        }
        self.last_request = None;
        let mut next = self
            .working
            .take()
            .map(Arc::unwrap_or_clone)
            .unwrap_or_else(|| (*self.source).clone());
        if self.last_valid.is_none_or(|old| old.options != options) {
            self.saved = Saved {
                stale: self.source.pours_stale,
                ..Default::default()
            };
            self.last_valid = Some(self.original_request(request));
        }
        let result = self.attempt(&mut next, request);
        self.constrained = false;
        match result {
            Ok(()) => self.save_valid(&mut next, request),
            Err(error) if matches!(error.as_str(), CONNECTION_BLOCKED | CLEARANCE_BLOCKED) => {
                self.constrained = true;
                let from = self.last_valid.unwrap();
                if from.angle == request.angle && from.pivot == request.pivot {
                    let (mut low, mut high) = (0., 1.);
                    let mut previous = None;
                    for _ in 0..12 {
                        let t = (low + high) / 2.;
                        let point = Point {
                            x: from.point.x
                                + ((request.point.x - from.point.x) as f64 * t).round() as i64,
                            y: from.point.y
                                + ((request.point.y - from.point.y) as f64 * t).round() as i64,
                        };
                        if previous == Some(point) {
                            break;
                        }
                        previous = Some(point);
                        let middle = Request { point, ..request };
                        match self.attempt(&mut next, middle) {
                            Ok(()) => {
                                low = t;
                                self.save_valid(&mut next, middle);
                            }
                            Err(error)
                                if matches!(
                                    error.as_str(),
                                    CONNECTION_BLOCKED | CLEARANCE_BLOCKED
                                ) =>
                            {
                                high = t
                            }
                            Err(error) => {
                                self.restore_saved(&mut next);
                                self.working = Some(Arc::new(next));
                                return Err(error);
                            }
                        }
                    }
                }
                self.restore_saved(&mut next);
            }
            Err(error) => {
                self.restore_saved(&mut next);
                self.working = Some(Arc::new(next));
                return Err(error);
            }
        }
        self.working = Some(Arc::new(next));
        self.last_request = Some(request);
        Ok(self.output())
    }
    pub fn update_endpoint(
        &mut self,
        at: Point,
        options: DragOptions,
    ) -> Result<TrackDragUpdate, String> {
        if self.endpoint.is_none() {
            return Err("Not an endpoint drag".into());
        }
        self.update(Point::default(), at, 0., options)
    }
    pub fn finish(self) -> Result<Board, String> {
        let board = self
            .working
            .map(Arc::unwrap_or_clone)
            .unwrap_or_else(|| (*self.source).clone());
        board.validate()?;
        Ok(board)
    }
}
