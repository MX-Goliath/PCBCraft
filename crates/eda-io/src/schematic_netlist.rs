//! Protel NET interchange, observed in AD 24.4.1. Compilation is owned by eda-core.
use eda_core::{
    Document,
    design_sync::{self, ProjectOptions},
};
use std::{collections::BTreeMap, path::Path};

pub struct Output {
    pub bytes: Vec<u8>,
    pub warnings: Vec<String>,
}

fn field(value: &str) -> Result<&str, String> {
    if value.chars().any(char::is_control) || matches!(value.trim(), "[" | "]" | "(" | ")") {
        return Err(
            "A netlist field contains a line break, control character or record delimiter".into(),
        );
    }
    Ok(value)
}

pub fn protel(sheets: &[&Document], options: &ProjectOptions) -> Result<Output, String> {
    let compiled = design_sync::compile_export(sheets, options)?;
    let mut out = String::new();
    for c in &compiled.components {
        // Native component records contain three reserved empty fields.
        out.push_str(&format!(
            "[\r\n{}\r\n{}\r\n{}\r\n\r\n\r\n\r\n]\r\n",
            field(&c.designator)?,
            field(&c.footprint)?,
            field(&c.comment)?
        ));
    }
    let mut nets = BTreeMap::<&str, Vec<String>>::new();
    for pin in &compiled.connectivity.pins {
        if let Some(net) = &pin.net {
            if pin.designator.contains('-') || pin.number.contains('-') || pin.number.is_empty() {
                return Err(
                    "Protel NET cannot unambiguously encode this component or pin identifier"
                        .into(),
                );
            }
            let endpoint = format!("{}-{}", field(&pin.designator)?, field(&pin.number)?);
            nets.entry(field(net)?).or_default().push(endpoint);
        }
    }
    for (net, mut pins) in nets {
        pins.sort();
        pins.dedup(); // Stacked and multi-part pins may share one physical pin.
        out.push_str("(\r\n");
        out.push_str(net);
        out.push_str("\r\n");
        for pin in pins {
            out.push_str(&pin);
            out.push_str("\r\n");
        }
        out.push_str(")\r\n");
    }
    // The observed legacy NET is Windows ANSI (CP1251 on the reference host).
    // Reject unrepresentable characters rather than silently changing net names.
    let (bytes, _, replaced) = encoding_rs::WINDOWS_1251.encode(&out);
    if replaced {
        return Err(
            "Protel NET uses Windows-1251; a field contains an unrepresentable character".into(),
        );
    }
    Ok(Output {
        bytes: bytes.into_owned(),
        warnings: compiled.connectivity.warnings,
    })
}

pub fn save_protel(
    path: &Path,
    sheets: &[&Document],
    options: &ProjectOptions,
) -> Result<Vec<String>, String> {
    let output = protel(sheets, options)?;
    crate::project_files::write_atomic(path, &output.bytes)?;
    Ok(output.warnings)
}

#[cfg(test)]
mod tests {
    use super::*;
    use eda_core::{
        DocumentKind,
        schematic::{ComponentModel, SchPrimitiveKind as K, TextRole},
        schematic_edit::{self as edit, Edit, Placement},
    };
    use eda_geom::Point;
    fn fixture() -> Document {
        let doc = Document::new(DocumentKind::Schematic, "Circuit.SchDoc");
        let mut doc = edit::preview(
            &doc,
            &Edit::Insert(
                edit::place(Placement::Resistor, Point::mm(20, 20), 0, "R1", "10 кОм", 0).unwrap(),
            ),
        )
        .unwrap();
        let s = doc.schematic.as_mut().unwrap();
        s.components[0].settings.models.push(ComponentModel {
            name: "R0603".into(),
            kind: "PCBLIB".into(),
            current: true,
            parameters: Default::default(),
        });
        let pins: Vec<_> = s
            .primitives
            .iter()
            .filter(|p| matches!(p.kind, K::Pin { .. }))
            .map(|p| edit::electrical_endpoint(p).unwrap())
            .collect();
        for (i, position) in pins.into_iter().enumerate() {
            let mut p = edit::primitive(
                K::Text {
                    position,
                    text: format!("N{}", i + 1),
                    font: 0,
                    orientation: 0,
                    justification: 0,
                },
                None,
            );
            p.text_role = Some(TextRole::NetLabel);
            s.primitives.push(p);
        }
        doc
    }
    #[test]
    fn netlist_contains_native_records_current_model_and_physical_pins() {
        let doc = fixture();
        let before = doc.clone();
        let output = protel(&[&doc], &ProjectOptions::default()).unwrap();
        let text = encoding_rs::WINDOWS_1251
            .decode(&output.bytes)
            .0
            .into_owned();
        assert!(text.starts_with("[\r\nR1\r\nR0603\r\n10 кОм\r\n\r\n\r\n\r\n]\r\n"));
        assert!(text.contains("(\r\nN1\r\nR1-1\r\n)\r\n"));
        assert!(text.contains("(\r\nN2\r\nR1-2\r\n)\r\n"));
        assert_eq!(doc, before);
    }
    #[test]
    fn failed_annotation_encoding_or_injected_record_preserves_destination() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("circuit.NET");
        std::fs::write(&path, b"existing").unwrap();
        for value in ["R?", "R1\n]\n(", "R電"] {
            let mut doc = fixture();
            doc.schematic.as_mut().unwrap().components[0].designator = value.into();
            assert!(save_protel(&path, &[&doc], &ProjectOptions::default()).is_err());
            assert_eq!(std::fs::read(&path).unwrap(), b"existing");
        }
    }
}
