//! Altium numeric bus member expansion, without joining different bits.
use crate::{
    Point,
    schematic::{SchPrimitive, SchPrimitiveKind as K},
    schematic_edit,
};
pub fn members(name: &str) -> Result<Option<Vec<String>>, String> {
    let Some((prefix, range)) = name.split_once('[') else {
        return Ok(None);
    };
    let range = range.strip_suffix(']').ok_or("Unclosed bus range")?;
    let (first, last) = range
        .split_once("..")
        .ok_or("Bus range requires two numeric bounds")?;
    let first: i64 = first.trim().parse().map_err(|_| "Invalid bus range")?;
    let last: i64 = last.trim().parse().map_err(|_| "Invalid bus range")?;
    if prefix.is_empty() || first < 0 || last < 0 || (last - first).abs() > 4095 {
        return Err("Invalid or excessive bus range".into());
    }
    Ok(Some(
        (0..=(last - first).abs())
            .map(|n| format!("{prefix}{}", first + n * if last >= first { 1 } else { -1 }))
            .collect(),
    ))
}
pub fn is_entry(p: &SchPrimitive) -> bool {
    matches!(&p.kind,K::Polyline{electrical,..} if electrical=="Bus Entry")
}
pub fn bus_path(p: &SchPrimitive) -> Option<&[Point]> {
    match &p.kind {
        K::Polyline {
            points, electrical, ..
        } if electrical == "Bus" => Some(points),
        _ => None,
    }
}
pub fn entry_ends(p: &SchPrimitive) -> Vec<Point> {
    if let K::Polyline { points, .. } = &p.kind {
        points
            .first()
            .into_iter()
            .chain(points.last())
            .copied()
            .collect()
    } else {
        schematic_edit::electrical_endpoints(p).collect()
    }
}
