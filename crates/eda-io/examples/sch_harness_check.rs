//! Read-only harness inventory and an isolated native blank for Altium observation.
use std::io::{Cursor, Read, Write};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|s| s == "--exercise") {
        use eda_core::{
            schematic::SchPrimitiveKind as K,
            schematic_edit::{self as edit, Edit},
            schematic_harness as harness,
        };
        use eda_geom::Point;
        let destination = std::path::Path::new(&args[2]);
        if destination.exists() {
            return Err("Destination must be new".into());
        }
        let mut d = eda_io::load(std::path::Path::new(&args[1]))?;
        let s = d.schematic.as_ref().unwrap();
        let mut c = s
            .primitives
            .iter()
            .find(|p| matches!(p.kind, K::HarnessConnector { .. }))
            .ok_or("No Harness Connector")?
            .clone();
        let id = c.id;
        c.presentation.harness_type = "Serial".into();
        let entry = s
            .primitives
            .iter()
            .find(|p| matches!(p.kind, K::HarnessEntry { .. }))
            .ok_or("No Entry")?
            .id;
        d = edit::preview(
            &d,
            &Edit::Batch(vec![
                Edit::PrimitiveProperties {
                    id,
                    value: Box::new(c),
                },
                Edit::Text {
                    id: entry,
                    text: "RX".into(),
                },
            ]),
        )?;
        let s = d.schematic.as_ref().unwrap();
        let mut e = harness::entry(
            s,
            id,
            Point {
                x: 101_600_000,
                y: 121_920_000,
            },
            "TX",
        )?;
        e.presentation.harness_type = "Logic".into();
        d = edit::preview(
            &d,
            &Edit::Insert(edit::Fragment {
                primitives: vec![e],
                ..Default::default()
            }),
        )?;
        let f = harness::connector(
            Point {
                x: 180_340_000,
                y: 137_160_000,
            },
            Point {
                x: 205_740_000,
                y: 111_760_000,
            },
            0,
            "Logic",
        )?;
        d = edit::preview(&d, &Edit::Insert(f))?;
        eda_io::save(destination, &d)?;
        println!(
            "Verified native harness edits saved to {}",
            destination.display()
        );
        return Ok(());
    }
    if args.first().is_some_and(|s| s == "--blank") {
        let bytes = std::fs::read(&args[1])?;
        let mut source = cfb::CompoundFile::open(Cursor::new(bytes))?;
        let mut data = vec![];
        source.open_stream("FileHeader")?.read_to_end(&mut data)?;
        let mut blocks = vec![];
        let mut at = 0;
        while at + 4 <= data.len() {
            let size = u32::from_le_bytes(data[at..at + 4].try_into()?) as usize & 0xffffff;
            blocks.extend_from_slice(&data[at..at + 4 + size]);
            at += 4 + size;
            if blocks.windows(10).any(|w| w == b"|RECORD=31") {
                break;
            }
        }
        let mut file = cfb::CompoundFile::create(Cursor::new(vec![]))?;
        file.create_stream("FileHeader")?.write_all(&blocks)?;
        std::fs::write(&args[2], file.into_inner().into_inner())?;
        return Ok(());
    }
    for path in args {
        let d = eda_io::altium::load_schematic(std::path::Path::new(&path))?;
        let s = d.schematic.unwrap();
        for r in s
            .records
            .iter()
            .filter(|r| matches!(r.record_type, 16 | 18 | 215..=218))
        {
            println!("{} {} {} {:?}", r.index, r.stream, r.record_type, r.fields);
        }
        println!(
            "{} primitives; warnings: {:?}",
            s.primitives.len(),
            s.warnings
        );
    }
    Ok(())
}
