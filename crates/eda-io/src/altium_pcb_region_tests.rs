use super::*;

fn record(extended: bool, outer: &[Point], holes: &[Vec<Point>]) -> Vec<u8> {
    let mut p = vec![0; 18];
    p[0] = TOP as u8;
    p[1] = 4;
    p[3..13].fill(255);
    p[14..16].copy_from_slice(&(holes.len() as u16).to_le_bytes());
    let props = b"|KIND=0|V7_LAYER=TOP|ISSHAPEBASED=FALSE|NAME=Native pour fragment\0";
    p.extend_from_slice(&(props.len() as u32).to_le_bytes());
    p.extend_from_slice(props);
    p.extend_from_slice(&(outer.len() as u32).to_le_bytes());
    let points: Vec<_> = outer
        .iter()
        .chain(outer.first().filter(|_| extended))
        .collect();
    for point in points {
        if extended {
            let mut v = vec![0; 37];
            v[1..5].copy_from_slice(&((point.x as f64 / 2.54).round() as i32).to_le_bytes());
            v[5..9].copy_from_slice(&((point.y as f64 / 2.54).round() as i32).to_le_bytes());
            p.extend(v);
        } else {
            p.extend_from_slice(&(point.x as f64 / 2.54).to_le_bytes());
            p.extend_from_slice(&(point.y as f64 / 2.54).to_le_bytes());
        }
    }
    for hole in holes {
        p.extend_from_slice(&(hole.len() as u32).to_le_bytes());
        for point in hole {
            p.extend_from_slice(&(point.x as f64 / 2.54).to_le_bytes());
            p.extend_from_slice(&(point.y as f64 / 2.54).to_le_bytes());
        }
    }
    p.extend_from_slice(b"opaque-native-region-tail");
    let mut frame = vec![11];
    frame.extend_from_slice(&(p.len() as u32).to_le_bytes());
    frame.extend(p);
    frame
}
fn native(outer: &[Point], holes: &[Vec<Point>]) -> Vec<u8> {
    let mut file = cfb::CompoundFile::create(Cursor::new(Vec::new())).unwrap();
    file.create_storage("/Board6").unwrap();
    let props = b"|VX0=0mm|VY0=0mm|VX1=20mm|VY1=0mm|VX2=20mm|VY2=20mm|VX3=0mm|VY3=20mm\0";
    let mut board = (props.len() as u32).to_le_bytes().to_vec();
    board.extend_from_slice(props);
    file.create_stream("/Board6/Data")
        .unwrap()
        .write_all(&board)
        .unwrap();
    for (family, extended) in [("Regions6", false), ("ShapeBasedRegions6", true)] {
        if extended && outer.is_empty() {
            continue;
        }
        file.create_storage(format!("/{family}")).unwrap();
        file.create_stream(format!("/{family}/Header"))
            .unwrap()
            .write_all(&1u32.to_le_bytes())
            .unwrap();
        file.create_stream(format!("/{family}/Data"))
            .unwrap()
            .write_all(&record(extended, outer, holes))
            .unwrap();
    }
    file.into_inner().into_inner()
}
fn stream(bytes: &[u8], path: &str) -> Vec<u8> {
    let mut file = cfb::CompoundFile::open(Cursor::new(bytes)).unwrap();
    let mut data = vec![];
    file.open_stream(path)
        .unwrap()
        .read_to_end(&mut data)
        .unwrap();
    data
}
fn outer() -> Vec<Point> {
    vec![
        Point::mm(1, 1),
        Point::mm(19, 1),
        Point::mm(19, 19),
        Point::mm(1, 19),
    ]
}

#[test]
fn point_and_line_native_holes_do_not_reject_valid_region_or_lose_source_records() {
    let (a, b, c) = (Point::mm(3, 3), Point::mm(4, 3), Point::mm(3, 4));
    let valid = vec![a, b, c, a];
    let tiny = vec![
        Point { x: 1, y: 1 },
        Point { x: 2, y: 1 },
        Point { x: 1, y: 2 },
    ];
    let bytes = native(
        &outer(),
        &[
            vec![],
            vec![a],
            vec![a, a, a],
            vec![a, b],
            vec![a, b, a, b, a],
            valid,
            tiny.clone(),
        ],
    );
    let mut doc = import_pcb(&bytes, "native.PcbDoc").unwrap();
    let board = doc.pcb.as_ref().unwrap();
    assert_eq!(board.items.len(), 2);
    for item in &board.items {
        let Shape::Region { contour, holes, .. } = &item.shape else {
            panic!()
        };
        assert_eq!(contour.len(), 4);
        assert_eq!(holes, &[vec![a, b, c], tiny.clone()]);
    }
    assert!(
        board
            .warnings
            .iter()
            .any(|w| w.contains("5 interior contour(s)"))
    );
    assert_eq!(
        STANDARD
            .decode(&board.source.as_ref().unwrap().bytes_base64)
            .unwrap(),
        bytes
    );
    assert_eq!(
        writer::serialize(&doc, writer::Format::Preserve).unwrap(),
        bytes
    );
    // An unrelated edit must copy both native region streams exactly, including
    // omitted point/line holes and opaque extensions, instead of rewriting them.
    doc.pcb.as_mut().unwrap().items.push(Item::new(
        TOP,
        None,
        Shape::Track {
            start: Point::mm(2, 10),
            end: Point::mm(8, 10),
            width: 200_000,
        },
    ));
    let output = writer::serialize(&doc, writer::Format::Preserve).unwrap();
    for family in ["Regions6", "ShapeBasedRegions6"] {
        let path = format!("/{family}/Data");
        assert_eq!(stream(&output, &path), stream(&bytes, &path));
    }
    let reopened = import_pcb(&output, "roundtrip.PcbDoc").unwrap();
    assert_eq!(reopened.pcb.as_ref().unwrap().items.len(), 3);
    reopened.validate().unwrap();
}

#[test]
fn malformed_outer_rings_coordinates_and_truncated_holes_still_fail() {
    let bad = native(&[], &[outer()]);
    let error = import_pcb(&bad, "bad-outer.PcbDoc").unwrap_err();
    assert!(error.contains("Invalid PCB Region geometry"));
    assert!(error.contains("Regions6"));
    let mut bad = record(false, &outer(), &[vec![Point::mm(3, 3)]]);
    // Replace the first hole's coordinate with NaN: this is not a point contour
    // that can be normalized. The decoder must still reject non-finite fields.
    let properties = (u32_at(&bad, 5 + 18).unwrap() & 0xffffff) as usize;
    let hole_start = 5 + 22 + properties + 4 + 4 * 16 + 4;
    bad[hole_start..hole_start + 8].copy_from_slice(&f64::NAN.to_le_bytes());
    assert!(
        region_item(&bad[5..], false, &mut Board::default())
            .unwrap_err()
            .contains("Non-finite")
    );
    let mut truncated = record(false, &outer(), &[vec![Point::mm(3, 3)]]);
    truncated.truncate(hole_start + 8);
    assert!(region_item(&truncated[5..], false, &mut Board::default()).is_err());
    // Core editing/JSON validation retains its stricter polygon invariant.
    let mut board = Board::default();
    board.items.push(Item::new(
        TOP,
        None,
        Shape::Region {
            contour: outer(),
            holes: vec![vec![Point::mm(3, 3)]],
            cutout: false,
        },
    ));
    assert!(board.validate().is_err());
}

#[test]
fn short_native_region_contours_keep_source_without_inventing_material() {
    for contour in [
        vec![],
        vec![Point::mm(1, 1)],
        vec![Point::mm(1, 1), Point::mm(2, 2)],
    ] {
        let bytes = native(&contour, &[]);
        let mut doc = import_pcb(&bytes, "short.PcbDoc").unwrap();
        let b = doc.pcb.as_ref().unwrap();
        assert!(b.items.iter().all(Item::is_empty_region));
        assert_eq!(
            b.empty_region_records.len(),
            usize::from(contour.is_empty())
        );
        let json = serde_json::to_vec(&doc).unwrap();
        let recovered: Document = serde_json::from_slice(&json).unwrap();
        recovered.validate().unwrap();
        assert_eq!(
            writer::serialize(&recovered, writer::Format::Preserve).unwrap(),
            bytes
        );
        doc.pcb.as_mut().unwrap().items.push(Item::new(
            TOP,
            None,
            Shape::Track {
                start: Point::mm(1, 1),
                end: Point::mm(2, 1),
                width: 200_000,
            },
        ));
        let edited = writer::serialize(&doc, writer::Format::Preserve).unwrap();
        assert_eq!(
            stream(&edited, "/Regions6/Data"),
            stream(&bytes, "/Regions6/Data")
        );
        let reopened = import_pcb(&edited, "edited.PcbDoc").unwrap();
        reopened.validate().unwrap();
        if contour.is_empty() {
            // Keep a real editable region after the empty native slot.
            let mut cfb = cfb::CompoundFile::open(Cursor::new(bytes)).unwrap();
            let mut raw = record(false, &[], &[]);
            raw.extend(record(false, &outer(), &[]));
            cfb.create_stream("/Regions6/Data")
                .unwrap()
                .write_all(&raw)
                .unwrap();
            cfb.create_stream("/Regions6/Header")
                .unwrap()
                .write_all(&2u32.to_le_bytes())
                .unwrap();
            let with_slot = cfb.into_inner().into_inner();
            let mut doc = import_pcb(&with_slot, "slot.PcbDoc").unwrap();
            assert_eq!(
                doc.pcb.as_ref().unwrap().items[0]
                    .source
                    .as_ref()
                    .unwrap()
                    .index,
                1
            );
            doc.pcb.as_mut().unwrap().items.push(Item::new(
                TOP,
                None,
                Shape::Track {
                    start: Point::mm(1, 1),
                    end: Point::mm(2, 1),
                    width: 200_000,
                },
            ));
            let unchanged_family = writer::serialize(&doc, writer::Format::Preserve).unwrap();
            assert_eq!(stream(&unchanged_family, "/Regions6/Data"), raw);
            doc.pcb.as_mut().unwrap().items[0].locked = true;
            assert!(
                writer::serialize(&doc, writer::Format::Preserve)
                    .unwrap_err()
                    .contains("archived empty regions")
            );
        }
    }
}

#[test]
fn repeated_native_outer_remnants_are_preserved_without_becoming_copper() {
    let a = Point::mm(3, 3);
    let b = Point::mm(4, 3);
    let repeated = vec![a, b, b, b, b, b, b, b, b, b, b, b, a];
    let bytes = native(&repeated, &[]);
    let mut doc = import_pcb(&bytes, "remnant.PcbDoc").unwrap();
    let board = doc.pcb.as_ref().unwrap();
    assert_eq!(board.items.len(), 2);
    for item in &board.items {
        assert!(item.is_empty_region());
        assert!(!eda_core::pcb_polygon::on_layer(board, item, TOP));
        let Shape::Region { contour, holes, .. } = &item.shape else {
            panic!()
        };
        assert!(contour.len() >= repeated.len());
        assert!(holes.is_empty());
    }
    assert!(
        board
            .warnings
            .iter()
            .any(|w| w.contains("point/line-only outer contour"))
    );
    assert_eq!(
        writer::serialize(&doc, writer::Format::Preserve).unwrap(),
        bytes
    );
    let board = doc.pcb.as_mut().unwrap();
    board.items.push(Item::new(
        TOP,
        None,
        Shape::Track {
            start: Point::mm(1, 1),
            end: Point::mm(2, 1),
            width: 200_000,
        },
    ));
    let output = writer::serialize(&doc, writer::Format::Preserve).unwrap();
    for family in ["Regions6", "ShapeBasedRegions6"] {
        let path = format!("/{family}/Data");
        assert_eq!(stream(&output, &path), stream(&bytes, &path));
    }
    import_pcb(&output, "roundtrip.PcbDoc")
        .unwrap()
        .validate()
        .unwrap();
}
