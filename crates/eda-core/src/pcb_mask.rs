//! Mask overrides and bounded evaluation of imported mask-expansion rules.
use super::*;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MaskKind {
    Paste,
    Solder,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum MaskMode {
    #[default]
    Rule,
    Manual,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MaskSide {
    pub mode: MaskMode,
    pub expansion: i64,
    pub percentage: f64,
    pub enabled: bool,
    pub tented: bool,
    #[serde(default)]
    pub from_hole_edge: bool,
}
impl Default for MaskSide {
    fn default() -> Self {
        Self {
            mode: MaskMode::Rule,
            expansion: 0,
            percentage: 0.0,
            enabled: true,
            tented: false,
            from_hole_edge: false,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MaskSettings {
    pub paste: [MaskSide; 2],
    pub solder: [MaskSide; 2],
    #[serde(default = "linked")]
    pub linked_paste: bool,
    #[serde(default = "linked")]
    pub linked_solder: bool,
}
fn linked() -> bool {
    true
}
impl Default for MaskSettings {
    fn default() -> Self {
        Self {
            paste: Default::default(),
            solder: Default::default(),
            linked_paste: true,
            linked_solder: true,
        }
    }
}
impl MaskSettings {
    pub fn for_item(item: &Item) -> Self {
        let mut value = Self::default();
        if !matches!(item.shape, Shape::Pad { hole: 0, .. }) {
            for s in &mut value.paste {
                s.enabled = false;
            }
        }
        value
    }
    pub fn side(&self, kind: MaskKind, bottom: bool) -> &MaskSide {
        match kind {
            MaskKind::Paste => &self.paste[usize::from(bottom)],
            MaskKind::Solder => &self.solder[usize::from(bottom)],
        }
    }
    pub fn side_mut(&mut self, kind: MaskKind, bottom: bool) -> &mut MaskSide {
        match kind {
            MaskKind::Paste => &mut self.paste[usize::from(bottom)],
            MaskKind::Solder => &mut self.solder[usize::from(bottom)],
        }
    }
}
#[derive(Clone, Debug)]
pub struct ResolvedMask {
    pub expansion: i64,
    pub percentage: f64,
    pub open: bool,
    pub rule: Option<usize>,
}
fn yes(fields: &Fields, key: &str) -> bool {
    fields
        .get(key)
        .is_some_and(|v| v.eq_ignore_ascii_case("TRUE") || v == "1")
}
pub fn dimension(value: &str) -> Result<i64, String> {
    let s = value.trim().to_ascii_lowercase();
    let (n, scale) = if let Some(n) = s.strip_suffix("mm") {
        (n, 1e6)
    } else if let Some(n) = s.strip_suffix("mil") {
        (n, 25_400.0)
    } else if let Some(n) = s.strip_suffix("inch") {
        (n, 25_400_000.0)
    } else {
        (s.as_str(), 25_400.0)
    };
    let n = n
        .trim()
        .parse::<f64>()
        .map_err(|_| "Invalid mask dimension")?
        * scale;
    if !n.is_finite() || n.abs() > 100_000_000_000.0 {
        return Err("Mask dimension outside coordinate limits".into());
    }
    Ok(n.round() as i64)
}
fn percentage(fields: &Fields) -> Result<f64, String> {
    let value = fields
        .get("PERCENTAGE")
        .map(|s| s.parse::<f64>())
        .transpose()
        .map_err(|_| "Invalid mask rule percentage")?
        .unwrap_or(0.0);
    if !value.is_finite() || !(-100.0..=1000.0).contains(&value) {
        return Err("Invalid mask rule percentage".into());
    }
    Ok(value)
}
// Unknown predicates deliberately propagate; a higher-priority unknown rule
// must not silently be replaced with a lower-priority All rule.
fn scope(board: &Board, item: &Item, s: &str, depth: usize) -> Option<bool> {
    if depth > 32 || s.len() > 8192 {
        return None;
    }
    let mut s = s.trim();
    if s.starts_with('(') && s.ends_with(')') {
        let mut level = 0;
        let mut quoted = false;
        let mut closes = false;
        for (n, c) in s.char_indices() {
            if c == '\'' {
                quoted = !quoted;
            }
            if !quoted {
                if c == '(' {
                    level += 1;
                }
                if c == ')' {
                    level -= 1;
                    if level == 0 && n != s.len() - 1 {
                        closes = true;
                        break;
                    }
                }
            }
        }
        if !closes {
            s = &s[1..s.len() - 1];
        }
    }
    let upper = s.to_ascii_uppercase();
    for op in [" OR ", " AND "] {
        let mut level = 0;
        let mut quoted = false;
        for (n, c) in s.char_indices() {
            if c == '\'' {
                quoted = !quoted;
            }
            if quoted {
                continue;
            }
            if c == '(' {
                level += 1;
            } else if c == ')' {
                level -= 1;
            }
            if level == 0 && upper[n..].starts_with(op) {
                let a = scope(board, item, &s[..n], depth + 1);
                let b = scope(board, item, &s[n + op.len()..], depth + 1);
                return if op == " AND " {
                    if a == Some(false) || b == Some(false) {
                        Some(false)
                    } else {
                        a.zip(b).map(|(a, b)| a && b)
                    }
                } else if a == Some(true) || b == Some(true) {
                    Some(true)
                } else {
                    a.zip(b).map(|(a, b)| a || b)
                };
            }
        }
    }
    if upper.starts_with("NOT ") {
        return scope(board, item, &s[4..], depth + 1).map(|v| !v);
    }
    match upper.as_str() {
        "ALL" | "TRUE" => return Some(true),
        "FALSE" => return Some(false),
        "ISPAD" => return Some(matches!(item.shape, Shape::Pad { .. })),
        "ISVIA" => return Some(matches!(item.shape, Shape::Via { .. })),
        "ISTRACK" => return Some(matches!(item.shape, Shape::Track { .. })),
        "ISARC" => return Some(matches!(item.shape, Shape::Arc { .. })),
        "ISSMDPAD" => return Some(matches!(item.shape, Shape::Pad { hole: 0, .. })),
        _ => {}
    }
    for name in ["INNET", "INCOMPONENT", "HASFOOTPRINT"] {
        if let Some(arg) = upper.strip_prefix(&format!("{name}("))
            && arg.ends_with(')')
        {
            let raw = s[name.len() + 1..s.len() - 1].trim();
            let value = raw.strip_prefix('\'')?.strip_suffix('\'')?;
            let component = item
                .component
                .and_then(|index| board.components.iter().find(|c| c.index == index));
            return Some(match name {
                "INNET" => board.net_name(item.net).eq_ignore_ascii_case(value),
                "INCOMPONENT" => {
                    component.is_some_and(|c| c.designator.eq_ignore_ascii_case(value))
                }
                _ => component.is_some_and(|c| c.footprint.eq_ignore_ascii_case(value)),
            });
        }
    }
    None
}
impl Board {
    pub fn resolve_mask(
        &self,
        item: &Item,
        kind: MaskKind,
        bottom: bool,
    ) -> Result<ResolvedMask, String> {
        let settings = item
            .masks
            .as_deref()
            .cloned()
            .unwrap_or_else(|| MaskSettings::for_item(item));
        let side = settings.side(kind, bottom);
        let mut result = ResolvedMask {
            expansion: side.expansion,
            percentage: side.percentage,
            open: side.enabled && !side.tented,
            rule: None,
        };
        if side.mode == MaskMode::Manual {
            return Ok(result);
        }
        let key = match kind {
            MaskKind::Paste => "PasteMaskExpansion",
            MaskKind::Solder => "SolderMaskExpansion",
        };
        let mut rules: Vec<_> = self
            .rules
            .iter()
            .enumerate()
            .filter(|(_, r)| {
                r.get("RULEKIND")
                    .is_some_and(|v| v.eq_ignore_ascii_case(key))
                    && !r
                        .get("ENABLED")
                        .is_some_and(|v| v.eq_ignore_ascii_case("FALSE"))
            })
            .collect();
        rules.sort_by_key(|(i, r)| {
            (
                r.get("PRIORITY")
                    .and_then(|v| v.parse::<u32>().ok())
                    .unwrap_or(u32::MAX),
                *i,
            )
        });
        for (index, r) in rules {
            let expr = r
                .get("SCOPE1EXPRESSION")
                .map(String::as_str)
                .unwrap_or("All");
            match scope(self, item, expr, 0) {
                Some(false) => continue,
                None => return Err(format!("Unsupported mask rule scope: {expr}")),
                Some(true) => {}
            }
            result.expansion = dimension(r.get("EXPANSION").map(String::as_str).unwrap_or("0mil"))?;
            result.percentage = percentage(r)?;
            result.open &= !yes(
                r,
                if bottom {
                    "ISTENTINGBOTTOM"
                } else {
                    "ISTENTINGTOP"
                },
            );
            result.rule = Some(index);
            return Ok(result);
        }
        result.expansion = if kind == MaskKind::Solder { 100_000 } else { 0 };
        result.percentage = 0.0;
        Ok(result)
    }
    pub fn replace_mask_rule(&self, index: usize, rule: Fields) -> Result<Self, String> {
        let old = self.rules.get(index).ok_or("Mask rule no longer exists")?;
        if !matches!(
            old.get("RULEKIND").map(String::as_str),
            Some("PasteMaskExpansion" | "SolderMaskExpansion")
        ) || old.get("RULEKIND") != rule.get("RULEKIND")
        {
            return Err("Only existing mask-expansion rules can be edited".into());
        }
        for key in old.keys().chain(rule.keys()) {
            if old.get(key) != rule.get(key)
                && !matches!(
                    key.as_str(),
                    "NAME"
                        | "PRIORITY"
                        | "ENABLED"
                        | "SCOPE1EXPRESSION"
                        | "EXPANSION"
                        | "PERCENTAGE"
                        | "ISTENTINGTOP"
                        | "ISTENTINGBOTTOM"
                )
            {
                return Err(format!("Unsupported rule field: {key}"));
            }
        }
        dimension(rule.get("EXPANSION").map(String::as_str).unwrap_or("0mil"))?;
        if rule
            .get("PRIORITY")
            .is_some_and(|v| v.parse::<u32>().is_err())
        {
            return Err("Invalid rule priority".into());
        }
        percentage(&rule)?;
        let mut next = self.clone();
        next.rules[index] = rule;
        next.validate()?;
        Ok(next)
    }
}
impl Item {
    pub(super) fn validate_masks(&self) -> Result<(), String> {
        if let Some(m) = &self.masks {
            for s in m.paste.iter().chain(&m.solder) {
                if s.expansion.unsigned_abs() > 100_000_000_000
                    || !s.percentage.is_finite()
                    || s.percentage.abs() > 1000.0
                {
                    return Err("Invalid mask expansion".into());
                }
            }
        }
        Ok(())
    }
    pub fn mask_outline(
        &self,
        board: &Board,
        kind: MaskKind,
        bottom: bool,
    ) -> Result<Vec<Point>, String> {
        let resolved = board.resolve_mask(self, kind, bottom)?;
        if !resolved.open {
            return Ok(vec![]);
        }
        let layer = if bottom { BOTTOM } else { TOP };
        let Some(mut g) = self.pad_geometry(layer) else {
            return Ok(vec![]);
        };
        if matches!(self.shape, Shape::Via { .. })
            && !crate::pcb_polygon::on_layer(board, self, layer)
        {
            return Ok(vec![]);
        }
        let from_hole = (g.size == Point::default()
            || self
                .masks
                .as_ref()
                .is_some_and(|m| m.side(kind, bottom).from_hole_edge))
            && self.drill_size() > 0;
        if from_hole {
            g.offset = Point::default();
            g.shape = if self
                .pad_stack
                .as_ref()
                .is_some_and(|s| s.drill_shape == DrillShape::Square)
            {
                PadShape::Rectangle
            } else {
                PadShape::Round
            };
            g.size = Point {
                x: self
                    .pad_stack
                    .as_ref()
                    .filter(|s| s.drill_shape == DrillShape::Slot)
                    .map(|s| s.slot_length)
                    .unwrap_or(self.drill_size()),
                y: self.drill_size(),
            };
        }
        if self.layer != MULTI_LAYER && self.layer != layer {
            return Ok(vec![]);
        }
        for n in [&mut g.size.x, &mut g.size.y] {
            *n = (*n as f64 * (1.0 + resolved.percentage / 100.0) + 2.0 * resolved.expansion as f64)
                .round() as i64;
        }
        if g.size.x <= 0 || g.size.y <= 0 {
            return Ok(vec![]);
        }
        let rotation = match self.shape {
            Shape::Pad { rotation, .. } => rotation,
            _ => 0.0,
        } + if from_hole {
            self.pad_stack
                .as_ref()
                .map(|s| s.drill_rotation)
                .unwrap_or(0.0)
        } else {
            0.0
        };
        let local = crate::pcb_polygon::pad_outline_radius(
            g.offset,
            g.size,
            0.0,
            g.shape,
            g.corner_percent,
        );
        local
            .into_iter()
            .map(|p| transform_point(p, Point::default(), self.anchor(), rotation))
            .collect()
    }
}
