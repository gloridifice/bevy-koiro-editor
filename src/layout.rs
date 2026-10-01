//! Persistent docking data. No ECS entities or render resources are serialized.
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

pub type Id = u64;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PaneType {
    World,
    Inspector,
    AssetsTree,
    AssetsGallery,
    Viewport,
    Camera,
}
impl PaneType {
    pub const ALL: [Self; 6] = [
        Self::World,
        Self::Inspector,
        Self::AssetsTree,
        Self::AssetsGallery,
        Self::Viewport,
        Self::Camera,
    ];
    pub fn title(self) -> &'static str {
        match self {
            Self::World => "World",
            Self::Inspector => "Inspector",
            Self::AssetsTree => "Assets Tree",
            Self::AssetsGallery => "Assets",
            Self::Viewport => "Viewport",
            Self::Camera => "Camera",
        }
    }
    pub fn description(self) -> &'static str {
        match self {
            Self::World => "World Inspector",
            Self::Inspector => "Entity Component Inspector",
            Self::AssetsTree => "Local asset file tree",
            Self::AssetsGallery => "Asset thumbnail gallery",
            Self::Viewport => "Interactive editor world",
            Self::Camera => "Game camera live preview",
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Pane {
    pub id: Id,
    pub kind: PaneType,
    #[serde(default)]
    pub query: String,
    #[serde(default)]
    pub collapsed: Vec<Id>,
    #[serde(default = "default_orbit")]
    pub orbit: [f32; 3],
    #[serde(default = "default_center")]
    pub center: [f32; 3],
}
fn default_center() -> [f32; 3] {
    [0., 1., 0.]
}
fn default_orbit() -> [f32; 3] {
    [0.75, 0.6, 14.5]
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Group {
    pub id: Id,
    pub tabs: Vec<Pane>,
    pub active: Id,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Axis {
    X,
    Y,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "node")]
pub enum DockNode {
    Group(Group),
    Split {
        id: Id,
        axis: Axis,
        ratio: f32,
        a: Box<Self>,
        b: Box<Self>,
    },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Zone {
    Center,
    Left,
    Right,
    Top,
    Bottom,
}
impl DockNode {
    pub fn id(&self) -> Id {
        match self {
            Self::Group(g) => g.id,
            Self::Split { id, .. } => *id,
        }
    }
    pub fn find(&self, id: Id) -> Option<&Self> {
        if self.id() == id {
            return Some(self);
        }
        match self {
            Self::Split { a, b, .. } => a.find(id).or_else(|| b.find(id)),
            _ => None,
        }
    }
    pub fn find_mut(&mut self, id: Id) -> Option<&mut Self> {
        if self.id() == id {
            return Some(self);
        }
        match self {
            Self::Split { a, b, .. } => a.find_mut(id).or_else(|| b.find_mut(id)),
            _ => None,
        }
    }
    pub fn group(&self, id: Id) -> Option<&Group> {
        match self.find(id)? {
            Self::Group(g) => Some(g),
            _ => None,
        }
    }
    pub fn group_mut(&mut self, id: Id) -> Option<&mut Group> {
        match self.find_mut(id)? {
            Self::Group(g) => Some(g),
            _ => None,
        }
    }
    pub fn pane(&self, id: Id) -> Option<&Pane> {
        match self {
            Self::Group(g) => g.tabs.iter().find(|p| p.id == id),
            Self::Split { a, b, .. } => a.pane(id).or_else(|| b.pane(id)),
        }
    }
    pub fn pane_mut(&mut self, id: Id) -> Option<&mut Pane> {
        match self {
            Self::Group(g) => g.tabs.iter_mut().find(|p| p.id == id),
            Self::Split { a, b, .. } => a.pane_mut(id).or_else(|| b.pane_mut(id)),
        }
    }
    pub fn pane_group(&self, id: Id) -> Option<Id> {
        match self {
            Self::Group(g) => g.tabs.iter().any(|p| p.id == id).then_some(g.id),
            Self::Split { a, b, .. } => a.pane_group(id).or_else(|| b.pane_group(id)),
        }
    }
    pub fn first_group(&self) -> Id {
        match self {
            Self::Group(g) => g.id,
            Self::Split { a, .. } => a.first_group(),
        }
    }
    fn take(&mut self, pane: Id) -> Option<Pane> {
        match self {
            Self::Group(g) => {
                let i = g.tabs.iter().position(|p| p.id == pane)?;
                let p = g.tabs.remove(i);
                if g.active == pane {
                    g.active = g.tabs.get(i.saturating_sub(1)).map_or(0, |p| p.id);
                }
                Some(p)
            }
            Self::Split { a, b, .. } => a.take(pane).or_else(|| b.take(pane)),
        }
    }
    fn prune(&mut self) {
        if let Self::Split { a, b, .. } = self {
            a.prune();
            b.prune();
            let replacement = if matches!(a.as_ref(), Self::Group(g) if g.tabs.is_empty()) {
                Some(b.as_ref().clone())
            } else if matches!(b.as_ref(), Self::Group(g) if g.tabs.is_empty()) {
                Some(a.as_ref().clone())
            } else {
                None
            };
            if let Some(node) = replacement {
                *self = node;
            }
        }
    }
    pub fn dock(&mut self, pane: Id, target: Id, zone: Zone) -> Result<(), String> {
        let source = self.pane_group(pane).ok_or("Missing source pane")?;
        if self.group(target).is_none() {
            return Err("Missing target group".into());
        }
        if zone != Zone::Center && self.id_count() > 148 {
            return Err("Layout exceeds 150 nodes/panes".into());
        }
        if source == target
            && (zone == Zone::Center || self.group(source).is_some_and(|g| g.tabs.len() == 1))
        {
            return Ok(());
        }
        let next = self.max_id() + 1;
        let p = self.take(pane).ok_or("Missing source pane")?;
        if zone == Zone::Center {
            let g = self.group_mut(target).ok_or("Missing target")?;
            g.active = pane;
            g.tabs.push(p);
        } else {
            let node = self.find_mut(target).ok_or("Missing target")?;
            let old = node.clone();
            let new = Self::Group(Group {
                id: next,
                active: p.id,
                tabs: vec![p],
            });
            let (a, b) = if matches!(zone, Zone::Left | Zone::Top) {
                (new, old)
            } else {
                (old, new)
            };
            *node = Self::Split {
                id: next + 1,
                axis: if matches!(zone, Zone::Left | Zone::Right) {
                    Axis::X
                } else {
                    Axis::Y
                },
                ratio: 0.5,
                a: Box::new(a),
                b: Box::new(b),
            };
        }
        self.prune();
        Ok(())
    }
    pub fn close(&mut self, id: Id) {
        self.take(id);
        self.prune();
        if let Self::Group(g) = self
            && g.tabs.is_empty()
        {
            let id = g.id + 1;
            g.tabs.push(Pane {
                id,
                kind: PaneType::World,
                query: String::new(),
                collapsed: vec![],
                orbit: default_orbit(),
                center: default_center(),
            });
            g.active = id;
        }
    }
    pub fn add(&mut self, group: Id, kind: PaneType) -> Option<Id> {
        let id = self.max_id() + 1;
        if self.id_count() >= 150 {
            return None;
        }
        let g = self.group_mut(group)?;
        if g.tabs.len() >= 30 {
            return None;
        }
        g.tabs.push(Pane {
            id,
            kind,
            query: String::new(),
            collapsed: vec![],
            orbit: default_orbit(),
            center: default_center(),
        });
        g.active = id;
        Some(id)
    }
    fn id_count(&self) -> usize {
        match self {
            Self::Group(g) => g.tabs.len() + 1,
            Self::Split { a, b, .. } => 1 + a.id_count() + b.id_count(),
        }
    }
    pub fn max_id(&self) -> Id {
        match self {
            Self::Group(g) => g.tabs.iter().map(|p| p.id).chain([g.id]).max().unwrap_or(0),
            Self::Split { id, a, b, .. } => (*id).max(a.max_id()).max(b.max_id()),
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Layout {
    pub name: String,
    pub root: DockNode,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LayoutDocument {
    pub version: u32,
    pub active: usize,
    pub layouts: Vec<Layout>,
}
impl Default for LayoutDocument {
    fn default() -> Self {
        Self {
            version: 1,
            active: 0,
            layouts: ["Default", "Scene", "Assets"].map(preset).to_vec(),
        }
    }
}
impl LayoutDocument {
    pub fn root(&self) -> &DockNode {
        &self.layouts[self.active].root
    }
    pub fn root_mut(&mut self) -> &mut DockNode {
        &mut self.layouts[self.active].root
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.version != 1 {
            return Err("Unsupported layout version".into());
        }
        if self.layouts.is_empty() || self.layouts.len() > 20 || self.active >= self.layouts.len() {
            return Err("Invalid layout count or active layout".into());
        }
        for layout in &self.layouts {
            if layout.name.trim().is_empty() || layout.name.len() > 100 {
                return Err("Invalid layout name".into());
            }
            validate_node(&layout.root, &mut HashSet::new(), 0)?;
        }
        Ok(())
    }
    pub fn from_json(json: &str) -> Result<Self, String> {
        if json.len() > 1_048_576 {
            return Err("Layout exceeds 1 MiB".into());
        }
        let doc: Self = serde_json::from_str(json).map_err(|e| e.to_string())?;
        doc.validate()?;
        Ok(doc)
    }
}
fn validate_node(node: &DockNode, seen: &mut HashSet<Id>, depth: usize) -> Result<(), String> {
    if depth > 18
        || seen.len() >= 150
        || node.id() == 0
        || node.id() > 1_000_000_000
        || !seen.insert(node.id())
    {
        return Err("Invalid, duplicate or excessive node IDs".into());
    }
    match node {
        DockNode::Group(g) => {
            if g.tabs.is_empty() || g.tabs.len() > 30 || !g.tabs.iter().any(|p| p.id == g.active) {
                return Err("Empty group or invalid active tab".into());
            }
            for p in &g.tabs {
                if p.id == 0
                    || p.id > 1_000_000_000
                    || !seen.insert(p.id)
                    || seen.len() > 150
                    || p.query.len() > 200
                    || p.collapsed.len() > 500
                    || p.orbit
                        .iter()
                        .chain(p.center.iter())
                        .any(|v| !v.is_finite())
                    || !(0.05..=1.5).contains(&p.orbit[1])
                    || !(2.0..=80.0).contains(&p.orbit[2])
                {
                    return Err("Invalid pane data or duplicate ID".into());
                }
            }
        }
        DockNode::Split { ratio, a, b, .. } => {
            if !ratio.is_finite() || !(0.08..=0.92).contains(ratio) {
                return Err("Invalid split ratio".into());
            }
            validate_node(a, seen, depth + 1)?;
            validate_node(b, seen, depth + 1)?;
        }
    }
    Ok(())
}
pub fn preset(name: &str) -> Layout {
    struct Builder(Id);
    impl Builder {
        fn next(&mut self) -> Id {
            self.0 += 1;
            self.0
        }
        fn group(&mut self, kinds: &[PaneType]) -> DockNode {
            let id = self.next();
            let tabs: Vec<_> = kinds
                .iter()
                .map(|kind| Pane {
                    id: self.next(),
                    kind: *kind,
                    query: String::new(),
                    collapsed: vec![],
                    orbit: default_orbit(),
                    center: default_center(),
                })
                .collect();
            DockNode::Group(Group {
                id,
                active: tabs[0].id,
                tabs,
            })
        }
        fn split(&mut self, axis: Axis, ratio: f32, a: DockNode, b: DockNode) -> DockNode {
            DockNode::Split {
                id: self.next(),
                axis,
                ratio,
                a: Box::new(a),
                b: Box::new(b),
            }
        }
    }
    use PaneType::*;
    let mut b = Builder(0);
    let left = b.group(if name == "Assets" {
        &[AssetsTree, World]
    } else {
        &[World]
    });
    let root = if name == "Scene" {
        let center = b.group(&[Viewport, Camera]);
        let right = b.group(&[Inspector]);
        let rest = b.split(Axis::X, 0.77, center, right);
        b.split(Axis::X, 0.175, left, rest)
    } else if name == "Assets" {
        let center = b.group(&[AssetsGallery]);
        let top = b.group(&[Inspector]);
        let bottom = b.group(&[Viewport, Camera]);
        let right = b.split(Axis::Y, 0.55, top, bottom);
        let rest = b.split(Axis::X, 0.72, center, right);
        b.split(Axis::X, 0.185, left, rest)
    } else {
        let top = b.group(&[Viewport]);
        let bottom = b.group(&[AssetsGallery, AssetsTree]);
        let center = b.split(Axis::Y, 0.685, top, bottom);
        let top = b.group(&[Inspector]);
        let bottom = b.group(&[Camera]);
        let right = b.split(Axis::Y, 0.655, top, bottom);
        let rest = b.split(Axis::X, 0.72, center, right);
        b.split(Axis::X, 0.185, left, rest)
    };
    Layout {
        name: name.into(),
        root,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn panes(node: &DockNode) -> Vec<Id> {
        match node {
            DockNode::Group(g) => g.tabs.iter().map(|p| p.id).collect(),
            DockNode::Split { a, b, .. } => {
                let mut ids = panes(a);
                ids.extend(panes(b));
                ids
            }
        }
    }
    #[test]
    fn moving_last_source_tab_preserves_other_panes_and_prunes_empty_group() {
        for zone in [
            Zone::Center,
            Zone::Left,
            Zone::Right,
            Zone::Top,
            Zone::Bottom,
        ] {
            let mut doc = LayoutDocument::default();
            let before = panes(doc.root());
            let source = doc.root().first_group();
            let pane = doc.root().group(source).unwrap().active;
            let target = doc.root().pane_group(*before.last().unwrap()).unwrap();
            doc.root_mut().dock(pane, target, zone).unwrap();
            doc.validate().unwrap();
            let mut after = panes(doc.root());
            let mut expected = before;
            after.sort_unstable();
            expected.sort_unstable();
            assert_eq!(after, expected);
            assert!(doc.root().group(source).is_none());
            if zone == Zone::Center {
                assert_eq!(doc.root().pane_group(pane), Some(target));
            }
        }
    }
    #[test]
    fn failed_and_same_group_drops_do_not_change_layout() {
        let mut doc = LayoutDocument::default();
        let group = doc.root().first_group();
        let pane = doc.root().group(group).unwrap().active;
        let before = serde_json::to_string(&doc).unwrap();
        doc.root_mut().dock(pane, group, Zone::Center).unwrap();
        assert!(doc.root_mut().dock(pane, 9999, Zone::Left).is_err());
        assert_eq!(before, serde_json::to_string(&doc).unwrap());
    }
    #[test]
    fn edited_layout_roundtrips_and_last_tab_close_keeps_workspace() {
        let mut doc = LayoutDocument::default();
        let g = doc.root().first_group();
        let p = doc.root_mut().add(g, PaneType::Camera).unwrap();
        doc.root_mut().pane_mut(p).unwrap().query = "test".into();
        let json = serde_json::to_string(&doc).unwrap();
        assert_eq!(
            json,
            serde_json::to_string(&LayoutDocument::from_json(&json).unwrap()).unwrap()
        );
        for id in panes(doc.root()) {
            doc.root_mut().close(id);
        }
        doc.validate().unwrap();
        assert_eq!(panes(doc.root()).len(), 1);
    }
    #[test]
    fn malformed_documents_are_rejected_at_import_boundary() {
        let doc = LayoutDocument::default();
        let mut bad = doc.clone();
        bad.version = 2;
        assert!(bad.validate().is_err());
        let mut bad = doc.clone();
        bad.active = 100;
        assert!(bad.validate().is_err());
        let mut bad = doc.clone();
        let id = bad.root().first_group();
        let g = bad.root_mut().group_mut(id).unwrap();
        g.tabs[0].id = g.id;
        g.active = g.id;
        assert!(bad.validate().is_err());
        let mut bad = doc.clone();
        if let DockNode::Split { ratio, .. } = bad.root_mut() {
            *ratio = f32::NAN;
        }
        assert!(bad.validate().is_err());
        assert!(LayoutDocument::from_json("{}").is_err());
    }
}
