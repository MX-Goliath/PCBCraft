//! Editable rigid-board fabrication stack. Lengths are integer nanometres.
use crate::pcb::{self, BOTTOM, Board, PcbLayer, Shape, TOP};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LayerKind {
    Signal,
    Plane,
    Core,
    Prepreg,
    SolderMask,
}
impl LayerKind {
    pub fn copper(self) -> bool {
        matches!(self, Self::Signal | Self::Plane)
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Layer {
    pub id: Uuid,
    pub name: String,
    pub kind: LayerKind,
    pub copper_id: Option<u16>,
    pub thickness: i64,
    pub material: String,
    pub er: f64,
    pub loss_tangent: f64,
}
impl Layer {
    pub fn dielectric() -> Self {
        Self {
            id: Uuid::new_v4(),
            name: "Dielectric".into(),
            kind: LayerKind::Core,
            copper_id: None,
            thickness: 200_000,
            material: "FR-4".into(),
            er: 4.2,
            loss_tangent: 0.02,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ViaKind {
    Through,
    BlindBuried,
    Micro,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ViaType {
    pub id: Uuid,
    pub name: String,
    pub kind: ViaKind,
    pub start: u16,
    pub end: u16,
    pub diameter: i64,
    pub hole: i64,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Impedance {
    pub id: Uuid,
    pub name: String,
    pub differential: bool,
    pub target: f64,
    pub tolerance: f64,
    pub signal: u16,
    pub reference: u16,
    pub reference2: Option<u16>,
    pub width: i64,
    pub gap: i64,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Stack {
    pub layers: Vec<Layer>,
    pub vias: Vec<ViaType>,
    pub impedances: Vec<Impedance>,
}
impl Stack {
    /// Reads V9 stack rows without changing the source; old files get explicit defaults.
    pub fn from_board(board: &Board) -> Self {
        if let Some(stack) = &board.layer_stack {
            return stack.clone();
        }
        let f = &board.properties;
        let mut rows = std::collections::BTreeSet::new();
        for key in f.keys() {
            if let Some(s) = key.strip_prefix("V9_STACK_LAYER")
                && let Some((n, _)) = s.split_once('_')
                && let Ok(n) = n.parse::<usize>()
            {
                rows.insert(n);
            }
        }
        let mut layers = vec![];
        for n in rows {
            let prefix = format!("V9_STACK_LAYER{n}_");
            let value = |key: &str| f.get(&format!("{prefix}{key}"));
            let raw = value("LAYERID").and_then(|s| s.parse::<u32>().ok());
            let cid = raw
                .and_then(pcb::native_layer)
                .filter(|id| pcb::copper_layer(*id));
            let kind = if cid.is_some() {
                LayerKind::Signal
            } else {
                match value("DIELTYPE").map(String::as_str) {
                    Some("1") => LayerKind::Core,
                    Some("2") => LayerKind::Prepreg,
                    Some("3") => LayerKind::SolderMask,
                    _ => continue,
                }
            };
            let mut l = Layer::dielectric();
            l.id = value("ID")
                .and_then(|s| Uuid::parse_str(s.trim_matches(['{', '}'])).ok())
                .unwrap_or(l.id);
            l.name = value("NAME").cloned().unwrap_or(l.name);
            l.kind = kind;
            l.copper_id = cid;
            l.thickness = value(if kind.copper() {
                "COPTHICK"
            } else {
                "DIELHEIGHT"
            })
            .and_then(|s| pcb::dimension_nm(s).ok())
            .unwrap_or(if kind.copper() { 35_000 } else { 200_000 });
            l.material = value("DIELMATERIAL").cloned().unwrap_or_else(|| {
                if kind.copper() {
                    "Copper".into()
                } else {
                    l.material.clone()
                }
            });
            l.er = value("DIELCONST")
                .and_then(|s| s.parse().ok())
                .unwrap_or(4.2);
            l.loss_tangent = value("DIELLOSSTANGENT")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0.02);
            layers.push(l);
        }
        let mut copper: Vec<_> = board.layers.iter().filter(|l| l.copper).collect();
        copper.sort_by_key(|l| l.order);
        if layers
            .iter()
            .filter_map(|l| l.copper_id)
            .collect::<HashSet<_>>()
            != copper.iter().map(|l| l.id).collect()
            || layers.is_empty()
        {
            layers.clear();
            for (i, c) in copper.iter().enumerate() {
                if i > 0 {
                    let mut l = Layer::dielectric();
                    l.thickness = 1_530_000 / (copper.len().saturating_sub(1).max(1) as i64);
                    l.name = format!("Dielectric {i}");
                    layers.push(l);
                }
                let mut l = Layer::dielectric();
                l.name = c.name.clone();
                l.kind = if (39..=54).contains(&c.id) {
                    LayerKind::Plane
                } else {
                    LayerKind::Signal
                };
                l.copper_id = Some(c.id);
                l.thickness = 35_000;
                l.material = "Copper".into();
                layers.push(l);
            }
        }
        Self {
            layers,
            vias: vec![],
            impedances: vec![],
        }
    }
    pub fn copper(&self) -> Vec<&Layer> {
        self.layers.iter().filter(|l| l.kind.copper()).collect()
    }
    pub fn thickness(&self) -> i64 {
        self.layers
            .iter()
            .filter(|l| l.kind != LayerKind::SolderMask)
            .map(|l| l.thickness)
            .sum()
    }
    pub fn add_copper(&mut self, after: usize) -> Result<(), String> {
        let id = (2..32)
            .find(|id| !self.layers.iter().any(|l| l.copper_id == Some(*id)))
            .ok_or("All 32 signal layer identities are used")?;
        let top = self
            .layers
            .iter()
            .position(|l| l.copper_id == Some(TOP))
            .ok_or("Top layer missing")?;
        let bottom = self
            .layers
            .iter()
            .position(|l| l.copper_id == Some(BOTTOM))
            .ok_or("Bottom layer missing")?;
        let at = (after + 1).clamp(top + 1, bottom);
        let mut l = Layer::dielectric();
        l.name = pcb::layer_name(id);
        l.kind = LayerKind::Signal;
        l.copper_id = Some(id);
        l.material = "Copper".into();
        l.thickness = 35_000;
        // Split a dielectric when possible; otherwise insert a new dielectric on each side.
        if !self.layers[at - 1].kind.copper() {
            let mut d = self.layers[at - 1].clone();
            d.id = Uuid::new_v4();
            d.thickness = (d.thickness / 2).max(1);
            self.layers[at - 1].thickness -= d.thickness;
            self.layers.splice(at..at, [l, d]);
        } else {
            self.layers
                .splice(at..at, [Layer::dielectric(), l, Layer::dielectric()]);
        }
        Ok(())
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.layers.len() > 256 || self.vias.len() > 256 || self.impedances.len() > 256 {
            return Err("Stack table limit is 256 rows".into());
        }
        let mut ids = HashSet::new();
        let mut copper = HashSet::new();
        for l in &self.layers {
            if !ids.insert(l.id)
                || l.name.trim().is_empty()
                || !(1..=100_000_000).contains(&l.thickness)
                || !l.er.is_finite()
                || !(1.0..=100.0).contains(&l.er)
                || !l.loss_tangent.is_finite()
                || !(0.0..=1.0).contains(&l.loss_tangent)
            {
                return Err("Invalid layer name, identity, thickness, Dk or Df".into());
            }
            if l.kind.copper() {
                if l.copper_id
                    .is_none_or(|id| !pcb::copper_layer(id) || !copper.insert(id))
                {
                    return Err("Copper layers need unique valid identities".into());
                }
            } else if l.copper_id.is_some() {
                return Err("Dielectric cannot have a copper identity".into());
            }
        }
        let c = self.copper();
        if c.first().and_then(|l| l.copper_id) != Some(TOP)
            || c.last().and_then(|l| l.copper_id) != Some(BOTTOM)
        {
            return Err("Top and Bottom must be the outer copper layers".into());
        }
        if self
            .layers
            .windows(2)
            .any(|p| p[0].kind.copper() && p[1].kind.copper())
        {
            return Err("Copper layers must be separated by dielectric".into());
        }
        let position = |id| {
            c.iter()
                .position(|l| l.copper_id == Some(id))
                .ok_or("Unknown copper layer")
        };
        let mut names = HashSet::new();
        for v in &self.vias {
            if !ids.insert(v.id)
                || v.name.trim().is_empty()
                || !names.insert(v.name.to_lowercase())
                || v.hole <= 0
                || v.diameter <= v.hole
                || v.diameter > 100_000_000
            {
                return Err("Via types need unique names and diameter greater than drill".into());
            }
            let a = position(v.start)?;
            let b = position(v.end)?;
            if a >= b {
                return Err("Via start must precede its end in the stack".into());
            }
            match v.kind {
                ViaKind::Through if a != 0 || b != c.len() - 1 => {
                    return Err("Through via must span Top to Bottom".into());
                }
                ViaKind::BlindBuried if a == 0 && b == c.len() - 1 => {
                    return Err("Use Through for a full-stack via".into());
                }
                ViaKind::Micro if b != a + 1 => {
                    return Err("Microvia must connect adjacent copper layers".into());
                }
                _ => {}
            }
        }
        names.clear();
        for p in &self.impedances {
            if !ids.insert(p.id)
                || p.name.trim().is_empty()
                || !names.insert(p.name.to_lowercase())
                || !p.target.is_finite()
                || !(1.0..=1000.0).contains(&p.target)
                || !p.tolerance.is_finite()
                || !(0.0..=50.0).contains(&p.tolerance)
                || p.width <= 0
                || p.width > 100_000_000
                || p.gap <= 0
                || p.gap > 100_000_000
            {
                return Err("Invalid impedance profile name, target, tolerance or geometry".into());
            }
            let s = position(p.signal)?;
            let r = position(p.reference)?;
            if s == r {
                return Err("Signal and reference must be different layers".into());
            }
            if let Some(r2) = p.reference2 {
                let r2 = position(r2)?;
                if !(r.min(r2) < s && s < r.max(r2)) {
                    return Err("Stripline references must surround the signal layer".into());
                }
            }
        }
        Ok(())
    }
    pub fn validate_for_board(&self, board: &Board) -> Result<(), String> {
        self.validate()?;
        let mut actual: Vec<_> = board.layers.iter().filter(|l| l.copper).collect();
        actual.sort_by_key(|l| l.order);
        let expected = self.copper();
        if actual.len() != expected.len()
            || actual
                .iter()
                .zip(expected)
                .any(|(a, b)| Some(a.id) != b.copper_id || a.name != b.name)
        {
            return Err("Board layer identities/order differ from fabrication stack".into());
        }
        Ok(())
    }
    pub fn apply(&self, board: &Board) -> Result<Board, String> {
        self.validate()?;
        let present: HashSet<_> = self.copper().iter().filter_map(|l| l.copper_id).collect();
        for old in board
            .layers
            .iter()
            .filter(|l| l.copper && !present.contains(&l.id))
        {
            let id = old.id;
            if board.items.iter().any(|i|i.layer==id || matches!(i.shape,Shape::Via{from_layer,to_layer,..} if from_layer==id||to_layer==id) || i.pad_stack.as_ref().is_some_and(|s|s.layers.contains_key(&id))) || board.components.iter().any(|c|c.layer==id) || board.polygon_pours.iter().any(|p|p.layer==id) { return Err(format!("Layer {} is used by board objects",old.name)); }
        }
        let mut b = board.clone();
        b.layers.retain(|l| !l.copper);
        for (order, l) in self.copper().iter().enumerate() {
            b.layers.push(PcbLayer {
                id: l.copper_id.unwrap(),
                name: l.name.clone(),
                copper: true,
                order: order as u16,
            });
        }
        let n = self.copper().len() as u16;
        for l in b.layers.iter_mut().filter(|l| !l.copper) {
            l.order = n.saturating_add(l.id);
        }
        b.layer_stack = Some(self.clone());
        b.pours_stale = true;
        b.validate()?;
        Ok(b)
    }
    /// Quasi-static engineering estimate, not a field solver. No solder-mask correction.
    pub fn estimate(&self, p: &Impedance) -> Result<f64, String> {
        let index = |id| {
            self.layers
                .iter()
                .position(|l| l.copper_id == Some(id))
                .ok_or("Unknown layer")
        };
        let s = index(p.signal)?;
        let r = index(p.reference)?;
        let dielectric = |a: usize, b: usize| -> Result<(f64, f64), String> {
            let rows = &self.layers[a.min(b) + 1..a.max(b)];
            if rows.is_empty() || rows.iter().any(|l| l.kind.copper()) {
                return Err(
                    "Estimate requires adjacent reference planes without intervening copper".into(),
                );
            }
            let h: f64 = rows.iter().map(|l| l.thickness as f64).sum();
            Ok((
                h,
                rows.iter().map(|l| l.er * l.thickness as f64).sum::<f64>() / h,
            ))
        };
        if s == r {
            return Err("Signal equals reference".into());
        }
        let (h, er) = dielectric(s, r)?;
        let t = self.layers[s].thickness as f64;
        let w = p.width as f64;
        if h <= 0.0 || w <= 0.0 {
            return Err("Non-positive geometry".into());
        }
        let z = if let Some(r2) = p.reference2 {
            let r2 = index(r2)?;
            if !(r.min(r2) < s && s < r.max(r2)) {
                return Err("References must surround signal".into());
            }
            let (h2, er2) = dielectric(s, r2)?;
            if (h - h2).abs() > 0.05 * h || (er - er2).abs() > 0.05 * er {
                return Err("Estimate supports symmetric stripline only".into());
            }
            60.0 / er.sqrt()
                * (4.0 * (h + h2 + t) / (0.67 * std::f64::consts::PI * (0.8 * w + t))).ln()
        } else {
            let c = self.copper();
            if c.first().and_then(|l| l.copper_id) != Some(p.signal)
                && c.last().and_then(|l| l.copper_id) != Some(p.signal)
            {
                return Err("Internal signal requires two reference planes".into());
            }
            87.0 / (er + 1.41).sqrt() * (5.98 * h / (0.8 * w + t)).ln()
        };
        let z = if p.differential {
            2.0 * z * (1.0 - 0.48 * (-0.96 * p.gap as f64 / h).exp())
        } else {
            z
        };
        if !z.is_finite() || z <= 0.0 {
            Err("Geometry outside approximation range".into())
        } else {
            Ok(z)
        }
    }
    pub fn solve_width(&self, p: &Impedance) -> Result<i64, String> {
        let mut p = p.clone();
        let target = p.target;
        let mut lo = 1_000;
        let mut hi = 20_000_000;
        p.width = lo;
        if self.estimate(&p)? < target {
            return Err("Target exceeds width search range".into());
        }
        for _ in 0..40 {
            let m = (lo + hi) / 2;
            p.width = m;
            match self.estimate(&p) {
                Ok(z) if z > target => lo = m + 1,
                _ => hi = m,
            }
            if lo >= hi {
                break;
            }
        }
        p.width = hi;
        if (self.estimate(&p)? - target).abs() > 0.1 {
            return Err("Target cannot be solved within approximation".into());
        }
        Ok(hi)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn profile() -> Impedance {
        Impedance {
            id: Uuid::new_v4(),
            name: "50R".into(),
            differential: false,
            target: 50.,
            tolerance: 10.,
            signal: TOP,
            reference: BOTTOM,
            reference2: None,
            width: 200_000,
            gap: 200_000,
        }
    }
    #[test]
    fn add_layers_preserves_ids_and_apply_undo_roundtrip() {
        let b = Board::default();
        let mut s = Stack::from_board(&b);
        s.add_copper(1).unwrap();
        s.validate().unwrap();
        assert_eq!(s.copper().len(), 3);
        assert_eq!(s.copper()[1].copper_id, Some(2));
        let mut doc = crate::Document::new(crate::DocumentKind::Board, "stack");
        doc.pcb = Some(b.clone());
        let before = doc.clone();
        let mut next = doc.clone();
        next.pcb = Some(s.apply(&b).unwrap());
        let mut history = crate::history::History::default();
        history.commit(&mut doc, next.clone()).unwrap();
        assert_eq!(
            doc.pcb.as_ref().unwrap().stack_thickness_nm(),
            s.thickness()
        );
        history.undo(&mut doc);
        assert_eq!(doc, before);
        history.redo(&mut doc);
        assert_eq!(doc, next);
        let json = serde_json::to_vec(&doc).unwrap();
        assert_eq!(
            serde_json::from_slice::<crate::Document>(&json).unwrap(),
            doc
        );
    }
    #[test]
    fn referenced_layer_and_invalid_spans_are_rejected() {
        let mut s = Stack::from_board(&Board::default());
        s.add_copper(1).unwrap();
        let mut b = s.apply(&Board::default()).unwrap();
        b.items.push(pcb::Item::new(
            2,
            None,
            Shape::Track {
                start: eda_geom::Point::mm(1, 1),
                end: eda_geom::Point::mm(2, 2),
                width: 200_000,
            },
        ));
        s.layers.retain(|l| l.copper_id != Some(2));
        assert!(s.apply(&b).unwrap_err().contains("used"));
        s.vias.push(ViaType {
            id: Uuid::new_v4(),
            name: "via".into(),
            kind: ViaKind::Through,
            start: BOTTOM,
            end: TOP,
            diameter: 600_000,
            hole: 300_000,
        });
        assert!(s.validate().is_err());
        s.vias[0].start = TOP;
        s.vias[0].end = BOTTOM;
        s.validate().unwrap();
        s.add_copper(1).unwrap();
        s.vias[0].kind = ViaKind::Micro;
        assert!(s.validate().unwrap_err().contains("adjacent"));
        s.vias[0].end = 2;
        s.validate().unwrap();
        s.vias[0].hole = s.vias[0].diameter;
        assert!(s.validate().is_err());
    }
    #[test]
    fn impedance_estimate_and_width_solver_have_physical_response() {
        let mut s = Stack::from_board(&Board::default());
        s.layers[1].thickness = 200_000;
        let mut p = profile();
        let z = s.estimate(&p).unwrap();
        assert!((z - 66.6).abs() < 1.0);
        p.width = 400_000;
        assert!(s.estimate(&p).unwrap() < z);
        p.width = s.solve_width(&p).unwrap();
        assert!((s.estimate(&p).unwrap() - 50.).abs() < 0.1);
        p.differential = true;
        p.target = 100.;
        p.width = s.solve_width(&p).unwrap();
        assert!((s.estimate(&p).unwrap() - 100.).abs() < 0.1);
        p.reference = TOP;
        assert!(s.estimate(&p).is_err());
        s.impedances.push(p);
        assert!(s.validate().is_err());
    }
    #[test]
    fn native_stack_rows_use_native_order_material_and_units() {
        let mut b = Board::default();
        for (n, id, name, t) in [
            (0, "16777217", "Top", "1.4mil"),
            (2, "16842751", "Bottom", "1.4mil"),
        ] {
            for (key, value) in [("LAYERID", id), ("NAME", name), ("COPTHICK", t)] {
                b.properties
                    .insert(format!("V9_STACK_LAYER{n}_{key}"), value.into());
            }
        }
        for (key, value) in [
            ("LAYERID", "17039361"),
            ("NAME", "Core"),
            ("DIELTYPE", "1"),
            ("DIELHEIGHT", "1.2mm"),
            ("DIELCONST", "4.6"),
            ("DIELMATERIAL", "FR408"),
        ] {
            b.properties
                .insert(format!("V9_STACK_LAYER1_{key}"), value.into());
        }
        let s = Stack::from_board(&b);
        assert_eq!(s.layers.len(), 3);
        assert_eq!(s.layers[0].thickness, 35_560);
        assert_eq!(s.layers[1].thickness, 1_200_000);
        assert_eq!(s.layers[1].material, "FR408");
        assert_eq!(s.layers[1].er, 4.6);
        s.validate().unwrap();
        assert!(b.layer_stack.is_none());
    }
    #[test]
    fn invalid_material_and_dangling_profile_cannot_commit() {
        let mut s = Stack::from_board(&Board::default());
        s.layers[1].er = f64::NAN;
        assert!(s.apply(&Board::default()).is_err());
        s.layers[1].er = 4.;
        let mut p = profile();
        p.reference = 12;
        s.impedances.push(p);
        assert!(s.validate().is_err());
    }
}
