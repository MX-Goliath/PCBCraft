//! Native offsets checked against AltiumSharp and native AD 24.4.1 copy saves.
//! Only understood fields are patched; modern independent-mask shapes are guarded.
use super::*;
fn shape(n: u8) -> Result<PadShape> {
    match n {
        1 => Ok(PadShape::Round),
        2 => Ok(PadShape::Rectangle),
        3 => Ok(PadShape::Octagonal),
        9 => Ok(PadShape::RoundedRectangle),
        _ => Err(format!("Unsupported padstack shape {n}")),
    }
}
fn shape_id(s: PadShape) -> u8 {
    match s {
        PadShape::Round => 1,
        PadShape::Rectangle => 2,
        PadShape::Octagonal => 3,
        PadShape::RoundedRectangle => 9,
    }
}
fn mode(n: u8) -> StackMode {
    match n {
        1 => StackMode::TopMiddleBottom,
        2 => StackMode::Full,
        _ => StackMode::Simple,
    }
}
fn read_masks(
    item: &mut Item,
    p: &[u8],
    paste_at: usize,
    solder_at: usize,
    paste_mode: usize,
    solder_mode: usize,
) -> Result<()> {
    let mut m = MaskSettings::for_item(item);
    for (kind, at, mode_at) in [
        (MaskKind::Paste, paste_at, paste_mode),
        (MaskKind::Solder, solder_at, solder_mode),
    ] {
        if p.len() < at + 4 {
            continue;
        }
        for bottom in [false, true] {
            let s = m.side_mut(kind, bottom);
            let n = p.get(mode_at).copied().unwrap_or(1);
            s.mode = if n == 2 {
                MaskMode::Manual
            } else {
                MaskMode::Rule
            };
            s.expansion = coord(p, at)?;
            s.enabled &= n != 0;
            if kind == MaskKind::Solder {
                s.tented = p[1] & (if bottom { 64 } else { 32 }) != 0;
            }
        }
    }
    item.masks = Some(Box::new(m));
    Ok(())
}
pub(super) fn read_pad(item: &mut Item, p: &[u8], e: &[u8], board: &Board) -> Result<()> {
    let mut stack = PadStack {
        mode: mode(p[62]),
        ..Default::default()
    };
    let layers: Vec<_> = match stack.mode {
        StackMode::Simple => vec![TOP],
        StackMode::TopMiddleBottom => vec![TOP, 2, BOTTOM],
        StackMode::Full => {
            // Board6 is read before primitives; finish() builds its layer list later.
            let mut active: BTreeSet<u16> = board
                .layers
                .iter()
                .filter(|l| l.copper && l.id <= 32)
                .map(|l| l.id)
                .collect();
            let mut layer = TOP;
            let mut seen = BTreeSet::new();
            while (1..=32).contains(&layer) && seen.insert(layer) {
                active.insert(layer);
                layer = board
                    .properties
                    .get(&format!("LAYER{layer}NEXT"))
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0);
            }
            active.into_iter().collect()
        }
    };
    for layer in layers {
        let base = if layer == TOP {
            21
        } else if layer == BOTTOM {
            37
        } else {
            29
        };
        let mut g = PadGeometry {
            size: point(p, base)?,
            shape: shape(
                p[if layer == TOP {
                    49
                } else if layer == BOTTOM {
                    51
                } else {
                    50
                }],
            )?,
            corner_percent: 25,
            offset: Point::default(),
        };
        if e.len() >= 596 {
            if stack.mode == StackMode::Full && (2..=30).contains(&layer) {
                let n = (layer - 2) as usize;
                g.size = Point {
                    x: coord(e, n * 4)?,
                    y: coord(e, 116 + n * 4)?,
                };
                g.shape = shape(e[232 + n])?;
            }
            let n = (layer - 1) as usize;
            if e[531] != 0 {
                g.shape = shape(e[532 + n])?;
            }
            g.corner_percent = e[564 + n].min(100);
            g.offset = Point {
                x: coord(e, 275 + n * 4)?,
                y: coord(e, 403 + n * 4)?,
            };
        }
        stack.layers.insert(layer, g);
    }
    if e.len() >= 596 {
        stack.drill_shape = match e[262] {
            1 => DrillShape::Square,
            2 => DrillShape::Slot,
            _ => DrillShape::Round,
        };
        stack.slot_length = coord(e, 263)?;
        stack.drill_rotation = f64_at(e, 267)?;
        if stack.mode == StackMode::Full && e.len() >= 636 {
            let count = u32_at(e, 628)? as usize;
            let stride = u32_at(e, 632)? as usize;
            if count > 4096
                || !(15..=4096).contains(&stride)
                || 636usize
                    .checked_add(count.saturating_mul(stride))
                    .is_none_or(|n| n > e.len())
            {
                return Err("Malformed full padstack table".into());
            }
            for n in 0..count {
                let at = 636 + n * stride;
                if let Some(layer) = native_layer(u32_at(e, at)?)
                    && let Some(g) = stack.layers.get_mut(&layer)
                {
                    g.size = point(e, at + 5)?;
                    g.shape = shape(e[at + 4])?;
                    g.corner_percent = e[at + 13].min(100);
                }
            }
        }
    }
    item.pad_stack = Some(Box::new(stack));
    read_masks(item, p, 86, 90, 101, 102)
}
pub(super) fn read_via(item: &mut Item, p: &[u8]) -> Result<()> {
    let mut stack = PadStack {
        mode: mode(p.get(74).copied().unwrap_or(0)),
        ..Default::default()
    };
    for layer in 1..=32 {
        let mut g = item.pad_geometry(layer).unwrap();
        if stack.mode != StackMode::Simple && p.len() >= 203 {
            let d = coord(p, 75 + (layer as usize - 1) * 4)?;
            if d > 0 {
                g.size = Point { x: d, y: d };
            }
        }
        if stack.mode == StackMode::Full || layer == TOP || layer == BOTTOM || layer == 2 {
            stack.layers.insert(layer, g);
        }
    }
    item.pad_stack = Some(Box::new(stack));
    read_masks(item, p, 50, 54, 65, 66)?;
    if let Some(m) = &mut item.masks {
        for side in &mut m.solder {
            side.from_hole_edge = p.get(258).is_some_and(|v| *v != 0);
        }
    }
    Ok(())
}
fn dim(b: &mut [u8], at: usize, nm: i64) -> Result<()> {
    let raw = (nm as f64 / 2.54).round();
    if raw < i32::MIN as f64 || raw > i32::MAX as f64 {
        return Err("Padstack dimension outside native range".into());
    }
    b.get_mut(at..at + 4)
        .ok_or("Short padstack field")?
        .copy_from_slice(&(raw as i32).to_le_bytes());
    Ok(())
}
fn real(b: &mut [u8], at: usize, value: f64) -> Result<()> {
    b.get_mut(at..at + 8)
        .ok_or("Short stack angle")?
        .copy_from_slice(&value.to_le_bytes());
    Ok(())
}
pub(super) fn patch(item: &Item, old: Option<&Item>, blocks: &mut [Vec<u8>]) -> Result<()> {
    let is_pad = matches!(item.shape, Shape::Pad { .. });
    let is_via = matches!(item.shape, Shape::Via { .. });
    if !is_pad && !is_via {
        if item.masks.is_some() && old.is_none_or(|o| o.masks != item.masks) {
            return Err("Native mask overrides for this primitive are not supported yet".into());
        }
        return Ok(());
    }
    if old.is_none_or(|o| o.pad_stack != item.pad_stack)
        && let Some(stack) = &item.pad_stack
    {
        let base = if is_pad { 4 } else { 0 };
        let required = if is_pad { 114 } else { 209 };
        if blocks[base].len() < required {
            blocks[base].resize(required, 0);
        }
        blocks[base][if is_pad { 62 } else { 74 }] = match stack.mode {
            StackMode::Simple => 0,
            StackMode::TopMiddleBottom => 1,
            StackMode::Full => 2,
        };
        if is_pad {
            if blocks[5].len() < 596 {
                blocks[5].resize(596, 0);
            }
            for layer in 1..=32 {
                let g = item.pad_geometry(layer).unwrap();
                let n = (layer - 1) as usize;
                if layer == TOP || layer == BOTTOM || layer == 2 {
                    let at = if layer == TOP {
                        21
                    } else if layer == BOTTOM {
                        37
                    } else {
                        29
                    };
                    dim(&mut blocks[4], at, g.size.x)?;
                    dim(&mut blocks[4], at + 4, g.size.y)?;
                    blocks[4][if layer == TOP {
                        49
                    } else if layer == BOTTOM {
                        51
                    } else {
                        50
                    }] = if g.shape == PadShape::RoundedRectangle {
                        2
                    } else {
                        shape_id(g.shape)
                    };
                }
                if (2..=30).contains(&layer) {
                    let j = (layer - 2) as usize;
                    dim(&mut blocks[5], j * 4, g.size.x)?;
                    dim(&mut blocks[5], 116 + j * 4, g.size.y)?;
                    blocks[5][232 + j] = if g.shape == PadShape::RoundedRectangle {
                        2
                    } else {
                        shape_id(g.shape)
                    };
                }
                dim(&mut blocks[5], 275 + n * 4, g.offset.x)?;
                dim(&mut blocks[5], 403 + n * 4, g.offset.y)?;
                blocks[5][532 + n] = shape_id(g.shape);
                blocks[5][564 + n] = g.corner_percent;
            }
            blocks[5][531] = 1;
            blocks[5][262] = match stack.drill_shape {
                DrillShape::Round => 0,
                DrillShape::Square => 1,
                DrillShape::Slot => 2,
            };
            dim(&mut blocks[5], 263, stack.slot_length)?;
            real(&mut blocks[5], 267, stack.drill_rotation)?;
            if stack.mode == StackMode::Full {
                let e = &mut blocks[5];
                if e.len() < 636 {
                    e.resize(636, 0);
                }
                let count = u32_at(e, 628)? as usize;
                let stride = u32_at(e, 632)? as usize;
                if count > 4096
                    || (count > 0 && (stride < 15 || 636 + count.saturating_mul(stride) > e.len()))
                {
                    return Err("Cannot patch malformed native full-stack entries".into());
                }
                let mut entries = Vec::new();
                let mut seen = BTreeSet::new();
                for n in 0..count {
                    let mut row = e[636 + n * stride..636 + (n + 1) * stride].to_vec();
                    if let Some(layer) = native_layer(u32_at(&row, 0)?)
                        && let Some(g) = stack.layers.get(&layer)
                    {
                        dim(&mut row, 5, g.size.x)?;
                        dim(&mut row, 9, g.size.y)?;
                        row[4] = shape_id(g.shape);
                        row[13] = g.corner_percent;
                        seen.insert(layer);
                    }
                    entries.push(row);
                }
                let stride = if count == 0 { 15 } else { stride };
                for (layer, g) in &stack.layers {
                    if !seen.contains(layer) {
                        let mut row = vec![0; stride];
                        let id = if *layer == BOTTOM {
                            0x0100ffff
                        } else {
                            0x01000000 | u32::from(*layer)
                        };
                        row[0..4].copy_from_slice(&id.to_le_bytes());
                        row[4] = shape_id(g.shape);
                        dim(&mut row, 5, g.size.x)?;
                        dim(&mut row, 9, g.size.y)?;
                        row[13] = g.corner_percent;
                        entries.push(row);
                    }
                }
                let tail = e[636 + count * if count == 0 { 0 } else { stride }..].to_vec();
                e.truncate(636);
                e[628..632].copy_from_slice(&(entries.len() as u32).to_le_bytes());
                e[632..636].copy_from_slice(&(stride as u32).to_le_bytes());
                for row in entries {
                    e.extend(row);
                }
                e.extend(tail);
            }
        } else {
            for layer in 1..=32 {
                dim(
                    &mut blocks[0],
                    75 + (layer - 1) as usize * 4,
                    item.pad_geometry(layer).unwrap().size.x,
                )?;
            }
        }
    }
    if old.is_none_or(|o| o.masks != item.masks)
        && let Some(m) = &item.masks
    {
        let p = &mut blocks[if is_pad { 4 } else { 0 }];
        for (kind, at, mode_at) in if is_pad {
            [(MaskKind::Paste, 86, 101), (MaskKind::Solder, 90, 102)]
        } else {
            [(MaskKind::Paste, 50, 65), (MaskKind::Solder, 54, 66)]
        } {
            let a = m.side(kind, false);
            let z = m.side(kind, true);
            if a.from_hole_edge != z.from_hole_edge || (is_pad && a.from_hole_edge) {
                return Err("Native pad hole-edge or independent side masks are not supported; keep this edit in JSON".into());
            }
            if is_via && kind == MaskKind::Solder && a.from_hole_edge {
                if p.len() < 259 {
                    p.resize(259, 0);
                }
                p[258] = 1;
            } else if is_via && kind == MaskKind::Solder && p.len() > 258 {
                p[258] = 0;
            }
            if a.percentage != 0.0
                || z.percentage != 0.0
                || a.mode != z.mode
                || a.expansion != z.expansion
                || (item.layer == MULTI_LAYER && kind == MaskKind::Paste && a.enabled != z.enabled)
            {
                return Err("Independent or percentage mask apertures need modern native mask records; keep this edit in PCBCraft JSON".into());
            }
            if p.len() <= mode_at {
                p.resize(mode_at + 1, 0);
            }
            dim(p, at, a.expansion)?;
            p[mode_at] = if !a.enabled && !z.enabled {
                0
            } else if a.mode == MaskMode::Manual {
                2
            } else {
                1
            };
        }
        p[1] = (p[1] & !96)
            | if m.solder[0].tented { 32 } else { 0 }
            | if m.solder[1].tented { 64 } else { 0 };
    }
    Ok(())
}
