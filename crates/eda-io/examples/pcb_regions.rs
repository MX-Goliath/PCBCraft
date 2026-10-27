//! Read-only native region framing/coordinate diagnostic; does not import STEP.
use std::{io::Read, path::Path};
fn u32_at(b: &[u8], n: usize) -> Result<u32, Box<dyn std::error::Error>> {
    Ok(u32::from_le_bytes(
        b.get(n..n + 4).ok_or("Truncated u32")?.try_into()?,
    ))
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args_os().nth(1).ok_or("Pass a PcbDoc path")?;
    let bytes = eda_io::altium::read_bounded(Path::new(&path))?;
    let mut file = cfb::CompoundFile::open(std::io::Cursor::new(bytes))?;
    for (family, extended) in [
        ("Regions6", false),
        ("ShapeBasedRegions6", true),
        ("ComponentBodies6", false),
        ("ShapeBasedComponentBodies6", true),
    ] {
        let path = format!("/{family}/Data");
        if !file.is_stream(&path) {
            continue;
        }
        let mut data = vec![];
        file.open_stream(path)?.read_to_end(&mut data)?;
        let mut offset = 0;
        let mut index = 0;
        let mut small = 0;
        while offset < data.len() {
            let length = (u32_at(&data, offset + 1)? & 0xffffff) as usize;
            let payload = data
                .get(offset + 5..offset + 5 + length)
                .ok_or("Truncated region payload")?;
            let properties = (u32_at(payload, 18)? & 0xffffff) as usize;
            let begin = 22 + properties;
            let count = u32_at(payload, begin)? as usize + usize::from(extended);
            let stride = if extended { 37 } else { 16 };
            if count > 500_000 || count > payload.len().saturating_sub(begin + 4) / stride {
                return Err("Region vertex count exceeds payload limits".into());
            }
            {
                let mut points = vec![];
                let mut arcs = false;
                for n in 0..count {
                    let at = begin + 4 + n * stride;
                    if extended {
                        arcs |= payload[at] != 0;
                        let x = i32::from_le_bytes(
                            payload
                                .get(at + 1..at + 5)
                                .ok_or("Truncated x")?
                                .try_into()?,
                        ) as f64;
                        let y = i32::from_le_bytes(
                            payload
                                .get(at + 5..at + 9)
                                .ok_or("Truncated y")?
                                .try_into()?,
                        ) as f64;
                        points.push((x, y));
                    } else {
                        let x = f64::from_le_bytes(
                            payload.get(at..at + 8).ok_or("Truncated x")?.try_into()?,
                        );
                        let y = f64::from_le_bytes(
                            payload
                                .get(at + 8..at + 16)
                                .ok_or("Truncated y")?
                                .try_into()?,
                        );
                        points.push((x, y));
                    }
                }
                let mut normalized: Vec<_> = points
                    .iter()
                    .map(|(x, y)| ((x * 2.54).round() as i64, (y * 2.54).round() as i64))
                    .collect();
                normalized.dedup();
                if normalized.len() > 1 && normalized.first() == normalized.last() {
                    normalized.pop();
                }
                if !arcs && normalized.len() < 3 {
                    small += 1;
                    if small <= 12 {
                        println!(
                            "{family}/{index} raw_count={} first_raw={:?} first_nm={:?} header={:02x?} props={}",
                            points.len(),
                            &points[..points.len().min(16)],
                            &normalized[..normalized.len().min(16)],
                            &payload[..18],
                            String::from_utf8_lossy(&payload[22..begin])
                        );
                    }
                }
            }
            offset += 5 + length;
            index += 1;
        }
        println!("{family}: {index} records, {small} short contours");
    }
    Ok(())
}
