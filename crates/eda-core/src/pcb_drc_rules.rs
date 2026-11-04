//! One immutable board/check owns its priority index and bounded scope memo.
//! No cache survives edits, preview changes, settings changes or another board.
use super::*;
use std::{
    cell::RefCell,
    collections::{BTreeMap, HashMap},
};

const SCOPE_BYTES: usize = 16 * 1024 * 1024;
type GapCache = HashMap<(usize, &'static str, &'static str, bool), Result<i64, String>>;
pub(super) struct Rules<'a> {
    board: &'a Board,
    kinds: BTreeMap<&'a str, Vec<(usize, &'a Fields)>>,
    items: HashMap<usize, usize>,
    scopes: RefCell<HashMap<&'a str, Vec<u8>>>,
    gaps: RefCell<GapCache>,
    cancel: Option<Arc<AtomicBool>>,
}
impl<'a> Rules<'a> {
    pub fn new(board: &'a Board) -> Self {
        let mut kinds: BTreeMap<&str, Vec<_>> = BTreeMap::new();
        for (index, rule) in board.rules.iter().enumerate().filter(|(_, r)| enabled(r)) {
            kinds
                .entry(text(rule, "RULEKIND"))
                .or_default()
                .push((index, rule));
        }
        for rules in kinds.values_mut() {
            rules.sort_by_key(|(index, r)| {
                (
                    text(r, "PRIORITY").parse::<u32>().unwrap_or(u32::MAX),
                    *index,
                )
            });
        }
        Self {
            board,
            kinds,
            items: board
                .items
                .iter()
                .enumerate()
                .map(|(n, i)| (i as *const Item as usize, n))
                .collect(),
            scopes: RefCell::new(HashMap::new()),
            gaps: RefCell::new(HashMap::new()),
            cancel: None,
        }
    }
    pub fn with_cancel(board: &'a Board, cancel: Option<Arc<AtomicBool>>) -> Self {
        let mut rules = Self::new(board);
        rules.cancel = cancel;
        rules
    }
    pub fn active(&self, kind: &str) -> bool {
        self.kinds.contains_key(kind)
    }
    pub fn clearance(
        &self,
        index: usize,
        rule: &Fields,
        a: &Item,
        b: &Item,
    ) -> Result<i64, String> {
        if net_tie_pair(self.board, a, b) {
            return Ok(0);
        }
        let key = (
            index,
            object_kind(a),
            object_kind(b),
            a.component.is_some()
                && a.component == b.component
                && matches!(a.shape, Shape::Pad { .. })
                && matches!(b.shape, Shape::Pad { .. }),
        );
        self.gaps
            .borrow_mut()
            .entry(key)
            .or_insert_with(|| clearance_for_rule(rule, a, b))
            .clone()
    }
    /// A large gap in a rare scope must not expand every object on the board.
    /// Keep the original broad phase if pruning could hide invalid queries/data.
    pub fn clearance_bounds(&self, items: &[&Item]) -> Option<Vec<i64>> {
        if !self.known_scopes("Clearance", items) {
            return None;
        }
        let mut gaps = Vec::new();
        for &(_, r) in self.kinds.get("Clearance").into_iter().flatten() {
            let mut gap = dimension(r, "GAP").ok()?.max(0);
            for entry in text(r, "OBJECTCLEARANCES").split(';') {
                if let Some((_, value)) = entry.split_once(':') {
                    let value = value.parse::<f64>().ok()?;
                    if !value.is_finite() {
                        return None;
                    }
                    gap = gap.max((value * 2.54).round() as i64);
                }
            }
            gaps.push((r, gap));
        }
        Some(
            items
                .iter()
                .map(|item| {
                    gaps.iter()
                        .filter(|(r, _)| {
                            ["SCOPE1EXPRESSION", "SCOPE2EXPRESSION"]
                                .into_iter()
                                .any(|key| {
                                    let expr = if text(r, key).is_empty() {
                                        "All"
                                    } else {
                                        text(r, key)
                                    };
                                    self.matches(item, expr) == Some(true)
                                })
                        })
                        .map(|(_, gap)| *gap)
                        .max()
                        .unwrap_or(0)
                })
                .collect(),
        )
    }
    /// Spatial pruning may skip rule evaluation only if it cannot hide a query
    /// error. Unknown expressions retain the exhaustive diagnostic path.
    pub fn known_scopes(&self, kind: &str, items: &[&Item]) -> bool {
        self.kinds.get(kind).is_none_or(|rules| {
            rules.iter().all(|(_, r)| {
                ["SCOPE1EXPRESSION", "SCOPE2EXPRESSION"]
                    .into_iter()
                    .all(|key| {
                        let expr = if text(r, key).is_empty() {
                            "All"
                        } else {
                            text(r, key)
                        };
                        items.iter().all(|item| self.matches(item, expr).is_some())
                    })
            })
        })
    }
    fn matches(&self, item: &Item, expression: &'a str) -> Option<bool> {
        if self
            .cancel
            .as_ref()
            .is_some_and(|c| c.load(Ordering::Relaxed))
        {
            return None;
        }
        let trimmed = expression.trim();
        if trimmed.eq_ignore_ascii_case("All") || trimmed.eq_ignore_ascii_case("True") {
            return Some(true);
        }
        if trimmed.eq_ignore_ascii_case("False") {
            return Some(false);
        }
        // Synthetic probes and imported duplicate IDs must never alias an item.
        let Some(&index) = self.items.get(&(item as *const Item as usize)) else {
            return scope(self.board, item, expression, 0);
        };
        let mut scopes = self.scopes.borrow_mut();
        if !scopes.contains_key(expression)
            && (scopes.len() + 1).saturating_mul(self.board.items.len()) <= SCOPE_BYTES
        {
            scopes.insert(expression, vec![0; self.board.items.len()]);
        }
        let Some(cells) = scopes.get_mut(expression) else {
            return scope(self.board, item, expression, 0);
        };
        match cells[index] {
            1 => Some(false),
            2 => Some(true),
            3 => None,
            _ => {
                let value = scope(self.board, item, expression, 0);
                cells[index] = match value {
                    Some(false) => 1,
                    Some(true) => 2,
                    None => 3,
                };
                value
            }
        }
    }
    pub fn resolve(
        &self,
        kind: &str,
        a: &Item,
        b: Option<&Item>,
    ) -> Result<Option<(usize, &'a Fields)>, String> {
        let Some(rules) = self.kinds.get(kind) else {
            return Ok(None);
        };
        for &(index, rule) in rules {
            let expression = |key| {
                if text(rule, key).is_empty() {
                    "All"
                } else {
                    text(rule, key)
                }
            };
            let first = expression("SCOPE1EXPRESSION");
            let second = expression("SCOPE2EXPRESSION");
            let evaluate = |item: &Item, expr: &'a str, key: &str| {
                self.matches(item, expr).ok_or_else(|| {
                    format!(
                        "{} ({}), {key}: Unsupported query: {expr}",
                        text(rule, "NAME"),
                        kind
                    )
                })
            };
            let matched = if let Some(b) = b {
                if !net_scope_matches(self.board, text(rule, "NETSCOPE"), a, b, false) {
                    continue;
                }
                (evaluate(a, first, "SCOPE1EXPRESSION")?
                    && evaluate(b, second, "SCOPE2EXPRESSION")?)
                    || (evaluate(b, first, "SCOPE1EXPRESSION")?
                        && evaluate(a, second, "SCOPE2EXPRESSION")?)
            } else {
                evaluate(a, first, "SCOPE1EXPRESSION")?
            };
            if matched {
                return Ok(Some((index, rule)));
            }
        }
        Ok(None)
    }
}
