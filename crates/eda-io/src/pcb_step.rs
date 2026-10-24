//! Embedded STEP import through the independent Rust Truck CAD kernel.
//! The native archive is never changed; meshes and AP214 face colours are shared.
use super::{property_records, warn};
use eda_core::{
    pcb::*,
    pcb_3d::{Model, ModelTriangle},
};
use flate2::read::ZlibDecoder;
use ruststep::ast::{EntityInstance, Name, Parameter, Record};
use std::{collections::BTreeMap, io::Read, sync::Arc};
use truck_meshalgo::prelude::*;
use truck_stepio::r#in::Table;

fn refs(p: &Parameter) -> Vec<u64> {
    match p {
        Parameter::Ref(Name::Entity(id)) => vec![*id],
        Parameter::List(p) => p.iter().flat_map(refs).collect(),
        _ => vec![],
    }
}
fn color(id: u64, records: &BTreeMap<u64, &Record>, depth: usize) -> Option<[u8; 4]> {
    if depth > 12 {
        return None;
    }
    let r = records.get(&id)?;
    if r.name == "COLOUR_RGB" {
        let Parameter::List(values) = &r.parameter else {
            return None;
        };
        let rgb: Vec<_> = values
            .iter()
            .filter_map(|p| match p {
                Parameter::Real(x) => Some(*x),
                Parameter::Integer(x) => Some(*x as f64),
                _ => None,
            })
            .collect();
        return (rgb.len() == 3).then(|| {
            [
                (rgb[0].clamp(0., 1.) * 255.) as u8,
                (rgb[1].clamp(0., 1.) * 255.) as u8,
                (rgb[2].clamp(0., 1.) * 255.) as u8,
                255,
            ]
        });
    }
    if !matches!(
        r.name.as_str(),
        "PRESENTATION_STYLE_ASSIGNMENT"
            | "SURFACE_STYLE_USAGE"
            | "SURFACE_SIDE_STYLE"
            | "SURFACE_STYLE_FILL_AREA"
            | "FILL_AREA_STYLE"
            | "FILL_AREA_STYLE_COLOUR"
    ) {
        return None;
    }
    refs(&r.parameter)
        .into_iter()
        .find_map(|id| color(id, records, depth + 1))
}
pub(super) fn mesh(source: &str, name: String) -> Result<Model, String> {
    let exchange = ruststep::parser::parse(source).map_err(|e| format!("STEP syntax: {e}"))?;
    let data = exchange.data.first().ok_or("STEP has no DATA section")?;
    if data.entities.len() > 500_000 {
        return Err("STEP entity limit".into());
    }
    let mut scale: Option<f64> = None;
    for entity in &data.entities {
        let rs: &[Record] = match entity {
            EntityInstance::Complex { subsuper, .. } => &subsuper.0,
            EntityInstance::Simple { record, .. } => std::slice::from_ref(record),
        };
        if rs.iter().any(|r| {
            matches!(
                r.name.as_str(),
                "MAPPED_ITEM" | "REPRESENTATION_RELATIONSHIP_WITH_TRANSFORMATION"
            )
        }) {
            return Err("STEP assembly transforms are not supported".into());
        }
        if !rs.iter().any(|r| r.name == "LENGTH_UNIT") {
            continue;
        }
        let Some(unit) = rs.iter().find(|r| r.name == "SI_UNIT") else {
            return Err("STEP conversion-based length units are not supported".into());
        };
        let Parameter::List(values) = &unit.parameter else {
            return Err("Invalid STEP length unit".into());
        };
        let Some(Parameter::Enumeration(name)) = values.last() else {
            return Err("Invalid STEP length unit".into());
        };
        if name != "METRE" {
            return Err("Unsupported STEP length unit".into());
        }
        let factor = match values.first() {
            Some(Parameter::Enumeration(prefix)) => match prefix.as_str() {
                "MILLI" => 1.,
                "CENTI" => 10.,
                "DECI" => 100.,
                "MICRO" => 0.001,
                "KILO" => 1e6,
                _ => return Err("Unsupported STEP length prefix".into()),
            },
            _ => 1000.,
        };
        if scale.is_some_and(|s| s != factor) {
            return Err("Mixed STEP length units are not supported".into());
        }
        scale = Some(factor);
    }
    let scale = scale.unwrap_or(1.);
    let records: BTreeMap<_, _> = data
        .entities
        .iter()
        .filter_map(|e| match e {
            EntityInstance::Simple { id, record } => Some((*id, record)),
            _ => None,
        })
        .collect();
    let mut colors = BTreeMap::new();
    for r in records.values().filter(|r| r.name == "STYLED_ITEM") {
        let ids = refs(&r.parameter);
        if let Some((&target, styles)) = ids.split_last()
            && let Some(c) = styles.iter().find_map(|id| color(*id, &records, 0))
        {
            colors.insert(target, c);
        }
    }
    // A style applied to a solid belongs to its shell, unless a face overrides it.
    for (id, r) in &records {
        if r.name == "MANIFOLD_SOLID_BREP"
            && let Some(c) = colors.get(id).copied()
        {
            for shell in refs(&r.parameter) {
                colors.entry(shell).or_insert(c);
            }
        }
    }
    let table = Table::from_data_section(data);
    let mut model = Model {
        name,
        triangles: vec![],
    };
    let mut shells: Vec<_> = table.shell.iter().collect();
    shells.sort_by_key(|(id, _)| **id);
    for (id, shell) in shells {
        let face_ids = records
            .get(id)
            .map(|r| refs(&r.parameter))
            .unwrap_or_default();
        let shell = table
            .to_compressed_shell(shell)
            .map_err(|e| format!("STEP topology: {e}"))?;
        if shell.faces.len() != face_ids.len() {
            return Err("STEP dropped faces".into());
        }
        let mesh = shell.robust_triangulation((0.02 / scale).max(1e-6));
        for (i, face) in mesh.faces.iter().enumerate() {
            let poly = face
                .surface
                .as_ref()
                .ok_or("STEP face could not be tessellated")?;
            let color = colors
                .get(&face_ids[i])
                .or_else(|| colors.get(id))
                .copied()
                .unwrap_or([128, 128, 128, 255]);
            for polygon in poly.face_iter() {
                for k in 1..polygon.len().saturating_sub(1) {
                    let mut vertices = [polygon[0], polygon[k], polygon[k + 1]].map(|v| {
                        let p = poly.positions()[v.pos];
                        [p.x * scale, p.y * scale, p.z * scale]
                    });
                    if !face.orientation {
                        vertices.swap(1, 2);
                    }
                    model.triangles.push(ModelTriangle { vertices, color });
                }
            }
            if model.triangles.len() > 1_000_000 {
                return Err("STEP triangle limit".into());
            }
        }
    }
    if !model.valid() {
        return Err("STEP produced no finite surface mesh".into());
    }
    Ok(model)
}
pub(super) fn read_models(streams: &BTreeMap<String, Vec<u8>>, board: &mut Board) {
    let Some(data) = streams.get("/Models/Data") else {
        return;
    };
    let records = match property_records(data, 0) {
        Ok(r) => r,
        Err(e) => {
            warn(board, format!("3D models: {e}"));
            return;
        }
    };
    let mut cache = BTreeMap::<Vec<u8>, Arc<Model>>::new();
    let mut total = 0;
    for (index, f) in records.iter().enumerate() {
        let Some(id) = f.get("ID") else { continue };
        if board.models.contains_key(id) {
            continue;
        }
        let Some(compressed) = streams.get(&format!("/Models/{index}")) else {
            continue;
        };
        if let Some(m) = cache.get(compressed) {
            board.models.insert(id.clone(), m.clone());
            continue;
        }
        let result = std::panic::catch_unwind(|| {
            let mut bytes = Vec::new();
            ZlibDecoder::new(compressed.as_slice())
                .take(32 * 1024 * 1024 + 1)
                .read_to_end(&mut bytes)
                .map_err(|e| e.to_string())?;
            if bytes.len() > 32 * 1024 * 1024 {
                return Err("Expanded STEP exceeds 32 MiB".into());
            }
            let source = String::from_utf8_lossy(&bytes);
            mesh(&source, f.get("NAME").cloned().unwrap_or_default())
        })
        .unwrap_or_else(|_| Err("CAD tessellation failed".into()));
        match result {
            Ok(m) => {
                total += m.triangles.len();
                if total > 2_000_000 {
                    warn(board, "3D mesh budget exceeded");
                    break;
                }
                let m = Arc::new(m);
                cache.insert(compressed.clone(), m.clone());
                board.models.insert(id.clone(), m);
            }
            Err(e) => warn(
                board,
                format!(
                    "3D model {}: {e}",
                    f.get("NAME").map_or(id.as_str(), String::as_str)
                ),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const FACE: &str = include_str!("../../../fixtures/pcb-update/colored-face.step");
    #[test]
    fn actual_step_topology_is_tessellated_with_face_color_and_finite_dimensions() {
        let m = mesh(FACE, "face".into()).unwrap();
        assert!(m.valid());
        assert!(m.triangles.iter().all(|t| t.color == [0, 255, 0, 255]));
        let area: f64 = m
            .triangles
            .iter()
            .map(|t| {
                let [a, b, c] = t.vertices;
                ((b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])).abs() * 0.5
            })
            .sum();
        assert!((area - 4.).abs() < 1e-8);
        assert!(
            m.triangles
                .iter()
                .flat_map(|t| t.vertices)
                .all(|p| p[2].abs() < 1e-9 && p[0].abs() <= 1. && p[1].abs() <= 1.)
        );
        assert!(mesh("invalid", "bad".into()).is_err());
        let centimetres = mesh(&FACE.replace(".MILLI.", ".CENTI."), "cm".into()).unwrap();
        assert!(
            centimetres
                .triangles
                .iter()
                .flat_map(|t| t.vertices)
                .any(|p| p[0] == 10.)
        );
    }
    #[test]
    fn compressed_models_share_geometry_and_bad_model_keeps_the_board_usable() {
        use std::io::Write;
        let mut compressed =
            flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        compressed.write_all(FACE.as_bytes()).unwrap();
        let compressed = compressed.finish().unwrap();
        let mut catalog = Vec::new();
        for (id, name) in [("a", "one"), ("b", "two"), ("c", "bad")] {
            let f = format!("ID={id}|NAME={name}|EMBED=TRUE\0");
            catalog.extend((f.len() as u32).to_le_bytes());
            catalog.extend(f.as_bytes());
        }
        let streams = BTreeMap::from([
            ("/Models/Data".into(), catalog),
            ("/Models/0".into(), compressed.clone()),
            ("/Models/1".into(), compressed),
            ("/Models/2".into(), vec![1, 2, 3]),
        ]);
        let original = streams.clone();
        let mut b = Board::default();
        read_models(&streams, &mut b);
        assert_eq!(b.models.len(), 2);
        assert!(Arc::ptr_eq(&b.models["a"], &b.models["b"]));
        assert!(b.warnings.iter().any(|w| w.contains("3D model bad")));
        assert_eq!(streams, original);
        assert_eq!(
            serde_json::from_str::<Board>(&serde_json::to_string(&b).unwrap()).unwrap(),
            b
        );
    }
}
