//! Observed AD24 PCB Active Bar menu labels and order. No vendor engine code.
use super::Group;
pub(super) struct Entry {
    pub id: &'static str,
    pub label: &'static str,
}
pub(super) const GROUPS: [Group; 14] = [
    Group {
        name: "Selection Filter",
        width: 363.,
        items: &[],
    },
    Group {
        name: "Objects for snapping",
        width: 425.,
        items: &[],
    },
    Group {
        name: "Move",
        width: 188.,
        items: &[
            Entry {
                id: "MoveObject",
                label: "Move",
            },
            Entry {
                id: "MoveDrag",
                label: "Drag",
            },
            Entry {
                id: "MoveComponent",
                label: "Component",
            },
            Entry { id: "", label: "" },
            Entry {
                id: "MoveReRoute",
                label: "Re-Route",
            },
            Entry {
                id: "BreakTrack",
                label: "Break Track",
            },
            Entry {
                id: "MoveDragTrackEnd",
                label: "Drag Track End",
            },
            Entry {
                id: "GroupResizeTracks",
                label: "Move / Resize Tracks",
            },
            Entry { id: "", label: "" },
            Entry {
                id: "MoveSelection",
                label: "Move Selection",
            },
            Entry {
                id: "OffsetSelection",
                label: "Move Selection by X, Y...",
            },
            Entry {
                id: "RotateSelectedObjects",
                label: "Rotate Selection...",
            },
            Entry {
                id: "FlipSelectedObjects",
                label: "Flip Selection",
            },
        ],
    },
    Group {
        name: "Select",
        width: 230.,
        items: &[
            Entry {
                id: "NextSelection",
                label: "Select overlapped",
            },
            Entry {
                id: "NextTopologySelection",
                label: "Select next",
            },
            Entry {
                id: "SelectLasso",
                label: "Lasso Select",
            },
            Entry {
                id: "SelectInsideArea",
                label: "Inside Area",
            },
            Entry {
                id: "SelectOutsideArea",
                label: "Outside Area",
            },
            Entry {
                id: "SelectTouchingRectangle",
                label: "Touching Rectangle",
            },
            Entry {
                id: "SelectTouchingLine",
                label: "Touching Line",
            },
            Entry {
                id: "SelectAll",
                label: "All",
            },
            Entry {
                id: "SelectBoard",
                label: "Board",
            },
            Entry { id: "", label: "" },
            Entry {
                id: "SelectNet",
                label: "Net",
            },
            Entry {
                id: "SelectConnectedCopper",
                label: "Connected Copper",
            },
            Entry {
                id: "SelectPhysicalConnection",
                label: "Physical Connection",
            },
            Entry {
                id: "SelectPhysicalConnectionOnLayer",
                label: "Physical Connection Single Layer",
            },
            Entry { id: "", label: "" },
            Entry {
                id: "SelectComponentConnections",
                label: "Component Connections",
            },
            Entry {
                id: "SelectComponentNets",
                label: "Component Nets",
            },
            Entry {
                id: "SelectRoomConnections",
                label: "Room Connections",
            },
            Entry { id: "", label: "" },
            Entry {
                id: "SelectAllOnLayer",
                label: "All on Layer",
            },
            Entry {
                id: "SelectFreeObjects",
                label: "Free Objects",
            },
            Entry {
                id: "SelectAllLocked",
                label: "All Locked",
            },
            Entry {
                id: "SelectOffGridPads",
                label: "Off Grid Pads",
            },
            Entry { id: "", label: "" },
            Entry {
                id: "ToggleSelection",
                label: "Toggle Selection",
            },
        ],
    },
    Group {
        name: "Align",
        width: 255.,
        items: &[
            Entry {
                id: "AlignComponentsDialog",
                label: "Align...",
            },
            Entry {
                id: "AutopositionComponentTexts",
                label: "Position Component Text...",
            },
            Entry { id: "", label: "" },
            Entry {
                id: "AlignComponentsLeft",
                label: "Align Left",
            },
            Entry {
                id: "AlignComponentsRight",
                label: "Align Right",
            },
            Entry {
                id: "AlignComponentsLeftSideBySide",
                label: "Align Left (maintain spacing)",
            },
            Entry {
                id: "AlignComponentsRightSideBySide",
                label: "Align Right (maintain spacing)",
            },
            Entry {
                id: "AlignComponentsCenterHorizontal",
                label: "Align Horizontal Centers",
            },
            Entry {
                id: "AlignComponentsSpreadHorizontal",
                label: "Distribute Horizontally",
            },
            Entry {
                id: "AlignComponentsExpandHorizontal",
                label: "Increase Horizontal Spacing",
            },
            Entry {
                id: "AlignComponentsContractHorizontal",
                label: "Decrease Horizontal Spacing",
            },
            Entry { id: "", label: "" },
            Entry {
                id: "AlignComponentsTop",
                label: "Align Top",
            },
            Entry {
                id: "AlignComponentsBottom",
                label: "Align Bottom",
            },
            Entry {
                id: "AlignComponentsTopSideBySide",
                label: "Align Top (maintain spacing)",
            },
            Entry {
                id: "AlignComponentsBottomSideBySide",
                label: "Align Bottom (maintain spacing)",
            },
            Entry {
                id: "AlignComponentsCenterVertical",
                label: "Align Vertical Centers",
            },
            Entry {
                id: "AlignComponentsSpreadVertical",
                label: "Distribute Vertically",
            },
            Entry {
                id: "AlignComponentsExpandVertical",
                label: "Increase Vertical Spacing",
            },
            Entry {
                id: "AlignComponentsContractVertical",
                label: "Decrease Vertical Spacing",
            },
            Entry { id: "", label: "" },
            Entry {
                id: "MoveComponentsToGrid",
                label: "Align To Grid",
            },
            Entry {
                id: "MoveAllComponentsOriginToGrid",
                label: "Move All Components Origin To Grid",
            },
        ],
    },
    Group {
        name: "Components",
        width: 155.,
        items: &[
            Entry {
                id: "PlaceComponent",
                label: "Component...",
            },
            Entry {
                id: "PlaceReuseBlock",
                label: "Reuse Block...",
            },
            Entry {
                id: "PlaceComponentBody",
                label: "Extruded 3D Body",
            },
            Entry {
                id: "PlaceStepModel",
                label: "3D Body",
            },
        ],
    },
    Group {
        name: "Routing",
        width: 240.,
        items: &[
            Entry {
                id: "AdvancedRouting",
                label: "Interactive Routing",
            },
            Entry {
                id: "AdvancedDiffPairRoute",
                label: "Interactive Differential Pair Routing",
            },
            Entry {
                id: "AdvancedQuickRouting",
                label: "Quick Routing",
            },
            Entry {
                id: "AdvancedQuickDiffPairRoute",
                label: "Quick Differential Pair Routing",
            },
            Entry {
                id: "AdvancedMultiRoute",
                label: "Interactive Multi-Routing",
            },
        ],
    },
    Group {
        name: "Length Tuning",
        width: 237.,
        items: &[
            Entry {
                id: "InteractiveLengthTuningSmartUnion",
                label: "Interactive Length Tuning",
            },
            Entry {
                id: "InteractiveDiffPairLengthTuning",
                label: "Interactive Diff Pair Length Tuning",
            },
        ],
    },
    Group {
        name: "Pad/Via",
        width: 79.,
        items: &[
            Entry {
                id: "PlaceVia",
                label: "Via",
            },
            Entry {
                id: "PlacePad",
                label: "Pad",
            },
        ],
    },
    Group {
        name: "Polygons",
        width: 166.,
        items: &[
            Entry {
                id: "PlacePolygonPlane",
                label: "Polygon Pour...",
            },
            Entry {
                id: "PlaceCutoutRegion",
                label: "Polygon Pour Cutout",
            },
            Entry {
                id: "CutPolygonPlane",
                label: "Slice Polygon Pour",
            },
        ],
    },
    Group {
        name: "Keepout",
        width: 125.,
        items: &[
            Entry {
                id: "PlaceKeepoutTrack",
                label: "Track",
            },
            Entry {
                id: "PlaceKeepoutArcCenter",
                label: "Arc (Center)",
            },
            Entry {
                id: "PlaceKeepoutSolidRegion",
                label: "Solid Region",
            },
            Entry {
                id: "PlaceKeepoutFill",
                label: "Fill",
            },
        ],
    },
    Group {
        name: "Dimensions",
        width: 203.,
        items: &[
            Entry {
                id: "PlaceDimensionLinear",
                label: "Linear Dimension",
            },
            Entry {
                id: "PlaceDimensionAngular",
                label: "Angular Dimension",
            },
            Entry {
                id: "PlaceDimensionRadial",
                label: "Radial Dimension",
            },
            Entry {
                id: "PlaceDimensionDatum",
                label: "Ordinate Dimension",
            },
            Entry {
                id: "PlaceDimensionLeader",
                label: "Leader Dimension",
            },
            Entry {
                id: "PlaceDimension",
                label: "Standard Dimension",
            },
            Entry {
                id: "PlaceDimensionCenter",
                label: "Center Dimension",
            },
            Entry {
                id: "PlaceDimensionBaseline",
                label: "Baseline Dimension",
            },
            Entry {
                id: "PlaceDimensionLinearDiameter",
                label: "Linear Diameter Dimension",
            },
            Entry {
                id: "PlaceDimensionRadialDiameter",
                label: "Radial Diameter Dimension",
            },
        ],
    },
    Group {
        name: "Strings",
        width: 115.,
        items: &[
            Entry {
                id: "PlaceString",
                label: "String",
            },
            Entry {
                id: "PlaceFrame",
                label: "Text Frame",
            },
        ],
    },
    Group {
        name: "Drawing Tools",
        width: 141.,
        items: &[
            Entry {
                id: "PlaceLine",
                label: "Line",
            },
            Entry {
                id: "PlaceArcCenter",
                label: "Arc (Center)",
            },
            Entry {
                id: "PlaceArcEdge",
                label: "Arc (Edge)",
            },
            Entry {
                id: "PlaceArcAnyAngle",
                label: "Arc (Any Angle)",
            },
            Entry {
                id: "PlaceFullCircle",
                label: "Full Circle",
            },
            Entry {
                id: "PlaceFill",
                label: "Fill",
            },
            Entry {
                id: "PlaceSolidRegion",
                label: "Solid Region",
            },
            Entry {
                id: "PlaceRectangle",
                label: "Rectangle",
            },
        ],
    },
];
