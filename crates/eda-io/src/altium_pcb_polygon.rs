//! Interpreted native pour options; private PCBCraft fields remain readable.
use super::*;
use eda_core::pcb_polygon::{FillMode, Polygon, SameNet};

pub(super) fn upgrade_options(p: &mut Polygon) -> Result<()> {
    let f = &p.properties;
    if f.contains_key("POUROVERSTYLE") {
        p.same_net = match f.num("POUROVERSTYLE", 1.0)? {
            0.0 => SameNet::None,
            1.0 => SameNet::All,
            2.0 => SameNet::PolygonsOnly,
            _ => return Err("Unsupported native polygon POUROVERSTYLE".into()),
        };
    } else if f.contains_key("POUROVER") {
        p.same_net = if f.yes("POUROVER") {
            SameNet::All
        } else {
            SameNet::None
        };
    }
    if let Some(style) = f.get("HATCHSTYLE") {
        p.fill_mode = if style.eq_ignore_ascii_case("Solid") {
            FillMode::Solid
        } else if style.eq_ignore_ascii_case("None") {
            FillMode::None
        } else {
            FillMode::Hatched
        };
    }
    if f.contains_key("REMOVEDEAD") {
        p.remove_islands = f.yes("REMOVEDEAD");
    }
    if f.contains_key("REMOVEISLANDSBYAREA") && !f.contains_key("PCBCRAFT_MINAREA") {
        // Native area is square 1/10000-mil coordinates, not square mm.
        p.min_island_area_mm2 = if f.yes("REMOVEISLANDSBYAREA") {
            let area = f.num("AREATHRESHOLD", 0.0)?;
            if !area.is_finite() || area < 0.0 {
                return Err("Invalid native polygon area threshold".into());
            }
            area * 2.54e-6_f64.powi(2)
        } else {
            0.0
        };
    }
    if f.contains_key("REMOVENECKS") {
        p.neck_width = if f.yes("REMOVENECKS") {
            f.dim("NECKWIDTHTHRESHOLD", 0)?
        } else {
            0
        };
    }
    Ok(())
}

pub(super) fn write_options(p: &Polygon, f: &mut Fields) {
    f.insert(
        "POUROVERSTYLE".into(),
        match p.same_net {
            SameNet::None => "0",
            SameNet::All => "1",
            SameNet::PolygonsOnly => "2",
        }
        .into(),
    );
    f.insert(
        "POUROVER".into(),
        if p.same_net == SameNet::None {
            "FALSE"
        } else {
            "TRUE"
        }
        .into(),
    );
    f.insert(
        "HATCHSTYLE".into(),
        match p.fill_mode {
            FillMode::Solid => "Solid",
            FillMode::None => "None",
            FillMode::Hatched => f.get("HATCHSTYLE").map_or("90Degree", String::as_str),
        }
        .into(),
    );
    f.insert(
        "REMOVEDEAD".into(),
        if p.remove_islands { "TRUE" } else { "FALSE" }.into(),
    );
    f.insert(
        "REMOVEISLANDSBYAREA".into(),
        if p.min_island_area_mm2 > 0.0 {
            "TRUE"
        } else {
            "FALSE"
        }
        .into(),
    );
    if p.min_island_area_mm2 > 0.0 {
        f.insert(
            "AREATHRESHOLD".into(),
            (p.min_island_area_mm2 / 2.54e-6_f64.powi(2)).to_string(),
        );
    }
    f.insert(
        "REMOVENECKS".into(),
        if p.neck_width > 0 { "TRUE" } else { "FALSE" }.into(),
    );
    if p.neck_width > 0 {
        f.insert(
            "NECKWIDTHTHRESHOLD".into(),
            format!("{}mm", p.neck_width as f64 / 1e6),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn input(options: &str) -> Document {
        let source = include_str!("../../../fixtures/pcb-update/editor-v2.PcbDoc")
            .replace("POURINDEX=0", &format!("POURINDEX=0|{options}"));
        import_pcb(source.as_bytes(), "Native polygon options.PcbDoc").unwrap()
    }
    #[test]
    fn native_dead_copper_area_and_neck_settings_are_independent() {
        let doc = input(
            "REMOVEDEAD=TRUE|REMOVEISLANDSBYAREA=TRUE|AREATHRESHOLD=250000000000|REMOVENECKS=TRUE|NECKWIDTHTHRESHOLD=5mil|HATCHSTYLE=Solid|POUROVERSTYLE=2",
        );
        let p = &doc.pcb.as_ref().unwrap().polygon_pours[0];
        assert!(p.remove_islands);
        assert!((p.min_island_area_mm2 - 1.6129).abs() < 1e-10);
        assert_eq!(p.neck_width, 127_000);
        assert_eq!(p.same_net, SameNet::PolygonsOnly);
        let doc = input(
            "REMOVEDEAD=FALSE|REMOVEISLANDSBYAREA=TRUE|AREATHRESHOLD=250000000000|REMOVENECKS=FALSE|NECKWIDTHTHRESHOLD=5mil",
        );
        let p = &doc.pcb.as_ref().unwrap().polygon_pours[0];
        assert!(!p.remove_islands);
        assert!(p.min_island_area_mm2 > 1.6);
        assert_eq!(p.neck_width, 0);
    }
    #[test]
    fn edited_options_roundtrip_through_real_native_fields() {
        for same_net in [SameNet::None, SameNet::All, SameNet::PolygonsOnly] {
            let mut doc = input("");
            let board = doc.pcb.as_mut().unwrap();
            board.source = None;
            let p = &mut board.polygon_pours[0];
            p.same_net = same_net;
            p.remove_islands = true;
            p.neck_width = 150_000;
            p.min_island_area_mm2 = 0.25;
            p.fill_mode = FillMode::None;
            board.pours_stale = true;
            let bytes = writer::serialize(&doc, writer::Format::Binary).unwrap();
            let decoded = import_pcb(&bytes, "Reopened.PcbDoc").unwrap();
            let p = &decoded.pcb.as_ref().unwrap().polygon_pours[0];
            assert_eq!(p.same_net, same_net);
            assert!(p.remove_islands);
            assert_eq!(p.neck_width, 150_000);
            assert_eq!(p.fill_mode, FillMode::None);
            assert!((p.min_island_area_mm2 - 0.25).abs() < 1e-10);
            assert_eq!(p.properties["REMOVEDEAD"], "TRUE");
            assert_eq!(p.properties["REMOVEISLANDSBYAREA"], "TRUE");
            assert!(
                decoded
                    .pcb
                    .as_ref()
                    .unwrap()
                    .items
                    .iter()
                    .all(|i| i.polygon.is_none())
            );
        }
    }
    #[test]
    fn legacy_json_options_upgrade_without_reintroducing_removed_polygons() {
        let mut doc = input(
            "REMOVEDEAD=TRUE|REMOVEISLANDSBYAREA=TRUE|AREATHRESHOLD=250000000000|POUROVERSTYLE=2",
        );
        let board = doc.pcb.as_mut().unwrap();
        board.polygon_model_version = 1;
        let p = &mut board.polygon_pours[0];
        p.remove_islands = false;
        p.same_net = SameNet::All;
        p.min_island_area_mm2 = 0.;
        board.pours_stale = false;
        upgrade_polygons(board, doc.id).unwrap();
        assert!(board.polygon_pours[0].remove_islands);
        assert_eq!(board.polygon_pours[0].same_net, SameNet::PolygonsOnly);
        assert!(board.pours_stale);
        board.polygon_pours.clear();
        board.polygon_model_version = 1;
        upgrade_polygons(board, doc.id).unwrap();
        assert!(board.polygon_pours.is_empty());
    }
    #[test]
    fn mask_polygons_are_editable_and_unknown_hatches_are_never_silently_solid() {
        let mut doc = input("HATCHSTYLE=45Degree|POUROVERSTYLE=1");
        let board = doc.pcb.as_mut().unwrap();
        assert_eq!(board.polygon_pours[0].fill_mode, FillMode::Hatched);
        assert!(
            eda_core::pcb_polygon::repour(board)
                .unwrap_err()
                .contains("hatched")
        );
        let source = include_str!("../../../fixtures/pcb-update/editor-v2.PcbDoc").replace(
            "NAME=Ground Top|NET=0|LAYER=TOP",
            "NAME=Mask polygon|NET=-1|LAYER=TOPSOLDER",
        );
        let doc = import_pcb(source.as_bytes(), "Mask.PcbDoc").unwrap();
        assert_eq!(doc.pcb.as_ref().unwrap().polygon_pours[0].layer, 37);
    }
    #[test]
    fn legacy_json_migration_preserves_user_edits_to_existing_typed_settings() {
        let mut doc = input("REMOVEDEAD=FALSE|REMOVEISLANDSBYAREA=TRUE|AREATHRESHOLD=250000000000");
        let board = doc.pcb.as_mut().unwrap();
        board.polygon_model_version = 1;
        board.polygon_pours[0].remove_islands = true;
        board.polygon_pours[0].min_island_area_mm2 = 0.125;
        upgrade_polygons(board, doc.id).unwrap();
        assert!(board.polygon_pours[0].remove_islands);
        assert_eq!(board.polygon_pours[0].min_island_area_mm2, 0.125);
    }
}
