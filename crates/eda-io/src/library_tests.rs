use crate::altium::library;
use eda_core::{
    Document, DocumentKind, History,
    schematic::{SchPrimitiveKind as K, TextRole},
    schematic_edit::{Edit, Request},
};
use eda_geom::Point;
use std::io::{Cursor, Write};

fn block(flag: u32, data: &[u8]) -> Vec<u8> {
    [
        ((flag << 24) | data.len() as u32).to_le_bytes().as_slice(),
        data,
    ]
    .concat()
}
fn text(s: &str) -> Vec<u8> {
    block(0, format!("{s}\0").as_bytes())
}
fn compress(data: &[u8]) -> Vec<u8> {
    let mut z = flate2::write::ZlibEncoder::new(vec![], flate2::Compression::default());
    z.write_all(data).unwrap();
    z.finish().unwrap()
}
fn container(streams: Vec<(&str, Vec<u8>)>) -> Vec<u8> {
    let mut f = cfb::CompoundFile::create(Cursor::new(vec![])).unwrap();
    for (path, data) in streams {
        let p = std::path::Path::new(path);
        if let Some(parent) = p.parent()
            && !parent.as_os_str().is_empty()
            && !f.is_storage(parent)
        {
            f.create_storage_all(parent).unwrap();
        }
        f.create_stream(p).unwrap().write_all(&data).unwrap();
    }
    f.into_inner().into_inner()
}
fn aux(key: &str, decoded: &[u8]) -> Vec<u8> {
    let z = compress(decoded);
    let raw = [
        &[0xd0, key.len() as u8][..],
        key.as_bytes(),
        &(z.len() as u32).to_le_bytes(),
        &z,
    ]
    .concat();
    [text("|HEADER=Pin auxiliary|Weight=1"), block(1, &raw)].concat()
}
fn wide(s: &str) -> Vec<u8> {
    let bytes: Vec<u8> = s.encode_utf16().flat_map(u16::to_le_bytes).collect();
    [&(bytes.len() as u32).to_le_bytes()[..], &bytes].concat()
}
fn pin(part: i16, mode: u8) -> Vec<u8> {
    let mut bytes = 2u32.to_le_bytes().to_vec();
    bytes.push(0);
    bytes.extend(part.to_le_bytes());
    bytes.extend([mode, 0, 0, 0, 0, 0, 1, 4, 24]);
    for v in [10i16, -1, 2] {
        bytes.extend(v.to_le_bytes());
    }
    bytes.extend(0x800000u32.to_le_bytes());
    bytes.extend([1, b'X', 1, b'1', 0, 3, b'|', b'&', b'|', 0]);
    bytes
}
fn fixture(pin_data: Vec<u8>) -> Vec<u8> {
    let header = text(
        "|HEADER=Protel for Windows - Schematic Library Editor Binary File Version 5.0|CompCount=1|FontIdCount=2|FontName1=Arial|Size1=10|FontName2=Times New Roman|Size2=8",
    );
    let data=[
        text("|RECORD=1|LIBREFERENCE=Unit|COMPONENTDESCRIPTION=Test symbol|PARTCOUNT=3|DISPLAYMODECOUNT=2|CURRENTPARTID=1"),
        block(1,&pin_data),
        text("|RECORD=6|OWNERPARTID=-1|LOCATIONCOUNT=2|X1=0|Y1=0|X2=10|Y2=10|COLOR=128"),
        text("|RECORD=6|OWNERPARTID=-1|OWNERPARTDISPLAYMODE=1|LOCATIONCOUNT=2|X1=0|Y1=0|X2=20|Y2=20|COLOR=128"),
        text("|RECORD=34|OWNERPARTID=-1|TEXT=U?|FONTID=2|LOCATION.Y=20"),
        text("|RECORD=41|OWNERPARTID=-1|NAME=Comment|TEXT==Value|FONTID=2|LOCATION.Y=-20"),
        text("|RECORD=41|OWNERPARTID=-1|NAME=Value|TEXT=10k|ISHIDDEN=T|FONTID=1"),
        text("|RECORD=44"),
        text("|RECORD=45|OWNERINDEX=7|MODELNAME=R0805|MODELTYPE=PCBLIB|ISCURRENT=T"),
    ].concat();
    let frac: Vec<u8> = [-25_000i32, 50_000, 0]
        .into_iter()
        .flat_map(i32::to_le_bytes)
        .collect();
    container(vec![
        ("FileHeader", header),
        ("Unit/Data", data),
        ("Unit/PinFrac", aux("0", &frac)),
        (
            "Unit/PinWideText",
            aux("0", &wide("|NAME=Вход|DESIGNATOR=α|DESC=Вывод")),
        ),
        (
            "Unit/PinTextData",
            aux("0", &[16, 2, 0, 0, 0, 0, 0, 16, 1, 0, 0, 0, 0, 0]),
        ),
    ])
}

#[test]
fn schlib_variants_binary_pins_unicode_fractions_fonts_and_models() {
    let bytes = fixture(pin(1, 0));
    let lib = library::import(&bytes, "parts.SchLib").unwrap();
    assert!(lib.warnings.is_empty(), "{:?}", lib.warnings);
    assert_eq!(lib.symbols.len(), 1);
    let symbol = &lib.symbols[0];
    assert_eq!((symbol.parts, symbol.display_modes), (2, 2));
    assert_eq!(symbol.variants.len(), 4);
    let f = &symbol.variant(1, 0).unwrap().fragment;
    assert_eq!(f.components[0].comment, "10k");
    assert_eq!(f.components[0].settings.models[0].name, "R0805");
    assert!(
        f.primitives
            .iter()
            .any(|p| p.presentation.hidden && p.presentation.name == "Value")
    );
    let p = f
        .primitives
        .iter()
        .find(|p| matches!(p.kind, K::Pin { .. }))
        .unwrap();
    assert!(
        matches!(&p.kind,K::Pin{position,length_nm,name,number,font,number_font,..} if *position==Point{x:-317_500,y:635_000} && *length_nm==2_540_000 && name=="Вход" && number=="α" && *font==1 && *number_font==Some(0))
    );
    assert!(
        !symbol
            .variant(2, 0)
            .unwrap()
            .fragment
            .primitives
            .iter()
            .any(|p| matches!(p.kind, K::Pin { .. }))
    );
    assert!(symbol.variant(1,1).unwrap().fragment.primitives.iter().any(|p|matches!(&p.kind,K::Polyline{points,..}if points[1]==Point{x:5_080_000,y:5_080_000})));
}

#[test]
fn intlib_instances_are_independent_and_survive_history_and_save_reopen() {
    let schlib = fixture(pin(1, 0));
    let intlib = container(vec![
        ("SchLib/0.schlib", [vec![2], compress(&schlib)].concat()),
        ("Version.Txt", vec![0, 2, 0, 0, 0]),
    ]);
    let library = library::import(&intlib, "parts.IntLib").unwrap();
    let mut doc = Document::new(DocumentKind::Schematic, "library instances");
    let initial = doc.clone();
    let mut history = History::default();
    for (n, turns) in [(1, 0), (2, 1)] {
        let f = library.symbols[0]
            .instantiate(1, 0, doc.schematic.as_ref().unwrap())
            .unwrap()
            .transformed(Point::default(), turns, Point::mm(n * 20, 30))
            .unwrap();
        assert_eq!(f.components[0].designator, format!("U{n}A"));
        assert!(
            f.primitives
                .iter()
                .any(|p| p.text_role == Some(TextRole::Designator)
                    && p.presentation.raw_text == Some(format!("U{n}A")))
        );
        let request = Request {
            document: doc.id,
            expected_revision: history.revision,
            edit: Edit::Insert(f),
        };
        history.schematic_edit(&mut doc, request).unwrap();
    }
    doc.validate().unwrap();
    let placed = doc.clone();
    let s = doc.schematic.as_ref().unwrap();
    assert_ne!(s.components[0].id, s.components[1].id);
    assert_eq!(s.fonts.len(), 2);
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("instances.pcbcraft.json");
    crate::save(&path, &doc).unwrap();
    drop(library);
    assert_eq!(crate::load(&path).unwrap(), doc);
    history.undo(&mut doc);
    history.undo(&mut doc);
    assert_eq!(doc, initial);
    history.redo(&mut doc);
    history.redo(&mut doc);
    assert_eq!(doc, placed);
    let original = directory.path().join("original.IntLib");
    std::fs::write(&original, &intlib).unwrap();
    assert!(crate::save(&original, &doc).is_err());
    assert_eq!(std::fs::read(original).unwrap(), intlib);
}

#[test]
fn library_rejects_damaged_binary_compression_and_auxiliary_records() {
    let valid = pin(1, 0);
    for end in 0..30 {
        assert!(
            library::import(&fixture(valid[..end].to_vec()), "truncated.SchLib").is_err(),
            "pin length {end}"
        );
    }
    let schlib = fixture(valid);
    for missing in 1..=5 {
        let mut damaged = compress(&schlib);
        damaged.truncate(damaged.len() - missing);
        let bytes = container(vec![("SchLib/0.schlib", [vec![2], damaged].concat())]);
        assert!(
            library::import(&bytes, "bad.IntLib").is_err(),
            "missing checksum bytes {missing}"
        );
    }
    let bytes = container(vec![("SchLib/0.schlib", vec![3, 1, 2, 3])]);
    assert!(library::import(&bytes, "unsupported.IntLib").is_err());
    for bytes in [&[][..], &[0xd0, 0xcf][..], &schlib[..schlib.len() / 2]] {
        assert!(library::import(bytes, "bad.SchLib").is_err());
    }
}

#[test]
fn library_images_travel_with_inserted_symbol() {
    use std::io::Read;
    let bytes = fixture(pin(1, 0));
    let mut file = cfb::CompoundFile::open(Cursor::new(bytes)).unwrap();
    let paths: Vec<_> = file
        .walk()
        .filter(|e| e.is_stream())
        .map(|e| e.path().to_owned())
        .collect();
    let mut files = vec![];
    for path in paths {
        let mut data = vec![];
        file.open_stream(&path)
            .unwrap()
            .read_to_end(&mut data)
            .unwrap();
        if path.file_name().unwrap() == "Data" {
            data.extend(text("|RECORD=30|OWNERPARTID=-1|FILENAME=Icon.PNG|LOCATION.X=0|LOCATION.Y=0|CORNER.X=10|CORNER.Y=10|KEEPASPECT=T"));
        }
        files.push((path.to_string_lossy().into_owned(), data));
    }
    let image = b"independent image payload";
    let compressed = compress(image);
    let raw = [
        &[0xd0, 8][..],
        &b"Icon.PNG"[..],
        &(compressed.len() as u32).to_le_bytes(),
        &compressed,
    ]
    .concat();
    files.push((
        "Storage".into(),
        [text("|HEADER=Icon storage"), block(1, &raw)].concat(),
    ));
    let library = library::import(
        &container(files.iter().map(|(k, v)| (k.as_str(), v.clone())).collect()),
        "images.SchLib",
    )
    .unwrap();
    assert!(library.warnings.is_empty(), "{:?}", library.warnings);
    let doc = Document::new(DocumentKind::Schematic, "image instance");
    let fragment = library.symbols[0]
        .instantiate(1, 0, doc.schematic.as_ref().unwrap())
        .unwrap();
    let placed = eda_core::schematic_edit::preview(&doc, &Edit::Insert(fragment)).unwrap();
    assert_eq!(
        placed.schematic.as_ref().unwrap().images["icon.png"],
        base64::Engine::encode(&base64::engine::general_purpose::STANDARD, image)
    );
    assert!(
        placed
            .schematic
            .as_ref()
            .unwrap()
            .primitives
            .iter()
            .any(|p| matches!(&p.kind,K::Image{filename,..}if filename=="Icon.PNG"))
    );
}

#[test]
fn library_limits_and_missing_auxiliary_owners_are_rejected() {
    let header = text(
        "|HEADER=Protel for Windows - Schematic Library Editor Binary File Version 5.0|CompCount=1",
    );
    for fields in [
        "PARTCOUNT=-9223372036854775808|DISPLAYMODECOUNT=1",
        "PARTCOUNT=2|DISPLAYMODECOUNT=bad",
        "PARTCOUNT=2|DISPLAYMODECOUNT=-1",
        "PARTCOUNT=1025|DISPLAYMODECOUNT=32",
    ] {
        let bytes = container(vec![
            ("FileHeader", header.clone()),
            (
                "Unit/Data",
                text(&format!("|RECORD=1|LIBREFERENCE=Unit|{fields}")),
            ),
        ]);
        assert!(library::import(&bytes, "bad.SchLib").is_err());
    }
    let bytes = container(vec![
        ("FileHeader", header),
        (
            "Unit/Data",
            text("|RECORD=1|LIBREFERENCE=Unit|PARTCOUNT=2|DISPLAYMODECOUNT=1"),
        ),
        ("Unit/PinFrac", aux("999", &[0; 12])),
    ]);
    assert!(library::import(&bytes, "bad.SchLib").is_err());
}
