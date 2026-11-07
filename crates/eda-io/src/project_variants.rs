//! Native project variant definitions, preserving opaque project/variation fields.
#[path = "project_variant_library.rs"]
mod alternate_library;
#[cfg(test)]
#[path = "project_variants_tests.rs"]
mod tests;
use crate::{altium, project_files};
use base64::{Engine, engine::general_purpose::STANDARD};
use eda_core::assembly_variants::{Variant, Variation};
use std::{collections::BTreeMap, path::Path};

fn get<'a>(f: &'a BTreeMap<String, String>, key: &str) -> &'a str {
    f.iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(key))
        .map_or("", |(_, v)| v)
}
fn set(f: &mut BTreeMap<String, String>, key: &str, value: String) {
    f.retain(|k, _| !k.eq_ignore_ascii_case(key));
    f.insert(key.into(), value);
}
fn tokens(s: &str) -> BTreeMap<String, String> {
    s.split('|')
        .filter_map(|s| s.split_once('='))
        .map(|(k, v)| (k.to_owned(), v.to_owned()))
        .collect()
}
fn number(key: &str, prefix: &str) -> Option<usize> {
    key.to_ascii_lowercase()
        .strip_prefix(prefix)?
        .parse::<usize>()
        .ok()
}
fn section(s: &str) -> bool {
    number(s, "projectvariant").is_some() || number(s, "variant").is_some()
}
fn parameter_section(s: &str) -> Option<(usize, usize)> {
    let lower = s.to_ascii_lowercase();
    let (owner, index) = lower.strip_prefix("parameter")?.split_once('_')?;
    Some((owner.parse().ok()?, index.parse().ok()?))
}

pub fn parse(text: &str) -> Result<Vec<Variant>, String> {
    let mut groups = vec![];
    let mut fields = BTreeMap::new();
    let mut active = false;
    let mut active_index = 0;
    let mut parameter = None;
    let mut global_parameters = BTreeMap::<(usize, usize), BTreeMap<String, String>>::new();
    for line in text
        .lines()
        .map(str::trim_start)
        .chain(std::iter::once("[End]"))
    {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            if active {
                groups.push((active_index, std::mem::take(&mut fields)));
            }
            active = section(&trimmed[1..trimmed.len() - 1]);
            active_index = number(&trimmed[1..trimmed.len() - 1], "projectvariant")
                .or_else(|| number(&trimmed[1..trimmed.len() - 1], "variant"))
                .unwrap_or(0);
            parameter = parameter_section(&trimmed[1..trimmed.len() - 1]);
        } else if active && let Some((k, v)) = line.split_once('=') {
            fields.insert(k.trim().to_owned(), v.to_owned());
        } else if let Some(key) = parameter
            && let Some((k, v)) = line.split_once('=')
        {
            global_parameters
                .entry(key)
                .or_default()
                .insert(k.trim().into(), v.into());
        }
    }
    let mut result = vec![];
    let mut positions = BTreeMap::new();
    groups.sort_by_key(|(n, _)| *n);
    for (index, fields) in groups {
        let mut v = Variant::new(get(&fields, "Description").to_owned());
        let id = get(&fields, "UniqueID");
        if !id.is_empty() {
            v.id = id.into();
        }
        let mut variations = fields
            .iter()
            .filter_map(|(k, s)| number(k, "variation").map(|n| (n, s)))
            .collect::<Vec<_>>();
        variations.sort_by_key(|(n, _)| *n);
        for (n, s) in variations {
            let f = tokens(s);
            let mut c = Variation {
                uid: get(&f, "UniqueId").into(),
                designator: get(&f, "Designator").into(),
                kind: get(&f, "Kind")
                    .parse()
                    .map_err(|_| "Invalid project variation Kind")?,
                fields: f.clone(),
                ..Default::default()
            };
            let data = get(&fields, &format!("PCBCraftAlternateData{n}"));
            if !data.is_empty() {
                let bytes = STANDARD.decode(data).map_err(|e| e.to_string())?;
                c.alternate = Some(serde_json::from_slice(&bytes).map_err(|e| e.to_string())?);
            }
            v.variations.push(c);
        }
        let mut parameter_variations = fields
            .iter()
            .filter_map(|(key, value)| number(key, "paramvariation").map(|n| (n, value)))
            .collect::<Vec<_>>();
        parameter_variations.sort_by_key(|(n, _)| *n);
        for (n, value) in parameter_variations {
            let f = tokens(value);
            let designator = get(&fields, &format!("ParamDesignator{n}"));
            let uid = get(&fields, &format!("PCBCraftParamUniqueId{n}"));
            let name = get(&f, "ParameterName");
            if !name.is_empty() {
                let matches = v
                    .variations
                    .iter()
                    .filter(|c| c.designator.eq_ignore_ascii_case(designator))
                    .count();
                if uid.is_empty() && matches > 1 {
                    return Err(format!(
                        "Ambiguous native parameter variation for {designator}; physical identity is required"
                    ));
                }
                set(
                    &mut v.variation_mut(uid, designator).parameters,
                    name,
                    get(&f, "VariantValue").into(),
                );
            }
        }
        for (key, value) in &fields {
            if number(key, "parameter").is_some() {
                let f = tokens(value);
                let name = get(&f, "Name");
                if !name.is_empty() {
                    set(&mut v.parameters, name, get(&f, "Value").into());
                }
            }
        }
        v.fields = fields;
        positions.insert(index + 1, result.len());
        result.push(v);
    }
    for ((owner, _), fields) in global_parameters {
        if owner >= 2
            && let Some(v) = positions.get(&owner).and_then(|n| result.get_mut(*n))
        {
            let name = get(&fields, "Name");
            if !name.is_empty() {
                set(&mut v.parameters, name, get(&fields, "Value").into());
                v.parameter_fields
                    .insert(name.to_lowercase(), fields.clone());
            }
        }
    }
    Ok(result)
}

/// Embedded alternates use the same independently decoded SchLib container.
pub fn resolve_alternates(variants: &mut [Variant], path: &Path, warnings: &mut Vec<String>) {
    if !variants
        .iter()
        .flat_map(|v| &v.variations)
        .any(|c| c.kind == 2 && c.alternate.is_none())
    {
        return;
    }
    let path = path.with_extension("PrjPcbVariants");
    match altium::library::load(&path) {
        Ok(lib) => {
            for c in variants
                .iter_mut()
                .flat_map(|v| &mut v.variations)
                .filter(|c| c.kind == 2 && c.alternate.is_none())
            {
                let name = get(&c.fields, "AltLibLink_DesignItemID").trim();
                let matches = lib
                    .symbols
                    .iter()
                    .filter(|s| {
                        s.name.eq_ignore_ascii_case(name)
                            || s.variants.iter().any(|v| {
                                v.fragment
                                    .components
                                    .iter()
                                    .any(|c| c.library_reference.eq_ignore_ascii_case(name))
                            })
                    })
                    .collect::<Vec<_>>();
                if matches.len() == 1 {
                    c.alternate = matches[0]
                        .variants
                        .iter()
                        .find(|v| v.part == 1 && v.display_mode == 0)
                        .map(|v| v.fragment.clone());
                }
                if c.alternate.is_none() {
                    warnings.push(format!(
                        "Alternate symbol unavailable or ambiguous for {} ({name})",
                        c.designator
                    ));
                }
            }
        }
        Err(e) => warnings.push(format!("Project alternate parts: {e}")),
    }
}
fn safe(value: &str) -> Result<(), String> {
    if value.contains(['\r', '\n', '|']) {
        Err("Native variant values cannot contain newlines or |".into())
    } else {
        Ok(())
    }
}
fn join(f: &BTreeMap<String, String>) -> Result<String, String> {
    for (k, v) in f {
        safe(k)?;
        safe(v)?;
    }
    let priority = [
        "Designator",
        "UniqueId",
        "Kind",
        "AlternatePart",
        "ParameterName",
        "VariantValue",
    ];
    let mut fields = f.iter().collect::<Vec<_>>();
    fields.sort_by_key(|(k, _)| {
        (
            priority
                .iter()
                .position(|name| k.eq_ignore_ascii_case(name))
                .unwrap_or(priority.len()),
            k.as_str(),
        )
    });
    Ok(fields
        .into_iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("|"))
}
pub fn rewrite(text: &str, variants: &[Variant]) -> Result<String, String> {
    eda_core::assembly_variants::validate(variants)?;
    let mut out = String::new();
    let mut skip = false;
    for line in text.lines() {
        let t = line.trim();
        if t.starts_with('[') && t.ends_with(']') {
            let name = &t[1..t.len() - 1];
            skip = section(name) || parameter_section(name).is_some_and(|(owner, _)| owner >= 2);
        }
        if !skip {
            out.push_str(line);
            out.push_str("\r\n");
        }
    }
    for (n, v) in variants.iter().enumerate() {
        let mut f = v.fields.clone();
        f.retain(|k, _| {
            let k = k.to_ascii_lowercase();
            !matches!(
                k.as_str(),
                "description"
                    | "uniqueid"
                    | "variationcount"
                    | "paramvariationcount"
                    | "parametercount"
            ) && ![
                "variation",
                "paramvariation",
                "paramdesignator",
                "parameter",
                "pcbcraftalternatedata",
                "pcbcraftparamuniqueid",
            ]
            .iter()
            .any(|p| number(&k, p).is_some())
        });
        safe(&v.name)?;
        f.insert("Description".into(), v.name.clone());
        f.insert("UniqueID".into(), v.id.clone());
        f.insert("VariationCount".into(), v.variations.len().to_string());
        let mut pn = 0;
        for (i, c) in v.variations.iter().enumerate() {
            let mut row = c.fields.clone();
            set(&mut row, "Designator", c.designator.clone());
            set(&mut row, "UniqueId", c.uid.clone());
            set(&mut row, "Kind", c.kind.to_string());
            row.entry("AlternatePart".into())
                .or_insert_with(String::new);
            f.insert(format!("Variation{}", i + 1), join(&row)?);
            if let Some(a) = &c.alternate {
                f.insert(
                    format!("PCBCraftAlternateData{}", i + 1),
                    STANDARD.encode(serde_json::to_vec(a).map_err(|e| e.to_string())?),
                );
            }
            for (key, value) in &c.parameters {
                pn += 1;
                safe(&c.designator)?;
                f.insert(format!("ParamDesignator{pn}"), c.designator.clone());
                if !c.uid.trim_matches('\\').is_empty() {
                    f.insert(format!("PCBCraftParamUniqueId{pn}"), c.uid.clone());
                }
                f.insert(
                    format!("ParamVariation{pn}"),
                    join(&BTreeMap::from([
                        ("ParameterName".into(), key.clone()),
                        ("VariantValue".into(), value.clone()),
                    ]))?,
                );
            }
        }
        f.insert("ParamVariationCount".into(), pn.to_string());
        f.insert("ParameterCount".into(), v.parameters.len().to_string());
        out.push_str(&format!("\r\n[ProjectVariant{}]\r\n", n + 1));
        // The native reader consumes component records before their parameter
        // records. Keep the observed ordering and interleaved designator pairs.
        let mut order = [
            "UniqueID",
            "Description",
            "AllowFabrication",
            "ParameterCount",
            "VariationCount",
        ]
        .map(str::to_owned)
        .to_vec();
        order.extend((1..=v.variations.len()).map(|i| format!("Variation{i}")));
        for i in 1..=pn {
            order.push(format!("ParamVariation{i}"));
            order.push(format!("ParamDesignator{i}"));
        }
        order.push("ParamVariationCount".into());
        let mut ordered = vec![];
        for key in order {
            if let Some(value) = f.remove(&key) {
                ordered.push((key, value));
            }
        }
        ordered.extend(f);
        for (key, value) in ordered {
            if key.contains(['\r', '\n', '=']) || value.contains(['\r', '\n']) {
                return Err("Invalid native variant field".into());
            }
            out.push_str(&format!("{key}={value}\r\n"));
        }
        for (i, (key, value)) in v.parameters.iter().enumerate() {
            safe(key)?;
            safe(value)?;
            let mut fields = v
                .parameter_fields
                .get(&key.to_lowercase())
                .cloned()
                .unwrap_or_default();
            set(&mut fields, "Name", key.clone());
            set(&mut fields, "Value", value.clone());
            out.push_str(&format!("\r\n[Parameter{}_{}]\r\n", n + 2, i + 1));
            for (key, value) in fields {
                if key.contains(['\r', '\n', '=']) || value.contains(['\r', '\n']) {
                    return Err("Invalid native parameter field".into());
                }
                out.push_str(&format!("{key}={value}\r\n"));
            }
        }
    }
    Ok(out)
}
pub fn save(project: &altium::Project, expected: &[u8]) -> Result<Vec<u8>, String> {
    let library = std::fs::read(project.path.with_extension("PrjPcbVariants")).ok();
    save_with_alternates(project, expected, library.as_deref())
}
pub fn save_with_alternates(
    project: &altium::Project,
    expected: &[u8],
    expected_library: Option<&[u8]>,
) -> Result<Vec<u8>, String> {
    let bytes = std::fs::read(&project.path).map_err(|e| e.to_string())?;
    if bytes != expected {
        return Err("Project changed on disk; reopen before saving variants".into());
    }
    let text = project_files::decode(&bytes)?;
    let (variants, library) = alternate_library::prepare(project, &project.path)?;
    let next = rewrite(&text, &variants)?;
    let bytes = std::iter::once(0xfeffu16)
        .chain(next.encode_utf16())
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    write_bundle(&project.path, &bytes, library.as_deref(), expected_library)?;
    Ok(bytes)
}

pub(crate) fn prepare_save_as(
    project: &altium::Project,
    destination: &Path,
) -> Result<(Vec<Variant>, Option<Vec<u8>>), String> {
    alternate_library::prepare(project, destination)
}
pub(crate) fn write_bundle(
    path: &Path,
    bytes: &[u8],
    library: Option<&[u8]>,
    expected_library: Option<&[u8]>,
) -> Result<(), String> {
    let library_path = path.with_extension("PrjPcbVariants");
    if let Some(library) = library {
        let current = std::fs::read(&library_path).ok();
        if current.as_deref() != expected_library {
            return Err("Project alternate library changed on disk; reopen before saving".into());
        }
        project_files::write_atomic(&library_path, library)?;
        if let Err(e) = project_files::write_atomic(path, bytes) {
            let rollback = if let Some(original) = current {
                project_files::write_atomic(&library_path, &original)
            } else {
                std::fs::remove_file(&library_path).map_err(|e| e.to_string())
            };
            return Err(format!(
                "{e}{}",
                rollback.err().map_or(String::new(), |e| format!(
                    "; alternate library rollback failed: {e}"
                ))
            ));
        }
        return Ok(());
    }
    project_files::write_atomic(path, bytes)
}
