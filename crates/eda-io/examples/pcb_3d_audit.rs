//! Read-only model/placement probe for a native reference board.
use eda_core::{pcb::*, pcb_3d};
fn main() -> Result<(), String> {
    let path = std::env::args_os()
        .nth(1)
        .ok_or("Usage: pcb_3d_audit INPUT.PcbDoc")?;
    let start = std::time::Instant::now();
    let doc = eda_io::load(std::path::Path::new(&path))?;
    let board = doc.pcb.as_ref().ok_or("No PCB")?;
    println!(
        "models={} triangles={} load_ms={}",
        board.models.len(),
        board
            .models
            .values()
            .map(|m| m.triangles.len())
            .sum::<usize>(),
        start.elapsed().as_millis()
    );
    for w in board.warnings.iter().filter(|w| w.starts_with("3D")) {
        println!("{w}");
    }
    let mut max_error = 0.0_f64;
    let mut missing = 0;
    let mut placements = 0;
    for item in board.items.iter().filter(|i| {
        i.properties.contains_key("PCBCRAFT_BODY_PROJECTION")
            && !i
                .properties
                .contains_key("PCBCRAFT_LEGACY_REGION_DUPLICATE")
    }) {
        let Some(model) = item
            .properties
            .get("MODELID")
            .and_then(|id| board.models.get(id))
        else {
            println!("missing: {:?}", item.properties.get("MODEL.NAME"));
            missing += 1;
            continue;
        };
        let mut b = Bounds::empty();
        let mut z = [f64::INFINITY, f64::NEG_INFINITY];
        for t in &model.triangles {
            for v in t.vertices {
                let p = pcb_3d::model_point(board, item, v).ok_or("Bad transform")?;
                b.include(eda_geom::Point {
                    x: (p[0] * 1e6).round() as i64,
                    y: (p[1] * 1e6).round() as i64,
                });
                z[0] = z[0].min(p[2]);
                z[1] = z[1].max(p[2]);
            }
        }
        let projection = item.bounds();
        let error = [
            b.min.x - projection.min.x,
            b.min.y - projection.min.y,
            b.max.x - projection.max.x,
            b.max.y - projection.max.y,
        ]
        .map(|x| x as f64 / 1e6);
        placements += 1;
        max_error = max_error.max(error.into_iter().map(f64::abs).fold(0., f64::max));
        let owner = item
            .component
            .and_then(|i| board.components.iter().find(|c| c.index == i))
            .map(|c| c.designator.as_str())
            .unwrap_or("free");
        println!("{owner}: {} bounds_error_mm={error:?} z={z:?}", model.name);
    }
    println!("placements={placements} missing={missing} max_projection_error_mm={max_error:.6}");
    if missing > 0 || max_error > 0.025 {
        return Err("STEP coverage/placement differs from the saved native projection".into());
    }
    Ok(())
}
