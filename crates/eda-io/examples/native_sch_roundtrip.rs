//! Native writer diagnostic. Writes only an explicitly supplied separate file.
use eda_core::schematic_edit::{self, Edit};
fn main() -> Result<(), String> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() < 2 {
        return Err("Pass source.SchDoc destination.SchDoc [--exercise]".into());
    }
    let source = std::path::Path::new(&args[0]);
    let destination = std::path::Path::new(&args[1]);
    if destination.exists() {
        return Err("Diagnostic destination must be a new file".into());
    }
    let mut doc = eda_io::load(source)?;
    let s = doc.schematic.as_ref().ok_or("Not a schematic")?;
    println!(
        "{}: {} components, {} records",
        doc.name,
        s.components.len(),
        s.records.len()
    );
    let mut kinds = std::collections::BTreeMap::new();
    for r in s.records.iter() {
        *kinds.entry((r.stream.clone(), r.record_type)).or_insert(0) += 1;
    }
    println!("Native record counts: {kinds:?}");
    let index_fields: std::collections::BTreeSet<_> = s
        .records
        .iter()
        .flat_map(|r| r.fields.keys())
        .filter(|k| k.contains("INDEX"))
        .collect();
    println!("Index fields: {index_fields:?}");
    for r in s.records.iter().filter(|r| r.record_type == 39) {
        println!("Template record: {:?}", r.fields);
    }
    for r in s
        .records
        .iter()
        .filter(|r| matches!(r.record_type, 44..=48))
        .take(6)
    {
        println!("Native model {}: {:?}", r.index, r.fields);
    }
    if args.iter().any(|a| a == "--exercise") {
        let c = s.components.first().ok_or("No components")?;
        let mut settings = c.settings.clone();
        for m in &mut settings.models {
            m.current = false;
        }
        settings.models.push(eda_core::schematic::ComponentModel {
            parameters: Default::default(),
            name: "PCBCraft_Test_0603".into(),
            kind: "PCBLIB".into(),
            current: true,
        });
        doc = schematic_edit::preview(
            &doc,
            &Edit::Batch(vec![
                Edit::ComponentProperties {
                    id: c.id,
                    designator: c.designator.clone(),
                    comment: "Проверка ±1%".into(),
                },
                Edit::ComponentDetails {
                    id: c.id,
                    description: "Native ECO roundtrip".into(),
                    settings,
                },
                Edit::ParameterValue {
                    component: Some(c.id),
                    name: "PCBCraft roundtrip".into(),
                    value: Some("µΩ; 125 °C".into()),
                },
            ]),
        )?;
    }
    eda_io::save(destination, &doc)?;
    println!("Verified and saved {}", destination.display());
    Ok(())
}
