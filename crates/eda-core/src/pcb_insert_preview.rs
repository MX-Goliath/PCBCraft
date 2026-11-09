//! Validated immutable source plus a reusable placement tail.
use super::*;
use std::sync::Arc;

pub struct InsertPreview {
    board: Arc<Board>,
    prefix: usize,
    nets: HashSet<u32>,
    components: HashSet<u32>,
    reserved: HashSet<Uuid>,
    ids: Vec<Uuid>,
    stale: bool,
}
impl InsertPreview {
    pub fn new(board: &Board) -> Result<Self, String> {
        board.validate()?;
        Ok(Self {
            board: Arc::new(board.clone()),
            prefix: board.items.len(),
            nets: board.nets.iter().map(|n| n.index).collect(),
            components: board.components.iter().map(|c| c.index).collect(),
            reserved: board
                .items
                .iter()
                .map(|i| i.id)
                .chain(board.components.iter().map(|c| c.id))
                .chain(board.polygon_pours.iter().map(|p| p.id))
                .collect(),
            ids: vec![],
            stale: board.pours_stale,
        })
    }
    pub fn source_len(&self) -> usize {
        self.prefix
    }
    pub fn is_current(&self, items: &[Item]) -> bool {
        let tail = &self.board.items[self.prefix..];
        tail.len() == items.len()
            && tail.iter().zip(items).all(|(old, item)| {
                let mut item = item.clone();
                item.id = old.id;
                old == &item
            })
    }
    pub fn update(&mut self, mut items: Vec<Item>) -> Result<Arc<Board>, String> {
        if self.prefix + items.len() + self.board.empty_region_records.len() > MAX_ITEMS {
            return Err("PCB entity limit exceeded".into());
        }
        for item in &items {
            item.validate_geometry()?;
            if item.source.as_ref().is_some_and(|source| {
                self.board
                    .empty_region_records
                    .iter()
                    .any(|record| record.stream == source.stream && record.index == source.index)
            }) {
                return Err("Invalid archived empty region reference".into());
            }
            if item.net.is_some_and(|n| !self.nets.contains(&n))
                || item
                    .component
                    .is_some_and(|c| !self.components.contains(&c))
            {
                return Err("Placement refers to missing net/component".into());
            }
        }
        while self.ids.len() < items.len() {
            let id = Uuid::new_v4();
            if self.reserved.insert(id) {
                self.ids.push(id);
            }
        }
        for (item, id) in items.iter_mut().zip(&self.ids) {
            item.id = *id;
        }
        let board = Arc::make_mut(&mut self.board);
        board.items.truncate(self.prefix);
        board.pours_stale = self.stale || !board.polygon_pours.is_empty();
        board.items.extend(items);
        Ok(self.board.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn placements_preserve_retained_snapshots_validate_atomically_and_match_transactions() {
        let b = Board::default();
        let mut preview = InsertPreview::new(&b).unwrap();
        let via = |x| {
            Item::new(
                MULTI_LAYER,
                None,
                Shape::Via {
                    position: Point::mm(x, 1),
                    diameter: 800_000,
                    hole: 300_000,
                    from_layer: TOP,
                    to_layer: BOTTOM,
                },
            )
        };
        let first = preview.update(vec![via(1)]).unwrap();
        let saved = first.clone();
        let next = preview.update(vec![via(2)]).unwrap();
        assert_ne!(first.items[0].anchor(), next.items[0].anchor());
        assert_eq!(*first, *saved);
        assert_eq!(first.items[0].id, next.items[0].id);
        assert_eq!(*next, b.edited(Edit::Insert(next.items.clone())).unwrap());
        assert!(preview.is_current(&[via(2)]));
        let mut invalid = via(3);
        invalid.net = Some(999);
        assert!(preview.update(vec![invalid]).is_err());
        assert!(preview.is_current(&[via(2)]));
        assert!(b.items.is_empty());
    }
}
