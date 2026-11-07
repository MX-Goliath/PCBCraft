use super::*;
#[test]
fn native_project_variant_states_parameters_and_unknown_fields_roundtrip() {
    let text = "[Design]\nUnknownOption=preserve\n[Document1]\nDocumentPath=main.SchDoc\n[ProjectVariant1]\nUniqueID=VARIANT_A\nDescription=Assembly A\nCustomFutureKey=keep\nVariationCount=2\nVariation1=Designator=R1|UniqueId=\\CHILD\\RESISTOR|Kind=1|AlternatePart= |FutureField=opaque\nVariation2=Designator=C1|UniqueId=\\CAP|Kind=0|AlternatePart=\nParamVariation1=ParameterName=Comment|VariantValue=22k\nParamDesignator1=R1\nParamVariationCount=1\nParameterCount=1\nParameter1=Name=Title|Value=Assembly title\n[Configuration1]\nVariant=Assembly A\n";
    let v = parse(text).unwrap();
    assert_eq!(v.len(), 1);
    assert_eq!(v[0].variations.len(), 2);
    assert_eq!(v[0].variations[0].kind, 1);
    assert_eq!(v[0].variations[0].parameters["Comment"], "22k");
    assert_eq!(v[0].parameters["Title"], "Assembly title");
    let next = rewrite(text, &v).unwrap();
    assert!(next.contains("UnknownOption=preserve"));
    assert!(next.contains("[Configuration1]"));
    assert!(next.contains("CustomFutureKey=keep"));
    assert!(next.contains("FutureField=opaque"));
    let roundtrip = parse(&next).unwrap();
    assert_eq!(roundtrip[0].variations, v[0].variations);
    assert_eq!(roundtrip[0].parameters, v[0].parameters);
    let mut bad = v.clone();
    bad[0].variations[0]
        .parameters
        .insert("Comment".into(), "bad|value".into());
    assert!(rewrite(text, &bad).is_err());
}
#[test]
fn project_variant_save_guards_external_changes_and_supports_unicode_save_as() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Variants.PrjPcb");
    std::fs::write(&path, "[Design]\nFuture=1\n").unwrap();
    let mut p = altium::load_project(&path).unwrap();
    let before = std::fs::read(&path).unwrap();
    p.variants
        .push(Variant::new("Assembly Unicode \u{416}".into()));
    p.variants[0].variation_mut("\\RUID", "R1").kind = 1;
    save(&p, &before).unwrap();
    let p2 = altium::load_project(&path).unwrap();
    assert_eq!(p2.variants[0].name, p.variants[0].name);
    assert_eq!(p2.variants[0].variations[0].kind, 1);
    let saved = std::fs::read(&path).unwrap();
    assert!(save(&p, &before).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), saved);
    p.variants[0].name = "Save As Current Draft".into();
    let copy = dir.path().join("Copy.PrjPcb");
    let p3 = project_files::save_project_as(&p, &copy).unwrap();
    assert_eq!(p3.variants[0].name, "Save As Current Draft");
    assert_eq!(std::fs::read(&path).unwrap(), saved);
}

#[test]
fn case_insensitive_native_fields_are_rewritten_without_stale_duplicates() {
    let source = "[projectvariant1]\nuniqueid=A\ndescription=A\nvariation1=designator=R1|uniqueid=\\R|kind=1|Future=X\nparamdesignator1=R1\nparamvariation1=parametername=Value|variantvalue=22k\n";
    let mut variants = parse(source).unwrap();
    variants[0].variations[0].kind = 0;
    let text = rewrite(source, &variants).unwrap();
    let next = parse(&text).unwrap();
    assert_eq!(next[0].variations[0].kind, 0);
    assert_eq!(next[0].variations[0].parameters["Value"], "22k");
    assert!(!text.contains("kind=1"));
    assert!(text.contains("Future=X"));
}

#[test]
fn physical_parameter_identity_survives_reopening_repeated_designators() {
    let mut v = Variant::new("A".into());
    v.variation_mut("\\LEFT\\R", "R1")
        .parameters
        .insert("Value".into(), "22k".into());
    v.variation_mut("\\RIGHT\\R", "R1")
        .parameters
        .insert("Value".into(), "33k".into());
    let text = rewrite("[Design]\n", &[v]).unwrap();
    let next = parse(&text).unwrap();
    assert_eq!(
        next[0].variation("\\LEFT\\R", "R1").unwrap().parameters["Value"],
        "22k"
    );
    assert_eq!(
        next[0].variation("\\RIGHT\\R", "R1").unwrap().parameters["Value"],
        "33k"
    );
    let ambiguous = text
        .lines()
        .filter(|l| !l.starts_with("PCBCraftParamUniqueId"))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(parse(&ambiguous).unwrap_err().contains("Ambiguous"));
}

#[test]
fn observed_native_variant_parameter_sections_reindex_with_variant_deletion() {
    let text = "[Design]\n[Parameter1_1]\nName=Project\nValue=Base\n[ProjectVariant1]\nUniqueID=A\nDescription=A\nParameterCount=1\n[Parameter2_1]\nName=Title\nValue=Assembly A\n[ProjectVariant2]\nUniqueID=B\nDescription=B\nParameterCount=1\n[Parameter3_1]\nName=Title\nValue=Assembly B\n";
    let mut v = parse(text).unwrap();
    assert_eq!(v[0].parameters["Title"], "Assembly A");
    assert_eq!(v[1].parameters["Title"], "Assembly B");
    v.remove(0);
    let rewritten = rewrite(text, &v).unwrap();
    assert!(rewritten.contains("[Parameter1_1]"));
    assert!(!rewritten.contains("[Parameter3_1]"));
    assert_eq!(
        parse(&rewritten).unwrap()[0].parameters["Title"],
        "Assembly B"
    );
}

#[test]
fn new_alternate_native_library_reopens_without_private_geometry_extension() {
    use eda_core::{DocumentKind, library_edit, schematic_edit};
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Assembly.PrjPcb");
    std::fs::write(&path, "[Design]\n").unwrap();
    let mut p = altium::load_project(&path).unwrap();
    let doc = library_edit::member(DocumentKind::Schematic, "Alternate resistor").unwrap();
    let s = doc.schematic.as_ref().unwrap();
    let fragment = schematic_edit::extract(s, s.components[0].id).unwrap();
    let mut v = Variant::new("Assembly".into());
    let c = v.variation_mut("\\RUID", "R1");
    c.kind = 2;
    c.alternate = Some(fragment);
    p.variants.push(v);
    let before = std::fs::read(&path).unwrap();
    save_with_alternates(&p, &before, None).unwrap();
    let library = path.with_extension("PrjPcbVariants");
    assert!(
        std::fs::read(&library)
            .unwrap()
            .starts_with(&[0xd0, 0xcf, 0x11, 0xe0])
    );
    let text = project_files::decode(&std::fs::read(&path).unwrap()).unwrap();
    assert!(!text.contains("AltLibLink_LibraryIdentifier=\\\\?\\"));
    let native = text
        .lines()
        .filter(|l| !l.starts_with("PCBCraftAlternateData"))
        .collect::<Vec<_>>()
        .join("\r\n");
    project_files::write_atomic(&path, native.as_bytes()).unwrap();
    let loaded = altium::load_project(&path).unwrap();
    let c = &loaded.variants[0].variations[0];
    assert_eq!(
        c.alternate.as_ref().unwrap().components[0].library_reference,
        "Alternate resistor"
    );
    let project_bytes = std::fs::read(&path).unwrap();
    let original_library = std::fs::read(&library).unwrap();
    std::fs::write(&library, b"external change").unwrap();
    assert!(save_with_alternates(&p, &project_bytes, Some(&original_library)).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), project_bytes);
    assert_eq!(std::fs::read(&library).unwrap(), b"external change");
}

#[test]
fn native_component_records_precede_interleaved_parameter_pairs() {
    let mut v = Variant::new("Assembly".into());
    let c = v.variation_mut("\\R", "R1");
    c.kind = 1;
    c.parameters.insert("Value".into(), " 22k ".into());
    c.parameters.insert("Tolerance".into(), "1%".into());
    let text = rewrite("[Design]\n", &[v]).unwrap();
    assert!(text.contains("Variation1=Designator=R1|UniqueId=\\R|Kind=1|AlternatePart="));
    assert!(text.find("Variation1=").unwrap() < text.find("ParamVariation1=").unwrap());
    assert!(text.find("ParamVariation1=").unwrap() < text.find("ParamDesignator1=").unwrap());
    assert!(text.find("ParamDesignator1=").unwrap() < text.find("ParamVariation2=").unwrap());
    assert_eq!(
        parse(&text).unwrap()[0].variations[0].parameters["Value"],
        " 22k "
    );
}

#[test]
fn legacy_project_without_cyrillic_paths_keeps_cyrillic_variant_values() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Assembly.PrjPcb");
    let text = "[Design]\r\n[ProjectVariant1]\r\nUniqueID=A\r\nDescription=Сборка\r\nVariation1=Designator=R1|UniqueId=\\R|Kind=0\r\nParamVariation1=ParameterName=Comment|VariantValue=Резистор\r\nParamDesignator1=R1\r\n";
    let bytes = encoding_rs::WINDOWS_1251.encode(text).0.into_owned();
    std::fs::write(&path, bytes).unwrap();
    let project = altium::load_project(&path).unwrap();
    assert_eq!(project.variants[0].name, "Сборка");
    assert_eq!(
        project.variants[0].variations[0].parameters["Comment"],
        "Резистор"
    );
}
