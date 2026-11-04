//! Independently authored native fixtures for repeatable Active Bar observation.
//! Always writes a new file; contains no user project data or vendor runtime.
use eda_core::{Document, DocumentKind, pcb::*};
use eda_geom::Point;
use eda_io::altium_pcb::writer::{self, Format};
use std::{io::Write, path::PathBuf};
use uuid::Uuid;

fn pad(x: i64, y: i64, net: u32, owner: Option<u32>, layer: u16) -> Item {
    let mut item = Item::new(
        layer,
        Some(net),
        Shape::Pad {
            position: Point::mm(x, y),
            size: Point::mm(2, 2),
            rotation: 0.,
            shape: PadShape::Round,
            hole: if layer == MULTI_LAYER { 600_000 } else { 0 },
            plated: true,
            name: format!("{net}"),
            bottom_size: None,
            middle_size: None,
        },
    );
    item.component = owner;
    item
}
fn route(a: (i64, i64), b: (i64, i64), layer: u16, net: u32) -> Item {
    Item::new(
        layer,
        Some(net),
        Shape::Track {
            start: Point::mm(a.0, a.1),
            end: Point::mm(b.0, b.1),
            width: 400_000,
        },
    )
}
fn run() -> Result<(), String> {
    let mut args = std::env::args_os().skip(1);
    let mode = args.next().ok_or("Expected arrange or selection")?;
    let output = PathBuf::from(args.next().ok_or("Expected NEW_OUTPUT.PcbDoc")?);
    let template = args.next();
    if args.next().is_some() {
        return Err("Too many arguments".into());
    }
    let mut doc = if let Some(template) = template {
        let bytes = std::fs::read(template).map_err(|e| e.to_string())?;
        eda_io::altium_pcb::import_pcb(&bytes, "ActiveBarTemplate.PcbDoc")?
    } else {
        Document::new(DocumentKind::Board, "ActiveBarProbe.PcbDoc")
    };
    let mut board = doc.pcb.take().unwrap_or_default();
    board.items.clear();
    board.components.clear();
    board.nets.clear();
    board.polygon_pours.clear();
    board.outline = [(0, 0), (60, 0), (60, 40), (0, 40)]
        .into_iter()
        .map(|(x, y)| ContourVertex {
            position: Point::mm(x, y),
            arc: None,
        })
        .collect();
    board
        .properties
        .insert("SNAPGRIDSIZE".into(), "20mil".into());
    if mode == "arrange" {
        board.items = [(5, 3, 7, 4), (15, 6, 19, 8), (30, 10, 33, 13)]
            .into_iter()
            .map(|(x, y, right, top)| {
                Item::new(
                    TOP,
                    None,
                    Shape::Fill {
                        start: Point::mm(x, y),
                        end: Point::mm(right, top),
                        rotation: 0.,
                    },
                )
            })
            .collect();
    } else if mode == "selection" {
        board.nets = ["Probe A", "Probe B"]
            .into_iter()
            .enumerate()
            .map(|(index, name)| Net {
                index: index as u32,
                name: name.into(),
                properties: Fields::new(),
            })
            .collect();
        for (index, x) in [10, 40].into_iter().enumerate() {
            board.components.push(Component {
                id: Uuid::new_v4(),
                index: index as u32,
                designator: format!("U{}", index + 1),
                comment: String::new(),
                footprint: "ActiveBarProbe".into(),
                parameters: None,
                position: Point::mm(x, 20),
                rotation: 0.,
                layer: TOP,
                locked: false,
                properties: Fields::new(),
            });
            board
                .items
                .push(pad(x, 20, 0, Some(index as u32), MULTI_LAYER));
            board.items.push(pad(x, 25, 1, Some(index as u32), TOP));
        }
        board.items.extend([
            route((10, 20), (20, 20), TOP, 0),
            Item::new(
                MULTI_LAYER,
                Some(0),
                Shape::Via {
                    position: Point::mm(20, 20),
                    diameter: 1_000_000,
                    hole: 400_000,
                    from_layer: TOP,
                    to_layer: BOTTOM,
                },
            ),
            route((20, 20), (30, 20), BOTTOM, 0),
            pad(30, 20, 0, None, BOTTOM),
            route((30, 20), (40, 20), BOTTOM, 0),
            route((10, 25), (40, 25), TOP, 1),
        ]);
    } else {
        return Err("Expected arrange or selection".into());
    }
    board.validate()?;
    doc.pcb = Some(board);
    let bytes = writer::serialize(&doc, Format::Binary)?;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)
        .map_err(|e| e.to_string())?;
    file.write_all(&bytes).map_err(|e| e.to_string())?;
    println!("Wrote synthetic Active Bar fixture ({} bytes)", bytes.len());
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
