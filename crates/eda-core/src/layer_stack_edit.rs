//! Transactional fabrication-stack edits. Physical symmetry never copies layer identities.
use crate::layer_stack::{Layer, LayerKind, Stack};
use crate::pcb::{BOTTOM, TOP};
use uuid::Uuid;

impl Stack {
    pub fn is_symmetric(&self) -> bool {
        let n = self.layers.len();
        n > 0
            && n % 2 == 1
            && !self.layers[n / 2].kind.copper()
            && self
                .layers
                .iter()
                .zip(self.layers.iter().rev())
                .all(|(a, b)| {
                    a.kind == b.kind
                        && a.thickness == b.thickness
                        && a.material == b.material
                        && a.er == b.er
                        && a.loss_tangent == b.loss_tangent
                })
    }

    /// Resolve property mismatches without renaming layers or changing their identities.
    pub fn mirror_half(&mut self, top_down: bool) -> Result<(), String> {
        let mut next = self.clone();
        let n = next.layers.len();
        if n == 0 || n.is_multiple_of(2) || next.layers[n / 2].kind.copper() {
            return Err("Symmetry requires a central dielectric and paired layers".into());
        }
        for a in 0..n / 2 {
            let b = n - 1 - a;
            if next.layers[a].kind.copper() != next.layers[b].kind.copper() {
                return Err("Opposite layers have incompatible copper/dielectric types".into());
            }
            let (source, target) = if top_down { (a, b) } else { (b, a) };
            let properties = next.layers[source].clone();
            copy_material(&properties, &mut next.layers[target]);
        }
        next.symmetric = true;
        next.validate()?;
        *self = next;
        Ok(())
    }

    pub fn edit_layer(&mut self, index: usize, layer: Layer) -> Result<(), String> {
        let old = self.layers.get(index).ok_or("Select a layer")?;
        if old.id != layer.id
            || old.copper_id != layer.copper_id
            || old.kind.copper() != layer.kind.copper()
        {
            return Err("A property edit cannot change the layer identity".into());
        }
        let mut next = self.clone();
        next.layers[index] = layer;
        if next.symmetric {
            let partner = next.layers.len() - 1 - index;
            let source = next.layers[index].clone();
            copy_material(&source, &mut next.layers[partner]);
        }
        next.validate()?;
        *self = next;
        Ok(())
    }

    /// Insert copper into a dielectric interval, preserving total dielectric thickness.
    /// Paired insertion uses independent, unused electrical identities.
    pub fn insert_copper(&mut self, selected: usize, kind: LayerKind) -> Result<usize, String> {
        if !kind.copper() {
            return Err("Choose Signal or Plane".into());
        }
        self.validate()?;
        let row = self.layers.get(selected).ok_or("Select a layer")?;
        let at = if row.kind.copper() {
            if row.copper_id == Some(BOTTOM) {
                selected.checked_sub(1)
            } else {
                Some(selected + 1)
            }
        } else {
            Some(selected)
        }
        .ok_or("No dielectric next to the selected layer")?;
        let dielectric = self
            .layers
            .get(at)
            .ok_or("No dielectric next to the selected layer")?;
        if dielectric.kind.copper() || dielectric.kind == LayerKind::SolderMask {
            return Err("Insert copper inside a core or prepreg interval".into());
        }
        let mut next = self.clone();
        let mirror = self.layers.len() - 1 - at;
        let paired = self.symmetric;
        let mut positions = vec![at];
        if paired && at != mirror {
            positions.push(mirror);
        }
        positions.sort_unstable_by(|a, b| b.cmp(a));
        let mut inserted = Vec::new();
        for pos in positions {
            let count = if paired && at == mirror { 2 } else { 1 };
            let old = next.layers[pos].clone();
            let parts = count + 1;
            let side = old.thickness / parts as i64;
            if side < 1 {
                return Err("Dielectric is too thin to split".into());
            }
            let mut rows = Vec::new();
            for j in 0..parts {
                let mut d = old.clone();
                if j != 0 {
                    d.id = Uuid::new_v4();
                }
                d.thickness = side;
                // A central pair must put the integer remainder in its central dielectric.
                let remainder_index = if count == 2 { 1 } else { parts - 1 };
                if j == remainder_index {
                    d.thickness += old.thickness % parts as i64;
                }
                rows.push(d);
                if j < count {
                    let range = if kind == LayerKind::Plane {
                        39..55
                    } else {
                        2..32
                    };
                    let id = range
                        .into_iter()
                        .find(|id| {
                            !next.layers.iter().any(|l| l.copper_id == Some(*id))
                                && !inserted.iter().any(|(_, used)| used == id)
                        })
                        .ok_or("No unused copper layer identities remain")?;
                    let mut copper = Layer::dielectric();
                    copper.kind = kind;
                    copper.copper_id = Some(id);
                    copper.name = crate::pcb::layer_name(id);
                    copper.material = "Copper".into();
                    copper.thickness = 35_000;
                    inserted.push((copper.id, id));
                    rows.push(copper);
                }
            }
            // Mirror split direction in the lower half, including odd-nanometre remainders.
            if paired && pos > mirror.min(at) && at != mirror {
                rows.reverse();
            }
            next.layers.splice(pos..=pos, rows);
        }
        next.validate()?;
        let selection = next
            .layers
            .iter()
            .position(|l| inserted.iter().any(|(id, _)| *id == l.id))
            .ok_or("No layer was inserted")?;
        *self = next;
        Ok(selection)
    }

    pub fn insert_dielectric(&mut self, selected: usize) -> Result<usize, String> {
        self.validate()?;
        let row = self.layers.get(selected).ok_or("Select a layer")?;
        let at = if row.kind.copper() {
            if row.copper_id == Some(BOTTOM) {
                selected.saturating_sub(1)
            } else {
                selected + 1
            }
        } else {
            selected
        };
        let row = self.layers.get(at).ok_or("No dielectric interval")?;
        if !matches!(row.kind, LayerKind::Core | LayerKind::Prepreg) {
            return Err("Select a core or prepreg interval".into());
        }
        let mut next = self.clone();
        let partner = next.layers.len() - 1 - at;
        let mut positions = vec![at];
        if next.symmetric && partner != at {
            positions.push(partner);
        }
        positions.sort_unstable_by(|a, b| b.cmp(a));
        for pos in positions {
            let old = next.layers[pos].clone();
            let parts = if next.symmetric && at == partner {
                3
            } else {
                2
            };
            let thickness = old.thickness / parts as i64;
            if thickness == 0 {
                return Err("Dielectric is too thin to split".into());
            }
            let mut rows = Vec::new();
            for i in 0..parts {
                let mut row = old.clone();
                if i != 0 {
                    row.id = Uuid::new_v4();
                }
                row.thickness = thickness;
                if i == 1 {
                    row.thickness += old.thickness % parts as i64;
                }
                rows.push(row);
            }
            if next.symmetric && at != partner && pos > at.min(partner) {
                rows.reverse();
            }
            next.layers.splice(pos..=pos, rows);
        }
        next.validate()?;
        *self = next;
        Ok(at.min(if self.symmetric { partner } else { at }))
    }

    pub fn remove_layer(&mut self, index: usize) -> Result<(), String> {
        let layer = self.layers.get(index).ok_or("Select a layer")?;
        if matches!(layer.copper_id, Some(TOP | BOTTOM)) {
            return Err("Outer copper layers cannot be deleted".into());
        }
        let mut next = self.clone();
        let partner = next.layers.len() - 1 - index;
        if next.symmetric && partner != index {
            next.layers.remove(index.max(partner));
            next.layers.remove(index.min(partner));
        } else {
            next.layers.remove(index);
        }
        next.validate()?;
        *self = next;
        Ok(())
    }

    /// Reorder within the same category, leaving dielectric separation intact.
    pub fn move_layer(&mut self, index: usize, up: bool) -> Result<usize, String> {
        let row = self.layers.get(index).ok_or("Select a layer")?;
        if matches!(row.copper_id, Some(TOP | BOTTOM)) {
            return Err("Outer copper layers cannot be moved".into());
        }
        let eligible = |i: &usize| {
            let other = &self.layers[*i];
            other.kind.copper() == row.kind.copper()
                && !matches!(other.copper_id, Some(TOP | BOTTOM))
                && (other.kind == LayerKind::SolderMask) == (row.kind == LayerKind::SolderMask)
        };
        let to = if up {
            (0..index).rev().find(eligible)
        } else {
            (index + 1..self.layers.len()).find(eligible)
        }
        .ok_or("No layer of the same type in that direction")?;
        let mut next = self.clone();
        if next.symmetric {
            let a = next.layers.len() - 1 - index;
            let b = next.layers.len() - 1 - to;
            if a == index || b == to {
                return Err("The symmetry reference cannot be moved".into());
            }
            next.layers.swap(index, to);
            if a != to {
                next.layers.swap(a, b);
            }
        } else {
            next.layers.swap(index, to);
        }
        next.validate()?;
        *self = next;
        Ok(to)
    }
}

fn copy_material(source: &Layer, target: &mut Layer) {
    target.kind = source.kind;
    target.thickness = source.thickness;
    target.material.clone_from(&source.material);
    target.er = source.er;
    target.loss_tangent = source.loss_tangent;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pcb::{Board, Item, Shape};
    use eda_geom::Point;

    #[test]
    fn symmetry_mirrors_properties_without_copying_identity_or_name() {
        let mut stack = Stack::from_board(&Board::default());
        stack.layers[2].thickness = 18_000;
        assert!(!stack.is_symmetric());
        let bottom = stack.layers[2].clone();
        stack.mirror_half(true).unwrap();
        assert_eq!(stack.layers[2].id, bottom.id);
        assert_eq!(stack.layers[2].name, bottom.name);
        assert_eq!(stack.layers[2].copper_id, Some(BOTTOM));
        let mut top = stack.layers[0].clone();
        top.thickness = 70_000;
        top.name = "RF signals".into();
        stack.edit_layer(0, top).unwrap();
        assert_eq!(stack.layers[2].thickness, 70_000);
        assert_eq!(stack.layers[2].name, bottom.name);
        assert!(stack.is_symmetric());
    }

    #[test]
    fn paired_insertion_preserves_dielectric_thickness_and_unique_ids() {
        for thickness in [6, 7, 8, 1_530_001] {
            let mut stack = Stack::from_board(&Board::default());
            stack.layers[1].thickness = thickness;
            stack.mirror_half(true).unwrap();
            stack.insert_copper(0, LayerKind::Signal).unwrap();
            assert_eq!(stack.copper().len(), 4);
            assert!(stack.is_symmetric());
            assert_eq!(
                stack
                    .layers
                    .iter()
                    .filter(|l| !l.kind.copper())
                    .map(|l| l.thickness)
                    .sum::<i64>(),
                thickness
            );
            if thickness > 8 {
                stack.insert_copper(1, LayerKind::Plane).unwrap();
                assert_eq!(stack.copper().len(), 6);
                assert!(stack.is_symmetric());
                assert_eq!(
                    stack
                        .layers
                        .iter()
                        .filter(|l| !l.kind.copper())
                        .map(|l| l.thickness)
                        .sum::<i64>(),
                    thickness
                );
                stack.validate().unwrap();
            }
        }
    }

    #[test]
    fn failed_edits_are_atomic_and_used_layers_cannot_apply_removal() {
        let mut stack = Stack::from_board(&Board::default());
        stack.mirror_half(true).unwrap();
        stack.insert_copper(1, LayerKind::Signal).unwrap();
        let mut board = stack.apply(&Board::default()).unwrap();
        let id = stack.layers[2].copper_id.unwrap();
        board.items.push(Item::new(
            id,
            None,
            Shape::Track {
                start: Point::mm(0, 0),
                end: Point::mm(1, 1),
                width: 100_000,
            },
        ));
        let before = stack.clone();
        assert!(stack.remove_layer(0).is_err());
        assert_eq!(stack, before);
        assert!(stack.remove_layer(3).is_err());
        assert_eq!(stack, before);
        let mut invalid = stack.layers[2].clone();
        invalid.thickness = 0;
        assert!(stack.edit_layer(2, invalid).is_err());
        assert_eq!(stack, before);
        stack.remove_layer(2).unwrap();
        assert_eq!(stack.copper().len(), 2);
        assert!(stack.apply(&board).unwrap_err().contains("used"));
    }

    #[test]
    fn moving_copper_keeps_separators_and_mirrors_material_order() {
        let mut stack = Stack::from_board(&Board::default());
        stack.mirror_half(true).unwrap();
        stack.insert_copper(1, LayerKind::Signal).unwrap();
        stack.insert_copper(1, LayerKind::Plane).unwrap();
        let before = stack.clone();
        let selected = stack.move_layer(2, false).unwrap();
        assert_eq!(selected, 4);
        assert_eq!(stack.layers[4].id, before.layers[2].id);
        assert!(stack.is_symmetric());
        stack.validate().unwrap();
        stack.move_layer(selected, true).unwrap();
        assert_eq!(stack, before);
    }

    #[test]
    fn symmetry_is_backward_compatible_and_preserved_by_history() {
        let original = Board::default();
        let mut stack = Stack::from_board(&original);
        let mut json = serde_json::to_value(&stack).unwrap();
        json.as_object_mut().unwrap().remove("symmetric");
        assert!(!serde_json::from_value::<Stack>(json).unwrap().symmetric);
        stack.mirror_half(true).unwrap();
        stack.insert_copper(1, LayerKind::Signal).unwrap();
        let mut document = crate::Document::new(crate::DocumentKind::Board, "stack");
        document.pcb = Some(original);
        let before = document.clone();
        let mut next = document.clone();
        next.pcb = Some(stack.apply(document.pcb.as_ref().unwrap()).unwrap());
        let mut history = crate::history::History::default();
        history.commit(&mut document, next.clone()).unwrap();
        history.undo(&mut document);
        assert_eq!(document, before);
        history.redo(&mut document);
        assert_eq!(document, next);
        assert_eq!(
            serde_json::from_slice::<Stack>(&serde_json::to_vec(&stack).unwrap()).unwrap(),
            stack
        );
    }

    #[test]
    fn dielectric_splits_preserve_symmetry_and_reject_sub_nanometre_rows() {
        let mut stack = Stack::from_board(&Board::default());
        stack.layers[1].thickness = 1_530_001;
        stack.mirror_half(true).unwrap();
        let thickness = stack.thickness();
        stack.insert_dielectric(0).unwrap();
        assert_eq!(stack.layers.len(), 5);
        assert_eq!(stack.thickness(), thickness);
        stack.insert_dielectric(1).unwrap();
        assert_eq!(stack.layers.len(), 7);
        assert_eq!(stack.thickness(), thickness);
        assert!(stack.is_symmetric());
        let mut tiny = Stack::from_board(&Board::default());
        tiny.layers[1].thickness = 1;
        let before = tiny.clone();
        assert!(tiny.insert_copper(0, LayerKind::Signal).is_err());
        assert!(tiny.insert_dielectric(0).is_err());
        assert_eq!(tiny, before);
    }
}
