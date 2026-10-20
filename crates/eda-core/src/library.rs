//! Portable schematic library definitions. Instances contain their own geometry and resources.
use crate::schematic::{SchPrimitiveKind, Schematic, TextRole};
use crate::schematic_edit::Fragment;
use eda_geom::Point;

#[derive(Clone, Debug)]
pub struct Library {
    pub name: String,
    pub symbols: Vec<Symbol>,
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct Symbol {
    pub name: String,
    pub description: String,
    pub parts: u32,
    pub display_modes: u32,
    pub variants: Vec<SymbolVariant>,
}

#[derive(Clone, Debug)]
pub struct SymbolVariant {
    pub part: u32,
    pub display_mode: u32,
    pub fragment: Fragment,
}

impl Symbol {
    pub fn variant(&self, part: u32, display_mode: u32) -> Option<&SymbolVariant> {
        self.variants
            .iter()
            .find(|v| v.part == part && v.display_mode == display_mode)
    }

    /// Create independent UUIDs and an unused annotation before preview or insertion.
    pub fn instantiate(
        &self,
        part: u32,
        display_mode: u32,
        sheet: &Schematic,
    ) -> Result<Fragment, String> {
        let variant = self
            .variant(part, display_mode)
            .ok_or("Symbol variant is unavailable")?;
        let mut fragment = variant.fragment.fresh_copy();
        let c = fragment
            .components
            .first_mut()
            .ok_or("Library symbol has no component")?;
        let base = if self.parts > 1 {
            c.designator
                .strip_suffix(char::from_u32('A' as u32 + part - 1).unwrap_or('?'))
                .unwrap_or(&c.designator)
        } else {
            &c.designator
        };
        let prefix = base.trim_end_matches(|ch: char| ch == '?' || ch.is_ascii_digit());
        let prefix = if prefix.is_empty() { "U" } else { prefix };
        let suffix = if self.parts > 1 {
            char::from_u32('A' as u32 + part - 1)
                .unwrap_or('?')
                .to_string()
        } else {
            String::new()
        };
        let designator = (1..)
            .map(|n| format!("{prefix}{n}{suffix}"))
            .find(|d| {
                !sheet
                    .components
                    .iter()
                    .any(|c| c.designator.eq_ignore_ascii_case(d))
            })
            .unwrap();
        c.designator = designator.clone();
        for p in &mut fragment.primitives {
            if p.text_role == Some(TextRole::Designator) {
                p.presentation.raw_text = Some(designator.clone());
                if let SchPrimitiveKind::Text { text, .. } = &mut p.kind {
                    *text = designator.clone();
                }
            }
        }
        let origin = fragment.components[0].position;
        fragment.transformed(
            origin,
            0,
            Point {
                x: -origin.x,
                y: -origin.y,
            },
        )
    }
}
