//! Regression scopes found in native ReferenceD, ReferenceE, ReferenceC, ReferenceF and ReferenceG boards.
use super::*;
use crate::pcb_route::RouteSession;

fn board_and_pad() -> (Board, Item) {
    let mut board = Board::default();
    board.components.push(Component {
        id: Uuid::new_v4(),
        index: 7,
        designator: "DA1".into(),
        comment: String::new(),
        footprint: "QFN'32".into(),
        footprint_parameters: None,
        parameters: Some(Fields::new()),
        position: Point::default(),
        rotation: 0.,
        layer: TOP,
        locked: false,
        properties: Fields::new(),
    });
    let mut pad = Item::new(
        TOP,
        Some(0),
        Shape::Pad {
            position: Point::mm(0, 0),
            size: Point::mm(1, 1),
            rotation: 0.,
            shape: PadShape::Round,
            hole: 0,
            plated: true,
            name: "25".into(),
            bottom_size: None,
            middle_size: None,
        },
    );
    pad.component = Some(7);
    (board, pad)
}

#[test]
fn layer_aliases_and_boolean_strings_keep_surface_and_copper_scopes_distinct() {
    let board = Board::default();
    for (layer, top, top_copper, bottom, bottom_copper, mid, silk) in [
        (TOP, true, true, false, false, false, false),
        (TOP_OVERLAY, true, false, false, false, false, true),
        (35, true, false, false, false, false, false),
        (BOTTOM, false, false, true, true, false, false),
        (BOTTOM_OVERLAY, false, false, true, false, false, true),
        (38, false, false, true, false, false, false),
        (2, false, false, false, false, true, false),
        (39, false, false, false, false, false, false),
        (MULTI_LAYER, false, false, false, false, false, false),
    ] {
        let item = probe(layer, None, Point::default(), 200_000);
        for (query_, expected) in [
            ("OnTop", top),
            ("OnTopLayer", top_copper),
            ("OnBottom", bottom),
            ("OnBottomLayer", bottom_copper),
            ("OnMid", mid),
            ("OnSilkscreen", silk),
        ] {
            assert_eq!(
                query(&board, &item, query_),
                Ok(expected),
                "{layer}: {query_}"
            );
            assert_eq!(
                query(&board, &item, &format!("{query_} = 'False'")),
                Ok(!expected)
            );
            assert_eq!(
                query(&board, &item, &format!("{query_} <> False")),
                Ok(expected)
            );
        }
    }
}

#[test]
fn footprint_pad_scopes_match_only_owned_named_pads_and_accept_native_literal_identifiers() {
    let (board, mut pad) = board_and_pad();
    for expression in [
        "HasFootprintPad('QFN''32','25')",
        "HasFootprintPad ('QFN*',25)",
        "HasFootprintPad('','2?') = True",
        "IsPad And Name Like '2*'",
        "HasFootPrint ('QFN''32')",
        "InComponent ('DA1') && !IsVia",
        "NOT\tIsVia",
    ] {
        assert_eq!(query(&board, &pad, expression), Ok(true), "{expression}");
    }
    for expression in [
        "HasFootprintPad('other','25')",
        "HasFootprintPad('QFN*','1')",
    ] {
        assert_eq!(query(&board, &pad, expression), Ok(false));
    }
    for expression in [
        "HasFootprintPad('QFN*')",
        "HasFootprintPad('QFN*','25','26')",
        "HasFootprintPad('QFN*',25+1)",
        "HasFootprintPad('QFN*',)",
        "All || UnknownPredicate",
        "False && UnknownPredicate",
    ] {
        assert!(query(&board, &pad, expression).is_err(), "{expression}");
    }
    pad.component = None;
    assert_eq!(query(&board, &pad, "HasFootprintPad('','25')"), Ok(false));
    let mut track = probe(TOP, None, Point::default(), 200_000);
    track.component = Some(7);
    assert_eq!(
        query(&board, &track, "HasFootprintPad('QFN*','25')"),
        Ok(false)
    );
}

#[test]
fn native_hole_scopes_distinguish_plating_round_slot_and_drill_type() {
    let (board, mut pad) = board_and_pad();
    let expression = "(IsThruPin) and (DrillType = 'Drilled') and ((HasRoundHole = True) and (PadIsPlated = True))";
    assert_eq!(query(&board, &pad, expression), Ok(false));
    if let Shape::Pad { hole, .. } = &mut pad.shape {
        *hole = 400_000;
    }
    assert_eq!(query(&board, &pad, expression), Ok(true));
    if let Shape::Pad { plated, .. } = &mut pad.shape {
        *plated = false;
    }
    assert_eq!(query(&board, &pad, expression), Ok(false));
    assert_eq!(
        query(&board, &pad, "IsPad AND (PadIsPlated = 'False')"),
        Ok(true)
    );
    pad.pad_stack = Some(Box::new(PadStack {
        drill_shape: DrillShape::Slot,
        slot_length: 800_000,
        ..Default::default()
    }));
    assert_eq!(
        query(
            &board,
            &pad,
            "IsThruPin and (HasSlotHole = True) and (DrillType = 'Drilled')"
        ),
        Ok(true)
    );
    assert_eq!(query(&board, &pad, "HasRoundHole"), Ok(false));
    pad.properties.insert("DRILLTYPE".into(), "Punched".into());
    assert_eq!(query(&board, &pad, "DrillType = 'Drilled'"), Ok(false));
    assert_eq!(query(&board, &pad, "DrillType <> 'Drilled'"), Ok(true));
}

fn add_pair(board: &mut Board) {
    board.nets = ["D_P", "D_N", "OTHER"]
        .into_iter()
        .enumerate()
        .map(|(index, name)| Net {
            index: index as u32,
            name: name.into(),
            properties: Fields::new(),
        })
        .collect();
    board.differential_pairs.push(Fields::from([
        ("NAME".into(), "D".into()),
        ("POSITIVENETNAME".into(), "D_P".into()),
        ("NEGATIVENETNAME".into(), "D_N".into()),
    ]));
    board.classes.push(Fields::from([
        ("NAME".into(), "ETHERNET".into()),
        ("KIND".into(), "6".into()),
        ("M0".into(), "D".into()),
    ]));
}

#[test]
fn pair_membership_reads_native_net_identity_and_kind_six_classes() {
    let (mut board, mut pad) = board_and_pad();
    add_pair(&mut board);
    for net in [Some(0), Some(1), Some(2), None] {
        pad.net = net;
        for expression in [
            "InDifferentialPair ('D')",
            "InDifferentialPair('D*')",
            "InAnyDifferentialPair",
            "InDifferentialPairClass('ETHERNET')",
        ] {
            assert_eq!(
                query(&board, &pad, expression),
                Ok(matches!(net, Some(0 | 1))),
                "{net:?}: {expression}"
            );
        }
    }
    pad.net = Some(0);
    board.classes[0].insert("KIND".into(), "0".into());
    assert_eq!(
        query(&board, &pad, "InDifferentialPairClass('ETHERNET')"),
        Ok(false)
    );
    board.classes[0].insert("KIND".into(), "6".into());
    board.classes[0].remove("M0");
    board.classes[0].insert("SUPERCLASS".into(), "TRUE".into());
    assert_eq!(
        query(&board, &pad, "InDifferentialPairClass('ETHERNET')"),
        Ok(true)
    );
}

#[test]
fn flat_boolean_chains_do_not_hit_the_nested_expression_depth_limit() {
    let board = Board::default();
    let item = probe(TOP, None, Point::default(), 200_000);
    let chain = std::iter::repeat_n("IsVia", 80)
        .chain(["IsTrack"])
        .collect::<Vec<_>>()
        .join(" Or ");
    assert_eq!(query(&board, &item, &chain), Ok(true));
    assert_eq!(query(&board, &item, "True OR True XOR True"), Ok(false));
    assert_eq!(
        query(&board, &item, "IsTrack && (OnTopLayer || OnBottomLayer)"),
        Ok(true)
    );
    assert!(query(&board, &item, &format!("{chain} Or UnknownPredicate")).is_err());
}

#[test]
fn imported_pair_width_and_footprint_clearance_allow_route_preview_commit_and_exact_undo() {
    let (mut board, pad) = board_and_pad();
    add_pair(&mut board);
    board.items.push(pad);
    let mut width = default_rule("Width");
    width.insert("SCOPE1EXPRESSION".into(), "InDifferentialPair('D')".into());
    width.insert("PREFEREDWIDTH".into(), "0.4mm".into());
    let mut clearance = default_rule("Clearance");
    clearance.insert(
        "SCOPE2EXPRESSION".into(),
        "IsVia or (IsPad and Not (HasFootprintPad('QFN''32','25')))".into(),
    );
    let mut edge = default_rule("BoardOutlineClearance");
    edge.insert(
        "SCOPE1EXPRESSION".into(),
        "InComponentClass('All Components') and Not OnSilkscreen".into(),
    );
    board.rules = vec![width, clearance, edge];
    let mut route =
        RouteSession::begin(&board, 0, Point::default(), TOP, Some(0), 200_000, 0).unwrap();
    assert_eq!(route.width, 400_000);
    let end = Point::mm(5, 0);
    let preview = route.preview(end).unwrap();
    route.place(end, None).unwrap();
    let finished = route.finish(0).unwrap();
    assert_eq!(
        preview.items.last().unwrap().shape,
        finished.items.last().unwrap().shape
    );
    let mut document = crate::Document::new(crate::DocumentKind::Board, "Query regression");
    document.pcb = Some(board);
    let original = document.clone();
    let mut history = crate::History::default();
    history.commit_pcb(&mut document, finished).unwrap();
    history.undo(&mut document);
    assert_eq!(document, original);
    document.pcb.as_mut().unwrap().rules[0]
        .insert("SCOPE1EXPRESSION".into(), "UnknownPredicate".into());
    let error = RouteSession::begin(
        document.pcb.as_ref().unwrap(),
        0,
        Point::default(),
        TOP,
        Some(0),
        200_000,
        0,
    )
    .err()
    .unwrap();
    assert!(
        error.contains("Width (Width), SCOPE1EXPRESSION: Unsupported query"),
        "{error}"
    );
}
