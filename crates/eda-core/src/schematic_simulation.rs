//! Simulation placement data. Geometry and model expressions are independent of any simulator.
use crate::{
    Point,
    schematic::{Component, ComponentModel, SchPrimitiveKind as K, TextRole},
    schematic_edit::{self as edit, AUTHORED, Fragment, Placement},
};
use std::collections::BTreeMap;
use uuid::Uuid;

pub fn parameter(
    owner: Uuid,
    name: &str,
    value: &str,
    at: Point,
    font: usize,
    hidden: bool,
) -> crate::schematic::SchPrimitive {
    let mut p = edit::primitive(
        K::Text {
            position: at,
            text: value.into(),
            font,
            orientation: 0,
            justification: 0,
        },
        None,
    );
    p.presentation.parent = Some(owner);
    p.presentation.record_type = 41;
    p.presentation.name = name.into();
    p.presentation.raw_text = Some(value.into());
    p.presentation.hidden = hidden;
    p.presentation.auto_position = true;
    p.presentation.allow_sync = true;
    p.width_nm = 101_600;
    p
}

pub fn probe_kind(s: &crate::schematic::Schematic, id: Uuid) -> Option<usize> {
    [
        "ProbeVoltage",
        "ProbeVoltageDiff",
        "ProbeCurrent",
        "ProbePower",
    ]
    .iter()
    .position(|name| {
        s.primitives.iter().chain(&s.parameter_objects).any(|p| {
            p.presentation.parent == Some(id)
                && p.presentation.name.eq_ignore_ascii_case(name)
                && matches!(&p.kind,K::Text{text,..} if text.eq_ignore_ascii_case("True"))
        })
    })
}

pub fn place(
    kind: Placement,
    designator: &str,
    value: &str,
    font: usize,
) -> Result<Fragment, String> {
    let mut f = Fragment::default();
    let probe = match kind {
        Placement::VoltageProbe => Some(0),
        Placement::VoltageDiffProbe => Some(1),
        Placement::CurrentProbe => Some(2),
        Placement::PowerProbe => Some(3),
        _ => None,
    };
    if let Some(n) = probe {
        let mut p = edit::primitive(
            K::ParameterSet {
                position: Point::default(),
                orientation: 0,
                label: "Simulation Probe".into(),
                style: 0,
            },
            None,
        );
        p.presentation.record_type = 210;
        p.presentation.name = "Simulation Probe".into();
        p.presentation.allow_sync = true;
        p.color = 0x808080;
        p.width_nm = 101_600;
        let id = p.id;
        f.primitives.push(p);
        for (name, value, x, y, hidden, color) in [
            (
                "SimulationProbe",
                ["V", if value == "VD-" { "VD-" } else { "VD+" }, "I", "P"][n],
                2_540_000,
                2_540_000,
                false,
                255,
            ),
            (
                [
                    "ProbeVoltage",
                    "ProbeVoltageDiff",
                    "ProbeCurrent",
                    "ProbePower",
                ][n],
                "True",
                0,
                5_080_000,
                true,
                0x800000,
            ),
            ("ProbeName", "Empty Probe", 0, 2_540_000, true, 0x808080),
            ("ProbeValueDisplay", "", 2_540_000, 0, false, 255),
            ("ProbeCustomName", "", 0, 2_540_000, true, 0x808080),
        ] {
            let mut p = parameter(id, name, value, Point { x, y }, font, hidden);
            p.color = color;
            f.primitives.push(p);
        }
        if n == 1 {
            f.primitives.push(parameter(
                id,
                "VD_ID",
                designator,
                Point { x: 0, y: 5_080_000 },
                font,
                true,
            ));
        }
        return Ok(f);
    }
    let initial = kind == Placement::InitialCondition;
    let current = kind == Placement::CurrentSource;
    if !initial && !matches!(kind, Placement::VoltageSource | Placement::CurrentSource) {
        return Err("Unsupported simulation placement".into());
    }
    let id = Uuid::new_v4();
    let (reference, prefix, model, default_comment) = if initial {
        (".IC", "", ".ic", "Initial Condition")
    } else if current {
        ("ISRC", "I", "ISRC", "DC 1m")
    } else {
        ("VSRC", "V", "VSRC", "DC 5")
    };
    let magnitude = if value.is_empty() {
        if current { "1m" } else { "5" }
    } else {
        value
    };
    let comment = if initial {
        default_comment.to_owned()
    } else {
        format!("DC {magnitude}")
    };
    let mut params = BTreeMap::new();
    params.insert("Comment".into(), comment.clone());
    if initial {
        params.insert("Initial Voltage".into(), String::new());
    } else {
        params.insert("Value".into(), magnitude.into());
        params.insert("AC Magnitude".into(), "1".into());
        params.insert("AC Phase".into(), "0".into());
    }
    if initial {
        params.insert("SimulationGenericComponent".into(), "True".into());
    }
    let model_params = BTreeMap::from([
        ("Spice Prefix".into(), prefix.into()),
        ("Kind".into(), kind.label().into()),
        ("SubKind".into(), if initial { "Set Initial Condition" } else { "DC Source" }.into()),
        ("Netlist".into(), if initial { "?\"INITIAL VOLTAGE\"\u{a6}.IC V(%1)=@\"INITIAL VOLTAGE\"\u{a6}" }
            else { "@DESIGNATOR %1 %2 @VALUE #\"AC MAGNITUDE\"\u{a6}AC @\"AC MAGNITUDE\"\u{a6} @\"AC PHASE\"" }.into()),
    ]);
    f.components.push(Component {
        id,
        source_index: AUTHORED,
        position: Point::default(),
        designator: designator.into(),
        comment: comment.clone(),
        library_reference: reference.into(),
        description: kind.label().into(),
        part: 1,
        display_mode: 0,
        parameters: params,
        settings: crate::schematic::ComponentSettings {
            source: "Simulation Generic Components.IntLib".into(),
            design_item_id: reference.into(),
            models: vec![ComponentModel {
                name: model.into(),
                kind: "SIM".into(),
                current: true,
                parameters: model_params,
            }],
            ..Default::default()
        },
    });
    let unit = 254_000;
    let pt = |x, y| Point {
        x: x * unit,
        y: y * unit,
    };
    for (y, orientation, number) in if initial {
        vec![(-14, 3, "1")]
    } else {
        vec![(0, 1, "1"), (-20, 3, "2")]
    } {
        let mut p = edit::primitive(
            K::Pin {
                position: pt(10, y),
                length_nm: if initial { 6 * unit } else { 10 * unit },
                orientation,
                name: number.into(),
                number: number.into(),
                show_name: false,
                show_number: false,
                font,
                number_font: Some(font),
                electrical: 4,
                outer_symbol: 0,
                inner_symbol: 0,
            },
            Some(id),
        );
        p.color = 128;
        p.width_nm = 101_600;
        p.presentation.allow_sync = true;
        p.presentation.name = number.into();
        f.primitives.push(p);
    }
    let mut circle = edit::primitive(
        K::Arc {
            centre: if initial { pt(10, -7) } else { pt(10, -10) },
            radii: if initial { pt(10, 7) } else { pt(10, 10) },
            start: 0.,
            end: 360.,
        },
        Some(id),
    );
    circle.fill = Some(0xb0ffff);
    circle.color = 128;
    circle.width_nm = 101_600;
    circle.presentation.allow_sync = true;
    f.primitives.push(circle);
    let segments = if initial {
        let mut p = edit::primitive(
            K::Text {
                position: pt(4, -12),
                text: "=ic".into(),
                font,
                orientation: 0,
                justification: 0,
            },
            Some(id),
        );
        p.presentation.raw_text = Some("=ic".into());
        p.presentation.allow_sync = true;
        p.width_nm = 101_600;
        p.color = 0;
        f.primitives.push(p);
        vec![]
    } else if current {
        vec![
            (pt(10, -16), pt(10, -4)),
            (pt(7, -8), pt(10, -4)),
            (pt(10, -4), pt(13, -8)),
        ]
    } else {
        vec![
            (pt(10, -2), pt(10, -6)),
            (pt(8, -4), pt(12, -4)),
            (pt(8, -16), pt(12, -16)),
        ]
    };
    for (a, b) in segments {
        let mut p = edit::primitive(
            K::Polyline {
                points: vec![a, b],
                closed: false,
                electrical: "Graphic".into(),
            },
            Some(id),
        );
        p.color = 128;
        p.width_nm = 101_600;
        p.presentation.allow_sync = true;
        f.primitives.push(p);
    }
    for (name, text, role, at, hidden) in [
        (
            "Designator",
            designator,
            TextRole::Designator,
            if initial { pt(0, 0) } else { pt(20, -10) },
            false,
        ),
        (
            "Comment",
            comment.as_str(),
            TextRole::Comment,
            pt(20, -20),
            initial,
        ),
    ] {
        let mut p = parameter(id, name, text, at, font, hidden);
        p.component = Some(id);
        p.presentation.parent = None;
        p.text_role = Some(role);
        p.presentation.record_type = if role == TextRole::Designator { 34 } else { 41 };
        f.primitives.push(p);
    }
    Ok(f)
}
