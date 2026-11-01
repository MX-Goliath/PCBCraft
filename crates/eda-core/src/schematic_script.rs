//! Bounded, declarative schematic automation; never invokes an external runtime.
use crate::{
    Document, History,
    schematic_edit::{Edit, Request},
};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Script {
    pub version: u32,
    pub name: String,
    pub edits: Vec<Edit>,
}
impl Script {
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > 2 * 1024 * 1024 {
            return Err("Script exceeds 2 MiB".into());
        }
        let script: Self = serde_json::from_slice(bytes)
            .map_err(|e| format!("Invalid PCBCraft command script: {e}"))?;
        if script.version != 1 || script.edits.is_empty() || script.edits.len() > 1000 {
            return Err("Expected script version 1 with 1–1000 commands".into());
        }
        Ok(script)
    }
    pub fn execute(&self, doc: &mut Document, history: &mut History) -> Result<bool, String> {
        if self.version != 1 || self.edits.is_empty() || self.edits.len() > 1000 {
            return Err("Invalid script version or command count".into());
        }
        history.schematic_edit(
            doc,
            Request {
                document: doc.id,
                expected_revision: history.revision,
                edit: Edit::Batch(self.edits.clone()),
            },
        )
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn batch_failure_is_atomic_and_success_undoes_as_one_step() {
        assert!(
            Script::parse(include_bytes!(
                "../../../fixtures/schematic/set-title.pcbscript.json"
            ))
            .is_ok()
        );
        let mut doc = Document::new(crate::DocumentKind::Schematic, "script");
        let before = doc.clone();
        let mut history = History::default();
        let valid = Edit::ParameterValue {
            component: None,
            name: "Title".into(),
            value: Some("Scripted".into()),
        };
        let mut script = Script {
            version: 1,
            name: "test".into(),
            edits: vec![valid, Edit::Delete(uuid::Uuid::new_v4())],
        };
        assert!(script.execute(&mut doc, &mut history).is_err());
        assert_eq!(doc, before);
        assert!(!history.can_undo());
        script.edits.pop();
        assert!(script.execute(&mut doc, &mut history).unwrap());
        assert_eq!(
            doc.schematic.as_ref().unwrap().sheet.parameters["Title"],
            "Scripted"
        );
        history.undo(&mut doc);
        assert_eq!(doc, before);
        assert!(Script::parse(br#"{"version":2,"name":"bad","edits":[]}"#).is_err());
    }
}
