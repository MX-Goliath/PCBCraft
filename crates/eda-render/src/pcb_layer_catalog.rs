//! Display-only layer catalog, including configured but empty mechanical layers.
use eda_core::pcb::*;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug)]
pub(super) struct Layer {
    pub id: u16,
    pub name: String,
    pub role: String,
    pub copper: bool,
    pub order: u16,
    pub used: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn configured_empty_mechanical_layers_roles_and_pairs_follow_native_order() {
        let mut board = Board::default();
        for (n, role) in [
            (2, "AssemblyTop"),
            (3, "AssemblyBottom"),
            (12, "3DBodyTop"),
            (20, "3DBodyBottom"),
            (5, ""),
            (6, ""),
            (7, ""),
            (10, ""),
            (15, ""),
            (16, ""),
            (35, ""),
        ] {
            for (key, value) in [
                ("LAYERID", (0x0102_0000u32 + n).to_string()),
                ("NAME", format!("Mechanical {n}")),
                ("MECHENABLED", "TRUE".into()),
                ("MECHKIND", role.into()),
            ] {
                board
                    .properties
                    .insert(format!("V9_CACHE_LAYER{n}_{key}"), value);
            }
        }
        for (n, a, b) in [(0, 5, 6), (1, 15, 16), (2, 2, 3)] {
            board
                .properties
                .insert(format!("MECHPAIR{n}L1"), format!("MECHANICAL{a}"));
            board
                .properties
                .insert(format!("MECHPAIR{n}L2"), format!("MECHANICAL{b}\r"));
        }
        let before = board.clone();
        let catalog = Catalog::from_board(&board);
        let names: Vec<_> = catalog.pairs.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "3D Body",
                "Assembly",
                "Mechanical 5 / 6",
                "Mechanical 15 / 16",
                "Overlay",
                "Paste",
                "Solder"
            ]
        );
        assert_eq!(
            catalog
                .group(2)
                .iter()
                .map(|l| l.name.as_str())
                .collect::<Vec<_>>(),
            ["Mechanical 7", "Mechanical 10", "Mechanical 35"]
        );
        assert!(!catalog.layer(mechanical_layer(35).unwrap()).unwrap().used);
        assert_eq!(board, before);
    }
}
#[derive(Clone, Debug)]
pub(super) struct Pair {
    pub top: u16,
    pub bottom: u16,
    pub name: String,
}
#[derive(Clone, Debug, Default)]
pub(super) struct Catalog {
    pub layers: Vec<Layer>,
    pub pairs: Vec<Pair>,
}
pub(super) fn mechanical(id: u16) -> Option<u16> {
    match id {
        57..=72 => Some(id - 56),
        83..=1090 => Some(id - 66),
        _ => None,
    }
}
impl Catalog {
    pub fn from_board(board: &Board) -> Self {
        let used: BTreeSet<_> = board.items.iter().map(|i| i.layer).collect();
        let mut layers: BTreeMap<u16, Layer> = board
            .layers
            .iter()
            .map(|l| {
                (
                    l.id,
                    Layer {
                        id: l.id,
                        name: l.name.clone(),
                        role: String::new(),
                        copper: l.copper,
                        order: l.order,
                        used: used.contains(&l.id),
                    },
                )
            })
            .collect();
        for (key, value) in &board.properties {
            let Some(prefix) = key
                .strip_suffix("_LAYERID")
                .filter(|k| k.starts_with("V9_CACHE_LAYER"))
            else {
                continue;
            };
            let Some(id) = value.trim().parse().ok().and_then(native_layer) else {
                continue;
            };
            let get = |suffix: &str| {
                board
                    .properties
                    .get(&format!("{prefix}_{suffix}"))
                    .map(|v| v.trim())
                    .unwrap_or("")
            };
            if !(layers.contains_key(&id)
                || mechanical(id).is_some() && get("MECHENABLED") == "TRUE")
            {
                continue;
            }
            let model_name = layers.contains_key(&id);
            let layer = layers.entry(id).or_insert_with(|| Layer {
                id,
                name: layer_name(id),
                role: String::new(),
                copper: copper_layer(id),
                order: id,
                used: false,
            });
            if !model_name && !get("NAME").is_empty() {
                layer.name = get("NAME").into();
            }
            layer.role = get("MECHKIND").into();
            layer.used |= get("USEDBYPRIMS") == "TRUE";
        }
        // The standard pairs exist independently of whether a side has primitives.
        for id in [
            TOP_OVERLAY,
            BOTTOM_OVERLAY,
            35,
            36,
            37,
            38,
            55,
            KEEP_OUT,
            73,
            MULTI_LAYER,
        ] {
            layers.entry(id).or_insert_with(|| Layer {
                id,
                name: layer_name(id),
                role: String::new(),
                copper: false,
                order: id,
                used: used.contains(&id),
            });
        }
        let mut pairs = vec![];
        let mut paired = BTreeSet::new();
        for layer in layers.values() {
            let Some(kind) = layer.role.strip_suffix("Top") else {
                continue;
            };
            if let Some(bottom) = layers.values().find(|l| l.role == format!("{kind}Bottom")) {
                let name = match kind {
                    "3DBody" => "3D Body",
                    "ComponentCenter" => "Component Center",
                    "ComponentOutline" => "Component Outline",
                    other => other,
                };
                pairs.push(Pair {
                    top: layer.id,
                    bottom: bottom.id,
                    name: name.into(),
                });
                paired.extend([layer.id, bottom.id]);
            }
        }
        let raw_layer = |v: &str| {
            v.trim()
                .strip_prefix("MECHANICAL")
                .and_then(|n| n.parse().ok())
                .and_then(mechanical_layer)
        };
        for (key, value) in &board.properties {
            let Some(prefix) = key.strip_suffix("L1").filter(|k| k.starts_with("MECHPAIR")) else {
                continue;
            };
            let Some((top, bottom)) = raw_layer(value).zip(
                board
                    .properties
                    .get(&format!("{prefix}L2"))
                    .and_then(|v| raw_layer(v)),
            ) else {
                continue;
            };
            if paired.contains(&top)
                || paired.contains(&bottom)
                || !layers.contains_key(&top)
                || !layers.contains_key(&bottom)
            {
                continue;
            }
            pairs.push(Pair {
                top,
                bottom,
                name: format!(
                    "Mechanical {} / {}",
                    mechanical(top).unwrap(),
                    mechanical(bottom).unwrap()
                ),
            });
            paired.extend([top, bottom]);
        }
        pairs.extend([
            Pair {
                top: TOP_OVERLAY,
                bottom: BOTTOM_OVERLAY,
                name: "Overlay".into(),
            },
            Pair {
                top: 35,
                bottom: 36,
                name: "Paste".into(),
            },
            Pair {
                top: 37,
                bottom: 38,
                name: "Solder".into(),
            },
        ]);
        pairs.sort_by_key(|p| {
            let rank = match p.name.as_str() {
                "3D Body" => 0,
                "Assembly" => 1,
                "Component Center" => 2,
                "Component Outline" => 3,
                "Courtyard" => 4,
                "Designator" => 5,
                "Overlay" => 7,
                "Paste" => 8,
                "Solder" => 9,
                _ => 6,
            };
            (rank, mechanical(p.top).unwrap_or(p.top))
        });
        let mut layers: Vec<_> = layers.into_values().collect();
        layers.sort_by_key(|l| (l.order, l.id));
        Self { layers, pairs }
    }
    pub fn layer(&self, id: u16) -> Option<&Layer> {
        self.layers.iter().find(|l| l.id == id)
    }
    pub fn group(&self, group: usize) -> Vec<&Layer> {
        let paired: BTreeSet<_> = self.pairs.iter().flat_map(|p| [p.top, p.bottom]).collect();
        let mut layers: Vec<_> = self
            .layers
            .iter()
            .filter(|l| match group {
                0 => l.copper,
                2 => mechanical(l.id).is_some() && !paired.contains(&l.id),
                3 => !l.copper && mechanical(l.id).is_none() && !paired.contains(&l.id),
                _ => false,
            })
            .collect();
        if group == 2 {
            layers.sort_by(|a, b| {
                let number = |l: &Layer| {
                    l.name
                        .strip_prefix("Mechanical ")
                        .and_then(|s| s.parse::<u16>().ok())
                };
                match (number(a), number(b)) {
                    (Some(a), Some(b)) => a.cmp(&b),
                    (Some(_), None) => std::cmp::Ordering::Less,
                    (None, Some(_)) => std::cmp::Ordering::Greater,
                    _ => a.name.cmp(&b.name),
                }
            });
        } else if group == 3 {
            layers.sort_by_key(|l| match l.id {
                73 => 0,
                55 => 1,
                KEEP_OUT => 2,
                MULTI_LAYER => 3,
                _ => 4,
            });
        }
        layers
    }
}
