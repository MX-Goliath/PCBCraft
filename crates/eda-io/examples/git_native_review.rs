//! Exercise native Git comparison in an explicitly marked, disposable project copy.
//! This fixture targets the Bip_adaptive QA copy. Inspect without flags, compare
//! HEAD with --compare, or author schematic edits once with --apply.
//! Author board revisions in a native editor when the native writer refuses them.
use eda_core::{Document, History, document_diff, pcb, schematic_edit as sch};
use eda_geom::Point;
use std::path::Path;

fn changes(before: &Document, after: &Document) {
    let diff = document_diff::compare(Some(before), Some(after));
    println!("{}: {} semantic changes", before.name, diff.changes.len());
    for c in &diff.changes {
        println!(
            "  {:?}: {} ({} properties)",
            c.kind,
            c.label,
            c.fields.len()
        );
        for field in c.fields.iter().take(8) {
            println!(
                "    {}: {} -> {}",
                field.name,
                field.before.chars().take(180).collect::<String>(),
                field.after.chars().take(180).collect::<String>()
            );
        }
    }
}

fn main() -> Result<(), String> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let root = Path::new(
        args.first()
            .ok_or("Pass a marked Git review directory [--apply]")?,
    );
    if !root.join(".git/pcbcraft-review").is_file() {
        return Err("Refusing an unmarked directory. Work only in a disposable copy.".into());
    }
    let files = std::fs::read_dir(root)
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .collect::<Vec<_>>();
    let schematic = files
        .iter()
        .find(|p| {
            p.extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("SchDoc"))
        })
        .ok_or("No SchDoc")?;
    let board = files
        .iter()
        .find(|p| {
            p.extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("PcbDoc"))
        })
        .ok_or("No PcbDoc")?;
    let mut a = eda_io::load(schematic)?;
    let b = eda_io::load(board)?;
    if args.iter().any(|arg| arg == "--compare") {
        let repo = eda_io::git::Repository::discover(root)?;
        for (path, after) in [(schematic, &a), (board, &b)] {
            let name = path.file_name().unwrap().to_str().ok_or("Invalid path")?;
            let bytes = repo.blob("HEAD", name)?.ok_or("Missing HEAD file")?;
            let before = eda_io::git::import_document(&bytes, name)?;
            changes(&before, after);
        }
        return Ok(());
    }
    let s = a.schematic.as_ref().ok_or("No schematic model")?;
    println!(
        "SCH: {} components, {} primitives",
        s.components.len(),
        s.primitives.len()
    );
    for c in &s.components {
        println!("  {} = {} at {:?}", c.designator, c.comment, c.position);
    }
    let pcb = b.pcb.as_ref().ok_or("No PCB model")?;
    println!(
        "PCB: {} components, {} items",
        pcb.components.len(),
        pcb.items.len()
    );
    for i in pcb
        .items
        .iter()
        .filter(|i| i.component.is_none() && !i.locked)
    {
        if let pcb::Shape::Text { text, position, .. } = &i.shape {
            println!("  Text {}: {text} at {position:?}", i.id);
        }
    }
    if !args.iter().any(|arg| arg == "--apply") {
        return Ok(());
    }
    let original_a = a.clone();
    // Real component edits, preserving the original native archive via the writer.
    let r = s
        .components
        .iter()
        .find(|c| c.designator == "R1")
        .ok_or("No R1")?
        .clone();
    let c = s
        .components
        .iter()
        .find(|c| c.designator == "C1")
        .ok_or("No C1")?
        .clone();
    let removed = s
        .components
        .iter()
        .find(|c| c.designator == "R11")
        .ok_or("No R11")?
        .id;
    let mut added = sch::extract(s, c.id)?.fresh_copy();
    added = added.transformed(c.position, 0, Point::mm(10, -10))?;
    let new_id = added.components.first().ok_or("Empty component copy")?.id;
    let new_comment = c.comment.clone();
    let edit = sch::Edit::Batch(vec![
        sch::Edit::Drag {
            ids: vec![r.id],
            pivot: r.position,
            quarter_turns: 0,
            delta: Point::mm(5, 0),
        },
        sch::Edit::ComponentProperties {
            id: c.id,
            designator: c.designator,
            comment: "220nF".into(),
        },
        sch::Edit::Delete(removed),
        sch::Edit::Insert(added),
        sch::Edit::ComponentProperties {
            id: new_id,
            designator: "C99".into(),
            comment: new_comment,
        },
    ]);
    let next_a = sch::preview(&a, &edit)?;
    History::default().commit(&mut a, next_a)?;

    // Serialize and verify before touching the disposable working file.
    let bytes = eda_io::altium::writer::serialize(&a)?;
    let reread =
        eda_io::git::import_document(&bytes, &schematic.file_name().unwrap().to_string_lossy())?;
    changes(&original_a, &reread);
    std::fs::write(schematic, bytes).map_err(|e| e.to_string())?;
    println!("Saved native schematic working changes. Commit through the Git panel.");
    Ok(())
}
