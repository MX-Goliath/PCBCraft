//! Read-only project compilation and electrical rule checks. No GUI or file access.
use crate::{
    Document, design_sync,
    schematic::{SchPrimitive, SchPrimitiveKind as K},
    schematic_compile, schematic_edit,
};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Warning,
    Error,
    Fatal,
}
impl Severity {
    pub fn label(self) -> &'static str {
        match self {
            Self::Warning => "Warning",
            Self::Error => "Error",
            Self::Fatal => "Fatal Error",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Target {
    pub document: Uuid,
    pub object: Uuid,
    pub description: String,
}
#[derive(Clone, Debug)]
pub struct Diagnostic {
    pub severity: Severity,
    pub message: String,
    pub targets: Vec<Target>,
}
#[derive(Default)]
pub struct Report {
    pub diagnostics: Vec<Diagnostic>,
    pub compiled: bool,
}

/// Native ERC matrix: eight pins, four ports, four sheet entries, unconnected.
/// N/W/E/F mean no report, warning, error and fatal error respectively.
#[derive(Clone, Debug)]
pub struct Options {
    pub connection_matrix: [[Option<Severity>; 17]; 17],
    /// Native Electrical Rules Check levels. Unknown IDs are retained for
    /// round trips, but only independently observed rule mappings are applied.
    pub reporting_levels: BTreeMap<u16, Option<Severity>>,
}
impl Default for Options {
    fn default() -> Self {
        Self::from_native_matrix(&[
            "NNNNNNNNNNNWNNNWW",
            "NNWNNNNWWWNWNWNWN",
            "NWEENEEEENEWNEEWN",
            "NNENNNWEENNWNENWN",
            "NNNNNNNNNNNNNNNNN",
            "NNENNNNEENNWNENWN",
            "NNEWNNWEENNWNENWN",
            "NWEENEENEEENNEENN",
            "NWEENEEEENEWNEEWW",
            "NWNNNNNENNEWNNEWN",
            "NNENNNNEEENWNENWN",
            "WWWWNWWNWWWNWWWNN",
            "NNNNNNNNNNNWNNNWW",
            "NWEENEEEENEWNEEWW",
            "NNENNNNEEENWNENWW",
            "WWWWNWWNWWWNWWWNW",
            "WNNNNNNNWNNNWWWWN",
        ])
        .expect("static ERC matrix")
    }
}
impl Options {
    pub fn from_native_matrix(rows: &[&str]) -> Result<Self, String> {
        if !matches!(rows.len(), 8 | 17) {
            return Err("Incomplete ERC connection matrix".into());
        }
        let mut connection_matrix = [[None; 17]; 17];
        for (i, row) in rows.iter().enumerate() {
            if row.len() < rows.len() {
                return Err("Incomplete ERC connection matrix row".into());
            }
            for (j, value) in row.bytes().take(17).enumerate() {
                connection_matrix[i][j] = match value.to_ascii_uppercase() {
                    b'N' => None,
                    b'W' => Some(Severity::Warning),
                    b'E' => Some(Severity::Error),
                    b'F' => Some(Severity::Fatal),
                    _ => return Err("Unknown ERC connection matrix value".into()),
                };
            }
        }
        Ok(Self {
            connection_matrix,
            reporting_levels: BTreeMap::new(),
        })
    }
    pub fn set_native_level(&mut self, rule: u16, level: u8) -> Result<(), String> {
        let severity = match level {
            0 => None,
            1 => Some(Severity::Warning),
            2 => Some(Severity::Error),
            3 => Some(Severity::Fatal),
            _ => {
                return Err(format!(
                    "Invalid ERC reporting level {level} for Type{rule}"
                ));
            }
        };
        self.reporting_levels.insert(rule, severity);
        Ok(())
    }
    fn level(&self, rule: u16, default: Severity) -> Option<Severity> {
        self.reporting_levels
            .get(&rule)
            .copied()
            .unwrap_or(Some(default))
    }
}
pub const CONNECTION_TYPES: [&str; 17] = [
    "Input Pin",
    "I/O Pin",
    "Output Pin",
    "Open Collector Pin",
    "Passive Pin",
    "HiZ Pin",
    "Open Emitter Pin",
    "Power Pin",
    "Input Port",
    "Output Port",
    "Bidirectional Port",
    "Unspecified Port",
    "Input Sheet Entry",
    "Output Sheet Entry",
    "Bidirectional Sheet Entry",
    "Unspecified Sheet Entry",
    "Unconnected",
];
fn target(doc: &Document, p: &SchPrimitive) -> Target {
    let s = doc.schematic.as_ref().unwrap();
    let description = match &p.kind {
        K::Pin { number, .. } => {
            let c = s.components.iter().find(|c| Some(c.id) == p.component);
            format!("Pin {}-{number}", c.map_or("?", |c| c.designator.as_str()),)
        }
        K::Polyline { electrical, .. } => format!("{electrical} on {}", doc.name),
        K::Port { name, .. } => format!("Port {name}"),
        K::SheetEntry { name, .. } => format!("Sheet Entry {name}"),
        K::PowerPort { text, .. } => format!("Power Object {text}"),
        K::Text { text, .. } => format!("Net Label {text}"),
        _ => format!("Object on {}", doc.name),
    };
    Target {
        document: doc.id,
        object: p.id,
        description,
    }
}
fn push(report: &mut Report, severity: Severity, message: String, targets: Vec<Target>) {
    report.diagnostics.push(Diagnostic {
        severity,
        message,
        targets,
    });
}

/// Compile snapshots without changing their model, history, annotation or source.
/// Unsupported compiler semantics are fatal diagnostics, never a clean result.
pub fn validate(
    sheets: &[&Document],
    project: &design_sync::ProjectOptions,
    options: &Options,
) -> Report {
    let mut report = Report::default();
    if sheets.is_empty() {
        push(
            &mut report,
            Severity::Fatal,
            "No schematic documents in project".into(),
            vec![],
        );
        return report;
    }
    for doc in sheets {
        if let Err(error) = doc.validate() {
            push(
                &mut report,
                Severity::Fatal,
                format!("{}: {error}", doc.name),
                vec![],
            );
        }
        if doc.schematic.is_none() {
            push(
                &mut report,
                Severity::Fatal,
                format!("{} is not a schematic", doc.name),
                vec![],
            );
        }
    }
    if !report.diagnostics.is_empty() {
        return report;
    }
    let mut components: BTreeMap<String, Vec<(&Document, &crate::schematic::Component)>> =
        BTreeMap::new();
    let mut owners = BTreeMap::new();
    for doc in sheets {
        let s = doc.schematic.as_ref().unwrap();
        let excluded = schematic_compile::exclusions(s);
        for c in s.components.iter().filter(|c| {
            !matches!(c.settings.kind, 1 | 2 | 6) && !excluded.components.contains(&c.id)
        }) {
            owners.insert((doc.id, c.id), (doc.id, c.id));
            let t = Target {
                document: doc.id,
                object: c.id,
                description: format!("Component {}", c.designator),
            };
            if c.designator.trim().is_empty() || c.designator.contains('?') {
                push(
                    &mut report,
                    Severity::Error,
                    format!("Component {} has an unannotated designator", c.designator),
                    vec![t],
                );
            } else {
                components
                    .entry(c.designator.to_uppercase())
                    .or_default()
                    .push((doc, c));
            }
        }
    }
    for (name, group) in components {
        let first = group[0].1;
        let mut parts = BTreeSet::new();
        let multipart = group.iter().all(|(_, c)| {
            c.settings.part_count > 1
                && c.settings.part_count == first.settings.part_count
                && c.library_reference == first.library_reference
                && c.settings.kind == first.settings.kind
                && parts.insert(c.part)
        });
        if group.len() > 1 && multipart {
            for (d, c) in &group {
                owners.insert((d.id, c.id), (group[0].0.id, first.id));
            }
        }
        if group.len() > 1 && !multipart {
            push(
                &mut report,
                Severity::Error,
                format!("Duplicate Component Designators {name}"),
                group
                    .iter()
                    .map(|(d, c)| Target {
                        document: d.id,
                        object: c.id,
                        description: format!("Component {} on {}", c.designator, d.name),
                    })
                    .collect(),
            );
        }
    }
    let mut compile_options = project.clone();
    compile_options.review_annotation = true;
    compile_options.report_label_conflicts = true;
    // ERC needs one-pin islands even when interchange output omits them.
    compile_options.single_pin_nets = true;
    let netlist = match design_sync::compile_project(sheets, &compile_options) {
        Ok(n) => n,
        Err(error) => {
            push(&mut report, Severity::Fatal, error, vec![]);
            return report;
        }
    };
    for issue in &netlist.issues {
        push(&mut report, Severity::Error, issue.clone(), vec![]);
    }
    let objects: BTreeMap<_, _> = sheets
        .iter()
        .flat_map(|d| {
            d.schematic
                .as_ref()
                .unwrap()
                .primitives
                .iter()
                .chain(&d.schematic.as_ref().unwrap().parameter_objects)
                .map(move |p| ((d.id, p.id), (*d, p)))
        })
        .collect();
    let tolerance = project.connection_precision_nm.unwrap_or(0);
    let markers: BTreeMap<_, Vec<_>> = sheets
        .iter()
        .map(|d| {
            let s = d.schematic.as_ref().unwrap();
            let excluded = schematic_compile::exclusions(s);
            (
                d.id,
                s.primitives
                    .iter()
                    .filter(|m| !excluded.primitives.contains(&m.id) && !m.presentation.hidden)
                    .filter_map(|m| {
                        if let K::NoErc { position } = m.kind {
                            Some(position)
                        } else {
                            None
                        }
                    })
                    .collect(),
            )
        })
        .collect();
    let suppressed: BTreeSet<_> = objects
        .iter()
        .filter(|(_, (doc, p))| {
            let endpoint = schematic_edit::electrical_endpoints(p).any(|at| {
                markers[&doc.id].iter().any(|position| {
                    (position.x - at.x).abs() <= tolerance && (position.y - at.y).abs() <= tolerance
                })
            });
            endpoint || matches!(&p.kind,K::Polyline{points,electrical,..} if electrical=="Wire" && markers[&doc.id].iter().any(|at| points.windows(2).any(|e|at.on_segment(e[0],e[1]))))
        })
        .map(|(id, _)| *id)
        .collect();
    let pin_nets: BTreeMap<_, _> = netlist
        .pins
        .iter()
        .map(|p| {
            (
                (p.sheet, p.component, p.number.to_uppercase()),
                p.net.clone(),
            )
        })
        .collect();
    for conflict in &netlist.label_conflicts {
        let Some(severity) = options.level(14, Severity::Warning) else {
            continue;
        };
        if conflict.objects.iter().any(|id| {
            suppressed.contains(id)
                && objects.get(id).is_some_and(
                    |(_, p)| matches!(&p.kind,K::Polyline{electrical,..}if electrical=="Wire"),
                )
        }) {
            continue;
        }
        let mut names: BTreeMap<String, Vec<Target>> = BTreeMap::new();
        for (d, p) in conflict.objects.iter().filter_map(|id| objects.get(id)) {
            if matches!(
                p.kind,
                K::PowerPort { .. } | K::Port { .. } | K::SheetEntry { .. }
            ) || matches!(p.text_role, Some(crate::schematic::TextRole::NetLabel))
            {
                let t = target(d, p);
                names.entry(t.description.clone()).or_default().push(t);
            }
        }
        let mut names: Vec<_> = names.into_iter().collect();
        names.sort_by(|a, b| design_sync::natural_order(&a.0, &b.0));
        let descriptions = names
            .iter()
            .map(|(name, items)| {
                if items.len() > 1 {
                    format!("{name} ({})", items.len())
                } else {
                    name.clone()
                }
            })
            .collect::<Vec<_>>();
        push(
            &mut report,
            severity,
            format!(
                "Nets Wire {} has multiple names ({})",
                conflict.net,
                descriptions.join(", ")
            ),
            names.into_iter().flat_map(|(_, items)| items).collect(),
        );
    }
    for island in &netlist.electrical_islands {
        let nodes: Vec<_> = island
            .iter()
            .filter_map(|id| objects.get(id).copied())
            .collect();
        if nodes.iter().any(|(d, p)| {
            suppressed.contains(&(d.id, p.id))
                && matches!(&p.kind,K::Polyline{electrical,..} if electrical=="Wire")
        }) {
            continue;
        }
        let mut physical_pins = BTreeSet::new();
        let mut pins: Vec<_> = nodes
            .iter()
            .copied()
            .filter(|(_, p)| matches!(p.kind, K::Pin { .. }))
            .filter(|(d, p)| {
                if let K::Pin { number, .. } = &p.kind {
                    let owner = p
                        .component
                        .and_then(|c| owners.get(&(d.id, c)).copied())
                        .unwrap_or((d.id, p.id));
                    physical_pins.insert((owner, number.to_uppercase()))
                } else {
                    false
                }
            })
            .collect();
        pins.sort_by(|(a, ap), (b, bp)| {
            design_sync::natural_order(&target(a, ap).description, &target(b, bp).description)
        });
        let active_pins: Vec<_> = pins
            .iter()
            .copied()
            .filter(|(d, p)| !suppressed.contains(&(d.id, p.id)))
            .collect();
        let targets = || pins.iter().map(|(d, p)| target(d, p)).collect::<Vec<_>>();
        let name = pins
            .iter()
            .find_map(|(d, p)| {
                if let K::Pin { number, .. } = &p.kind {
                    p.component.and_then(|c| {
                        pin_nets
                            .get(&(d.id, c, number.to_uppercase()))
                            .cloned()
                            .flatten()
                    })
                } else {
                    None
                }
            })
            .or_else(|| {
                nodes.iter().find_map(|(_, p)| match &p.kind {
                    K::PowerPort { text, .. } | K::Text { text, .. } => Some(text.clone()),
                    K::Port { name, .. } | K::SheetEntry { name, .. } => Some(name.clone()),
                    _ => None,
                })
            })
            .unwrap_or_else(|| "<unnamed>".into());
        if pins.len() == 1
            && nodes.iter().any(|(_, p)| !matches!(p.kind, K::Pin { .. }))
            && !active_pins.is_empty()
        {
            push(
                &mut report,
                Severity::Warning,
                format!(
                    "Net {name} has only one pin ({})",
                    target(pins[0].0, pins[0].1).description
                ),
                targets(),
            );
        }
        let driven = pins.iter().any(|(_, p)| {
            matches!(
                p.kind,
                K::Pin {
                    electrical: 1 | 2 | 3 | 5 | 6 | 7,
                    ..
                }
            )
        }) || nodes.iter().any(|(_, p)| {
            matches!(
                p.kind,
                K::Port { io_type: 2 | 3, .. } | K::SheetEntry { io_type: 1 | 3, .. }
            )
        });
        let requires_driver = pins
            .iter()
            .any(|(_, p)| matches!(p.kind, K::Pin { electrical: 0, .. }))
            || nodes.iter().any(|(_, p)| {
                matches!(
                    p.kind,
                    K::Port { io_type: 1, .. } | K::SheetEntry { io_type: 2, .. }
                )
            });
        if requires_driver
            && !driven
            && pins.len() > 1
            && !active_pins.is_empty()
            && let Some(severity) = options.level(10, Severity::Warning)
        {
            push(
                &mut report,
                severity,
                format!(
                    "Net {name} has no driving source ({})",
                    pins.iter()
                        .map(|(d, p)| pin_label(d, p))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                targets(),
            );
        }
        for &(doc, p) in &active_pins {
            if pins.len() == 1 && nodes.iter().all(|(_, p)| matches!(p.kind, K::Pin { .. })) {
                let K::Pin { electrical, .. } = p.kind else {
                    unreachable!()
                };
                let Some(severity) = options
                    .connection_matrix
                    .get(electrical as usize)
                    .and_then(|r| r[16])
                else {
                    continue;
                };
                push(
                    &mut report,
                    severity,
                    format!("Unconnected {}", target(doc, p).description),
                    vec![target(doc, p)],
                );
            }
        }
        // One diagnostic per type pair on a net; large buses must not generate
        // quadratic numbers of pairwise messages.
        let mut types: BTreeMap<usize, Vec<Target>> = BTreeMap::new();
        for (d, p) in active_pins {
            if let K::Pin { electrical, .. } = p.kind {
                if electrical > 7 {
                    push(
                        &mut report,
                        Severity::Fatal,
                        format!("Unsupported pin electrical type {electrical}"),
                        vec![target(d, p)],
                    );
                } else {
                    types
                        .entry(electrical as usize)
                        .or_default()
                        .push(target(d, p));
                }
            }
        }
        for (d, p) in &nodes {
            if suppressed.contains(&(d.id, p.id)) {
                continue;
            }
            let index = match p.kind {
                K::Port { io_type, .. } => Some(
                    8 + match io_type {
                        2 => 0,
                        1 => 1,
                        3 => 2,
                        _ => 3,
                    },
                ),
                K::SheetEntry { io_type, .. } => Some(
                    12 + match io_type {
                        2 => 0,
                        1 => 1,
                        3 => 2,
                        _ => 3,
                    },
                ),
                _ => None,
            };
            if let Some(index) = index {
                types.entry(index).or_default().push(target(d, p));
            }
        }
        for (&a, ap) in &types {
            if pins.is_empty()
                && types.values().map(Vec::len).sum::<usize>() == 1
                && nodes.iter().all(
                    |(_, p)| !matches!(&p.kind,K::Polyline{electrical,..} if electrical=="Wire"),
                )
                && let Some(severity) = options.connection_matrix[a][16]
            {
                push(
                    &mut report,
                    severity,
                    format!("Unconnected {}", ap[0].description),
                    ap.clone(),
                );
            }
            for (&b, bp) in types.range(a..) {
                if a == b && ap.len() < 2 {
                    continue;
                }
                if let Some(severity) = options.connection_matrix[a][b] {
                    let mut pair = ap.clone();
                    if a != b {
                        pair.extend(bp.clone());
                    }
                    push(
                        &mut report,
                        severity,
                        format!(
                            "Net {name} contains {0} and {1} objects",
                            CONNECTION_TYPES[a], CONNECTION_TYPES[b]
                        ),
                        pair,
                    );
                }
            }
        }
    }
    report.compiled = !report
        .diagnostics
        .iter()
        .any(|d| d.severity == Severity::Fatal);
    report.diagnostics.sort_by(|a, b| {
        b.severity
            .cmp(&a.severity)
            .then_with(|| {
                a.message
                    .contains("has multiple names")
                    .cmp(&b.message.contains("has multiple names"))
            })
            .then_with(|| design_sync::natural_order(&a.message, &b.message))
    });
    report
}
fn pin_label(doc: &Document, p: &SchPrimitive) -> String {
    if let K::Pin { number, .. } = &p.kind {
        let c = doc
            .schematic
            .as_ref()
            .unwrap()
            .components
            .iter()
            .find(|c| Some(c.id) == p.component);
        format!("Pin {}-{number}", c.map_or("?", |c| c.designator.as_str()))
    } else {
        target(doc, p).description
    }
}

#[cfg(test)]
#[path = "schematic_validation_tests.rs"]
mod tests;
