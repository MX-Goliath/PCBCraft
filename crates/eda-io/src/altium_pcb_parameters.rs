//! Observed PrimitiveParameters property blocks, grouped by owner and variant.
use super::*;
#[cfg(test)]
#[path = "altium_pcb_parameters_tests.rs"]
mod tests;

/// Older native files identify parameter owners only by their unique ID.
/// Never infer an owner from record order, and reject ambiguous identities.
pub(super) fn component_owner(board: &Board, header: &Fields) -> Result<Option<u32>> {
    let uid = header
        .get("PRIMITIVEID")
        .filter(|v| !v.is_empty())
        .ok_or("Parameter group has no primitive identity")?;
    if let Some(id) = header.get("ID") {
        let Some(index) = id.strip_prefix("Component#") else {
            return Ok(None);
        };
        let index = index
            .parse::<u32>()
            .map_err(|_| "Invalid parameter component index")?;
        let component = board
            .components
            .iter()
            .find(|c| c.index == index)
            .ok_or("Parameters refer to missing component")?;
        if component.properties.get("UNIQUEID") != Some(uid) {
            return Err("Parameter component identity does not match".into());
        }
        return Ok(Some(index));
    }
    let mut matches = board
        .components
        .iter()
        .filter(|c| c.properties.get("UNIQUEID") == Some(uid));
    let owner = matches.next().map(|c| c.index);
    if matches.next().is_some() {
        return Err("Ambiguous parameter component identity".into());
    }
    Ok(owner)
}

pub(super) fn import(board: &mut Board, data: &[u8]) -> Result<()> {
    let records = property_records(data, 0)?;
    let mut n = 0;
    // Stage all fields first, so malformed input cannot partially upgrade JSON.
    let mut parameters = BTreeMap::<u32, Fields>::new();
    while n < records.len() {
        let header = &records[n];
        let count = header
            .get("COUNT")
            .ok_or("Parameter group has no count")?
            .parse::<usize>()
            .map_err(|_| "Invalid parameter count")?;
        let end = n
            .checked_add(1)
            .and_then(|n| n.checked_add(count))
            .filter(|end| *end <= records.len())
            .ok_or("Truncated parameter group")?;
        // Footprint/System parameters and named variants are distinct namespaces.
        // Retain their native records, but do not use them as base component values.
        if header.get("VARIANTGUID").is_none_or(String::is_empty)
            && header.get("APPURTENANCE").is_none_or(String::is_empty)
            && let Some(index) = component_owner(board, header)
                .map_err(|error| format!("PrimitiveParameters/Data record {n}: {error}"))?
        {
            let fields = parameters.entry(index).or_default();
            for record in &records[n + 1..end] {
                let name = record
                    .get("NAME")
                    .filter(|name| !name.is_empty())
                    .ok_or("Parameter has no name")?;
                let value = record.get("VALUE").ok_or("Parameter has no value")?;
                if fields.keys().any(|key| key.eq_ignore_ascii_case(name)) {
                    return Err("Duplicate component parameter".into());
                }
                fields.insert(name.clone(), value.clone());
            }
        }
        n = end;
    }
    for component in &mut board.components {
        if let Some(fields) = parameters.remove(&component.index) {
            component.parameters = Some(fields);
        }
    }
    Ok(())
}

pub(crate) fn upgrade_component_parameters(board: &mut Board) -> Result<()> {
    if board.components.iter().all(|c| c.parameters.is_some()) {
        return Ok(());
    }
    let Some(source) = &board.source else {
        return Ok(());
    };
    if source.bytes_base64.len() > LIMIT * 4 / 3 + 8 {
        return Err("Archived PCB exceeds parameter recovery limit".into());
    }
    let bytes = STANDARD
        .decode(&source.bytes_base64)
        .map_err(|e| e.to_string())?;
    let mut recovered = Board::default();
    if bytes.starts_with(MAGIC) {
        let mut cfb = cfb::CompoundFile::open(Cursor::new(bytes)).map_err(|e| e.to_string())?;
        let mut components = vec![];
        cfb.open_stream("/Components6/Data")
            .map_err(|e| e.to_string())?
            .take(LIMIT as u64 + 1)
            .read_to_end(&mut components)
            .map_err(|e| e.to_string())?;
        if components.len() > LIMIT {
            return Err("Component stream exceeds limit".into());
        }
        recovered.components = property_records(&components, 0)?
            .into_iter()
            .enumerate()
            .map(|(n, fields)| component(fields, Uuid::nil(), n))
            .collect::<Result<_>>()?;
        if cfb.is_stream("/PrimitiveParameters/Data") {
            let mut data = vec![];
            cfb.open_stream("/PrimitiveParameters/Data")
                .map_err(|e| e.to_string())?
                .take(LIMIT as u64 + 1)
                .read_to_end(&mut data)
                .map_err(|e| e.to_string())?;
            if data.len() > LIMIT {
                return Err("Parameter stream exceeds limit".into());
            }
            import(&mut recovered, &data)?;
        }
    } else {
        // The supported ASCII component records contain no parameter table.
        for component in &mut board.components {
            component.parameters.get_or_insert_with(Fields::new);
        }
    }
    for component in &mut board.components {
        if component.parameters.is_none() {
            // Component indices may have changed in the saved JSON. Resolve the
            // archived owner by its native identity, never by its current slot.
            if let Some(original) = recovered.components.iter().find(|c| {
                c.properties.contains_key("UNIQUEID")
                    && c.properties.get("UNIQUEID") == component.properties.get("UNIQUEID")
            }) {
                component.parameters.clone_from(&original.parameters);
            }
        }
    }
    board
        .warnings
        .retain(|w| !w.starts_with("PrimitiveParameters: retained"));
    Ok(())
}
