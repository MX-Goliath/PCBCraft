//! Logical schematic annotation. Planning is read-only; applying preserves component identities.
use crate::{
    Document, DocumentKind,
    schematic::{Component, SchPrimitiveKind, TextRole},
};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Order {
    DownAcross,
    AcrossDown,
    UpAcross,
    AcrossUp,
}
impl Order {
    pub const ALL: [Self; 4] = [
        Self::DownAcross,
        Self::AcrossDown,
        Self::UpAcross,
        Self::AcrossUp,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::DownAcross => "Down Then Across",
            Self::AcrossDown => "Across Then Down",
            Self::UpAcross => "Up Then Across",
            Self::AcrossUp => "Across Then Up",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operation {
    Update,
    ResetAll,
    ResetDuplicates,
}
#[derive(Clone, Debug)]
pub struct Options {
    pub order: Order,
    pub reset_existing: bool,
    pub pack_parts: bool,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            order: Order::DownAcross,
            reset_existing: false,
            pack_parts: true,
        }
    }
}
pub struct Sheet<'a> {
    pub document: &'a Document,
    pub enabled: bool,
    pub start: u32,
    pub selected: Option<&'a HashSet<Uuid>>,
}
#[derive(Clone, Debug)]
pub struct Change {
    pub document: Uuid,
    pub component: Uuid,
    pub sheet: String,
    pub current: String,
    pub proposed: String,
    pub locked: bool,
    pub eligible: bool,
    pub package: usize,
    pub part: u32,
    pub library: String,
}
#[derive(Clone, Debug, Default)]
pub struct Plan {
    pub changes: Vec<Change>,
}
struct Candidate<'a> {
    sheet: usize,
    c: &'a Component,
    prefix: String,
    number: Option<u32>,
    suffix: String,
    eligible: bool,
    compatible: String,
}
struct Package {
    members: Vec<usize>,
    base: Option<String>,
    fixed: bool,
}
fn designation(c: &Component, value: &str) -> Result<(String, Option<u32>, String), String> {
    let mut base = value.trim();
    let mut suffix = String::new();
    if c.settings.part_count > 1 {
        if !(1..=26).contains(&c.part) {
            return Err(format!(
                "{}: section numbering beyond A–Z is unsupported",
                c.designator
            ));
        }
        suffix.push(char::from_u32(64 + c.part).unwrap());
        if let Some(s) = base.strip_suffix(&suffix) {
            base = s;
        } else if !base.ends_with('?') {
            return Err(format!(
                "{}: multipart designator does not match section {}",
                value, suffix
            ));
        }
    }
    let prefix = base.trim_end_matches(|ch: char| ch.is_ascii_digit() || ch == '?');
    let tail = &base[prefix.len()..];
    if prefix.is_empty()
        || prefix
            .chars()
            .any(|ch| ch.is_whitespace() || ch.is_control() || ch == '?')
    {
        return Err(format!(
            "Unsupported designator '{value}'; use a prefix followed by a number or ?"
        ));
    }
    let number = if tail == "?" {
        None
    } else {
        Some(
            tail.parse::<u32>()
                .ok()
                .filter(|n| *n > 0 && *n <= 999_999)
                .ok_or_else(|| format!("Invalid designator '{value}'"))?,
        )
    };
    Ok((prefix.into(), number, suffix))
}
fn reservation(base: &str) -> String {
    let prefix = base.trim_end_matches(|c: char| c.is_ascii_digit());
    match base[prefix.len()..].parse::<u32>() {
        Ok(n) => format!("{}{n}", prefix.to_lowercase()),
        Err(_) => base.to_lowercase(),
    }
}
fn compatibility(c: &Component) -> String {
    // Do not pack visually similar but electrically different library components.
    format!(
        "{}\0{}\0{}\0{}\0{}\0{:?}\0{:?}",
        c.library_reference,
        c.settings.design_item_id,
        c.settings.source,
        c.settings.part_count,
        c.comment,
        c.settings.models,
        c.parameters
    )
}
pub fn plan(sheets: &[Sheet<'_>], options: &Options, operation: Operation) -> Result<Plan, String> {
    if sheets.is_empty() {
        return Err("No schematic sheets in scope".into());
    }
    let mut document_ids = HashSet::new();
    let mut rows = vec![];
    for (sheet, s) in sheets.iter().enumerate() {
        if s.document.kind != DocumentKind::Schematic || !document_ids.insert(s.document.id) {
            return Err("Scope must contain distinct schematic documents".into());
        }
        if s.start == 0 || s.start > 999_999 {
            return Err("Start index must be between 1 and 999999".into());
        }
        let schematic = s
            .document
            .schematic
            .as_ref()
            .ok_or("Schematic model missing")?;
        let mut components: Vec<_> = schematic
            .components
            .iter()
            .filter(|c| c.settings.kind != 2)
            .collect();
        components.sort_by_key(|c| {
            let x = c.position.x;
            let y = c.position.y;
            let key = match options.order {
                Order::DownAcross => (x, -y),
                Order::AcrossDown => (-y, x),
                Order::UpAcross => (x, y),
                Order::AcrossUp => (y, x),
            };
            (key, c.id)
        });
        for c in components {
            let (prefix, number, suffix) = designation(c, &c.designator)?;
            rows.push(Candidate {
                sheet,
                c,
                prefix,
                number,
                suffix,
                eligible: s.enabled
                    && s.selected.is_none_or(|set| set.contains(&c.id))
                    && !c.settings.designator_locked,
                compatible: compatibility(c),
            });
        }
    }
    if rows.len() > 100_000 {
        return Err("Annotation scope exceeds 100000 components".into());
    }
    let mut groups: Vec<Package> = vec![];
    let mut available: HashMap<(String, String, u32), std::collections::VecDeque<usize>> =
        HashMap::new();
    for (i, r) in rows.iter().enumerate() {
        let base = r.number.map(|_| {
            r.c.designator
                .strip_suffix(&r.suffix)
                .unwrap_or(&r.c.designator)
                .to_owned()
        });
        let key = base
            .as_ref()
            .map(|s| reservation(s))
            .unwrap_or_else(|| format!("{}?{}", r.prefix.to_lowercase(), r.eligible));
        let pack = r.c.settings.part_count > 1 && (r.number.is_some() || options.pack_parts);
        let group = if pack {
            available
                .get_mut(&(key.clone(), r.compatible.clone(), r.c.part))
                .and_then(|q| q.pop_front())
        } else {
            None
        };
        if let Some(g) = group {
            groups[g].members.push(i);
            groups[g].fixed |= !r.eligible;
        } else {
            let g = groups.len();
            groups.push(Package {
                members: vec![i],
                base,
                fixed: !r.eligible,
            });
            if pack {
                for part in 1..=r.c.settings.part_count.min(26) {
                    if part != r.c.part {
                        available
                            .entry((key.clone(), r.compatible.clone(), part))
                            .or_default()
                            .push_back(g);
                    }
                }
            }
        }
    }
    // Fixed packages reserve first, even when they occur later in traversal.
    let mut order: Vec<_> = (0..groups.len()).collect();
    order.sort_by_key(|g| (!groups[*g].fixed, *g));
    let mut reserved = HashSet::new();
    let mut assigned: Vec<Option<String>> = vec![None; groups.len()];
    let mut duplicates = HashSet::new();
    for g in order {
        let p = &groups[g];
        let reset = options.reset_existing || operation == Operation::ResetAll;
        if let Some(base) = &p.base
            && (p.fixed || !reset)
        {
            if !reserved.insert(reservation(base)) {
                if p.fixed {
                    return Err(format!(
                        "Duplicate fixed/locked designator {base}; resolve the collision or include/unlock its components"
                    ));
                }
                duplicates.insert(g);
            } else {
                assigned[g] = Some(base.clone());
            }
        }
    }
    let mut result = Plan::default();
    let mut cursors: HashMap<(String, u32), u32> = HashMap::new();
    for (g, p) in groups.iter().enumerate() {
        let first = &rows[p.members[0]];
        let reset = operation == Operation::ResetAll
            || operation == Operation::ResetDuplicates && duplicates.contains(&g);
        let base = if p.fixed {
            p.base
                .clone()
                .unwrap_or_else(|| format!("{}?", first.prefix))
        } else if reset {
            format!("{}?", first.prefix)
        } else if operation == Operation::ResetDuplicates {
            p.base
                .clone()
                .unwrap_or_else(|| format!("{}?", first.prefix))
        } else if let Some(base) = &assigned[g] {
            base.clone()
        } else {
            let start = sheets[first.sheet].start;
            let cursor = cursors
                .entry((first.prefix.to_lowercase(), start))
                .or_insert(start);
            let n = (*cursor..=999_999)
                .find(|n| !reserved.contains(&format!("{}{n}", first.prefix).to_lowercase()))
                .ok_or("No available designator in range")?;
            *cursor = n + 1;
            let base = format!("{}{n}", first.prefix);
            reserved.insert(base.to_lowercase());
            base
        };
        for i in &p.members {
            let r = &rows[*i];
            result.changes.push(Change {
                document: sheets[r.sheet].document.id,
                component: r.c.id,
                sheet: sheets[r.sheet].document.name.clone(),
                current: r.c.designator.clone(),
                proposed: if p.fixed {
                    r.c.designator.clone()
                } else {
                    format!("{base}{}", r.suffix)
                },
                locked: r.c.settings.designator_locked,
                eligible: !p.fixed,
                package: g,
                part: r.c.part,
                library: r.c.library_reference.clone(),
            });
        }
    }
    Ok(result)
}
impl Plan {
    pub fn changed(&self) -> usize {
        self.changes
            .iter()
            .filter(|r| r.current != r.proposed)
            .count()
    }
    /// Recheck user overrides and all source identities before producing any documents.
    pub fn prepare(&self, documents: &[&Document]) -> Result<Vec<Document>, String> {
        let docs: HashMap<_, _> = documents.iter().map(|d| (d.id, *d)).collect();
        if docs.len() != documents.len() {
            return Err("Duplicate document in annotation scope".into());
        }
        let components: HashMap<_, _> = documents
            .iter()
            .filter_map(|d| d.schematic.as_ref().map(|s| (d.id, s)))
            .flat_map(|(id, s)| s.components.iter().map(move |c| ((id, c.id), c)))
            .collect();
        let mut seen = HashSet::new();
        let mut packages: BTreeMap<usize, String> = BTreeMap::new();
        let mut names: HashMap<String, usize> = HashMap::new();
        for r in &self.changes {
            if !seen.insert((r.document, r.component)) {
                return Err("Duplicate annotation row".into());
            }
            let d = docs
                .get(&r.document)
                .ok_or("Schematic document was closed")?;
            let c = components
                .get(&(d.id, r.component))
                .ok_or("Component no longer exists")?;
            if c.designator != r.current {
                return Err(format!("{} changed after annotation preview", r.current));
            }
            if r.current != r.proposed && (!r.eligible || r.locked || c.settings.designator_locked)
            {
                return Err(format!("{} is locked or excluded from scope", r.current));
            }
            let (prefix, n, _) = designation(c, &r.proposed)?;
            let base = n
                .map(|n| format!("{prefix}{n}").to_lowercase())
                .unwrap_or_else(|| format!("{prefix}?").to_lowercase());
            if let Some(prior) = packages.insert(r.package, base.clone())
                && prior != base
            {
                return Err("All sections of a package must use the same designator number".into());
            }
            if n.is_some()
                && let Some(other) = names.insert(base.clone(), r.package)
                && other != r.package
            {
                return Err(format!("Duplicate proposed designator {base}"));
            }
        }
        // Every component must participate, including excluded sheets which reserve their names.
        for d in documents {
            for c in d
                .schematic
                .as_ref()
                .ok_or("Not a schematic")?
                .components
                .iter()
                .filter(|c| c.settings.kind != 2)
            {
                if !seen.contains(&(d.id, c.id)) {
                    return Err("Annotation scope changed; rebuild the preview".into());
                }
            }
        }
        let mut results = vec![];
        for d in documents {
            let changes: HashMap<_, _> = self
                .changes
                .iter()
                .filter(|r| r.document == d.id && r.current != r.proposed)
                .map(|r| (r.component, r.proposed.as_str()))
                .collect();
            if changes.is_empty() {
                continue;
            }
            let mut next = (*d).clone();
            let s = next.schematic.as_mut().ok_or("Not a schematic")?;
            for c in &mut s.components {
                if let Some(value) = changes.get(&c.id) {
                    c.designator = (*value).into();
                }
            }
            let native_designators: BTreeSet<_> = s
                .records
                .iter()
                .filter(|r| r.record_type == 34)
                .map(|r| r.index)
                .collect();
            for p in s.primitives.iter_mut().chain(&mut s.parameter_objects) {
                if let Some(value) = p.component.and_then(|id| changes.get(&id))
                    && (p.text_role == Some(TextRole::Designator)
                        || native_designators.contains(&p.source_index))
                    && let SchPrimitiveKind::Text { text, .. } = &mut p.kind
                {
                    *text = (*value).into();
                    p.presentation.raw_text = Some((*value).into());
                    p.text_role = Some(TextRole::Designator);
                }
            }
            if s.source.is_some() {
                s.source_modified = true;
            }
            next.validate()?;
            results.push(next);
        }
        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Point,
        schematic::{ComponentSettings, SchPrimitiveKind},
        schematic_edit,
    };
    fn add(d: &mut Document, name: &str, x: i64, y: i64) -> Uuid {
        let id = Uuid::new_v4();
        let s = d.schematic.as_mut().unwrap();
        s.components.push(Component {
            settings: ComponentSettings::default(),
            id,
            source_index: u32::MAX,
            position: Point::mm(x, y),
            designator: name.into(),
            comment: "10k".into(),
            library_reference: "R".into(),
            description: String::new(),
            part: 1,
            display_mode: 0,
            parameters: Default::default(),
        });
        let mut p = schematic_edit::primitive(
            SchPrimitiveKind::Text {
                position: Point::mm(x, y),
                text: name.into(),
                font: 0,
                orientation: 0,
                justification: 0,
            },
            Some(id),
        );
        p.text_role = Some(TextRole::Designator);
        p.presentation.raw_text = Some(name.into());
        s.primitives.push(p);
        id
    }
    fn sheet(d: &Document) -> Sheet<'_> {
        Sheet {
            document: d,
            enabled: true,
            start: 1,
            selected: None,
        }
    }
    fn next(p: &Plan, id: Uuid) -> &str {
        &p.changes
            .iter()
            .find(|r| r.component == id)
            .unwrap()
            .proposed
    }
    #[test]
    fn order_and_prefix_counters_reserve_later_locked_and_disabled_sheets() {
        let mut a = Document::new(DocumentKind::Schematic, "a");
        let low = add(&mut a, "R?", 1, 1);
        let high = add(&mut a, "R?", 1, 10);
        let cap = add(&mut a, "C?", 2, 2);
        let mut b = Document::new(DocumentKind::Schematic, "b");
        add(&mut b, "R1", 0, 0);
        let p = plan(
            &[
                sheet(&a),
                Sheet {
                    enabled: false,
                    ..sheet(&b)
                },
            ],
            &Options::default(),
            Operation::Update,
        )
        .unwrap();
        assert_eq!(next(&p, high), "R2");
        assert_eq!(next(&p, low), "R3");
        assert_eq!(next(&p, cap), "C1");
        assert_eq!(p.prepare(&[&a, &b]).unwrap().len(), 1);
        let p = plan(
            &[sheet(&a)],
            &Options {
                order: Order::UpAcross,
                ..Options::default()
            },
            Operation::Update,
        )
        .unwrap();
        assert_eq!(next(&p, low), "R1");
    }
    #[test]
    fn reset_duplicate_and_leading_zero_keep_are_deterministic() {
        let mut d = Document::new(DocumentKind::Schematic, "d");
        let a = add(&mut d, "R01", 0, 1);
        let b = add(&mut d, "R1", 0, 0);
        let c = add(&mut d, "C9", 0, 2);
        let p = plan(&[sheet(&d)], &Options::default(), Operation::Update).unwrap();
        assert_eq!(next(&p, a), "R01");
        assert_eq!(next(&p, b), "R2");
        assert_eq!(next(&p, c), "C9");
        p.prepare(&[&d]).unwrap();
        let p = plan(
            &[sheet(&d)],
            &Options::default(),
            Operation::ResetDuplicates,
        )
        .unwrap();
        assert_eq!(next(&p, b), "R?");
        assert_eq!(next(&p, c), "C9");
        p.prepare(&[&d]).unwrap();
    }
    #[test]
    fn packages_keep_parts_and_locked_section_protects_whole_package() {
        let mut d = Document::new(DocumentKind::Schematic, "d");
        let a = add(&mut d, "U7A", 0, 2);
        let b = add(&mut d, "U7B", 0, 1);
        let s = d.schematic.as_mut().unwrap();
        for (i, c) in s.components.iter_mut().enumerate() {
            c.settings.part_count = 2;
            c.part = i as u32 + 1;
            c.library_reference = "DualOpamp".into();
        }
        s.components[1].settings.designator_locked = true;
        let p = plan(
            &[sheet(&d)],
            &Options {
                reset_existing: true,
                ..Options::default()
            },
            Operation::Update,
        )
        .unwrap();
        assert_eq!(next(&p, a), "U7A");
        assert_eq!(next(&p, b), "U7B");
        assert_eq!(p.changed(), 0);
        let s = d.schematic.as_mut().unwrap();
        s.components[1].settings.designator_locked = false;
        s.components[0].designator = "U?A".into();
        s.components[1].designator = "U?B".into();
        let p = plan(&[sheet(&d)], &Options::default(), Operation::Update).unwrap();
        assert_eq!(next(&p, a), "U1A");
        assert_eq!(next(&p, b), "U1B");
        p.prepare(&[&d]).unwrap();
    }
    #[test]
    fn overrides_collision_stale_source_and_incomplete_packages_are_refused() {
        let mut d = Document::new(DocumentKind::Schematic, "d");
        let a = add(&mut d, "R?", 0, 1);
        add(&mut d, "R?", 0, 0);
        let mut p = plan(&[sheet(&d)], &Options::default(), Operation::Update).unwrap();
        p.changes[1].proposed = p.changes[0].proposed.clone();
        assert!(p.prepare(&[&d]).is_err());
        let p = plan(&[sheet(&d)], &Options::default(), Operation::Update).unwrap();
        d.schematic
            .as_mut()
            .unwrap()
            .components
            .iter_mut()
            .find(|c| c.id == a)
            .unwrap()
            .designator = "R20".into();
        assert!(p.prepare(&[&d]).unwrap_err().contains("changed"));
    }
    #[test]
    fn apply_updates_visible_and_hidden_text_preserves_ids_and_history() {
        let mut d = Document::new(DocumentKind::Schematic, "d");
        let id = add(&mut d, "R?", 1, 1);
        let s = d.schematic.as_mut().unwrap();
        let mut hidden = s.primitives[0].clone();
        hidden.id = Uuid::new_v4();
        hidden.presentation.hidden = true;
        s.parameter_objects.push(hidden);
        let before = d.clone();
        let p = plan(&[sheet(&d)], &Options::default(), Operation::Update).unwrap();
        let next = p.prepare(&[&d]).unwrap().remove(0);
        let s = next.schematic.as_ref().unwrap();
        assert_eq!(s.components[0].id, id);
        for t in s.primitives.iter().chain(&s.parameter_objects) {
            assert_eq!(t.presentation.raw_text.as_deref(), Some("R1"));
            assert!(matches!(&t.kind,SchPrimitiveKind::Text{text,..} if text=="R1"));
        }
        let mut history = crate::History::default();
        history.commit(&mut d, next.clone()).unwrap();
        history.undo(&mut d);
        assert_eq!(d, before);
        history.redo(&mut d);
        assert_eq!(d, next);
        assert_eq!(
            serde_json::from_str::<Document>(&serde_json::to_string(&d).unwrap()).unwrap(),
            d
        );
    }
    #[test]
    fn sheet_start_selected_only_and_locked_duplicates() {
        let mut d = Document::new(DocumentKind::Schematic, "d");
        let a = add(&mut d, "R?", 0, 1);
        let b = add(&mut d, "R2", 0, 0);
        let selection = HashSet::from([a]);
        let p = plan(
            &[Sheet {
                start: 100,
                selected: Some(&selection),
                ..sheet(&d)
            }],
            &Options::default(),
            Operation::Update,
        )
        .unwrap();
        assert_eq!(next(&p, a), "R100");
        assert_eq!(next(&p, b), "R2");
        let s = d.schematic.as_mut().unwrap();
        for c in &mut s.components {
            c.designator = "R1".into();
            c.settings.designator_locked = true;
        }
        assert!(plan(&[sheet(&d)], &Options::default(), Operation::Update).is_err());
    }
    #[test]
    fn ten_thousand_unannotated_parts_use_unique_bounded_counters() {
        let mut d = Document::new(DocumentKind::Schematic, "large");
        for i in 0..10_000 {
            add(&mut d, "R?", i, 0);
        }
        let p = plan(&[sheet(&d)], &Options::default(), Operation::Update).unwrap();
        assert_eq!(p.changed(), 10_000);
        assert_eq!(
            p.changes
                .iter()
                .map(|r| &r.proposed)
                .collect::<HashSet<_>>()
                .len(),
            10_000
        );
    }
    #[test]
    fn annotation_preserves_native_unique_id_and_original_source() {
        use std::sync::Arc;
        let mut d = Document::new(DocumentKind::Schematic, "native");
        let id = add(&mut d, "R?", 1, 1);
        let s = d.schematic.as_mut().unwrap();
        s.components[0].source_index = 0;
        s.records = Arc::new(vec![crate::schematic::SourceRecord {
            index: 0,
            stream: "FileHeader".into(),
            record_type: 1,
            fields: BTreeMap::from([("UNIQUEID".into(), "LINK1234".into())]),
        }]);
        s.source = Some(Arc::new(crate::schematic::NativeSource {
            format: "test".into(),
            filename: "native.SchDoc".into(),
            original_base64: "c291cmNl".into(),
        }));
        let source = s.source.clone();
        let records = s.records.clone();
        let p = plan(&[sheet(&d)], &Options::default(), Operation::Update).unwrap();
        let next = p.prepare(&[&d]).unwrap().remove(0);
        let s = next.schematic.as_ref().unwrap();
        assert!(s.source_modified);
        assert_eq!(s.source, source);
        assert!(Arc::ptr_eq(&s.records, &records));
        match crate::cross_probe::source(&next, id).unwrap() {
            crate::cross_probe::Probe::Component(k) => {
                assert_eq!(k.unique_id.as_deref(), Some("LINK1234"));
                assert_eq!(k.designator, "R1");
            }
            _ => panic!("Expected component"),
        }
    }
    #[test]
    fn punctuation_prefixes_from_real_boards_are_preserved() {
        let mut d = Document::new(DocumentKind::Schematic, "hardware");
        let a = add(&mut d, "!PCB700", 0, 0);
        let b = add(&mut d, "#PWR?", 0, 1);
        let p = plan(&[sheet(&d)], &Options::default(), Operation::Update).unwrap();
        assert_eq!(next(&p, a), "!PCB700");
        assert_eq!(next(&p, b), "#PWR1");
        p.prepare(&[&d]).unwrap();
    }
}
