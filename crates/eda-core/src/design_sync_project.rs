//! Project compilation settings and resolved sheet identities, without file access.
use super::*;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NetScope {
    #[default]
    Automatic,
    Flat,
    Hierarchical,
    StrictHierarchical,
    Global,
}

#[derive(Clone, Debug)]
pub struct SheetLink {
    pub parent: Uuid,
    pub symbol: Uuid,
    pub child: Uuid,
}

#[derive(Clone, Debug)]
pub struct ProjectOptions {
    /// None is the legacy standalone API; project callers resolve relative paths.
    pub links: Option<Vec<SheetLink>>,
    pub scope: NetScope,
    pub allow_port_names: bool,
    pub allow_entry_names: bool,
    pub power_names_priority: bool,
    pub higher_level_names_priority: bool,
    pub hierarchical_net_names: bool,
    pub append_sheet_number: bool,
    pub single_pin_nets: bool,
    pub merge_duplicate_net_names: bool,
    /// Electrical compilation precision for native schematic coordinates.
    /// Drawing/saving and standalone exact-geometry operations remain unchanged.
    pub connection_precision_nm: Option<i64>,
    pub sheet_numbers: BTreeMap<Uuid, usize>,
    pub sheet_paths: BTreeMap<Uuid, String>,
    /// ECO review can diagnose incomplete annotation per component while retaining identities.
    pub review_annotation: bool,
    pub unsupported_connectivity: Option<String>,
}
impl Default for ProjectOptions {
    fn default() -> Self {
        Self {
            links: None,
            scope: NetScope::Automatic,
            allow_port_names: true,
            allow_entry_names: true,
            power_names_priority: true,
            higher_level_names_priority: true,
            hierarchical_net_names: true,
            append_sheet_number: false,
            single_pin_nets: true,
            merge_duplicate_net_names: false,
            connection_precision_nm: None,
            sheet_numbers: BTreeMap::new(),
            sheet_paths: BTreeMap::new(),
            review_annotation: false,
            unsupported_connectivity: None,
        }
    }
}

pub(super) struct Hierarchy {
    pub links: Vec<SheetLink>,
    pub depths: HashMap<Uuid, usize>,
    pub paths: HashMap<Uuid, String>,
    pub scope: NetScope,
}
pub(super) fn hierarchy(
    sheets: &[&Document],
    options: &ProjectOptions,
) -> Result<Hierarchy, String> {
    let mut links = vec![];
    for doc in sheets {
        let s = doc.schematic.as_ref().ok_or("Not a schematic")?;
        let excluded = crate::schematic_compile::exclusions(s);
        for symbol in &s.primitives {
            if let schematic::SchPrimitiveKind::SheetSymbol { filename, .. } = &symbol.kind {
                if excluded.primitives.contains(&symbol.id) {
                    return Err("A compile-masked Sheet Symbol requires whole-sheet instance exclusion, which is not yet supported".into());
                }
                let child = if let Some(resolved) = &options.links {
                    let matches: Vec<_> = resolved
                        .iter()
                        .filter(|l| l.parent == doc.id && l.symbol == symbol.id)
                        .collect();
                    if matches.len() != 1 {
                        return Err(format!(
                            "Sheet Symbol {filename} has no unique resolved project child"
                        ));
                    }
                    matches[0].child
                } else {
                    let filename = filename.rsplit(['/', '\\']).next().unwrap_or(filename);
                    let matches: Vec<_> = sheets
                        .iter()
                        .filter(|d| d.name.eq_ignore_ascii_case(filename))
                        .collect();
                    if matches.len() != 1 {
                        return Err(format!(
                            "Sheet Symbol file {filename} does not resolve to one project sheet"
                        ));
                    }
                    matches[0].id
                };
                if !sheets.iter().any(|d| d.id == child) {
                    return Err("Resolved child schematic is missing".into());
                }
                links.push(SheetLink {
                    parent: doc.id,
                    symbol: symbol.id,
                    child,
                });
            }
        }
    }
    let mut parents = HashMap::new();
    for link in &links {
        if parents.insert(link.child, link).is_some() {
            return Err(
                "Repeated sheet instances require multichannel annotation before synchronization"
                    .into(),
            );
        }
    }
    let mut depths = HashMap::new();
    let mut paths = HashMap::new();
    for doc in sheets {
        let mut at = doc.id;
        let mut seen = HashSet::new();
        let mut path = vec![];
        while let Some(link) = parents.get(&at) {
            if !seen.insert(at) || seen.len() > 64 {
                return Err("Cyclic or excessive schematic hierarchy".into());
            }
            let parent = sheets
                .iter()
                .find(|d| d.id == link.parent)
                .ok_or("Parent sheet missing")?
                .schematic
                .as_ref()
                .unwrap();
            let p = parent
                .primitive(link.symbol)
                .ok_or("Sheet Symbol missing")?;
            let uid = parent
                .records
                .iter()
                .find(|r| r.index == p.source_index)
                .and_then(|r| field(&r.fields, "UNIQUEID"))
                .map(str::to_owned)
                .unwrap_or_else(|| p.id.to_string());
            path.push(uid);
            at = link.parent;
        }
        depths.insert(doc.id, seen.len());
        path.reverse();
        paths.insert(
            doc.id,
            if path.is_empty() {
                String::new()
            } else {
                format!("\\{}", path.join("\\"))
            },
        );
    }
    let scope =
        if options.scope == NetScope::Automatic {
            let entries =
                sheets.iter().any(|d| {
                    !parents.contains_key(&d.id)
                        && d.schematic.as_ref().unwrap().primitives.iter().any(|p| {
                            matches!(p.kind, schematic::SchPrimitiveKind::SheetEntry { .. })
                        })
                });
            if entries {
                NetScope::Hierarchical
            } else if sheets.iter().any(|d| {
                d.schematic
                    .as_ref()
                    .unwrap()
                    .primitives
                    .iter()
                    .any(|p| matches!(p.kind, schematic::SchPrimitiveKind::Port { .. }))
            }) {
                NetScope::Flat
            } else if options.links.is_none() {
                // Standalone callers have no project's Automatic/Global setting.
                NetScope::Flat
            } else {
                NetScope::Global
            }
        } else {
            options.scope
        };
    Ok(Hierarchy {
        links,
        depths,
        paths,
        scope,
    })
}

pub(super) fn logical_designator(c: &schematic::Component) -> &str {
    if c.settings.part_count > 1
        && let Some(suffix) = char::from_u32('A' as u32 + c.part - 1)
    {
        c.designator.strip_suffix(suffix).unwrap_or(&c.designator)
    } else {
        &c.designator
    }
}

pub(super) fn natural_order(a: &str, b: &str) -> std::cmp::Ordering {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    let (mut i, mut j) = (0, 0);
    while i < a.len() && j < b.len() {
        if a[i].is_ascii_digit() && b[j].is_ascii_digit() {
            let (mut x, mut y) = (i, j);
            while i < a.len() && a[i].is_ascii_digit() {
                i += 1;
            }
            while j < b.len() && b[j].is_ascii_digit() {
                j += 1;
            }
            while x < i && a[x] == b'0' {
                x += 1;
            }
            while y < j && b[y] == b'0' {
                y += 1;
            }
            let order = (i - x).cmp(&(j - y)).then_with(|| a[x..i].cmp(&b[y..j]));
            if order != std::cmp::Ordering::Equal {
                return order;
            }
        } else {
            let order = a[i].to_ascii_uppercase().cmp(&b[j].to_ascii_uppercase());
            if order != std::cmp::Ordering::Equal {
                return order;
            }
            i += 1;
            j += 1;
        }
    }
    (a.len() - i).cmp(&(b.len() - j))
}
