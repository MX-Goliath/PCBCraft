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
