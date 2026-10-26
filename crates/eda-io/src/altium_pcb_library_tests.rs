use super::*;
use eda_core::{
    design_sync,
    schematic::ComponentModel,
    schematic_edit::{self, Edit, Placement},
};

fn block(out: &mut Vec<u8>, data: &[u8]) {
    out.extend((data.len() as u32).to_le_bytes());
    out.extend(data);
}
fn library(count: u32, bad_tag: bool) -> Vec<u8> {
    let mut file = cfb::CompoundFile::create(Cursor::new(vec![])).unwrap();
    file.create_storage("Library").unwrap();
    file.create_storage("LIBPART").unwrap();
    let mut data = vec![];
    block(&mut data, b"\x07LIBPART");
    for name in [b'1', b'2'] {
        data.push(if bad_tag { 99 } else { 2 });
        let mut pad = vec![0u8; 194];
        pad[0] = 1;
        pad[1] = 4;
        pad[3..13].fill(255);
        pad[13..17].copy_from_slice(&(i32::from(name - b'1') * 1_000_000).to_le_bytes());
        for pos in [21, 25, 29, 33, 37, 41] {
            pad[pos..pos + 4].copy_from_slice(&300_000i32.to_le_bytes());
        }
        pad[49..52].fill(2);
        pad[60] = 1;
        for bytes in [&[1, name][..], &[], &[], &[], &pad, &[]] {
            block(&mut data, bytes);
        }
    }
    let mut params = vec![];
    block(
        &mut params,
        b"|PATTERN=LIBPART|HEIGHT=1mm|DESCRIPTION=Real native pads\0",
    );
    let mut properties = vec![];
    block(
        &mut properties,
        b"|LAYER1NAME=Top Layer|LAYER32NAME=Bottom Layer\0",
    );
    for (name, bytes) in [
        (
            "FileHeader",
            b"\x1b\0\0\0\x1bPCB 6.0 Binary Library File".to_vec(),
        ),
        ("Library/Data", properties),
        ("LIBPART/Data", data),
        ("LIBPART/Parameters", params),
        ("LIBPART/Header", count.to_le_bytes().to_vec()),
        ("LIBPART/WideStrings", vec![1, 0, 0, 0, 0]),
    ] {
        file.create_stream(name).unwrap().write_all(&bytes).unwrap();
    }
    file.into_inner().into_inner()
}
#[test]
fn standalone_and_compressed_integrated_footprints_drive_real_eco_geometry() {
    let standalone = library(2, false);
    let mut encoder = flate2::write::ZlibEncoder::new(vec![], flate2::Compression::default());
    encoder.write_all(&standalone).unwrap();
    let mut compressed = vec![2];
    compressed.extend(encoder.finish().unwrap());
    let mut intlib = cfb::CompoundFile::create(Cursor::new(vec![])).unwrap();
    intlib.create_storage("PCBLib").unwrap();
    intlib
        .create_stream("PCBLib/Parts.PcbLib")
        .unwrap()
        .write_all(&compressed)
        .unwrap();
    for bytes in [standalone, intlib.into_inner().into_inner()] {
        let library = import(&bytes, "parts.PcbLib").unwrap();
        assert_eq!(library.footprints.len(), 1, "{:?}", library.warnings);
        let mut sch = Document::new(DocumentKind::Schematic, "Circuit.SchDoc");
        let fragment =
            schematic_edit::place(Placement::Resistor, Point::mm(10, 10), 0, "R1", "10k", 0)
                .unwrap();
        sch = schematic_edit::preview(&sch, &Edit::Insert(fragment)).unwrap();
        sch.schematic.as_mut().unwrap().components[0]
            .settings
            .models = vec![ComponentModel {
            name: "LIBPART".into(),
            kind: "PCBLIB".into(),
            current: true,
        }];
        let mut pcb = Document::new(DocumentKind::Board, "Board.PcbDoc");
        pcb.pcb = Some(Board::default());
        let mut plan = design_sync::compare(
            &[&sch],
            &pcb,
            design_sync::Direction::ToPcb,
            library.footprints,
        )
        .unwrap();
        let prepared = plan.validate(&[&sch, &pcb]).unwrap();
        assert_eq!(prepared.rows.len(), plan.changes.len(), "{}", plan.report());
        let b = prepared.documents[0].pcb.as_ref().unwrap();
        assert_eq!(b.items.len(), 2);
        assert_eq!(b.components[0].designator, "R1");
        assert_eq!(b.components[0].footprint, "LIBPART");
        assert!(
            matches!(&b.items[0].shape,Shape::Pad {name,size,..} if name=="1"&&size.x==762_000)
        );
        assert_ne!(b.items[0].anchor(), b.items[1].anchor());
    }
}

#[test]
fn count_mismatch_unknown_primitives_and_truncation_never_create_partial_footprints() {
    for bytes in [library(99, false), library(2, true)] {
        let result = import(&bytes, "broken.PcbLib").unwrap();
        assert!(result.footprints.is_empty());
        assert!(!result.warnings.is_empty());
    }
    assert!(import(&library(2, false)[..90], "broken.PcbLib").is_err());
}
