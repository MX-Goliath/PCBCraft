//! Read-only native-board check of footprint picking and whole-component moves.
//! Only in-memory drafts are edited. The input file is never saved.
use eda_core::pcb::MovePreview;
use eda_geom::Point;
use eda_render::pcb::BoardView;
use std::path::PathBuf;

fn main() -> Result<(), String> {
    let mut args = std::env::args_os().skip(1);
    let path = PathBuf::from(
        args.next()
            .ok_or("Usage: pcb_component_check INPUT.PcbDoc [DESIGNATORS...]")?,
    );
    let names: Vec<_> = args.map(|n| n.to_string_lossy().into_owned()).collect();
    let original_bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
    let doc = eda_io::load(&path)?;
    let board = doc.pcb.as_ref().ok_or("Not a PCB document")?;
    let mut view = BoardView::default();
    view.initialize(board);
    // Selecting components must work when the primitive categories are off.
    view.filters.fill(false);
    let mut checked = 0;
    for component in &board.components {
        if component.locked || (!names.is_empty() && !names.contains(&component.designator)) {
            continue;
        }
        view.active_layer = component.layer;
        if !view
            .component_hits(board, component.position)
            .contains(&component.id)
        {
            return Err(format!(
                "{}: component origin is not pickable",
                component.designator
            ));
        }
        let delta = Point::mm(1, 1);
        let mut movement = MovePreview::new(board, &[component.id])?;
        let next = movement.update(component.position, delta, 90.0)?;
        let updated = next
            .components
            .iter()
            .find(|c| c.id == component.id)
            .unwrap();
        assert_eq!(
            updated.position,
            Point {
                x: component.position.x + delta.x,
                y: component.position.y + delta.y
            }
        );
        assert_eq!(
            updated.rotation,
            (component.rotation + 90.0).rem_euclid(360.0)
        );
        let mut locked_children = 0;
        for (before, after) in board.items.iter().zip(&next.items) {
            if before.component == Some(component.index) {
                assert_eq!(before.id, after.id);
                assert_eq!(before.component, after.component);
                assert_eq!(before.locked, after.locked);
                locked_children += usize::from(before.locked);
            } else {
                assert_eq!(before, after);
            }
        }
        println!(
            "{} side={} component-only body pick=true move/rotate=true retained_locked_children={locked_children}",
            component.designator, component.layer
        );
        checked += 1;
    }
    if checked == 0 {
        return Err("No matching unlocked components".into());
    }
    assert_eq!(
        std::fs::read(&path).map_err(|e| e.to_string())?,
        original_bytes
    );
    println!("Checked {checked} components; source bytes unchanged");
    Ok(())
}
