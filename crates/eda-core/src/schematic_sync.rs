//! Sheet Entry/Port comparison and edits, independent of project paths and UI.
use crate::{
    Document, Point,
    schematic::{SchPrimitive, SchPrimitiveKind as K},
    schematic_edit::{self as edit, Edit, Fragment},
};
use uuid::Uuid;
#[cfg(test)]
#[path = "schematic_sync_tests.rs"]
mod tests;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Signal {
    pub id: Uuid,
    pub name: String,
    pub io: u32,
    pub harness: String,
}
impl Signal {
    pub fn read(p: &SchPrimitive) -> Option<Self> {
        let (name, io) = match &p.kind {
            K::SheetEntry { name, io_type, .. } | K::Port { name, io_type, .. } => (name, *io_type),
            _ => return None,
        };
        Some(Self {
            id: p.id,
            name: name.clone(),
            io,
            harness: p.presentation.harness_type.clone(),
        })
    }
    pub fn matches(&self, other: &Self) -> bool {
        self.name.eq_ignore_ascii_case(&other.name)
            && self.io == other.io
            && self.harness.eq_ignore_ascii_case(&other.harness)
    }
}
#[derive(Clone, Debug, Default)]
pub struct Comparison {
    pub entries: Vec<Signal>,
    pub ports: Vec<Signal>,
    pub links: Vec<(Signal, Signal)>,
}

pub fn compare(
    parent: &Document,
    child: &Document,
    symbol: Uuid,
    excluded: &[(Uuid, Uuid)],
) -> Result<Comparison, String> {
    let s = parent
        .schematic
        .as_ref()
        .ok_or("Parent schematic is unavailable")?;
    if !s
        .primitive(symbol)
        .is_some_and(|p| matches!(p.kind, K::SheetSymbol { .. }))
    {
        return Err("Sheet Symbol no longer exists".into());
    }
    let ports = child
        .schematic
        .as_ref()
        .ok_or("Child schematic is unavailable")?
        .primitives
        .iter()
        .filter(|p| matches!(p.kind, K::Port { .. }) && p.component.is_none())
        .filter_map(Signal::read)
        .collect();
    let mut out = Comparison {
        ports,
        ..Default::default()
    };
    for entry in s
        .primitives
        .iter()
        .filter(|p| p.presentation.parent == Some(symbol))
        .filter_map(Signal::read)
    {
        if let Some(n) = out
            .ports
            .iter()
            .position(|port| entry.matches(port) && !excluded.contains(&(entry.id, port.id)))
        {
            out.links.push((entry, out.ports.remove(n)));
        } else {
            out.entries.push(entry);
        }
    }
    Ok(out)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Destination {
    Entries(Uuid),
    Ports,
}
#[derive(Clone, Debug)]
pub struct Transfer {
    pub destination: Destination,
    pub updates: Vec<Edit>,
    pub create: Vec<Signal>,
}

pub fn transfer(
    target: &Document,
    destination: Destination,
    source: &[Signal],
    targets: &[Uuid],
) -> Result<Transfer, String> {
    if source.is_empty() {
        return Err("Select at least one source signal".into());
    }
    let s = target
        .schematic
        .as_ref()
        .ok_or("Target schematic is unavailable")?;
    let mut result = Transfer {
        destination,
        updates: vec![],
        create: vec![],
    };
    for (n, signal) in source.iter().enumerate() {
        if let Some(&id) = targets.get(n) {
            let mut p = s
                .primitive(id)
                .ok_or("Target signal no longer exists")?
                .clone();
            match (&mut p.kind, destination) {
                (K::SheetEntry { name, io_type, .. }, Destination::Entries(owner))
                    if p.presentation.parent == Some(owner) =>
                {
                    *name = signal.name.clone();
                    *io_type = signal.io;
                    p.presentation.name = signal.name.clone();
                }
                (K::Port { name, io_type, .. }, Destination::Ports) => {
                    *name = signal.name.clone();
                    *io_type = signal.io;
                    p.presentation.name = signal.name.clone();
                }
                _ => return Err("Signal belongs to a different sheet or symbol".into()),
            }
            p.presentation.harness_type = signal.harness.clone();
            result.updates.push(Edit::PrimitiveProperties {
                id,
                value: Box::new(p),
            });
        } else {
            result.create.push(signal.clone());
        }
    }
    Ok(result)
}

impl Transfer {
    /// Place missing counterparts as one group. Existing wires are never renamed or removed.
    pub fn edit_at(&self, target: &Document, at: Point) -> Result<Edit, String> {
        let s = target
            .schematic
            .as_ref()
            .ok_or("Target schematic is unavailable")?;
        let mut edits = self.updates.clone();
        if self.create.is_empty() {
            return Ok(Edit::Batch(edits));
        }
        if self.create.len() > 10_000 {
            return Err("Too many signals to place".into());
        }
        let font = s.sheet.system_font;
        let step = s.sheet.snap_nm.max(1_270_000);
        let mut fragment = Fragment {
            fonts: s.fonts.clone(),
            ..Default::default()
        };
        let edge = if let Destination::Entries(owner) = self.destination {
            let p = s.primitive(owner).ok_or("Sheet Symbol no longer exists")?;
            let K::SheetSymbol { a, b, .. } = p.kind else {
                return Err("Expected a Sheet Symbol".into());
            };
            let side = [
                (at.x - a.x).abs(),
                (at.x - b.x).abs(),
                (at.y - a.y).abs(),
                (at.y - b.y).abs(),
            ]
            .iter()
            .enumerate()
            .min_by_key(|(_, d)| *d)
            .unwrap()
            .0 as u32;
            let size = if side <= 1 { a.y - b.y } else { b.x - a.x };
            let spread = step
                .checked_mul(self.create.len().saturating_sub(1) as i64)
                .ok_or("Placement exceeds coordinate limits")?;
            if spread > size {
                return Err("The Sheet Symbol edge is too short for this group; place fewer entries or enlarge the symbol".into());
            }
            let offset = if side <= 1 { a.y - at.y } else { at.x - a.x };
            Some((owner, a, b, side, offset.clamp(0, size - spread)))
        } else {
            None
        };
        for (n, signal) in self.create.iter().enumerate() {
            let mut p = if let Some((owner, a, b, side, start)) = edge {
                let d = start + n as i64 * step;
                let position = match side {
                    0 => Point { x: a.x, y: a.y - d },
                    1 => Point { x: b.x, y: a.y - d },
                    2 => Point { x: a.x + d, y: a.y },
                    _ => Point { x: a.x + d, y: b.y },
                };
                let mut p = edit::primitive(
                    K::SheetEntry {
                        position,
                        side,
                        io_type: signal.io,
                        name: signal.name.clone(),
                        font,
                        text_color: 128,
                        arrow_kind: "Block & Triangle".into(),
                    },
                    None,
                );
                p.presentation.parent = Some(owner);
                p.presentation.record_type = 16;
                p.width_nm = 101_600;
                p
            } else {
                let y =
                    at.y.checked_sub(n as i64 * step)
                        .ok_or("Placement exceeds coordinate limits")?;
                let width =
                    (signal.name.chars().count() as i64 * 1_524_000 + 2_540_000).max(10_160_000);
                let mut p = edit::primitive(
                    K::Port {
                        position: Point { x: at.x, y },
                        width_nm: width,
                        height_nm: 2_540_000,
                        style: 0,
                        io_type: signal.io,
                        name: signal.name.clone(),
                        font,
                    },
                    None,
                );
                p.presentation.record_type = 18;
                p.presentation.alignment = Some(0);
                p.presentation.text_color = Some(128);
                p
            };
            p.color = 128;
            p.fill = Some(0x80ffff);
            p.presentation.allow_sync = true;
            p.presentation.name = signal.name.clone();
            p.presentation.harness_type = signal.harness.clone();
            fragment.primitives.push(p);
        }
        edits.push(Edit::Insert(fragment));
        let result = Edit::Batch(edits);
        edit::preview(target, &result)?;
        Ok(result)
    }
}
