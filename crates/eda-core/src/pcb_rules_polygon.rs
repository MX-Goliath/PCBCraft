//! Native polygon connection constraints shared by the editor and pour engine.
use super::*;
use crate::pcb_polygon::Connection;

pub const CONNECTION_KEYS: &[&str] = &[
    "CONNECTSTYLE",
    "AIRGAPWIDTH",
    "RELIEFCONDUCTORWIDTH",
    "RELIEFENTRIES",
    "POLYGONRELIEFANGLE",
];
pub const CONNECTION_PREFIXES: &[&str] = &["THPAD.", "SMDPAD.", "VIA."];

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PolygonConnection {
    pub style: Connection,
    pub gap: i64,
    pub width: i64,
    pub conductors: u8,
    pub angle: f64,
}

pub fn advanced_connection(r: &Fields) -> bool {
    CONNECTION_PREFIXES
        .iter()
        .any(|p| r.contains_key(&format!("{p}CONNECTSTYLE")))
}

/// Switching modes is an explicit edit. Merely displaying a rule never adds fields.
pub fn set_advanced_connection(r: &mut Fields, advanced: bool) {
    if advanced == advanced_connection(r) {
        return;
    }
    if advanced {
        for prefix in CONNECTION_PREFIXES {
            for key in CONNECTION_KEYS {
                if let Some(value) = r.get(*key).cloned() {
                    r.insert(format!("{prefix}{key}"), value);
                }
            }
        }
        for key in CONNECTION_KEYS {
            r.remove(*key);
        }
    } else {
        for key in CONNECTION_KEYS {
            if let Some(value) = r.get(&format!("THPAD.{key}")).cloned() {
                r.insert((*key).into(), value);
            }
        }
        for prefix in CONNECTION_PREFIXES {
            for key in CONNECTION_KEYS {
                r.remove(&format!("{prefix}{key}"));
            }
        }
    }
}

pub fn connection_constraints(r: &Fields, prefix: &str) -> Result<PolygonConnection, String> {
    let key = |k: &str| format!("{prefix}{k}");
    let style = match text(r, &key("CONNECTSTYLE")) {
        "Direct" => Connection::Direct,
        "Relief" => Connection::Thermal,
        "NoConnect" => Connection::None,
        other => {
            return Err(format!(
                "{}: unsupported polygon connection {other}",
                text(r, "NAME")
            ));
        }
    };
    // Direct/isolated connections retain their inactive thermal values verbatim.
    if style != Connection::Thermal {
        return Ok(PolygonConnection {
            style,
            gap: 0,
            width: 0,
            conductors: 4,
            angle: 90.0,
        });
    }
    let gap = dimension(r, &key("AIRGAPWIDTH"))?;
    let width = dimension(r, &key("RELIEFCONDUCTORWIDTH"))?;
    if !(1..=100_000_000).contains(&gap) || !(1..=100_000_000).contains(&width) {
        return Err(format!(
            "{}: thermal gap and conductor width must be 0..100 mm",
            text(r, "NAME")
        ));
    }
    let conductors = match text(r, &key("RELIEFENTRIES")) {
        "2" => 2,
        "4" => 4,
        _ => {
            return Err(format!(
                "{}: automatic/custom thermal conductors are not implemented",
                text(r, "NAME")
            ));
        }
    };
    let angle = match text(r, &key("POLYGONRELIEFANGLE")) {
        "45 Angle" => 45.0,
        "90 Angle" => 90.0,
        _ => {
            return Err(format!(
                "{}: invalid polygon relief rotation",
                text(r, "NAME")
            ));
        }
    };
    Ok(PolygonConnection {
        style,
        gap,
        width,
        conductors,
        angle,
    })
}

pub fn polygon_connection(
    board: &Board,
    pad: &Item,
    polygon: &Item,
) -> Result<Option<PolygonConnection>, String> {
    let Some((_, r)) = resolve(board, "PolygonConnect", pad, Some(polygon))? else {
        return Ok(None);
    };
    let prefix = if !advanced_connection(r) {
        ""
    } else {
        match pad.shape {
            Shape::Via { .. } => "VIA.",
            Shape::Pad { hole: 0, .. } => "SMDPAD.",
            Shape::Pad { .. } => "THPAD.",
            _ => return Ok(None),
        }
    };
    connection_constraints(r, prefix).map(Some)
}

pub(super) fn validate_connection(r: &Fields) -> Result<(), String> {
    if advanced_connection(r) {
        for prefix in CONNECTION_PREFIXES {
            connection_constraints(r, prefix)?;
        }
    } else {
        connection_constraints(r, "")?;
    }
    Ok(())
}

pub(super) fn connection_field(key: &str) -> bool {
    CONNECTION_KEYS.contains(&key)
        || CONNECTION_PREFIXES.iter().any(|p| {
            key.strip_prefix(p)
                .is_some_and(|k| CONNECTION_KEYS.contains(&k))
        })
}
