//! Harness objects use the Additional stream and its local owner index space.
use super::*;

pub(super) fn harness(p: &SchPrimitive) -> bool {
    matches!(
        p.kind,
        K::HarnessConnector { .. } | K::HarnessEntry { .. } | K::Blanket { .. }
    ) || p.presentation.record_type == 217
        || matches!(&p.kind,K::Polyline{electrical,..} if electrical=="Signal Harness")
}
pub(super) fn changes(
    patch: &mut Patch,
    source: &Schematic,
    target: &Schematic,
) -> Result<(), String> {
    for old in source
        .primitives
        .iter()
        .chain(&source.parameter_objects)
        .filter(|p| harness(p))
    {
        if target.primitive(old.id).is_none() {
            patch.records[old.source_index as usize] = None;
        }
    }
    let mut indices: BTreeMap<Uuid, u32> = target
        .primitives
        .iter()
        .filter(|p| p.source_index != eda_core::schematic_edit::AUTHORED)
        .map(|p| (p.id, p.source_index))
        .collect();
    // Allocate connectors before owned entries/type labels, regardless of model draw order.
    for connector_pass in [true, false] {
        for p in target
            .primitives
            .iter()
            .chain(&target.parameter_objects)
            .filter(|p| harness(p))
        {
            if matches!(p.kind, K::HarnessConnector { .. }) != connector_pass
                || source.primitive(p.id) == Some(p)
            {
                continue;
            }
            let kind = match p.kind {
                K::HarnessConnector { .. } => 215,
                K::Blanket { .. } => 225,
                K::HarnessEntry { .. } => 216,
                K::Text { .. } => 217,
                _ => 218,
            };
            let index = if let Some(old) = source.primitive(p.id) {
                old.source_index
            } else {
                let owner = p
                    .presentation
                    .parent
                    .map(|id| indices.get(&id).copied().ok_or("Missing harness owner"))
                    .transpose()?
                    .unwrap_or(0);
                let index = patch.add(kind, owner);
                patch.authored.insert(index, p.id);
                indices.insert(p.id, index);
                let uid = patch.fresh_uid(owner, kind, &p.id.to_string())?;
                let f = patch.added.last_mut().unwrap();
                set(f, "UNIQUEID", uid);
                if p.presentation.parent.is_some() {
                    set(f, "OWNERINDEXADDITIONALLIST", "T".into());
                }
                index
            };
            let count = patch.records.len();
            let f = if (index as usize) < count {
                patch.record(index)?
            } else {
                &mut patch.added[index as usize - count]
            };
            match &p.kind {
                K::Blanket { points, collapsed } => {
                    let min = Point {
                        x: points.iter().map(|p| p.x).min().unwrap(),
                        y: points.iter().map(|p| p.y).min().unwrap(),
                    };
                    let max = Point {
                        x: points.iter().map(|p| p.x).max().unwrap(),
                        y: points.iter().map(|p| p.y).max().unwrap(),
                    };
                    location(f, min);
                    coordinate_units(f, "CORNER.X", native_units(max.x));
                    coordinate_units(f, "CORNER.Y", native_units(max.y));
                    set(f, "LOCATIONCOUNT", points.len().to_string());
                    for (n, p) in points.iter().enumerate() {
                        coordinate_units(f, &format!("X{}", n + 1), native_units(p.x));
                        coordinate_units(f, &format!("Y{}", n + 1), native_units(p.y));
                    }
                    set(f, "COLLAPSED", boolean(*collapsed));
                    set(f, "LINESTYLE", p.line_style.to_string());
                    set(f, "LINESTYLEEXT", p.line_style.to_string());
                }

                K::HarnessConnector {
                    a,
                    b,
                    side,
                    primary_nm,
                    bus_text_style,
                } => {
                    location(f, *a);
                    coordinate_units(f, "XSIZE", native_units(b.x) - native_units(a.x));
                    coordinate_units(f, "YSIZE", native_units(a.y) - native_units(b.y));
                    coordinate_units(f, "PRIMARYCONNECTIONPOSITION", native_units(*primary_nm));
                    set(f, "HARNESSCONNECTORSIDE", side.to_string());
                    set(f, "BUSTEXTSTYLE", bus_text_style.clone());
                }
                K::HarnessEntry {
                    position,
                    side,
                    name,
                    font,
                    text_color,
                } => {
                    let parent = target
                        .primitive(p.presentation.parent.ok_or("Missing Harness Connector")?)
                        .ok_or("Missing Harness Connector")?;
                    let K::HarnessConnector { a, .. } = parent.kind else {
                        return Err("Invalid Harness Entry owner".into());
                    };
                    let distance = if *side <= 1 {
                        native_units(a.y) - native_units(position.y)
                    } else {
                        native_units(position.x) - native_units(a.x)
                    };
                    set(f, "DISTANCEFROMTOP", (distance / 1_000_000).to_string());
                    set(
                        f,
                        "DISTANCEFROMTOP_FRAC1",
                        (distance % 1_000_000).to_string(),
                    );
                    set(f, "SIDE", side.to_string());
                    set(f, "NAME", name.clone());
                    set(f, "TEXTFONTID", (font + 1).to_string());
                    set(f, "TEXTCOLOR", text_color.to_string());
                    set(f, "HARNESSTYPE", p.presentation.harness_type.clone());
                    set(f, "TEXTSTYLE", "Full".into());
                }
                K::Text {
                    position,
                    text,
                    font,
                    orientation,
                    justification,
                } => {
                    location(f, *position);
                    set(f, "TEXT", text.clone());
                    set(f, "FONTID", (font + 1).to_string());
                    set(f, "ORIENTATION", orientation.to_string());
                    set(f, "JUSTIFICATION", justification.to_string());
                    set(f, "ISHIDDEN", boolean(p.presentation.hidden));
                    set(f, "NOTAUTOPOSITION", "T".into());
                }
                K::Polyline { points, .. } => {
                    set(f, "LOCATIONCOUNT", points.len().to_string());
                    for (n, p) in points.iter().enumerate() {
                        coordinate_units(f, &format!("X{}", n + 1), native_units(p.x));
                        coordinate_units(f, &format!("Y{}", n + 1), native_units(p.y));
                    }
                }
                _ => unreachable!(),
            }
            set(f, "COLOR", p.color.to_string());
            set(f, "DISABLESYNC", boolean(!p.presentation.allow_sync));
            set(f, "ISREADONLY", boolean(p.presentation.locked));
            if let Some(fill) = p.fill {
                set(f, "AREACOLOR", fill.to_string());
            }
            set(f, "TRANSPARENT", boolean(p.fill.is_none()));
            set(
                f,
                "LINEWIDTH",
                match p.width_nm {
                    101_600 => "0",
                    254_000 => "1",
                    508_000 => "2",
                    1_016_000 => "3",
                    _ => return Err("Unsupported native harness width".into()),
                }
                .into(),
            );
        }
    }
    Ok(())
}
