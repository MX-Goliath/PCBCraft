//! Schematic/PCB comparison and engineering changes. No files or UI are accessed.
use crate::{Document, pcb, schematic, schematic_edit};
use eda_geom::Point;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use uuid::Uuid;

#[path = "design_sync_netlist.rs"]
mod netlist;
pub use netlist::{Netlist, Pin, compile, compile_project};
#[path = "design_sync_project.rs"]
mod project;
pub use project::{NetScope, ProjectOptions, SheetLink};
pub(crate) fn local_harness(
    s: &schematic::Schematic,
) -> Result<netlist::harness::Expansion, String> {
    let mut doc = Document::new(crate::DocumentKind::Schematic, "Local harness");
    doc.schematic = Some(s.clone());
    netlist::harness::expand(&[&doc], &ProjectOptions::default(), &[], false)
}
#[cfg(test)]
#[path = "design_sync_tests.rs"]
mod tests;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    ToPcb,
    ToSchematic,
}

#[derive(Clone, Debug)]
struct LogicalComponent {
    parts: Vec<(Uuid, Uuid)>,
    unique_ids: Vec<String>,
    designator: String,
    comment: String,
    description: String,
    footprint: String,
    parameters: pcb::Fields,
    excluded_parameters: BTreeSet<String>,
    sheet: String,
    library_source: String,
    library_reference: String,
    kind: u32,
    annotation_error: Option<String>,
}
fn field<'a>(fields: &'a pcb::Fields, key: &str) -> Option<&'a str> {
    fields
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(key))
        .map(|(_, v)| v.trim())
        .filter(|v| !v.is_empty())
}
fn pcb_kind(c: &pcb::Component) -> u32 {
    field(&c.properties, "COMPONENTKINDVERSION2")
        .or_else(|| field(&c.properties, "COMPONENTKIND"))
        .and_then(|v| v.parse().ok())
        .unwrap_or(0)
}
fn uid_matches(a: &str, b: &str) -> bool {
    let a = a.trim_matches(['/', '\\']).replace('/', "\\");
    let b = b.trim_matches(['/', '\\']).replace('/', "\\");
    a.eq_ignore_ascii_case(&b)
        || !(a.contains('\\') && b.contains('\\'))
            && a.rsplit('\\')
                .next()
                .unwrap()
                .eq_ignore_ascii_case(b.rsplit('\\').next().unwrap())
}
fn footprint(c: &schematic::Component) -> String {
    let models: Vec<_> = c
        .settings
        .models
        .iter()
        .filter(|m| m.kind.eq_ignore_ascii_case("PCBLIB") || m.kind.eq_ignore_ascii_case("PCB"))
        .collect();
    models
        .iter()
        .find(|m| m.current)
        .or_else(|| models.first())
        .map(|m| m.name.clone())
        .unwrap_or_default()
}
fn components(
    sheets: &[&Document],
    options: &ProjectOptions,
) -> Result<Vec<LogicalComponent>, String> {
    let graph = project::hierarchy(sheets, options)?;
    let excluded: HashMap<_, _> = sheets
        .iter()
        .map(|d| {
            (
                d.id,
                crate::schematic_compile::exclusions(d.schematic.as_ref().unwrap()),
            )
        })
        .collect();
    let mut slots = HashSet::new();
    let mut ambiguous = HashSet::new();
    for doc in sheets {
        for c in doc
            .schematic
            .as_ref()
            .unwrap()
            .components
            .iter()
            .filter(|c| {
                !matches!(c.settings.kind, 1 | 2) && !excluded[&doc.id].components.contains(&c.id)
            })
        {
            let name = project::logical_designator(c).to_uppercase();
            if !slots.insert((name.clone(), c.part)) {
                ambiguous.insert(name);
            }
        }
    }
    let mut result: Vec<LogicalComponent> = vec![];
    let mut parts: HashMap<String, HashSet<u32>> = HashMap::new();
    let mut unique_owners = HashMap::new();
    for doc in sheets {
        let s = doc.schematic.as_ref().ok_or("Expected a schematic sheet")?;
        for c in s
            .components
            .iter()
            .filter(|c| !excluded[&doc.id].components.contains(&c.id))
        {
            // Mechanical and graphical symbols have no electrical PCB counterpart.
            if matches!(c.settings.kind, 1 | 2) {
                continue;
            }
            if c.settings.kind > 6 {
                return Err(format!("Unsupported component type on {}", c.designator));
            }
            let designator = project::logical_designator(c);
            if !options.review_annotation
                && (designator.trim().is_empty() || designator.contains('?'))
            {
                return Err(format!(
                    "Annotate {} before synchronization ({})",
                    doc.name, c.designator
                ));
            }
            let key = designator.to_uppercase();
            let unique = s
                .records
                .iter()
                .find(|r| r.index == c.source_index && r.record_type == 1)
                .and_then(|r| field(&r.fields, "UNIQUEID"))
                .map(str::to_owned)
                .unwrap_or_else(|| c.id.to_string());
            let unique = if graph.paths[&doc.id].is_empty() {
                unique
            } else {
                format!("{}\\{}", graph.paths[&doc.id], unique)
            };
            if unique_owners
                .insert(unique.to_uppercase(), key.clone())
                .is_some_and(|owner| owner != key)
            {
                return Err(format!("Duplicate schematic Unique ID on {}", c.designator));
            }
            let params: pcb::Fields = c
                .parameters
                .iter()
                .filter(|(k, _)| {
                    !k.eq_ignore_ascii_case("Comment") && !k.eq_ignore_ascii_case("Designator")
                })
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            let excluded_parameters: BTreeSet<_> = s
                .primitives
                .iter()
                .chain(&s.parameter_objects)
                .filter(|p| {
                    p.component == Some(c.id)
                        && p.presentation.record_type == 41
                        && !p.presentation.allow_sync
                })
                .map(|p| p.presentation.name.to_uppercase())
                .collect();
            let params: pcb::Fields = params
                .into_iter()
                .filter(|(k, _)| !excluded_parameters.contains(&k.to_uppercase()))
                .collect();
            let mut names = HashSet::new();
            if params
                .keys()
                .any(|k| k.trim().is_empty() || !names.insert(k.to_uppercase()))
            {
                return Err(format!(
                    "Invalid or duplicate parameter on {}",
                    c.designator
                ));
            }
            if let Some(old) = result.iter_mut().find(|p| {
                p.designator.eq_ignore_ascii_case(designator)
                    && !(options.review_annotation && ambiguous.contains(&key))
            }) {
                let first = sheets
                    .iter()
                    .find_map(|d| {
                        d.schematic
                            .as_ref()?
                            .components
                            .iter()
                            .find(|p| p.id == old.parts[0].1)
                    })
                    .unwrap();
                if c.settings.part_count <= 1
                    || first.library_reference != c.library_reference
                    || old.comment != c.comment
                    || old.description != c.description
                    || old.kind != c.settings.kind
                    || old.footprint != footprint(c)
                    || old.parameters != params
                    || old.excluded_parameters != excluded_parameters
                    || !parts.get_mut(&key).unwrap().insert(c.part)
                {
                    return Err(format!(
                        "Ambiguous or inconsistent multi-part component {}",
                        c.designator
                    ));
                }
                old.parts.push((doc.id, c.id));
                old.unique_ids.push(unique);
            } else {
                parts.insert(key, HashSet::from([c.part]));
                result.push(LogicalComponent {
                    parts: vec![(doc.id, c.id)],
                    unique_ids: vec![unique],
                    designator: designator.to_owned(),
                    comment: c.comment.clone(),
                    description: c.description.clone(),
                    footprint: footprint(c),
                    parameters: params,
                    excluded_parameters,
                    sheet: doc.name.clone(),
                    library_source: c.settings.source.clone(),
                    library_reference: c.library_reference.clone(),
                    kind: c.settings.kind,
                    annotation_error: if designator.trim().is_empty() || designator.contains('?') { Some(format!("Annotate {} on {} before transferring this component",designator,doc.name)) } else if ambiguous.contains(&designator.to_uppercase()) { Some(format!("Duplicate schematic designator {} on {}; annotate the distinct components",designator,doc.name)) } else { None },
                });
            }
        }
    }
    Ok(result)
}

/// A real footprint captured from an available board/library document, normalized
/// to a top-side zero-degree origin. Geometry, pad stacks and 3D resources survive.
#[derive(Clone, Debug)]
pub struct Footprint {
    board: pcb::Board,
}
impl Footprint {
    pub fn library_source(&self) -> Option<&str> {
        field(
            &self.board.components[0].properties,
            "SOURCEFOOTPRINTLIBRARY",
        )
    }
    pub fn warnings(&self) -> &[String] {
        &self.board.warnings
    }
    pub fn from_board(board: &pcb::Board, id: Uuid) -> Result<Self, String> {
        let c = board
            .components
            .iter()
            .find(|c| c.id == id)
            .ok_or("Footprint component missing")?;
        let mut b = board.clone();
        b.source = None;
        b.rules.clear();
        b.classes.clear();
        b.polygons.clear();
        b.polygon_pours.clear();
        b.outline.clear();
        b.layer_stack = None;
        b.items.retain(|i| i.component == Some(c.index));
        b.components.retain(|p| p.id == id);
        b.nets.clear();
        b.components[0].locked = false;
        for p in &mut b.items {
            p.locked = false;
            p.net = None;
            p.polygon = None;
            p.source = None;
        }
        if c.layer == pcb::BOTTOM {
            b = crate::pcb_tools::flip_component(&b, id)?;
        }
        let c = b.components[0].clone();
        b = b.edited(pcb::Edit::Transform {
            selection: vec![id],
            pivot: c.position,
            translation: Point {
                x: -c.position.x,
                y: -c.position.y,
            },
            angle: -c.rotation,
        })?;
        if !b
            .items
            .iter()
            .any(|p| matches!(p.shape, pcb::Shape::Pad { .. }))
        {
            return Err("Footprint has no pads".into());
        }
        Ok(Self { board: b })
    }
    pub fn name(&self) -> &str {
        &self.board.components[0].footprint
    }
}

#[derive(Clone, Debug)]
enum Value {
    Designator(String),
    Comment(String),
    Parameters(pcb::Fields),
    Link(String, String),
    Footprint(String),
    Description(String),
    Kind(u32),
}
#[derive(Clone, Debug)]
enum Operation {
    AddComponent(Box<LogicalComponent>, Uuid, Point),
    RemoveComponent(Uuid),
    PcbValue(Uuid, Value),
    SchValue(Vec<(Uuid, Uuid)>, Value),
    AddNet(String),
    RemoveNet(u32),
    RenameNet(u32, String),
    PadNet {
        component: Uuid,
        pin: String,
        net: Option<String>,
    },
    SchNet {
        primitives: Vec<(Uuid, Uuid)>,
        name: String,
    },
    Unsupported(String),
}
#[derive(Clone, Debug)]
pub struct Change {
    pub enabled: bool,
    pub group: &'static str,
    pub action: &'static str,
    pub object: String,
    pub document: String,
    pub check: Option<Result<(), String>>,
    pub done: bool,
    operation: Operation,
}
impl Change {
    pub fn unavailable_reason(&self) -> Option<&str> {
        match &self.operation {
            Operation::Unsupported(reason) => Some(reason),
            _ => None,
        }
    }
    pub fn supported(&self) -> bool {
        !matches!(self.operation, Operation::Unsupported(_))
    }
}
#[derive(Clone, Debug)]
pub struct Plan {
    pub direction: Direction,
    pub changes: Vec<Change>,
    pub warnings: Vec<String>,
    snapshots: Vec<Document>,
    pcb: Uuid,
    footprints: Vec<Footprint>,
}
pub struct Prepared {
    pub documents: Vec<Document>,
    pub rows: Vec<usize>,
}
impl Plan {
    /// Native Altium back annotation limits ordinary component updates to
    /// designator/comment/footprint; arbitrary net renames and other properties
    /// are independent editor capabilities, not automatic native ECO operations.
    pub fn restrict_native_back_annotation(&mut self) {
        if self.direction != Direction::ToSchematic {
            return;
        }
        for change in &mut self.changes {
            if matches!(
                &change.operation,
                Operation::SchValue(
                    _,
                    Value::Description(_) | Value::Kind(_) | Value::Parameters(_)
                ) | Operation::SchNet { .. }
            ) {
                change.operation = Operation::Unsupported("Altium back annotation does not support this ordinary property/net update; edit the schematic source instead".into());
                change.enabled = false;
                change.check = None;
            }
        }
    }
    fn push(
        &mut self,
        group: &'static str,
        action: &'static str,
        object: String,
        document: String,
        operation: Operation,
    ) {
        let supported = !matches!(operation, Operation::Unsupported(_));
        self.changes.push(Change {
            enabled: supported,
            group,
            action,
            object,
            document,
            check: None,
            done: false,
            operation,
        });
    }
    pub fn reset_validation(&mut self) {
        for c in &mut self.changes {
            if !c.done {
                c.check = None;
            }
        }
    }
    pub fn acknowledge(&mut self, rows: &[usize], documents: &[&Document]) {
        for &n in rows {
            self.changes[n].done = true;
        }
        for snapshot in &mut self.snapshots {
            if let Some(d) = documents.iter().find(|d| d.id == snapshot.id) {
                *snapshot = (*d).clone();
            }
        }
    }
    pub fn validate(&mut self, documents: &[&Document]) -> Result<Prepared, String> {
        for old in &self.snapshots {
            if documents.iter().find(|d| d.id == old.id).copied() != Some(old) {
                return Err(format!("{} changed or was closed; compare again", old.name));
            }
        }
        let mut next = self.snapshots.clone();
        let mut rows = vec![];
        for (n, c) in self.changes.iter_mut().enumerate() {
            if c.done {
                continue;
            }
            c.check = None;
            if !c.enabled {
                continue;
            }
            let required_footprint = match &c.operation {
                Operation::AddComponent(component, ..) => Some(&component.footprint),
                Operation::PcbValue(_, Value::Footprint(name)) => Some(name),
                _ => None,
            };
            if let Some(name) = required_footprint
                && !self
                    .footprints
                    .iter()
                    .any(|f| f.name().eq_ignore_ascii_case(name))
            {
                c.check = Some(Err(format!("Footprint Not Found {name}")));
                continue;
            }
            if scalar_pcb_operation(&c.operation) {
                // These operations validate all dependencies before mutating.
                // Full geometry validation still runs on the final private result.
                let result = apply_pcb_scalar(
                    &c.operation,
                    next.iter_mut()
                        .find(|d| d.id == self.pcb)
                        .and_then(|d| d.pcb.as_mut())
                        .ok_or("PCB missing")?,
                );
                c.check = Some(result.clone());
                if result.is_ok() {
                    rows.push(n);
                }
                continue;
            }
            let affected = |d: &&Document| match &c.operation {
                Operation::SchValue(parts, _) => parts.iter().any(|(id, _)| *id == d.id),
                Operation::SchNet { primitives, .. } => {
                    primitives.iter().any(|(id, _)| *id == d.id)
                }
                _ => d.id == self.pcb,
            };
            let mut candidate: Vec<_> = next.iter().filter(affected).cloned().collect();
            let result = apply(&c.operation, self.pcb, &self.footprints, &mut candidate)
                .and_then(|()| candidate.iter().try_for_each(Document::validate));
            c.check = Some(result.clone());
            if result.is_ok() {
                for doc in candidate {
                    let id = doc.id;
                    *next.iter_mut().find(|d| d.id == id).unwrap() = doc;
                }
                rows.push(n);
            }
        }
        next.iter().try_for_each(Document::validate)?;
        // Designator swaps are legal; the final result must still be unique.
        let b = next
            .iter()
            .find(|d| d.id == self.pcb)
            .unwrap()
            .pcb
            .as_ref()
            .unwrap();
        let mut seen = HashSet::new();
        if b.components
            .iter()
            .filter(|c| !matches!(pcb_kind(c), 1 | 2))
            .any(|c| !seen.insert(c.designator.to_uppercase()))
        {
            return Err("Selected changes create duplicate PCB designators; include both sides of a rename/swap".into());
        }
        let documents = next
            .into_iter()
            .filter(|d| self.snapshots.iter().find(|s| s.id == d.id) != Some(d))
            .collect();
        Ok(Prepared { documents, rows })
    }
    pub fn report(&self) -> String {
        let mut s = String::from(
            "Enable\tAction\tAffected Object\tAffected Document\tCheck\tDone\tMessage\n",
        );
        for c in &self.changes {
            let (check, message) = match &c.check {
                None => {
                    if let Operation::Unsupported(e) = &c.operation {
                        ("Unavailable", e.as_str())
                    } else {
                        ("", "")
                    }
                }
                Some(Ok(())) => ("OK", ""),
                Some(Err(e)) => ("Error", e.as_str()),
            };
            s.push_str(&format!(
                "{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
                c.enabled,
                c.action,
                c.object.replace(['\t', '\n', '\r'], " "),
                c.document,
                check,
                c.done,
                message
            ));
        }
        s
    }
}

pub fn compare(
    sheets: &[&Document],
    board: &Document,
    direction: Direction,
    footprints: Vec<Footprint>,
) -> Result<Plan, String> {
    compare_project(
        sheets,
        board,
        direction,
        footprints,
        &ProjectOptions::default(),
    )
}

pub fn compare_project(
    sheets: &[&Document],
    board: &Document,
    direction: Direction,
    footprints: Vec<Footprint>,
    options: &ProjectOptions,
) -> Result<Plan, String> {
    board.validate()?;
    let mut document_ids = HashSet::from([board.id]);
    for s in sheets {
        s.validate()?;
        if !document_ids.insert(s.id) {
            return Err("Compared documents have duplicate document identities; resolve the copied document IDs first".into());
        }
    }
    let b = board.pcb.as_ref().ok_or("Select a PCB document")?;
    if sheets.is_empty() {
        return Err("Project has no schematic sheets".into());
    }
    let cs = components(sheets, options)?;
    let mut snapshots = vec![board.clone()];
    snapshots.extend(sheets.iter().map(|d| (*d).clone()));
    let mut p = Plan {
        direction,
        changes: vec![],
        warnings: vec![],
        snapshots,
        pcb: board.id,
        footprints,
    };
    let mut links: HashMap<(Uuid, Uuid), Uuid> = HashMap::new();
    let mut used = HashSet::new();
    let bounds = b.bounds();
    let origin_x = bounds.max.x + 2_540_000;
    let mut cursor = Point {
        x: origin_x,
        y: bounds.min.y,
    };
    let mut row_height = 0;
    let mut new_column = 0;
    for s in &cs {
        if let Some(reason) = &s.annotation_error {
            p.warnings.push(reason.clone());
            if direction == Direction::ToPcb {
                p.push(
                    "Annotation",
                    "Annotate",
                    s.designator.clone(),
                    s.sheet.clone(),
                    Operation::Unsupported(reason.clone()),
                );
            }
        }
        let by_uid: Vec<_> = b
            .components
            .iter()
            .filter(|c| {
                field(&c.properties, "SOURCEUNIQUEID")
                    .is_some_and(|id| s.unique_ids.iter().any(|v| uid_matches(v, id)))
            })
            .collect();
        let matches = if by_uid.is_empty() {
            b.components
                .iter()
                .filter(|c| {
                    c.designator.eq_ignore_ascii_case(&s.designator)
                        && field(&c.properties, "SOURCEUNIQUEID").is_none()
                })
                .collect::<Vec<_>>()
        } else {
            by_uid
        };
        if matches.len() > 1 {
            return Err(format!("Ambiguous PCB link for {}", s.designator));
        }
        if let Some(c) = matches.first() {
            if !used.insert(c.id) {
                return Err(format!(
                    "Multiple schematic components link to {}",
                    c.designator
                ));
            }
            for &part in &s.parts {
                links.insert(part, c.id);
            }
            let mut values = vec![];
            let pcb_description = field(&c.properties, "SOURCEDESCRIPTION").unwrap_or("");
            if pcb_description != s.description {
                values.push((
                    "Change Component Descriptions",
                    Value::Description(if direction == Direction::ToPcb {
                        s.description.clone()
                    } else {
                        pcb_description.to_owned()
                    }),
                    s.designator.clone(),
                ));
            }
            let pcb_kind = pcb_kind(c);
            if pcb_kind != s.kind {
                values.push((
                    "Change Component Types",
                    Value::Kind(if direction == Direction::ToPcb {
                        s.kind
                    } else {
                        pcb_kind
                    }),
                    s.designator.clone(),
                ));
            }
            if c.designator != s.designator
                && (direction == Direction::ToSchematic || s.annotation_error.is_none())
            {
                values.push((
                    "Change Component Designators",
                    Value::Designator(if direction == Direction::ToPcb {
                        s.designator.clone()
                    } else {
                        c.designator.clone()
                    }),
                    format!(
                        "{} → {}",
                        if direction == Direction::ToPcb {
                            &c.designator
                        } else {
                            &s.designator
                        },
                        if direction == Direction::ToPcb {
                            &s.designator
                        } else {
                            &c.designator
                        }
                    ),
                ));
            }
            if c.comment != s.comment && !s.excluded_parameters.contains("COMMENT") {
                values.push((
                    "Change Component Comments",
                    Value::Comment(if direction == Direction::ToPcb {
                        s.comment.clone()
                    } else {
                        c.comment.clone()
                    }),
                    format!(
                        "{}: {} → {}",
                        s.designator,
                        if direction == Direction::ToPcb {
                            &c.comment
                        } else {
                            &s.comment
                        },
                        if direction == Direction::ToPcb {
                            &s.comment
                        } else {
                            &c.comment
                        }
                    ),
                ));
            }
            if c.footprint != s.footprint {
                values.push((
                    "Change Component Footprints",
                    Value::Footprint(if direction == Direction::ToPcb {
                        s.footprint.clone()
                    } else {
                        c.footprint.clone()
                    }),
                    format!(
                        "{}: {} → {}",
                        s.designator,
                        if direction == Direction::ToPcb {
                            &c.footprint
                        } else {
                            &s.footprint
                        },
                        if direction == Direction::ToPcb {
                            &s.footprint
                        } else {
                            &c.footprint
                        }
                    ),
                ));
            }
            if let Some(params) = &c.parameters {
                let synced: pcb::Fields = params
                    .iter()
                    .filter(|(k, _)| {
                        !k.eq_ignore_ascii_case("Layer")
                            && !s.excluded_parameters.contains(&k.to_uppercase())
                    })
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect();
                let mut target = s.parameters.clone();
                target.extend(
                    params
                        .iter()
                        .filter(|(k, _)| {
                            k.eq_ignore_ascii_case("Layer")
                                || s.excluded_parameters.contains(&k.to_uppercase())
                        })
                        .map(|(k, v)| (k.clone(), v.clone())),
                );
                if synced != s.parameters {
                    values.push((
                        "Change Component Parameters",
                        Value::Parameters(if direction == Direction::ToPcb {
                            target
                        } else {
                            synced
                        }),
                        s.designator.clone(),
                    ));
                }
            }
            if direction == Direction::ToPcb && field(&c.properties, "SOURCEUNIQUEID").is_none() {
                values.push((
                    "Update Component Links",
                    Value::Link(s.unique_ids[0].clone(), s.sheet.clone()),
                    s.designator.clone(),
                ));
            }
            for (group, value, object) in values {
                let (doc, op) = if direction == Direction::ToPcb {
                    (board.name.clone(), Operation::PcbValue(c.id, value))
                } else {
                    (s.sheet.clone(), Operation::SchValue(s.parts.clone(), value))
                };
                p.push(group, "Change", object, doc, op);
            }
        } else if direction == Direction::ToPcb {
            let id = Uuid::new_v4();
            if s.annotation_error.is_some() {
                continue;
            }
            for &part in &s.parts {
                links.insert(part, id);
            }
            let extent = p
                .footprints
                .iter()
                .find(|f| f.name().eq_ignore_ascii_case(&s.footprint))
                .map(|f| f.board.bounds());
            let min = extent.map_or(Point::default(), |b| b.min);
            let size = extent.map_or(Point::mm(5, 5), |b| Point {
                x: (b.max.x - b.min.x).max(1_000_000),
                y: (b.max.y - b.min.y).max(1_000_000),
            });
            let position = Point {
                x: cursor.x - min.x,
                y: cursor.y - min.y,
            };
            cursor.x += size.x + 2_540_000;
            row_height = row_height.max(size.y);
            new_column += 1;
            if new_column == 4 {
                cursor.x = origin_x;
                cursor.y += row_height + 2_540_000;
                row_height = 0;
                new_column = 0;
            }
            p.push(
                "Add Components",
                "Add",
                s.designator.clone(),
                board.name.clone(),
                Operation::AddComponent(Box::new(s.clone()), id, position),
            );
        } else {
            p.push("Remove Components","Remove",s.designator.clone(),s.sheet.clone(),Operation::Unsupported("Automatic symbol removal from the schematic is not implemented; resolve in the schematic and compare again".into()));
        }
    }
    for c in &b.components {
        if matches!(pcb_kind(c), 1 | 2) {
            continue;
        }
        if sheets
            .iter()
            .filter_map(|d| d.schematic.as_ref())
            .flat_map(|s| &s.components)
            .any(|s| {
                matches!(s.settings.kind, 1 | 2) && s.designator.eq_ignore_ascii_case(&c.designator)
            })
        {
            continue;
        }
        if !used.contains(&c.id) {
            let op = if direction == Direction::ToPcb {
                Operation::RemoveComponent(c.id)
            } else {
                Operation::Unsupported(
                    "Place the matching library symbol on a schematic before back annotation"
                        .into(),
                )
            };
            p.push(
                if direction == Direction::ToPcb {
                    "Remove Components"
                } else {
                    "Add Components"
                },
                if direction == Direction::ToPcb {
                    "Remove"
                } else {
                    "Add"
                },
                c.designator.clone(),
                if direction == Direction::ToPcb {
                    board.name.clone()
                } else {
                    sheets[0].name.clone()
                },
                op,
            );
        }
    }
    match compile_project(sheets, options) {
        Ok(nets) => {
            p.warnings.extend(nets.warnings.iter().cloned());
            for issue in &nets.issues {
                p.warnings.push(issue.clone());
                p.push(
                    "Connectivity",
                    "Resolve",
                    "Project connectivity".into(),
                    board.name.clone(),
                    Operation::Unsupported(issue.clone()),
                );
            }
            net_changes(&mut p, &nets, b, &links, board);
            if options.review_annotation {
                // Native exported netlists and native ECO differ for omitted
                // single-pin nets. Do not infer destructive ECO from export loss.
                let omitted: HashSet<_> = nets
                    .omitted_single_pin_pins
                    .iter()
                    .filter_map(|(sheet, c, pin)| {
                        links.get(&(*sheet, *c)).map(|id| (*id, pin.clone()))
                    })
                    .collect();
                let omitted_nets: HashSet<_> = b
                    .items
                    .iter()
                    .filter_map(|item| {
                        let pcb::Shape::Pad { name, .. } = &item.shape else {
                            return None;
                        };
                        let component = b
                            .components
                            .iter()
                            .find(|c| Some(c.index) == item.component)?;
                        omitted
                            .contains(&(component.id, name.to_uppercase()))
                            .then_some(item.net)
                            .flatten()
                    })
                    .collect();
                let reason = "The project netlist omits single-pin nets, but native ECO removal semantics are not yet verified; retain this existing PCB connection and review it manually";
                let mut blocked = false;
                for row in &mut p.changes {
                    let uncertain = match &row.operation {
                        Operation::PadNet {
                            component,
                            pin,
                            net: None,
                        } => omitted.contains(&(*component, pin.to_uppercase())),
                        Operation::RemoveNet(n) => omitted_nets.contains(n),
                        _ => false,
                    };
                    if uncertain {
                        row.operation = Operation::Unsupported(reason.into());
                        row.enabled = false;
                        blocked = true;
                    }
                }
                if blocked {
                    p.warnings.push(reason.into());
                }
            }
        }
        Err(e) => {
            p.warnings.push(e.clone());
            p.push(
                "Connectivity",
                "Compare",
                "Project connectivity".into(),
                board.name.clone(),
                Operation::Unsupported(e),
            );
        }
    }
    // Altium presents changes grouped by modification type. Stable order also
    // preserves dependencies: components, nets, pin memberships, obsolete nets.
    Ok(p)
}

fn net_changes(
    p: &mut Plan,
    nets: &Netlist,
    b: &pcb::Board,
    links: &HashMap<(Uuid, Uuid), Uuid>,
    doc: &Document,
) {
    let mut protected: HashSet<_> = nets
        .unresolved_pins
        .iter()
        .filter_map(|(sheet, c, pin)| links.get(&(*sheet, *c)).map(|id| (*id, pin.clone())))
        .collect();
    let protected_nets: HashSet<_> = b
        .items
        .iter()
        .filter_map(|i| {
            let pcb::Shape::Pad { name, .. } = &i.shape else {
                return None;
            };
            let c = b.components.iter().find(|c| Some(c.index) == i.component)?;
            protected
                .contains(&(c.id, name.to_uppercase()))
                .then_some(i.net)
                .flatten()
        })
        .collect();
    for i in b
        .items
        .iter()
        .filter(|i| i.net.is_some_and(|n| protected_nets.contains(&n)))
    {
        if let pcb::Shape::Pad { name, .. } = &i.shape
            && let Some(c) = b.components.iter().find(|c| Some(c.index) == i.component)
        {
            protected.insert((c.id, name.to_uppercase()));
        }
    }
    let mut known: BTreeSet<String> = b.nets.iter().map(|n| n.name.to_uppercase()).collect();
    let mut desired: BTreeMap<(Uuid, String), Option<String>> = BTreeMap::new();
    for pin in &nets.pins {
        if let Some(&id) = links.get(&(pin.sheet, pin.component)) {
            if protected.contains(&(id, pin.number.to_uppercase())) {
                continue;
            }
            desired.insert((id, pin.number.to_uppercase()), pin.net.clone());
        }
    }
    for item in &b.items {
        if item.net.is_none() {
            continue;
        }
        let Some(c) = b
            .components
            .iter()
            .find(|c| Some(c.index) == item.component)
        else {
            continue;
        };
        if !links.values().any(|id| *id == c.id) {
            continue;
        }
        let jumper = p
            .snapshots
            .iter()
            .filter_map(|d| d.schematic.as_ref())
            .flat_map(|s| &s.components)
            .any(|s| s.designator.eq_ignore_ascii_case(&c.designator) && s.settings.kind == 6);
        if jumper {
            continue;
        }
        if let pcb::Shape::Pad { name, .. } = &item.shape {
            if protected.contains(&(c.id, name.to_uppercase())) {
                continue;
            }
            desired.entry((c.id, name.to_uppercase())).or_insert(None);
        }
    }
    if p.direction == Direction::ToPcb {
        let mut renamed = BTreeMap::new();
        for n in &b.nets {
            if protected_nets.contains(&n.index) {
                continue;
            }
            let pads: Vec<_> = b
                .items
                .iter()
                .filter(|i| i.net == Some(n.index) && matches!(i.shape, pcb::Shape::Pad { .. }))
                .collect();
            let mut names = BTreeSet::new();
            let mut complete = !pads.is_empty();
            for pad in &pads {
                let c = b.components.iter().find(|c| Some(c.index) == pad.component);
                let target = c.and_then(|c| {
                    if let pcb::Shape::Pad { name, .. } = &pad.shape {
                        desired
                            .get(&(c.id, name.to_uppercase()))
                            .and_then(|v| v.as_ref())
                    } else {
                        None
                    }
                });
                if let Some(name) = target {
                    names.insert(name.clone());
                } else {
                    complete = false;
                }
            }
            if complete && names.len() == 1 {
                let name = names.first().unwrap();
                let all=desired.iter().filter(|(_,v)|v.as_ref()==Some(name)).all(|((id,pin),_)|pads.iter().any(|i|b.components.iter().any(|c|c.id==*id&&Some(c.index)==i.component)&&matches!(&i.shape,pcb::Shape::Pad{name,..} if name.eq_ignore_ascii_case(pin))));
                if all && !known.contains(&name.to_uppercase()) {
                    p.push(
                        "Change Net Names",
                        "Change",
                        format!("{} → {}", n.name, name),
                        doc.name.clone(),
                        Operation::RenameNet(n.index, name.clone()),
                    );
                    known.insert(name.to_uppercase());
                    renamed.insert(n.index, name.clone());
                }
            }
        }
        for name in &nets.names {
            if known.insert(name.to_uppercase()) {
                p.push(
                    "Add Nets",
                    "Add",
                    name.clone(),
                    doc.name.clone(),
                    Operation::AddNet(name.clone()),
                );
            }
        }
        // Include new components and every physical pad sharing the same pin number.
        for ((id, pin), net) in &desired {
            let c = b.components.iter().find(|c| c.id == *id);
            let pads:Vec<_>=b.items.iter().filter(|i|c.is_some_and(|c|i.component==Some(c.index)) && matches!(&i.shape,pcb::Shape::Pad{name,..} if name.eq_ignore_ascii_case(pin))).collect();
            let footprint_changed = p.changes.iter().any(
                |row| matches!(&row.operation,Operation::PcbValue(c,Value::Footprint(_)) if c==id),
            );
            if net.is_none() && pads.is_empty() && !footprint_changed {
                continue;
            }
            if footprint_changed
                || pads.is_empty()
                || pads.iter().any(|i| {
                    let current = i.net.map(|n| {
                        renamed
                            .get(&n)
                            .cloned()
                            .unwrap_or_else(|| b.net_name(Some(n)).to_owned())
                    });
                    match (&current, net) {
                        (Some(a), Some(b)) => !a.eq_ignore_ascii_case(b),
                        (None, None) => false,
                        _ => true,
                    }
                })
            {
                let designator = c.map(|c| c.designator.as_str()).unwrap_or_else(|| {
                    nets.pins
                        .iter()
                        .find(|p| links.get(&(p.sheet, p.component)) == Some(id))
                        .unwrap()
                        .designator
                        .as_str()
                });
                p.push(
                    if net.is_some() {
                        "Add Pins To Nets"
                    } else {
                        "Remove Pins From Nets"
                    },
                    if net.is_some() { "Add" } else { "Remove" },
                    format!(
                        "{}-{} {} {}",
                        designator,
                        pin,
                        if net.is_some() { "to" } else { "from" },
                        net.as_deref().unwrap_or("Net")
                    ),
                    doc.name.clone(),
                    Operation::PadNet {
                        component: *id,
                        pin: pin.clone(),
                        net: net.clone(),
                    },
                );
            }
        }
        for n in &b.nets {
            if !protected_nets.contains(&n.index)
                && !renamed.contains_key(&n.index)
                && !nets.names.iter().any(|v| v.eq_ignore_ascii_case(&n.name))
            {
                p.push(
                    "Remove Nets",
                    "Remove",
                    n.name.clone(),
                    doc.name.clone(),
                    Operation::RemoveNet(n.index),
                );
            }
        }
    } else {
        // Back annotate only a bijective net rename: every schematic pin must
        // still belong to precisely the same PCB net. Rewiring is never guessed.
        for name in &nets.names {
            let pins: Vec<_> = nets
                .pins
                .iter()
                .filter(|pin| pin.net.as_ref() == Some(name))
                .collect();
            let mut targets = BTreeSet::new();
            let mut missing = false;
            for pin in &pins {
                let id = links.get(&(pin.sheet, pin.component));
                let c = b.components.iter().find(|c| Some(&c.id) == id);
                let pads:Vec<_>=b.items.iter().filter(|i|c.is_some_and(|c|i.component==Some(c.index)) && matches!(&i.shape,pcb::Shape::Pad{name,..} if name.eq_ignore_ascii_case(&pin.number))).collect();
                if pads.is_empty() {
                    missing = true;
                }
                for pad in pads {
                    if let Some(net) = pad.net {
                        targets.insert(net);
                    } else {
                        missing = true;
                    }
                }
            }
            let target = targets.first().copied();
            let exact=target.is_some() && !missing && targets.len()==1 && b.items.iter().filter(|i|i.net==target && matches!(i.shape,pcb::Shape::Pad{..})).all(|i| {
                let c=b.components.iter().find(|c|Some(c.index)==i.component);
                c.is_some_and(|c|matches!(&i.shape,pcb::Shape::Pad{name:pin,..} if desired.get(&(c.id,pin.to_uppercase())).and_then(|s|s.as_ref())==Some(name)))
            });
            let target_name = b.net_name(target);
            if exact && target_name != name {
                let labels = nets.labels.get(name).cloned().unwrap_or_default();
                let op = if labels.is_empty() {
                    Operation::Unsupported(
                        "An unnamed net has no label to back annotate; place a net label first"
                            .into(),
                    )
                } else {
                    Operation::SchNet {
                        primitives: labels,
                        name: target_name.to_owned(),
                    }
                };
                p.push(
                    "Change Net Names",
                    "Change",
                    format!("{} → {}", name, target_name),
                    "Schematic project".into(),
                    op,
                );
            } else if !exact {
                p.push("Connectivity Differences","Change",name.clone(),"Schematic project".into(),Operation::Unsupported("PCB and schematic pin memberships differ; automatic schematic rewiring is not implemented".into()));
            }
        }
    }
}

fn scalar_pcb_operation(op: &Operation) -> bool {
    matches!(op,Operation::PcbValue(_,v) if !matches!(v,Value::Footprint(_)))
        || matches!(
            op,
            Operation::AddNet(_)
                | Operation::RemoveNet(_)
                | Operation::RenameNet(..)
                | Operation::PadNet { .. }
        )
}
fn apply_pcb_scalar(op: &Operation, b: &mut pcb::Board) -> Result<(), String> {
    match op {
        Operation::PcbValue(id, value) => return apply_pcb_metadata(b, *id, value),
        Operation::AddNet(name) => {
            if b.nets.iter().any(|n| n.name.eq_ignore_ascii_case(name)) {
                return Ok(());
            }
            if b.nets.len() >= pcb::MAX_ITEMS {
                return Err("PCB entity limit exceeded".into());
            }
            let index = match b.nets.iter().map(|n| n.index).max() {
                Some(n) => n.checked_add(1).ok_or("Net index overflow")?,
                None => 0,
            };
            b.nets.push(pcb::Net {
                index,
                name: name.clone(),
                properties: Default::default(),
            });
        }
        Operation::RemoveNet(index) => {
            // Only remove the net after its pads were successfully transferred.
            if b.items
                .iter()
                .any(|i| i.net == Some(*index) && matches!(i.shape, pcb::Shape::Pad { .. }))
            {
                return Err("Net still contains pads; execute its pin changes first".into());
            }
            for i in &mut b.items {
                if i.net == Some(*index) {
                    i.net = None;
                }
            }
            for polygon in &mut b.polygon_pours {
                if polygon.net == Some(*index) {
                    polygon.net = None;
                }
            }
            b.nets.retain(|n| n.index != *index);
        }
        Operation::RenameNet(index, name) => {
            b.nets
                .iter_mut()
                .find(|n| n.index == *index)
                .ok_or("Net missing")?
                .name = name.clone()
        }
        Operation::PadNet {
            component,
            pin,
            net,
        } => {
            let c = b
                .components
                .iter()
                .find(|c| c.id == *component)
                .ok_or_else(|| format!("Unknown Pin: component for pin {pin} is missing"))?;
            let pads:Vec<_>=b.items.iter().enumerate().filter(|(_,i)|i.component==Some(c.index)&&matches!(&i.shape,pcb::Shape::Pad{name,..} if name.eq_ignore_ascii_case(pin))).map(|(n,_)|n).collect();
            if pads.is_empty() {
                return Err(format!("Unknown Pin: Pin {}-{pin}", c.designator));
            }
            if pads.iter().any(|&n| b.items[n].locked) {
                return Err("Pad is locked".into());
            }
            let target = match net {
                Some(name) => Some(
                    b.nets
                        .iter()
                        .find(|n| n.name.eq_ignore_ascii_case(name))
                        .ok_or_else(|| format!("Net {name} is missing; enable Add Net"))?
                        .index,
                ),
                None => None,
            };
            for n in pads {
                b.items[n].net = target;
            }
        }
        _ => return Err("Geometry operations require a private candidate".into()),
    }
    b.pours_stale |= !b.polygon_pours.is_empty();
    Ok(())
}

fn apply_pcb_metadata(b: &mut pcb::Board, id: Uuid, value: &Value) -> Result<(), String> {
    let c = b
        .components
        .iter_mut()
        .find(|c| c.id == id)
        .ok_or("Component missing")?;
    if c.locked {
        return Err("Component is locked".into());
    }
    match value {
        Value::Designator(v) => c.designator = v.clone(),
        Value::Comment(v) => c.comment = v.clone(),
        Value::Parameters(v) => c.parameters = Some(v.clone()),
        Value::Description(v) => {
            c.properties.insert("SOURCEDESCRIPTION".into(), v.clone());
        }
        Value::Kind(v) => {
            c.properties
                .insert("COMPONENTKINDVERSION2".into(), v.to_string());
        }
        Value::Link(uid, sheet) => {
            c.properties.insert("SOURCEUNIQUEID".into(), uid.clone());
            c.properties.insert("SOURCESHEET".into(), sheet.clone());
        }
        Value::Footprint(_) => {
            return Err("Footprint geometry requires a staged library transaction".into());
        }
    }
    for i in b.items.iter_mut().filter(|i| i.component == Some(c.index)) {
        if let pcb::Shape::Text { text, .. } = &mut i.shape {
            if i.properties
                .get("DESIGNATOR")
                .is_some_and(|v| v.eq_ignore_ascii_case("TRUE"))
            {
                *text = c.designator.clone();
            }
            if i.properties
                .get("COMMENT")
                .is_some_and(|v| v.eq_ignore_ascii_case("TRUE"))
            {
                *text = c.comment.clone();
            }
        }
    }
    b.pours_stale |= !b.polygon_pours.is_empty();
    Ok(())
}

fn apply(
    op: &Operation,
    pcb_id: Uuid,
    footprints: &[Footprint],
    docs: &mut [Document],
) -> Result<(), String> {
    if let Operation::Unsupported(e) = op {
        return Err(e.clone());
    }
    if let Operation::SchValue(parts, value) = op {
        for (doc, id) in parts {
            let d = docs
                .iter_mut()
                .find(|d| d.id == *doc)
                .ok_or("Schematic document missing")?;
            let c = d
                .schematic
                .as_ref()
                .unwrap()
                .components
                .iter()
                .find(|c| c.id == *id)
                .ok_or("Symbol missing")?;
            let edits = match value {
                Value::Designator(v) => {
                    if c.settings.designator_locked {
                        return Err("Schematic designator is locked".into());
                    }
                    let designator = if project::logical_designator(c) != c.designator {
                        format!("{v}{}", c.designator.chars().last().unwrap())
                    } else {
                        v.clone()
                    };
                    vec![schematic_edit::Edit::ComponentProperties {
                        id: *id,
                        designator,
                        comment: c.comment.clone(),
                    }]
                }
                Value::Comment(v) => vec![schematic_edit::Edit::ComponentProperties {
                    id: *id,
                    designator: c.designator.clone(),
                    comment: v.clone(),
                }],
                Value::Description(v) => vec![schematic_edit::Edit::ComponentDetails {
                    id: *id,
                    description: v.clone(),
                    settings: c.settings.clone(),
                }],
                Value::Kind(v) => {
                    let mut settings = c.settings.clone();
                    settings.kind = *v;
                    vec![schematic_edit::Edit::ComponentDetails {
                        id: *id,
                        description: c.description.clone(),
                        settings,
                    }]
                }
                Value::Footprint(v) => {
                    let mut settings = c.settings.clone();
                    for m in &mut settings.models {
                        if m.kind.eq_ignore_ascii_case("PCBLIB") {
                            m.current = false;
                        }
                    }
                    if let Some(m) = settings
                        .models
                        .iter_mut()
                        .find(|m| m.kind.eq_ignore_ascii_case("PCBLIB") && m.name == *v)
                    {
                        m.current = true;
                    } else {
                        settings.models.push(schematic::ComponentModel {
                            name: v.clone(),
                            kind: "PCBLIB".into(),
                            current: true,
                        });
                    }
                    vec![schematic_edit::Edit::ComponentDetails {
                        id: *id,
                        description: c.description.clone(),
                        settings,
                    }]
                }
                Value::Parameters(v) => {
                    let s = d.schematic.as_ref().unwrap();
                    let protected: BTreeSet<_> = s
                        .primitives
                        .iter()
                        .chain(&s.parameter_objects)
                        .filter(|p| {
                            p.component == Some(*id)
                                && p.presentation.record_type == 41
                                && !p.presentation.allow_sync
                        })
                        .map(|p| p.presentation.name.to_uppercase())
                        .collect();
                    let mut edits = vec![];
                    for k in c.parameters.keys().filter(|k| {
                        !k.eq_ignore_ascii_case("Comment") && !k.eq_ignore_ascii_case("Designator")
                    }) {
                        if !v.contains_key(k) && !protected.contains(&k.to_uppercase()) {
                            edits.push(schematic_edit::Edit::ParameterValue {
                                component: Some(*id),
                                name: k.clone(),
                                value: None,
                            });
                        }
                    }
                    for (k, v) in v {
                        if protected.contains(&k.to_uppercase()) {
                            continue;
                        }
                        edits.push(schematic_edit::Edit::ParameterValue {
                            component: Some(*id),
                            name: k.clone(),
                            value: Some(v.clone()),
                        });
                    }
                    edits
                }
                Value::Link(..) => return Err("Schematic link update unsupported".into()),
            };
            *d = schematic_edit::preview(d, &schematic_edit::Edit::Batch(edits))?;
        }
        return Ok(());
    }
    if let Operation::SchNet { primitives, name } = op {
        for (doc, id) in primitives {
            let d = docs
                .iter_mut()
                .find(|d| d.id == *doc)
                .ok_or("Schematic document missing")?;
            let mut value = d
                .schematic
                .as_ref()
                .unwrap()
                .primitive(*id)
                .ok_or("Net label missing")?
                .clone();
            use schematic::SchPrimitiveKind as K;
            match &mut value.kind {
                K::Text { text, .. } | K::PowerPort { text, .. } => *text = name.clone(),
                K::Port { name: v, .. } => *v = name.clone(),
                _ => return Err("Not a net label".into()),
            }
            if value.presentation.raw_text.is_some() {
                value.presentation.raw_text = Some(name.clone());
            }
            *d = schematic_edit::preview(
                d,
                &schematic_edit::Edit::PrimitiveProperties {
                    id: *id,
                    value: Box::new(value),
                },
            )?;
        }
        return Ok(());
    }
    let b = docs
        .iter_mut()
        .find(|d| d.id == pcb_id)
        .and_then(|d| d.pcb.as_mut())
        .ok_or("PCB missing")?;
    match op {
        Operation::AddComponent(c, id, position) => {
            let f = footprints
                .iter()
                .find(|f| f.name().eq_ignore_ascii_case(&c.footprint))
                .ok_or_else(|| format!("Footprint Not Found {}", c.footprint))?;
            install(b, f, *id, None, *position)?;
            let mut new = b.components.iter().find(|p| p.id == *id).unwrap().clone();
            new.designator = c.designator.clone();
            new.comment = c.comment.clone();
            new.parameters = Some(c.parameters.clone());
            new.properties
                .insert("SOURCEUNIQUEID".into(), c.unique_ids[0].clone());
            new.properties.insert(
                "SOURCEFOOTPRINTLIBRARY".into(),
                f.library_source().unwrap_or_default().into(),
            );
            new.properties
                .insert("SOURCELIBRARYNAME".into(), c.library_source.clone());
            new.properties
                .insert("SOURCELIBREFERENCE".into(), c.library_reference.clone());
            new.properties.insert("SOURCESHEET".into(), c.sheet.clone());
            new.properties
                .insert("COMPONENTKINDVERSION2".into(), c.kind.to_string());
            new.properties
                .insert("SOURCEDESCRIPTION".into(), c.description.clone());
            *b = crate::pcb_tools::edit_component(b, new)?;
        }
        Operation::RemoveComponent(id) => {
            let c = b
                .components
                .iter()
                .find(|c| c.id == *id)
                .ok_or("Component missing")?;
            if c.locked
                || b.items
                    .iter()
                    .any(|i| i.component == Some(c.index) && i.locked)
            {
                return Err("Component or owned primitives are locked".into());
            }
            let index = c.index;
            b.items.retain(|i| i.component != Some(index));
            b.components.retain(|c| c.id != *id);
        }
        Operation::PcbValue(id, value) => {
            let mut c = b
                .components
                .iter()
                .find(|c| c.id == *id)
                .ok_or("Component missing")?
                .clone();
            if c.locked {
                return Err("Component is locked".into());
            }
            match value {
                Value::Designator(v) => c.designator = v.clone(),
                Value::Comment(v) => c.comment = v.clone(),
                Value::Parameters(v) => c.parameters = Some(v.clone()),
                Value::Description(v) => {
                    c.properties.insert("SOURCEDESCRIPTION".into(), v.clone());
                }
                Value::Kind(v) => {
                    c.properties
                        .insert("COMPONENTKINDVERSION2".into(), v.to_string());
                }
                Value::Link(uid, sheet) => {
                    c.properties.insert("SOURCEUNIQUEID".into(), uid.clone());
                    c.properties.insert("SOURCESHEET".into(), sheet.clone());
                }
                Value::Footprint(v) => {
                    let f = footprints
                        .iter()
                        .find(|f| f.name().eq_ignore_ascii_case(v))
                        .ok_or_else(|| format!("Footprint Not Found {v}"))?;
                    install(b, f, *id, Some(&c), c.position)?;
                    c.footprint = v.clone();
                    c.properties.insert(
                        "SOURCEFOOTPRINTLIBRARY".into(),
                        f.library_source().unwrap_or_default().into(),
                    );
                }
            }
            *b = crate::pcb_tools::edit_component(b, c)?;
        }
        Operation::AddNet(_)
        | Operation::RemoveNet(_)
        | Operation::RenameNet(..)
        | Operation::PadNet { .. } => {
            return apply_pcb_scalar(op, b);
        }
        _ => unreachable!(),
    }
    b.pours_stale |= !b.polygon_pours.is_empty();
    b.validate()
}

fn install(
    b: &mut pcb::Board,
    f: &Footprint,
    id: Uuid,
    old: Option<&pcb::Component>,
    position: Point,
) -> Result<(), String> {
    let index = old.map(|c| c.index).unwrap_or_else(|| {
        b.components
            .iter()
            .map(|c| c.index)
            .max()
            .map_or(0, |n| n + 1)
    });
    let mut part = f.board.clone();
    let previous = part.components[0].id;
    if old.is_some_and(|c| c.layer == pcb::BOTTOM) {
        part = crate::pcb_tools::flip_component(&part, previous)?;
    }
    let rotation = part.components[0].rotation;
    part = part.edited(pcb::Edit::Transform {
        selection: vec![previous],
        pivot: Point::default(),
        translation: position,
        angle: old.map_or(0., |c| c.rotation) - rotation,
    })?;
    let old_nets: HashMap<_, _> = b
        .items
        .iter()
        .filter(|i| old.is_some() && i.component == Some(index))
        .filter_map(|i| {
            if let pcb::Shape::Pad { name, .. } = &i.shape {
                Some((name.to_uppercase(), i.net))
            } else {
                None
            }
        })
        .collect();
    if old.is_some() {
        b.items.retain(|i| i.component != Some(index));
        b.components.retain(|c| c.id != id);
    }
    let mut c = part.components.remove(0);
    c.id = id;
    c.index = index;
    c.properties.remove("SOURCEUNIQUEID");
    c.properties.insert(
        "UNIQUEID".into(),
        id.simple().to_string()[..8].to_ascii_uppercase(),
    );
    for i in &mut part.items {
        i.id = Uuid::new_v4();
        i.component = Some(index);
        i.source = None;
        i.properties.remove("UNIQUEID");
        if let pcb::Shape::Pad { name, .. } = &i.shape {
            i.net = old_nets.get(&name.to_uppercase()).copied().flatten();
        }
    }
    for layer in part.layers {
        if !b.layers.iter().any(|l| l.id == layer.id) {
            b.layers.push(layer);
        }
    }
    for (key, model) in part.models {
        if b.models.get(&key).is_some_and(|m| m != &model) {
            return Err(format!("Conflicting 3D model {key}"));
        }
        b.models.insert(key, model);
    }
    b.components.push(c);
    b.items.extend(part.items);
    Ok(())
}
