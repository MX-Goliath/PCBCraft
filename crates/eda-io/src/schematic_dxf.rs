//! ASCII DXF interchange. Unsupported entities fail before changing the document.
use eda_core::{
    Document,
    schematic::SchPrimitiveKind as K,
    schematic_edit::{self as edit, Fragment},
};
use eda_geom::Point;
use std::{fmt::Write, path::Path};

type Entity = Vec<(i32, String)>;
fn get(e: &Entity, n: i32) -> Option<&str> {
    e.iter().find(|(c, _)| *c == n).map(|(_, v)| v.as_str())
}
fn number(e: &Entity, n: i32, default: f64) -> Result<f64, String> {
    let v = get(e, n).map_or(Ok(default), |v| {
        v.parse::<f64>()
            .map_err(|_| format!("Invalid DXF group {n}"))
    })?;
    if !v.is_finite() || v.abs() > 1e9 {
        return Err("DXF coordinate is out of range".into());
    }
    Ok(v)
}
fn point(e: &Entity, x: i32, y: i32, scale: f64) -> Result<Point, String> {
    let p = Point {
        x: (number(e, x, 0.)? * scale * 1e6).round() as i64,
        y: (number(e, y, 0.)? * scale * 1e6).round() as i64,
    };
    if !p.valid() {
        return Err("DXF coordinate is outside the document range".into());
    }
    Ok(p)
}
pub fn import(path: &Path) -> Result<Fragment, String> {
    let bytes = crate::altium::read_bounded(path)?;
    if bytes.starts_with(b"AutoCAD Binary DXF") || bytes.starts_with(b"AC10") {
        return Err(
            "Only ASCII DXF is supported; convert binary DXF/DWG to ASCII DXF first".into(),
        );
    }
    let text = std::str::from_utf8(&bytes)
        .map(std::borrow::Cow::Borrowed)
        .unwrap_or_else(|_| encoding_rs::WINDOWS_1251.decode(&bytes).0);
    let lines: Vec<_> = text.lines().collect();
    if lines.len() % 2 != 0 {
        return Err("Truncated DXF group pair".into());
    }
    let pairs = lines
        .chunks_exact(2)
        .map(|p| {
            Ok((
                p[0].trim()
                    .parse::<i32>()
                    .map_err(|_| "Invalid DXF group code")?,
                p[1].trim().to_owned(),
            ))
        })
        .collect::<Result<Vec<_>, String>>()?;
    let unit = pairs
        .windows(2)
        .find(|w| w[0].0 == 9 && w[0].1 == "$INSUNITS")
        .and_then(|w| w[1].1.parse::<i32>().ok())
        .unwrap_or(4);
    let scale = match unit {
        0 | 4 => 1.,
        1 => 25.4,
        2 => 304.8,
        5 => 10.,
        6 => 1000.,
        _ => return Err(format!("DXF INSUNITS {unit} is not supported")),
    };
    let mut entities = Vec::<Entity>::new();
    let mut active = false;
    for (n, (code, value)) in pairs.iter().enumerate() {
        if *code == 2 && value == "ENTITIES" && n > 0 && pairs[n - 1] == (0, "SECTION".into()) {
            active = true;
            continue;
        }
        if *code == 0 && value == "ENDSEC" {
            active = false;
        }
        if !active {
            continue;
        }
        if *code == 0 {
            entities.push(vec![(*code, value.clone())]);
        } else if let Some(e) = entities.last_mut() {
            e.push((*code, value.clone()));
        }
    }
    let mut fragment = Fragment {
        fonts: eda_core::schematic::Schematic::blank().fonts,
        ..Default::default()
    };
    for e in entities {
        for code in [30, 31, 32, 33, 38, 39] {
            if number(&e, code, 0.)? != 0. {
                return Err("Only planar 2D DXF entities are supported".into());
            }
        }
        if number(&e, 210, 0.)? != 0. || number(&e, 220, 0.)? != 0. || number(&e, 230, 1.)? != 1. {
            return Err("Non-planar DXF extrusion is unsupported".into());
        }
        let kind = get(&e, 0).unwrap_or_default();
        if matches!(kind, "VERTEX" | "SEQEND") {
            return Err("Legacy POLYLINE requires conversion to LWPOLYLINE".into());
        }
        let primitive = match kind {
            "LINE" => K::Polyline {
                points: vec![point(&e, 10, 20, scale)?, point(&e, 11, 21, scale)?],
                closed: false,
                electrical: String::new(),
            },
            "LWPOLYLINE" => {
                if e.iter()
                    .any(|(c, v)| *c == 42 && v.parse::<f64>().unwrap_or(1.) != 0.)
                {
                    return Err("DXF bulged polylines are not supported; explode arcs first".into());
                }
                let mut points = Vec::new();
                let mut x = None;
                for (c, v) in &e {
                    if *c == 10 {
                        x = Some(v.clone());
                    }
                    if *c == 20 {
                        let px = x.take().ok_or("DXF vertex is missing X")?;
                        points.push(point(&vec![(10, px), (20, v.clone())], 10, 20, scale)?);
                    }
                }
                if points.len() < 2 {
                    return Err("DXF polyline needs two vertices".into());
                }
                K::Polyline {
                    points,
                    closed: (number(&e, 70, 0.)? as i32 & 1) != 0,
                    electrical: String::new(),
                }
            }
            "CIRCLE" | "ARC" => {
                let radius = (number(&e, 40, 0.)? * scale * 1e6).round() as i64;
                if radius <= 0 {
                    return Err("DXF arc radius must be positive".into());
                }
                K::Arc {
                    centre: point(&e, 10, 20, scale)?,
                    radii: Point {
                        x: radius,
                        y: radius,
                    },
                    start: if kind == "ARC" {
                        number(&e, 50, 0.)?
                    } else {
                        0.
                    },
                    end: if kind == "ARC" {
                        number(&e, 51, 360.)?
                    } else {
                        360.
                    },
                }
            }
            "TEXT" => {
                if number(&e, 72, 0.)? != 0.
                    || number(&e, 73, 0.)? != 0.
                    || number(&e, 51, 0.)? != 0.
                    || number(&e, 41, 1.)? != 1.
                    || number(&e, 71, 0.)? != 0.
                {
                    return Err("DXF aligned, oblique, stretched or mirrored text requires conversion before import".into());
                }
                let height = (number(&e, 40, 2.54)? * scale * 1e6).round() as i64;
                if !(1..=254_000_000).contains(&height) {
                    return Err("DXF text height is out of range".into());
                }
                let angle = number(&e, 50, 0.)?;
                if (angle / 90. - (angle / 90.).round()).abs() > 1e-6 {
                    return Err("DXF TEXT rotation must be a multiple of 90 degrees".into());
                }
                let mut font = fragment.fonts[0].clone();
                font.height_nm = height;
                let index = fragment
                    .fonts
                    .iter()
                    .position(|f| f == &font)
                    .unwrap_or_else(|| {
                        let index = fragment.fonts.len();
                        fragment.fonts.push(font);
                        index
                    });
                K::Text {
                    position: point(&e, 10, 20, scale)?,
                    text: get(&e, 1).unwrap_or_default().into(),
                    font: index,
                    orientation: (angle / 90.).round().rem_euclid(4.) as u32,
                    justification: 0,
                }
            }
            "POINT" => K::Junction {
                position: point(&e, 10, 20, scale)?,
                radius_nm: 100_000,
            },
            "SOLID" => {
                let mut points = vec![point(&e, 10, 20, scale)?, point(&e, 11, 21, scale)?];
                let c = point(&e, 12, 22, scale)?;
                let d = if get(&e, 13).is_some() {
                    point(&e, 13, 23, scale)?
                } else {
                    c
                };
                // DXF stores quadrilateral corners in 0,1,3,2 order.
                if d != c {
                    points.push(d);
                }
                points.push(c);
                K::Polyline {
                    points,
                    closed: true,
                    electrical: String::new(),
                }
            }
            _ => {
                return Err(format!(
                    "Unsupported DXF entity {kind}; import cancelled without changes"
                ));
            }
        };
        let mut p = edit::primitive(primitive, None);
        p.color = 0;
        if let Some(value) = get(&e, 420) {
            let rgb = value.parse::<u32>().map_err(|_| "Invalid DXF true color")?;
            if rgb > 0xffffff {
                return Err("Invalid DXF true color".into());
            }
            p.color = ((rgb & 255) << 16) | (rgb & 0xff00) | ((rgb >> 16) & 255);
        }
        if kind == "SOLID" {
            p.fill = Some(p.color);
        }
        p.width_nm = if kind == "SOLID" { 0 } else { 100_000 };
        fragment.primitives.push(p);
        if fragment.primitives.len() > 250_000 || fragment.fonts.len() > 128 {
            return Err("DXF object/font limit exceeded".into());
        }
    }
    if fragment.primitives.is_empty() {
        return Err("DXF has no supported entities".into());
    }
    Ok(fragment)
}
/// Complete visible linework, symbols and outlined fonts from the shared renderer.
/// The writer accepts document coordinates only and never accesses a GPU or GUI.
pub fn export_drawing(faces: &[eda_geom::DrawingFace]) -> Result<String, String> {
    if faces.len() > 250_000 {
        return Err("DXF drawing exceeds 250,000 faces".into());
    }
    let mut out = String::from(
        "0\nSECTION\n2\nHEADER\n9\n$ACADVER\n1\nAC1018\n9\n$INSUNITS\n70\n4\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n",
    );
    for face in faces {
        if face.points.iter().any(|p| !p.valid()) {
            return Err("Invalid drawing face coordinates".into());
        }
        pair(&mut out, 0, "SOLID");
        pair(&mut out, 8, "Schematic");
        pair(
            &mut out,
            420,
            (face.rgb[0] as u32) << 16 | (face.rgb[1] as u32) << 8 | face.rgb[2] as u32,
        );
        for (i, p) in [
            face.points[0],
            face.points[1],
            face.points[2],
            face.points[2],
        ]
        .into_iter()
        .enumerate()
        {
            xy(&mut out, 10 + i as i32, 20 + i as i32, p);
        }
    }
    out.push_str("0\nENDSEC\n0\nEOF\n");
    Ok(out)
}
fn pair(out: &mut String, code: i32, value: impl std::fmt::Display) {
    writeln!(out, "{code}\n{value}").unwrap();
}
fn xy(out: &mut String, x: i32, y: i32, p: Point) {
    pair(out, x, format_args!("{:.6}", p.x as f64 / 1e6));
    pair(out, y, format_args!("{:.6}", p.y as f64 / 1e6));
}
fn line(out: &mut String, a: Point, b: Point) {
    pair(out, 0, "LINE");
    pair(out, 8, "Schematic");
    xy(out, 10, 20, a);
    xy(out, 11, 21, b);
}
pub fn export(doc: &Document) -> Result<String, String> {
    doc.validate()?;
    let s = doc.schematic.as_ref().ok_or("Expected schematic")?;
    let mut out = String::from(
        "0\nSECTION\n2\nHEADER\n9\n$ACADVER\n1\nAC1015\n9\n$INSUNITS\n70\n4\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n",
    );
    for p in s
        .primitives
        .iter()
        .chain(&s.parameter_objects)
        .filter(|p| !p.presentation.hidden)
    {
        if p.fill.is_some() {
            return Err(
                "Filled primitives are outside the verified DXF export subset; use Smart PDF"
                    .into(),
            );
        }
        match &p.kind {
            K::Polyline{points,closed,..}=>{for e in points.windows(2){line(&mut out,e[0],e[1]);}if *closed && points.len()>1{line(&mut out,*points.last().unwrap(),points[0]);}}
            K::Rectangle{a,b,radius} if radius.x==0 && radius.y==0=>{let c=Point{x:b.x,y:a.y};let d=Point{x:a.x,y:b.y};for (a,b) in [(*a,c),(c,*b),(*b,d),(d,*a)]{line(&mut out,a,b);}}
            K::Arc{centre,radii,start,end} if radii.x==radii.y=>{pair(&mut out,0,if (end-start).abs()>=359.999{"CIRCLE"}else{"ARC"});pair(&mut out,8,"Schematic");xy(&mut out,10,20,*centre);pair(&mut out,40,radii.x as f64/1e6);if (end-start).abs()<359.999{pair(&mut out,50,start);pair(&mut out,51,end);}}
            K::Text{position,text,font,orientation,..}=>{if text.contains(['\n','\r']){return Err("Multiline DXF text is not supported".into());}pair(&mut out,0,"TEXT");pair(&mut out,8,"Schematic");xy(&mut out,10,20,*position);pair(&mut out,40,s.fonts[*font].height_nm as f64/1e6);pair(&mut out,1,text);pair(&mut out,50,orientation*90);}
            K::Junction{position,radius_nm}=>{pair(&mut out,0,"CIRCLE");pair(&mut out,8,"Schematic");xy(&mut out,10,20,*position);pair(&mut out,40,*radius_nm as f64/1e6);}
            _=>return Err("This schematic contains objects outside the verified DXF export subset. Use Smart PDF for the complete drawing".into()),
        }
    }
    out.push_str("0\nENDSEC\n0\nEOF\n");
    Ok(out)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn units_text_and_unsupported_import_are_checked_before_insert() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.dxf");
        std::fs::write(&path,"0\nSECTION\n2\nHEADER\n9\n$INSUNITS\n70\n1\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n0\nLINE\n10\n1\n20\n2\n11\n3\n21\n2\n0\nENDSEC\n0\nEOF\n").unwrap();
        let f = import(&path).unwrap();
        if let K::Polyline { points, .. } = &f.primitives[0].kind {
            assert_eq!(
                points[0],
                Point {
                    x: 25_400_000,
                    y: 50_800_000
                }
            );
        } else {
            panic!();
        }
        let mut doc = Document::new(eda_core::DocumentKind::Schematic, "dxf");
        doc.schematic.as_mut().unwrap().primitives = f.primitives;
        std::fs::write(&path, export(&doc).unwrap()).unwrap();
        assert_eq!(import(&path).unwrap().primitives.len(), 1);
        std::fs::write(&path, "0\nSECTION\n2\nENTITIES\n0\nINSERT\n0\nENDSEC\n").unwrap();
        assert!(
            import(&path)
                .unwrap_err()
                .contains("Unsupported DXF entity")
        );
    }
}
