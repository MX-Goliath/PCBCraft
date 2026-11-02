//! Native project variant definitions, preserving opaque project/variation fields.
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

pub fn parse(text: &str) -> Result<Vec<Variant>, String> {
    let mut groups = vec![];
    let mut fields = BTreeMap::new();
    let mut active = false;
    for line in text.lines().map(str::trim).chain(std::iter::once("[End]")) {
        if line.starts_with('[') && line.ends_with(']') {
            if active {
                groups.push(std::mem::take(&mut fields));
            }
            active = section(&line[1..line.len() - 1]);
        } else if active && let Some((k, v)) = line.split_once('=') {
            fields.insert(k.trim().to_owned(), v.to_owned());
        }
    }
    let mut result = vec![];
    for fields in groups {
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
            if let Some(data) = fields.get(&format!("PCBCraftAlternateData{n}")) {
                let bytes = STANDARD.decode(data).map_err(|e| e.to_string())?;
                c.alternate = Some(serde_json::from_slice(&bytes).map_err(|e| e.to_string())?);
            }
            v.variations.push(c);
        }
        for (key, value) in &fields {
            if let Some(n) = number(key, "paramvariation") {
                let f = tokens(value);
                let designator = get(&fields, &format!("ParamDesignator{n}"));
                let name = get(&f, "ParameterName");
                if !name.is_empty() {
                    v.variation_mut("", designator)
                        .parameters
                        .insert(name.into(), get(&f, "VariantValue").into());
                }
            } else if number(key, "parameter").is_some() {
                let f = tokens(value);
                let name = get(&f, "Name");
                if !name.is_empty() {
                    v.parameters.insert(name.into(), get(&f, "Value").into());
                }
            }
        }
        v.fields = fields;
        result.push(v);
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
    Ok(f.iter()
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
            skip = section(&t[1..t.len() - 1]);
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
            row.insert("Designator".into(), c.designator.clone());
            row.insert("UniqueId".into(), c.uid.clone());
            row.insert("Kind".into(), c.kind.to_string());
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
        for (i, (key, value)) in v.parameters.iter().enumerate() {
            f.insert(
                format!("Parameter{}", i + 1),
                join(&BTreeMap::from([
                    ("Name".into(), key.clone()),
                    ("Value".into(), value.clone()),
                ]))?,
            );
        }
        out.push_str(&format!("\r\n[ProjectVariant{}]\r\n", n + 1));
        for (key, value) in f {
            if key.contains(['\r', '\n', '=']) || value.contains(['\r', '\n']) {
                return Err("Invalid native variant field".into());
            }
            out.push_str(&format!("{key}={value}\r\n"));
        }
    }
    Ok(out)
}
pub fn save(project: &altium::Project, expected: &[u8]) -> Result<Vec<u8>, String> {
    let bytes = std::fs::read(&project.path).map_err(|e| e.to_string())?;
    if bytes != expected {
        return Err("Project changed on disk; reopen before saving variants".into());
    }
    let text = project_files::decode(&bytes)?;
    let next = rewrite(&text, &project.variants)?;
    let bytes = std::iter::once(0xfeffu16)
        .chain(next.encode_utf16())
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    project_files::write_atomic(&project.path, &bytes)?;
    Ok(bytes)
}
