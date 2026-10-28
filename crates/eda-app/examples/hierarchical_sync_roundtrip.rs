//! Exercise real hierarchical ECO and native persistence in a NEW destination.
//! cargo run -p eda-app --example hierarchical_sync_roundtrip -- project.PrjPcb new-directory
use eda_core::{
    History,
    design_sync::{self, Direction},
    schematic_edit::{self, Edit},
};
use std::path::Path;

fn main() -> Result<(), String> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("Pass source.PrjPcb and a new QA directory".into());
    }
    let destination = Path::new(&args[1]);
    if destination.exists() {
        return Err("QA destination must not exist".into());
    }
    let project = eda_io::altium::load_project(Path::new(&args[0]))?;
    let source_directory = project
        .path
        .parent()
        .unwrap()
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let entries: Vec<_> = project
        .documents
        .iter()
        .filter(|e| e.kind == "schdoc")
        .collect();
    let mut sheets: Vec<_> = entries
        .iter()
        .map(|e| eda_io::load(&e.path))
        .collect::<Result<_, _>>()?;
    let board_entry = project
        .documents
        .iter()
        .find(|e| e.kind == "pcbdoc")
        .ok_or("No PCB")?;
    let mut board = eda_io::load(&board_entry.path)?;
    // Only the in-memory QA copy is unlocked; the original board stays untouched.
    board
        .pcb
        .as_mut()
        .unwrap()
        .components
        .iter_mut()
        .find(|c| c.designator == "DD1")
        .ok_or("QA requires PCB DD1")?
        .locked = false;
    let options = project.compile_options(
        &entries
            .iter()
            .zip(&sheets)
            .map(|(e, d)| (e.path.as_path(), d))
            .collect::<Vec<_>>(),
    )?;
    let index = sheets
        .iter()
        .position(|d| {
            d.schematic
                .as_ref()
                .unwrap()
                .components
                .iter()
                .any(|c| c.designator == "DD1")
        })
        .ok_or("QA requires DD1")?;
    let component = sheets[index]
        .schematic
        .as_ref()
        .unwrap()
        .components
        .iter()
        .find(|c| c.designator == "DD1")
        .unwrap()
        .clone();
    sheets[index] = schematic_edit::preview(
        &sheets[index],
        &Edit::ComponentProperties {
            id: component.id,
            designator: component.designator.clone(),
            comment: "PCBCraft hierarchical ECO QA".into(),
        },
    )?;
    for sheet in &mut sheets {
        let ids: Vec<_> = sheet.schematic.as_ref().unwrap().primitives.iter().filter(|p| p.text_role == Some(eda_core::schematic::TextRole::NetLabel) && matches!(&p.kind, eda_core::schematic::SchPrimitiveKind::Text { text, .. } if text == "TCK")).map(|p| p.id).collect();
        for id in ids {
            *sheet = schematic_edit::preview(
                sheet,
                &Edit::Text {
                    id,
                    text: "QA_TCK".into(),
                },
            )?;
        }
    }
    let refs: Vec<_> = sheets.iter().collect();
    let mut forward =
        design_sync::compare_project(&refs, &board, Direction::ToPcb, vec![], &options)?;
    for row in &mut forward.changes {
        row.enabled = (row.group == "Change Component Comments" && row.object.starts_with("DD1:"))
            || (row.group == "Change Net Names" && row.object == "TCK → QA_TCK");
    }
    let mut all = refs;
    all.push(&board);
    let prepared = forward.validate(&all)?;
    if prepared.rows.len() != 2
        || prepared.documents.len() != 1
        || prepared.documents[0].id != board.id
    {
        return Err(forward.report());
    }
    let original = board.clone();
    let mut history = History::default();
    history.commit(&mut board, prepared.documents[0].clone())?;
    let changed = board.clone();
    history.undo(&mut board);
    assert_eq!(board, original);
    history.redo(&mut board);
    assert_eq!(board, changed);
    let old_pcb = original.pcb.as_ref().unwrap();
    let new_pcb = board.pcb.as_ref().unwrap();
    assert_eq!(
        old_pcb
            .items
            .iter()
            .filter(|p| !matches!(p.shape, eda_core::pcb::Shape::Text { .. }))
            .collect::<Vec<_>>(),
        new_pcb
            .items
            .iter()
            .filter(|p| !matches!(p.shape, eda_core::pcb::Shape::Text { .. }))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        old_pcb.nets.iter().find(|n| n.name == "TCK").unwrap().index,
        new_pcb
            .nets
            .iter()
            .find(|n| n.name == "QA_TCK")
            .unwrap()
            .index
    );
    // A reverse ECO must update DD1 on its child sheet, not any other sheet.
    let pcb = board.pcb.as_ref().unwrap();
    let mut component = pcb
        .components
        .iter()
        .find(|c| c.designator == "DD1")
        .unwrap()
        .clone();
    component.comment = "PCBCraft reverse hierarchy QA".into();
    board.pcb = Some(eda_core::pcb_tools::edit_component(pcb, component)?);
    let refs: Vec<_> = sheets.iter().collect();
    let mut reverse =
        design_sync::compare_project(&refs, &board, Direction::ToSchematic, vec![], &options)?;
    reverse.restrict_native_back_annotation();
    for row in &mut reverse.changes {
        row.enabled &= row.group == "Change Component Comments" && row.object.starts_with("DD1:");
    }
    let mut all = refs;
    all.push(&board);
    let prepared = reverse.validate(&all)?;
    if prepared.rows.len() != 1
        || prepared.documents.len() != 1
        || prepared.documents[0].id != sheets[index].id
    {
        return Err(reverse.report());
    }
    let original = sheets[index].clone();
    let mut history = History::default();
    history.commit(&mut sheets[index], prepared.documents[0].clone())?;
    let changed = sheets[index].clone();
    history.undo(&mut sheets[index]);
    assert_eq!(sheets[index], original);
    history.redo(&mut sheets[index]);
    assert_eq!(sheets[index], changed);
    std::fs::create_dir_all(destination).map_err(|e| e.to_string())?;
    for (entry, sheet) in entries.iter().zip(&sheets) {
        let relative = entry
            .path
            .strip_prefix(&source_directory)
            .map_err(|e| e.to_string())?;
        let path = destination.join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
        eda_io::save(&path, sheet)?;
        let read = eda_io::load(&path)?;
        assert_eq!(
            read.schematic.as_ref().unwrap().components,
            sheet.schematic.as_ref().unwrap().components
        );
    }
    let board_path = destination.join(board_entry.path.file_name().unwrap());
    eda_io::save(&board_path, &board)?;
    let read = eda_io::load(&board_path)?;
    assert_eq!(
        read.pcb
            .as_ref()
            .unwrap()
            .components
            .iter()
            .find(|c| c.designator == "DD1")
            .unwrap()
            .comment,
        "PCBCraft reverse hierarchy QA"
    );
    std::fs::copy(
        &project.path,
        destination.join(project.path.file_name().unwrap()),
    )
    .map_err(|e| e.to_string())?;
    println!(
        "PASS: forward/reverse child-sheet identity, native save/reimport and Undo/Redo; {}",
        destination.display()
    );
    Ok(())
}
