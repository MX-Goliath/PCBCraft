//! Serializable workspace topology. Hidden panels retain their place in the tree.
use super::{Panel, Side};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(in crate::app) struct Layout {
    pub roots: Vec<Root>,
    next_id: u64,
    #[serde(default)]
    panel_permissions: Vec<(Panel, bool, bool)>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(in crate::app) struct Root {
    pub id: u64,
    pub edge: Option<Side>,
    pub tree: Node,
    pub extent: f32,
    pub bounds: [f32; 4],
    pub auto_hide: bool,
    pub horizontal: bool,
    pub vertical: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(in crate::app) enum Node {
    Group {
        id: u64,
        tabs: Vec<Panel>,
        active: Panel,
    },
    Split {
        horizontal: bool,
        fraction: f32,
        first: Box<Node>,
        second: Box<Node>,
    },
}
#[derive(Clone, Debug)]
pub(in crate::app) enum Target {
    Edge(Side),
    Tab(u64, usize),
    Split(u64, Side),
    Float([f32; 4]),
}
impl Default for Layout {
    fn default() -> Self {
        Self {
            roots: vec![
                Root::new(
                    1,
                    Some(Side::Left),
                    Node::Group {
                        id: 2,
                        tabs: vec![
                            Panel::Projects,
                            Panel::Git,
                            Panel::ViewConfiguration,
                            Panel::Library,
                        ],
                        active: Panel::Projects,
                    },
                    334.,
                ),
                Root::new(
                    3,
                    Some(Side::Right),
                    Node::Group {
                        id: 4,
                        tabs: vec![
                            Panel::Properties,
                            Panel::Components,
                            Panel::Messages,
                            Panel::Changes,
                        ],
                        active: Panel::Properties,
                    },
                    316.,
                ),
            ],
            next_id: 5,
            panel_permissions: vec![],
        }
    }
}
impl Root {
    fn new(id: u64, edge: Option<Side>, tree: Node, extent: f32) -> Self {
        Self {
            id,
            edge,
            tree,
            extent,
            bounds: [180., 150., 334., 560.],
            auto_hide: false,
            horizontal: true,
            vertical: true,
        }
    }
}
impl Node {
    pub fn panels(&self) -> Vec<Panel> {
        match self {
            Self::Group { tabs, .. } => tabs.clone(),
            Self::Split { first, second, .. } => [first.panels(), second.panels()].concat(),
        }
    }
    pub fn group(&self, group: u64) -> Option<&Self> {
        match self {
            Self::Group { id, .. } => (*id == group).then_some(self),
            Self::Split { first, second, .. } => first.group(group).or_else(|| second.group(group)),
        }
    }
    pub fn group_mut(&mut self, group: u64) -> Option<&mut Self> {
        match self {
            Self::Group { id, .. } => (*id == group).then_some(self),
            Self::Split { first, second, .. } => {
                first.group_mut(group).or_else(|| second.group_mut(group))
            }
        }
    }
    pub fn select(&mut self, panel: Panel) {
        match self {
            Self::Group { tabs, active, .. } if tabs.contains(&panel) => *active = panel,
            Self::Split { first, second, .. } => {
                first.select(panel);
                second.select(panel);
            }
            _ => {}
        }
    }
    fn remove(self, panels: &[Panel]) -> Option<Self> {
        match self {
            Self::Group {
                id,
                mut tabs,
                mut active,
            } => {
                tabs.retain(|p| !panels.contains(p));
                if !tabs.contains(&active) {
                    active = *tabs.last()?;
                }
                Some(Self::Group { id, tabs, active })
            }
            Self::Split {
                horizontal,
                fraction,
                first,
                second,
            } => match (first.remove(panels), second.remove(panels)) {
                (Some(first), Some(second)) => Some(Self::Split {
                    horizontal,
                    fraction,
                    first: Box::new(first),
                    second: Box::new(second),
                }),
                (a, b) => a.or(b),
            },
        }
    }
}
impl Layout {
    fn id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }
    pub fn root_for(&self, panel: Panel) -> Option<&Root> {
        self.roots.iter().find(|r| r.tree.panels().contains(&panel))
    }
    pub fn select(&mut self, panel: Panel) {
        for r in &mut self.roots {
            r.tree.select(panel);
        }
    }
    pub fn group(&self, id: u64) -> Option<&Node> {
        self.roots.iter().find_map(|r| r.tree.group(id))
    }
    pub fn group_mut(&mut self, id: u64) -> Option<&mut Node> {
        self.roots.iter_mut().find_map(|r| r.tree.group_mut(id))
    }
    pub fn permissions(&self, panel: Panel) -> (bool, bool) {
        self.panel_permissions
            .iter()
            .find(|(p, _, _)| *p == panel)
            .map(|(_, h, v)| (*h, *v))
            .or_else(|| self.root_for(panel).map(|r| (r.horizontal, r.vertical)))
            .unwrap_or((true, true))
    }
    pub fn set_permissions(&mut self, panel: Panel, horizontal: bool, vertical: bool) {
        if let Some(entry) = self
            .panel_permissions
            .iter_mut()
            .find(|(p, _, _)| *p == panel)
        {
            *entry = (panel, horizontal, vertical);
        } else {
            self.panel_permissions.push((panel, horizontal, vertical));
        }
    }
    pub fn set_auto_hide(&mut self, panel: Panel, hidden: bool) {
        let Some(source) = self.root_for(panel).cloned() else {
            return;
        };
        if source.edge.is_none() {
            return;
        }
        // A pin belongs to the active panel, never to the other tabs or splits.
        if source.tree.panels().len() > 1 {
            self.move_panels(&[panel], Target::Float(source.bounds));
        }
        if let Some(root) = self
            .roots
            .iter_mut()
            .find(|r| r.tree.panels().contains(&panel))
        {
            root.edge = source.edge;
            root.extent = source.extent;
            root.auto_hide = hidden;
        }
    }
    pub fn move_panels(&mut self, panels: &[Panel], target: Target) {
        if panels.is_empty() {
            return;
        }
        // Dropping a complete group on itself is a no-op. A one-tab reorder is valid.
        if let Target::Tab(id, _) | Target::Split(id, _) = &target {
            let Some(group) = self.group(*id) else {
                return;
            };
            if group.panels().iter().all(|p| panels.contains(p)) {
                return;
            }
        }
        for panel in panels {
            let (h, v) = self.permissions(*panel);
            self.set_permissions(*panel, h, v);
        }
        let source = self.root_for(panels[0]).cloned();
        self.roots = std::mem::take(&mut self.roots)
            .into_iter()
            .filter_map(|mut root| {
                root.tree = root.tree.remove(panels)?;
                Some(root)
            })
            .collect();
        let active = panels[0];
        match target {
            Target::Tab(group, index) => {
                if let Some(Node::Group {
                    tabs,
                    active: selected,
                    ..
                }) = self.group_mut(group)
                {
                    let index = index.min(tabs.len());
                    tabs.splice(index..index, panels.iter().copied());
                    *selected = active;
                }
            }
            target => {
                let node = Node::Group {
                    id: self.id(),
                    tabs: panels.to_vec(),
                    active,
                };
                match target {
                    Target::Split(group, side) => {
                        if let Some(old) = self.group_mut(group) {
                            let (first, second) = if matches!(side, Side::Left | Side::Top) {
                                (node, old.clone())
                            } else {
                                (old.clone(), node)
                            };
                            *old = Node::Split {
                                horizontal: matches!(side, Side::Left | Side::Right),
                                fraction: 0.5,
                                first: Box::new(first),
                                second: Box::new(second),
                            };
                        }
                    }
                    Target::Edge(side) => {
                        if let Some(root) = self.roots.iter_mut().find(|r| r.edge == Some(side)) {
                            let old = root.tree.clone();
                            root.tree = Node::Split {
                                horizontal: matches!(side, Side::Top | Side::Bottom),
                                fraction: 0.5,
                                first: Box::new(old),
                                second: Box::new(node),
                            };
                        } else {
                            let id = self.id();
                            self.roots.push(Root::new(
                                id,
                                Some(side),
                                node,
                                if matches!(side, Side::Top | Side::Bottom) {
                                    220.
                                } else {
                                    334.
                                },
                            ));
                        }
                    }
                    Target::Float(bounds) => {
                        let id = self.id();
                        let mut root = Root::new(id, None, node, 334.);
                        root.bounds = bounds;
                        if let Some(source) = source {
                            root.horizontal = source.horizontal;
                            root.vertical = source.vertical;
                        }
                        self.roots.push(root);
                    }
                    Target::Tab(..) => unreachable!(),
                }
            }
        }
    }
}
