//! Retained Bevy UI, authored with composable BSN controls. Layout owns no ECS IDs.
use crate::{
    assets::AssetIndex,
    cursor::CursorRole,
    editor::*,
    icons::{self, Icon},
    layout::{Axis, *},
    scene::{self, EditorObject, Library, ModelKind, SceneView},
};
use bevy::{
    camera::{RenderTarget, visibility::RenderLayers},
    gizmos::transform_gizmo::{TransformGizmoMode, TransformGizmoSettings, TransformGizmoSpace},
    input_focus::tab_navigation::{TabGroup as FocusTabGroup, TabIndex},
    picking::{Pickable as PickingBehavior, hover::Hovered},
    prelude::*,
    render::render_resource::TextureFormat,
    text::{EditableText, FontSource, LineHeight, TextCursorStyle},
    ui::widget::ViewportNode,
    ui_widgets::{Button, ControlOrientation, Scrollbar, ScrollbarThumb, TextInput},
};

pub const BG: Color = Color::srgb_u8(20, 20, 20);
pub const PANEL: Color = Color::srgb_u8(32, 32, 32);
pub const MUTED: Color = Color::srgb_u8(156, 156, 156);
pub const INK: Color = Color::srgb_u8(229, 229, 229);
pub const LINE: Color = Color::srgb_u8(54, 54, 54);
pub const ACCENT: Color = Color::srgb_u8(214, 214, 214);
// Dense desktop rhythm: micro 4, control 8, panel 12, hierarchy 16 logical px.
const MICRO_GAP: f32 = 4.;
const CONTROL_GAP: f32 = 8.;
const PANEL_INSET: f32 = 12.;
const TREE_INDENT: f32 = 16.;
const TREE_DISCLOSURE_WIDTH: f32 = 16.;
const INPUT_HEIGHT: f32 = 26.;
const INPUT_LINE_HEIGHT: f32 = 14.;
const INPUT_PADDING_X: f32 = 8.;
// Center the single-line text box inside the 1px border, at every window DPI.
const INPUT_PADDING_Y: f32 = (INPUT_HEIGHT - INPUT_LINE_HEIGHT - 2.) * 0.5;
const INPUT_BORDER: Color = Color::srgb_u8(72, 72, 72);
pub const TAB_STRIP_HEIGHT: f32 = 34.;
pub const PANE_BORDER_WIDTH: f32 = 1.;
const COMPACT_WORKSPACE_WIDTH: f32 = 900.;

fn header_height(width: f32) -> f32 {
    if width < 640. { 76. } else { 40. }
}
pub(crate) fn workspace_top(width: f32) -> f32 {
    header_height(width)
        + if width < COMPACT_WORKSPACE_WIDTH {
            32.
        } else {
            0.
        }
}
pub(crate) fn workspace_size(window: &Window) -> Vec2 {
    Vec2::new(
        (window.width() - 10.).max(0.),
        (window.height() - workspace_top(window.width()) - 36.).max(0.),
    )
}
pub(crate) fn visible_group(workspace: &EditorWindow, width: f32) -> Option<Id> {
    workspace.maximized.or_else(|| {
        (width < COMPACT_WORKSPACE_WIDTH).then(|| {
            if workspace.document.root().group(workspace.focused).is_some() {
                workspace.focused
            } else {
                workspace.document.root().first_group()
            }
        })
    })
}

#[derive(Component)]
pub(crate) struct ShellWidth(pub f32);

#[derive(PartialEq, Eq)]
enum ShellPart {
    Split(Id, Axis),
    Group(Id, Id, Vec<(Id, PaneType)>),
}
#[derive(Component, PartialEq, Eq)]
struct RenderedShell {
    layout: usize,
    names: Vec<String>,
    visible: Option<Id>,
    responsive: [bool; 4],
    parts: Vec<ShellPart>,
}
impl RenderedShell {
    fn capture(workspace: &EditorWindow, width: f32) -> Self {
        fn walk(node: &DockNode, parts: &mut Vec<ShellPart>) {
            match node {
                DockNode::Group(group) => parts.push(ShellPart::Group(
                    group.id,
                    group.active,
                    group.tabs.iter().map(|p| (p.id, p.kind)).collect(),
                )),
                DockNode::Split { id, axis, a, b, .. } => {
                    // Ratios only change geometry; orbit/center only change live cameras.
                    parts.push(ShellPart::Split(*id, *axis));
                    walk(a, parts);
                    walk(b, parts);
                }
            }
        }
        let mut parts = Vec::new();
        walk(workspace.document.root(), &mut parts);
        Self {
            layout: workspace.document.active,
            names: workspace
                .document
                .layouts
                .iter()
                .map(|l| l.name.clone())
                .collect(),
            visible: visible_group(workspace, width),
            responsive: [width < 640., width < 900., width >= 1100., width > 1200.],
            parts,
        }
    }
}
#[derive(Component)]
struct RenderedMenu {
    menu: Option<Menu>,
    width: f32,
}
#[derive(Component)]
struct EditorPopup(Entity);
#[derive(Component)]
struct StatusMessage(Entity);
#[derive(Component)]
struct StatusText(Entity);
#[derive(Component)]
struct EntityName(Entity);
#[derive(Component, Clone)]
struct PaneBody {
    window: Entity,
    layout: usize,
    pane: Id,
    kind: PaneType,
    query: String,
    collapsed: Vec<Id>,
    selected: Option<Entity>,
}
impl PaneBody {
    fn capture(window: Entity, layout: usize, pane: &Pane, selected: Option<Entity>) -> Self {
        Self {
            window,
            layout,
            pane: pane.id,
            kind: pane.kind,
            query: pane.query.clone(),
            collapsed: pane.collapsed.clone(),
            selected,
        }
    }
}
#[derive(Resource, Default)]
struct RenderedContent {
    objects: Vec<TreeObject>,
    assets: Vec<crate::assets::AssetEntry>,
}
#[derive(Clone, Copy)]
pub(crate) struct ContentChanges {
    tree: bool,
    names: bool,
    assets: bool,
}
/// Shared content comparison; a dirty notification is not a request to recreate all UI.
pub(crate) fn content_changes(world: &mut World) -> ContentChanges {
    let objects = tree_objects(world);
    let assets = world.resource::<AssetIndex>().entries.clone();
    let previous = world.get_resource::<RenderedContent>();
    let changed = previous.map_or(
        ContentChanges {
            tree: true,
            names: true,
            assets: true,
        },
        |p| ContentChanges {
            tree: p.objects.len() != objects.len()
                || p.objects.iter().zip(&objects).any(|(a, b)| {
                    (a.entity, a.key, a.kind, a.parent) != (b.entity, b.key, b.kind, b.parent)
                }),
            names: p.objects != objects,
            assets: p.assets != assets,
        },
    );
    world.insert_resource(RenderedContent { objects, assets });
    changed
}
#[derive(Component)]
struct Tooltip(String);
#[derive(Component)]
struct PopupAnchor {
    window: Entity,
    menu: Menu,
}
#[derive(Resource, Default)]
pub(crate) struct TooltipState {
    target: Option<Entity>,
    message: Option<String>,
    since: f64,
    warm_until: f64,
    popup: Option<Entity>,
}

fn action_hint(action: &Action) -> Option<&'static str> {
    match action {
        Action::Close(_) => Some("Close tab"),
        Action::CopyLayout => Some("Copy current layout"),
        Action::Help => Some("Interaction help"),
        Action::GizmoMode(TransformGizmoMode::Translate) => {
            Some("Move along an axis or in the view plane")
        }
        Action::GizmoMode(TransformGizmoMode::Rotate) => Some("Rotate around an axis"),
        Action::GizmoMode(TransformGizmoMode::Scale) => {
            Some("Scale along a local axis; center scales uniformly")
        }
        Action::GizmoSpace => Some("Toggle world / local axes (scale always uses local axes)"),
        Action::Visible(_) => Some("Toggle entity visibility"),
        Action::RefreshAssets => Some("Refresh local assets"),
        Action::Maximize(_) => Some("Maximize / restore pane"),
        Action::Menu(Menu::PaneType { pane: Some(_), .. }) => Some("Change pane type"),
        Action::Menu(Menu::PaneType { pane: None, .. }) => Some("Add pane"),
        Action::Menu(Menu::Group(_)) => Some("Pane options: split, close and restore"),
        _ => None,
    }
}
#[derive(Component, Clone, PartialEq, Eq)]
pub(crate) struct PaneScroll {
    window: Entity,
    layout: usize,
    pane: Id,
}
#[derive(Component)]
pub struct FieldAxis(pub usize);
#[derive(Component)]
pub struct PlaceholderOf(Entity);
/// Stable viewport identity across shell refreshes; Pane IDs are only layout-local.
#[derive(Component, PartialEq, Eq)]
pub(crate) struct ViewportSurface {
    pub window: Entity,
    layout: usize,
    pane: Id,
    preview: bool,
}
pub fn placeholders(
    inputs: Query<&EditableText>,
    mut hints: Query<(&PlaceholderOf, &mut Visibility)>,
) {
    for (owner, mut visibility) in &mut hints {
        *visibility = if inputs
            .get(owner.0)
            .is_ok_and(|input| input.value().to_string().is_empty())
        {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
}

fn label_scene(value: &str, size: f32, color: Color) -> impl Scene {
    bsn! {Text(value) TextFont { font: bevy::text::FontSourceTemplate::Family("Segoe UI"), font_size: {FontSize::Px(size)}, } TextColor(color) TextLayout::no_wrap() PickingBehavior::IGNORE}
}
fn node(world: &mut World, parent: Entity, style: Node, color: Color) -> Entity {
    let e = world
        .spawn_scene(bsn! {style BackgroundColor(color)})
        .expect("valid inline BSN node")
        .id();
    world.entity_mut(e).insert(ChildOf(parent));
    e
}
fn label(world: &mut World, parent: Entity, value: &str, size: f32, color: Color) -> Entity {
    let e = world
        .spawn_scene(label_scene(value, size, color))
        .expect("valid inline BSN label")
        .id();
    world.entity_mut(e).insert(ChildOf(parent));
    e
}
fn wrapped_label(
    world: &mut World,
    parent: Entity,
    value: &str,
    size: f32,
    color: Color,
) -> Entity {
    let text = label(world, parent, value, size, color);
    world.entity_mut(text).insert((
        TextLayout::linebreak(bevy::text::LineBreak::WordOrCharacter),
        Node {
            width: percent(100),
            min_width: px(0),
            flex_shrink: 0.,
            ..default()
        },
    ));
    text
}
fn icon_label(
    world: &mut World,
    parent: Entity,
    symbols: &[Icon],
    value: &str,
    size: f32,
    color: Color,
) -> Entity {
    let holder = node(
        world,
        parent,
        Node {
            align_items: AlignItems::Center,
            column_gap: px(6),
            flex_shrink: 0.,
            min_width: px(0),
            ..default()
        },
        Color::NONE,
    );
    world.entity_mut(holder).insert(PickingBehavior::IGNORE);
    for &symbol in symbols {
        icons::spawn(world, holder, symbol, size.max(12.), color);
    }
    if !value.is_empty() {
        // Text is never interpreted as an icon token, including user names/paths.
        label(world, holder, value, size, color);
    }
    holder
}
fn pane_icon(kind: PaneType) -> Icon {
    match kind {
        PaneType::World => Icon::World,
        PaneType::Inspector => Icon::Inspector,
        PaneType::AssetsTree => Icon::Folder,
        PaneType::AssetsGallery => Icon::Gallery,
        PaneType::Viewport => Icon::Viewport,
        PaneType::Camera => Icon::Camera,
    }
}
fn model_icon(kind: ModelKind) -> Icon {
    match kind {
        ModelKind::Group => Icon::Folder,
        ModelKind::Camera => Icon::Camera,
        ModelKind::Light => Icon::Sun,
        ModelKind::Tree | ModelKind::Cypress => Icon::Trees,
        ModelKind::Orb => Icon::Orbit,
        _ => Icon::Box,
    }
}
fn file_icon(path: &std::path::Path) -> Icon {
    match path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "png" | "jpg" | "jpeg" | "svg" | "webp" | "gif" | "bmp" => Icon::FileImage,
        "txt" | "md" | "log" | "csv" => Icon::FileText,
        "rs" | "json" | "toml" | "yaml" | "yml" | "ron" | "wgsl" => Icon::FileCode,
        _ => Icon::File,
    }
}
fn button(
    world: &mut World,
    parent: Entity,
    window: Entity,
    title: &str,
    action: Action,
    style: Node,
    color: Color,
) -> Entity {
    let e = world
        .spawn_scene(bsn! {Button Hovered style BackgroundColor(color)})
        .expect("valid inline BSN button")
        .id();
    if !title.is_empty() {
        label(world, e, title, 11., INK);
    }
    world.entity_mut(e).insert((
        ChildOf(parent),
        UiAction { window, action },
        BaseColor(color),
        CursorRole::Click,
    ));
    e
}
fn compact(
    world: &mut World,
    parent: Entity,
    window: Entity,
    title: &str,
    action: Action,
) -> Entity {
    button(
        world,
        parent,
        window,
        title,
        action,
        Node {
            height: px(24),
            padding: UiRect::horizontal(px(CONTROL_GAP)),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            border_radius: BorderRadius::all(px(4)),
            flex_shrink: 0.,
            ..default()
        },
        Color::NONE,
    )
}
fn compact_icon(
    world: &mut World,
    parent: Entity,
    window: Entity,
    symbol: Icon,
    action: Action,
) -> Entity {
    let hint = action_hint(&action);
    let button = compact(world, parent, window, "", action);
    if let Some(hint) = hint {
        world.entity_mut(button).insert(Tooltip(hint.into()));
    }
    world.entity_mut(button).insert(Node {
        width: px(24),
        height: px(24),
        min_width: px(24),
        align_items: AlignItems::Center,
        justify_content: JustifyContent::Center,
        border_radius: BorderRadius::all(px(4)),
        flex_shrink: 0.,
        ..default()
    });
    icons::spawn(world, button, symbol, 12., INK);
    button
}
fn row_style(height: f32) -> Node {
    Node {
        width: percent(100),
        height: px(height),
        min_height: px(height),
        align_items: AlignItems::Center,
        column_gap: px(CONTROL_GAP),
        padding: UiRect::horizontal(px(PANEL_INSET)),
        flex_shrink: 0.,
        ..default()
    }
}
fn column() -> Node {
    Node {
        flex_direction: FlexDirection::Column,
        min_width: px(0),
        min_height: px(0),
        ..default()
    }
}
fn spacer(world: &mut World, parent: Entity) {
    node(
        world,
        parent,
        Node {
            flex_grow: 1.,
            ..default()
        },
        Color::NONE,
    );
}
fn scroll_area(world: &mut World, parent: Entity) -> Entity {
    let holder = node(
        world,
        parent,
        Node {
            flex_grow: 1.,
            width: percent(100),
            min_height: px(0),
            ..default()
        },
        PANEL,
    );
    let e = node(
        world,
        holder,
        Node {
            flex_grow: 1.,
            overflow: Overflow::scroll_y(),
            ..column()
        },
        PANEL,
    );
    world.entity_mut(e).insert(ScrollPosition::default());
    let mut ancestor = parent;
    loop {
        if let Some(region) = world.get::<DockRegion>(ancestor) {
            let ws = world.get::<EditorWindow>(region.window).unwrap();
            let identity = PaneScroll {
                window: region.window,
                layout: ws.document.active,
                pane: ws.document.root().group(region.id).unwrap().active,
            };
            world.entity_mut(e).insert(identity);
            break;
        }
        let Some(parent) = world.get::<ChildOf>(ancestor) else {
            break;
        };
        ancestor = parent.parent();
    }
    let track = node(
        world,
        holder,
        Node {
            width: px(8),
            flex_shrink: 0.,
            ..default()
        },
        PANEL,
    );
    world.entity_mut(track).insert((
        Scrollbar::new(e, ControlOrientation::Vertical, 24.),
        CursorRole::Click,
    ));
    world.spawn((
        ScrollbarThumb {
            border_radius: BorderRadius::all(px(4)),
            ..default()
        },
        BackgroundColor(Color::srgb_u8(102, 102, 102)),
        CursorRole::Click,
        ChildOf(track),
    ));
    e
}
pub(crate) fn scrollbar_visibility(
    targets: Query<&ComputedNode>,
    mut tracks: Query<(&Scrollbar, &mut Node)>,
) {
    for (bar, mut style) in &mut tracks {
        let overflow = targets
            .get(bar.target)
            .is_ok_and(|n| n.content_size().y > n.size().y + 1.);
        let display = if overflow {
            Display::Flex
        } else {
            Display::None
        };
        if style.display != display {
            style.display = display;
        }
    }
}
fn input(world: &mut World, parent: Entity, binding: InputBinding, width: Val) -> Entity {
    world
        .spawn((
            Node {
                height: px(INPUT_HEIGHT),
                width,
                min_width: px(30),
                flex_grow: 1.,
                padding: UiRect::axes(px(INPUT_PADDING_X), px(INPUT_PADDING_Y)),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(4)),
                overflow: Overflow::clip(),
                ..default()
            },
            TextInput,
            TabIndex(0),
            CursorRole::Text,
            EditableText::new(&binding.committed),
            TextFont {
                font: FontSource::family("Segoe UI"),
                font_size: FontSize::Px(10.),
                ..default()
            },
            TextColor(INK),
            TextLayout::no_wrap(),
            LineHeight::Px(INPUT_LINE_HEIGHT),
            TextCursorStyle {
                color: INK,
                unfocused_selection_color: Color::NONE,
                ..default()
            },
            BackgroundColor(Color::srgb_u8(24, 24, 24)),
            BorderColor::all(INPUT_BORDER),
            binding,
            ChildOf(parent),
        ))
        .id()
}
fn search(world: &mut World, parent: Entity, window: Entity, pane: &Pane, placeholder: &str) {
    let layout = world.get::<EditorWindow>(window).unwrap().document.active;
    icons::spawn(world, parent, Icon::Search, 12., MUTED);
    let e = input(
        world,
        parent,
        InputBinding {
            window,
            layout,
            pane: pane.id,
            entity: None,
            field: Field::Query(pane.id),
            committed: pane.query.clone(),
        },
        px(0),
    );
    world
        .entity_mut(e)
        .insert(Tooltip("Enter or click elsewhere to apply search".into()));
    let hint = label(world, e, placeholder, 10., MUTED);
    world.entity_mut(hint).insert((
        PlaceholderOf(e),
        LineHeight::Px(INPUT_LINE_HEIGHT),
        Node {
            position_type: PositionType::Absolute,
            left: px(INPUT_PADDING_X),
            top: px(INPUT_PADDING_Y),
            ..default()
        },
    ));
}

fn dismiss_menu(
    click: On<PointerClick>,
    roots: Query<&UiOwned>,
    actions: Query<&UiAction>,
    parents: Query<&ChildOf>,
    mut commands: Commands,
) {
    if click.button != PointerButton::Primary {
        return;
    }
    let Ok(owner) = roots.get(click.entity) else {
        return;
    };
    let window = owner.0;
    let mut target = click.original_event_target();
    loop {
        // Actions (including draggable tabs) already close or replace the menu in dispatch.
        if actions.contains(target) {
            return;
        }
        let Ok(parent) = parents.get(target) else {
            break;
        };
        target = parent.parent();
    }
    // Dismiss on click, not press: rebuilding during press would destroy the clicked
    // control before it can receive release/activation. No full-window picking shield.
    commands.queue(move |world: &mut World| {
        if world
            .get::<EditorWindow>(window)
            .is_some_and(|ws| ws.menu.is_some())
        {
            dispatch(world, window, Action::Dismiss);
        }
    });
}

pub fn render_window(world: &mut World, window: Entity, workspace: &EditorWindow) -> Entity {
    let width = world.get::<Window>(window).map_or(1440., Window::width);
    let root=world.spawn_scene(bsn! {Node{width:percent(100),height:percent(100),flex_direction:FlexDirection::Column,overflow:Overflow::clip(),} BackgroundColor(BG)}).unwrap().id();
    world
        .entity_mut(root)
        .insert((
            UiTargetCamera(workspace.camera),
            UiOwned(window),
            ShellWidth(width),
            RenderedShell::capture(workspace, width),
            RenderedMenu {
                menu: workspace.menu.clone(),
                width,
            },
            FocusTabGroup::new(0),
        ))
        .observe(dismiss_menu);
    header(world, root, window, workspace, width);
    if width < COMPACT_WORKSPACE_WIDTH {
        workspace_switcher(world, root, window, workspace);
    }
    let area = node(
        world,
        root,
        Node {
            flex_grow: 1.,
            width: percent(100),
            min_height: px(0),
            padding: UiRect::all(px(5)),
            ..default()
        },
        BG,
    );
    let available = workspace_size(world.get::<Window>(window).unwrap());
    for (id, rect, is_splitter) in dock_rects(
        workspace.document.root(),
        Rect::from_corners(Vec2::ZERO, available),
        visible_group(workspace, width),
    ) {
        if is_splitter {
            let Some(DockNode::Split { axis, .. }) = workspace.document.root().find(id) else {
                continue;
            };
            let e = node(world, area, rect_node(rect), Color::NONE);
            world.entity_mut(e).insert((
                DockRegion {
                    window,
                    id,
                    splitter: true,
                },
                DragHandle::Splitter {
                    window,
                    id,
                    axis: *axis,
                },
                CursorRole::Resize(*axis),
                Hovered::default(),
                BaseColor(Color::NONE),
            ));
        } else if let Some(group) = workspace.document.root().group(id) {
            render_group(world, area, window, workspace, group, rect);
        }
    }
    let preview = node(
        world,
        area,
        Node {
            position_type: PositionType::Absolute,
            display: Display::None,
            border: UiRect::all(px(2)),
            border_radius: BorderRadius::all(px(5)),
            ..default()
        },
        Color::srgba(0.85, 0.85, 0.85, 0.12),
    );
    world.entity_mut(preview).insert((
        DropPreview(window),
        BorderColor::all(ACCENT),
        PickingBehavior::IGNORE,
        GlobalZIndex(50),
    ));
    let status = node(
        world,
        root,
        Node {
            overflow: Overflow::clip(),
            ..row_style(26.)
        },
        Color::srgb_u8(33, 33, 33),
    );
    icons::spawn(world, status, Icon::Ready, 12., MUTED);
    let message = node(
        world,
        status,
        Node {
            flex_grow: 1.,
            min_width: px(0),
            overflow: Overflow::scroll_x(),
            ..default()
        },
        Color::NONE,
    );
    world.entity_mut(message).insert((
        ScrollPosition::default(),
        Tooltip(workspace.status.clone()),
        StatusMessage(window),
    ));
    let text = label(world, message, &workspace.status, 10., INK);
    world.entity_mut(text).insert(StatusText(window));
    if width >= 1100. {
        label(world, status, "SESSION ONLY · BEVY 0.20 RC", 9., MUTED);
    }
    if let Some(menu) = &workspace.menu {
        render_menu(world, root, window, workspace, menu);
    }
    root
}
fn header(world: &mut World, parent: Entity, window: Entity, workspace: &EditorWindow, width: f32) {
    let narrow = width < 640.;
    let h = node(
        world,
        parent,
        Node {
            height: px(header_height(width)),
            min_height: px(header_height(width)),
            padding: UiRect::horizontal(px(PANEL_INSET)),
            flex_direction: if narrow {
                FlexDirection::Column
            } else {
                FlexDirection::Row
            },
            align_items: AlignItems::Center,
            column_gap: px(CONTROL_GAP),
            flex_shrink: 0.,
            ..default()
        },
        Color::srgb_u8(27, 27, 27),
    );
    let menus = node(
        world,
        h,
        Node {
            height: px(36),
            min_height: px(36),
            width: if narrow { percent(100) } else { auto() },
            column_gap: px(6),
            align_items: AlignItems::Center,
            flex_shrink: 0.,
            ..default()
        },
        Color::NONE,
    );
    for name in ["File", "Edit", "Add", "Window"] {
        compact(world, menus, window, name, Action::Menu(Menu::Header(name)));
    }
    if narrow {
        spacer(world, menus);
    }
    compact_icon(world, menus, window, Icon::Help, Action::Help);
    if !narrow {
        spacer(world, h);
    }
    let layouts = node(
        world,
        h,
        Node {
            height: px(36),
            min_height: px(36),
            min_width: px(0),
            width: if narrow { percent(100) } else { auto() },
            align_items: AlignItems::Center,
            column_gap: px(8),
            ..default()
        },
        Color::NONE,
    );
    if narrow || width > 1200. {
        label(world, layouts, "LAYOUT", 9., MUTED);
    }
    let segment = node(
        world,
        layouts,
        Node {
            padding: UiRect::all(px(2)),
            column_gap: px(2),
            border: UiRect::all(px(1)),
            border_radius: BorderRadius::all(px(5)),
            overflow: Overflow::scroll_x(),
            min_width: px(0),
            ..default()
        },
        Color::srgb_u8(21, 21, 21),
    );
    world
        .entity_mut(segment)
        .insert((BorderColor::all(LINE), ScrollPosition::default()));
    for (i, layout) in workspace.document.layouts.iter().enumerate() {
        button(
            world,
            segment,
            window,
            &layout.name,
            Action::Layout(i),
            Node {
                height: px(24),
                padding: UiRect::horizontal(px(10)),
                align_items: AlignItems::Center,
                border_radius: BorderRadius::all(px(3)),
                flex_shrink: 0.,
                ..default()
            },
            if i == workspace.document.active {
                Color::srgb_u8(56, 56, 56)
            } else {
                Color::NONE
            },
        );
    }
    compact_icon(world, layouts, window, Icon::Plus, Action::CopyLayout);
}
fn workspace_switcher(world: &mut World, parent: Entity, window: Entity, workspace: &EditorWindow) {
    let strip = node(
        world,
        parent,
        Node {
            column_gap: px(MICRO_GAP),
            overflow: Overflow::scroll_x(),
            ..row_style(32.)
        },
        BG,
    );
    world.entity_mut(strip).insert(ScrollPosition::default());
    label(world, strip, "PANES", 9., MUTED);
    let groups = dock_rects(
        workspace.document.root(),
        Rect::from_corners(Vec2::ZERO, Vec2::ONE),
        None,
    );
    let selected = visible_group(workspace, 0.);
    for (id, _, splitter) in groups {
        if splitter {
            continue;
        }
        let group = workspace.document.root().group(id).unwrap();
        let pane = group.tabs.iter().find(|p| p.id == group.active).unwrap();
        let item = compact(
            world,
            strip,
            window,
            pane.kind.title(),
            Action::FocusGroup(id),
        );
        if selected == Some(id) {
            let color = Color::srgb_u8(56, 56, 56);
            world
                .entity_mut(item)
                .insert((BackgroundColor(color), BaseColor(color)));
        }
    }
}
fn render_group(
    world: &mut World,
    parent: Entity,
    window: Entity,
    workspace: &EditorWindow,
    group: &Group,
    rect: Rect,
) {
    let container = node(
        world,
        parent,
        Node {
            border: UiRect::all(px(PANE_BORDER_WIDTH)),
            border_radius: BorderRadius::all(px(5)),
            overflow: Overflow::clip(),
            ..rect_node(rect)
        },
        PANEL,
    );
    world.entity_mut(container).insert((
        DockRegion {
            window,
            id: group.id,
            splitter: false,
        },
        BorderColor::all(if workspace.focused == group.id {
            Color::srgb_u8(85, 85, 85)
        } else {
            Color::srgb_u8(61, 61, 61)
        }),
    ));
    let strip = node(
        world,
        container,
        Node {
            height: px(TAB_STRIP_HEIGHT),
            min_height: px(TAB_STRIP_HEIGHT),
            padding: UiRect::right(px(MICRO_GAP)),
            align_items: AlignItems::Center,
            overflow: Overflow::clip(),
            ..default()
        },
        Color::srgb_u8(31, 31, 31),
    );
    world.entity_mut(strip).insert(CursorRole::Click);
    let tabs = node(
        world,
        strip,
        Node {
            height: percent(100),
            flex_grow: 1.,
            min_width: px(0),
            overflow: Overflow::scroll_x(),
            ..default()
        },
        Color::NONE,
    );
    world.entity_mut(tabs).insert(ScrollPosition::default());
    for pane in &group.tabs {
        let active = group.active == pane.id;
        let tab = button(
            world,
            tabs,
            window,
            "",
            Action::Tab {
                group: group.id,
                pane: pane.id,
            },
            Node {
                height: percent(100),
                padding: UiRect::horizontal(px(6)),
                column_gap: px(0),
                align_items: AlignItems::Center,
                flex_shrink: 0.,
                border: UiRect {
                    top: px(2),
                    right: px(1),
                    ..default()
                },
                ..default()
            },
            if active { PANEL } else { Color::NONE },
        );
        icon_label(
            world,
            tab,
            &[pane_icon(pane.kind)],
            pane.kind.title(),
            11.,
            INK,
        );
        // A draggable tab is a pointer target, not a native Button: release-time
        // Button activation would destroy its captured entity before DragEnd.
        world.entity_mut(tab).remove::<Button>().insert((
            DragHandle::Tab {
                window,
                pane: pane.id,
            },
            CursorRole::Tab,
            BorderColor {
                top: if active { ACCENT } else { Color::NONE },
                right: LINE,
                ..default()
            },
        ));
        compact_icon(
            world,
            tab,
            window,
            Icon::ChevronDown,
            Action::Menu(Menu::PaneType {
                group: group.id,
                pane: Some(pane.id),
            }),
        );
        compact_icon(world, tab, window, Icon::Close, Action::Close(pane.id));
    }
    compact_icon(
        world,
        strip,
        window,
        Icon::Plus,
        Action::Menu(Menu::PaneType {
            group: group.id,
            pane: None,
        }),
    );
    compact_icon(
        world,
        strip,
        window,
        if workspace.maximized == Some(group.id) {
            Icon::Minimize
        } else {
            Icon::Maximize
        },
        Action::Maximize(group.id),
    );
    compact_icon(
        world,
        strip,
        window,
        Icon::More,
        Action::Menu(Menu::Group(group.id)),
    );
    let body = node(
        world,
        container,
        Node {
            flex_grow: 1.,
            width: percent(100),
            border: UiRect::top(px(PANE_BORDER_WIDTH)),
            overflow: Overflow::clip(),
            ..column()
        },
        PANEL,
    );
    world.entity_mut(body).insert(BorderColor::all(LINE));
    if let Some(pane) = group.tabs.iter().find(|p| p.id == group.active) {
        let state = PaneBody::capture(
            window,
            workspace.document.active,
            pane,
            world.resource::<Selection>().0,
        );
        world.entity_mut(body).insert(state);
        match pane.kind {
            PaneType::World => world_pane(world, body, window, pane),
            PaneType::Inspector => inspector(world, body, window, pane.id),
            PaneType::AssetsTree => assets_tree(world, body, window, pane),
            PaneType::AssetsGallery => gallery(world, body, window, pane),
            PaneType::Viewport | PaneType::Camera => viewport(world, body, window, pane),
        }
    }
}
#[derive(Clone, PartialEq)]
struct TreeObject {
    entity: Entity,
    key: u64,
    name: String,
    kind: ModelKind,
    parent: Option<Entity>,
}
fn world_pane(world: &mut World, parent: Entity, window: Entity, pane: &Pane) {
    let toolbar = node(world, parent, row_style(36.), PANEL);
    search(world, toolbar, window, pane, "Search entities");
    let scroll = scroll_area(world, parent);
    world_contents(world, scroll, window, pane);
}
fn tree_objects(world: &mut World) -> Vec<TreeObject> {
    let mut objects: Vec<_> = world
        .query::<(Entity, &EditorObject, &Name, Option<&ChildOf>)>()
        .iter(world)
        .map(|(e, o, n, p)| TreeObject {
            entity: e,
            key: o.key,
            name: n.as_str().into(),
            kind: o.kind,
            parent: p.map(ChildOf::parent),
        })
        .collect();
    objects.sort_by_key(|object| object.entity);
    objects
}
fn world_contents(world: &mut World, scroll: Entity, window: Entity, pane: &Pane) {
    let entities = tree_objects(world);
    let root = node(
        world,
        scroll,
        Node {
            column_gap: px(MICRO_GAP),
            border: UiRect::left(px(2)),
            padding: UiRect {
                left: px(MICRO_GAP),
                right: px(PANEL_INSET),
                ..default()
            },
            ..row_style(26.)
        },
        PANEL,
    );
    world.entity_mut(root).insert(BorderColor::all(Color::NONE));
    let disclosure = node(
        world,
        root,
        Node {
            width: px(TREE_DISCLOSURE_WIDTH),
            min_width: px(TREE_DISCLOSURE_WIDTH),
            height: px(24),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            ..default()
        },
        Color::NONE,
    );
    icons::spawn(world, disclosure, Icon::ChevronDown, 12., INK);
    let content = tree_row_content(world, root);
    icons::spawn(world, content, Icon::Globe, 12., INK);
    label(world, content, "Quiet Courtyard", 11., INK);
    let mut roots: Vec<_> = entities.iter().filter(|o| o.parent.is_none()).collect();
    roots.sort_by_key(|o| o.key);
    for object in roots {
        tree_object(world, scroll, window, pane, object, &entities, 0);
    }
}
// Keep disclosure-to-icon spacing independent of the icon-to-label spacing.
fn tree_row_content(world: &mut World, row: Entity) -> Entity {
    node(
        world,
        row,
        Node {
            flex_grow: 1.,
            min_width: px(0),
            align_items: AlignItems::Center,
            column_gap: px(CONTROL_GAP),
            ..default()
        },
        Color::NONE,
    )
}
fn tree_object(
    world: &mut World,
    parent: Entity,
    window: Entity,
    pane: &Pane,
    object: &TreeObject,
    all: &[TreeObject],
    depth: usize,
) {
    let mut children: Vec<_> = all
        .iter()
        .filter(|o| o.parent == Some(object.entity))
        .collect();
    children.sort_by_key(|o| o.key);
    let matches = pane.query.is_empty()
        || object
            .name
            .to_lowercase()
            .contains(&pane.query.to_lowercase())
        || children
            .iter()
            .any(|c| c.name.to_lowercase().contains(&pane.query.to_lowercase()));
    if !matches {
        return;
    }
    let selected = world.resource::<Selection>().0 == Some(object.entity);
    let closed = pane.collapsed.contains(&object.key);
    let row = button(
        world,
        parent,
        window,
        "",
        Action::Select(object.entity),
        Node {
            padding: UiRect {
                left: px(MICRO_GAP + depth as f32 * TREE_INDENT),
                right: px(PANEL_INSET),
                ..default()
            },
            border: UiRect::left(px(2)),
            column_gap: px(MICRO_GAP),
            ..row_style(24.)
        },
        if selected {
            Color::srgb_u8(56, 56, 56)
        } else {
            Color::NONE
        },
    );
    world.entity_mut(row).insert(BorderColor::all(if selected {
        ACCENT
    } else {
        Color::NONE
    }));
    if !children.is_empty() {
        let disclosure = compact_icon(
            world,
            row,
            window,
            if closed {
                Icon::ChevronRight
            } else {
                Icon::ChevronDown
            },
            Action::Collapse {
                pane: pane.id,
                key: object.key,
            },
        );
        // Native tree disclosures use a narrower slot than standalone icon buttons.
        // Otherwise the empty prefix reads as another layer of left padding.
        let mut style = world.get_mut::<Node>(disclosure).unwrap();
        style.width = px(TREE_DISCLOSURE_WIDTH);
        style.min_width = px(TREE_DISCLOSURE_WIDTH);
    } else {
        node(
            world,
            row,
            Node {
                width: px(TREE_DISCLOSURE_WIDTH),
                min_width: px(TREE_DISCLOSURE_WIDTH),
                flex_shrink: 0.,
                ..default()
            },
            Color::NONE,
        );
    }
    let content = tree_row_content(world, row);
    icons::spawn(world, content, model_icon(object.kind), 12., MUTED);
    let text = label(
        world,
        content,
        &object.name,
        11.,
        if selected { INK } else { MUTED },
    );
    world.entity_mut(text).insert(EntityName(object.entity));
    if !closed || !pane.query.is_empty() {
        for child in children {
            // Group rows are section headings: direct entities share the folder's
            // icon column. Nested groups and ordinary entity hierarchies still indent.
            let child_depth = if object.kind == ModelKind::Group && child.kind != ModelKind::Group {
                depth
            } else {
                depth + 1
            };
            tree_object(world, parent, window, pane, child, all, child_depth);
        }
    }
}
fn inspector(world: &mut World, parent: Entity, window: Entity, pane: Id) {
    let scroll = scroll_area(world, parent);
    inspector_contents(world, scroll, window, pane);
}
fn inspector_contents(world: &mut World, scroll: Entity, window: Entity, pane: Id) {
    let layout = world.get::<EditorWindow>(window).unwrap().document.active;
    let Some(entity) = world.resource::<Selection>().0 else {
        let message = node(
            world,
            scroll,
            Node {
                padding: UiRect::all(px(16)),
                row_gap: px(CONTROL_GAP),
                ..column()
            },
            Color::NONE,
        );
        wrapped_label(world, message, "No entity selected", 12., INK);
        wrapped_label(
            world,
            message,
            "Select an entity in World or Viewport to edit its properties.",
            11.,
            MUTED,
        );
        return;
    };
    let Some(transform) = world.get::<Transform>(entity).copied() else {
        return;
    };
    let name = world
        .get::<Name>(entity)
        .map_or("Entity", Name::as_str)
        .to_owned();
    let kind = world
        .get::<EditorObject>(entity)
        .map_or(ModelKind::Group, |o| o.kind);
    let heading = node(
        world,
        scroll,
        Node {
            padding: UiRect::all(px(PANEL_INSET)),
            column_gap: px(CONTROL_GAP),
            height: px(72),
            align_items: AlignItems::Center,
            flex_shrink: 0.,
            ..default()
        },
        PANEL,
    );
    icons::spawn(world, heading, model_icon(kind), 24., ACCENT);
    let info = node(
        world,
        heading,
        Node {
            flex_grow: 1.,
            row_gap: px(MICRO_GAP),
            ..column()
        },
        Color::NONE,
    );
    input(
        world,
        info,
        InputBinding {
            window,
            layout,
            pane,
            entity: Some(entity),
            field: Field::Name,
            committed: name,
        },
        percent(100),
    );
    label(world, info, &format!("ENTITY · {entity}"), 9., MUTED);
    let visible = world.get::<Visibility>(entity) != Some(&Visibility::Hidden);
    compact_icon(
        world,
        heading,
        window,
        if visible {
            Icon::SquareCheck
        } else {
            Icon::Square
        },
        Action::Visible(entity),
    );
    section(world, scroll, Icon::Move, "Transform");
    let properties = node(
        world,
        scroll,
        Node {
            padding: UiRect::all(px(PANEL_INSET)),
            row_gap: px(PANEL_INSET),
            flex_shrink: 0.,
            ..column()
        },
        Color::srgb_u8(37, 37, 37),
    );
    let (x, y, z) = transform.rotation.to_euler(EulerRot::XYZ);
    for (field, title, values) in [
        (Field::Position, "Position", transform.translation),
        (
            Field::Rotation,
            "Rotation (°)",
            Vec3::new(x, y, z) * 180. / std::f32::consts::PI,
        ),
        (Field::Scale, "Scale", transform.scale),
    ] {
        let row = node(
            world,
            properties,
            Node {
                row_gap: px(MICRO_GAP),
                ..column()
            },
            Color::NONE,
        );
        label(world, row, title, 10., MUTED);
        let fields = node(
            world,
            row,
            Node {
                column_gap: px(4),
                min_width: px(0),
                flex_wrap: FlexWrap::Wrap,
                row_gap: px(4),
                ..default()
            },
            Color::NONE,
        );
        for (axis, (name, color)) in [
            ("X", Color::srgb_u8(236, 149, 139)),
            ("Y", Color::srgb_u8(173, 210, 150)),
            ("Z", Color::srgb_u8(136, 185, 228)),
        ]
        .into_iter()
        .enumerate()
        {
            let field_container = node(
                world,
                fields,
                Node {
                    flex_grow: 1.,
                    flex_basis: px(0),
                    min_width: px(62),
                    align_items: AlignItems::Center,
                    column_gap: px(0),
                    ..default()
                },
                Color::srgb_u8(30, 30, 30),
            );
            let axis_label = node(
                world,
                field_container,
                Node {
                    width: px(20),
                    min_width: px(20),
                    height: px(26),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    ..default()
                },
                Color::NONE,
            );
            label(world, axis_label, name, 9., color);
            world.entity_mut(axis_label).insert((
                Tooltip("Drag horizontally to adjust this axis".into()),
                CursorRole::Scrub,
                DragHandle::Axis {
                    entity,
                    field,
                    axis,
                },
            ));
            let e = input(
                world,
                field_container,
                InputBinding {
                    window,
                    layout,
                    pane,
                    entity: Some(entity),
                    field,
                    committed: format!("{:.2}", values[axis]),
                },
                percent(100),
            );
            world.entity_mut(e).insert(FieldAxis(axis));
        }
    }
    section(world, scroll, Icon::Box, "Entity Components");
    let properties = node(
        world,
        scroll,
        Node {
            padding: UiRect::all(px(PANEL_INSET)),
            row_gap: px(CONTROL_GAP),
            flex_shrink: 0.,
            ..column()
        },
        Color::srgb_u8(37, 37, 37),
    );
    label(
        world,
        properties,
        &format!("Model       {}", kind.name()),
        10.,
        MUTED,
    );
    wrapped_label(
        world,
        properties,
        "Name / Transform / Visibility · editable",
        10.,
        MUTED,
    );
    wrapped_label(
        world,
        properties,
        "Other components are read-only",
        9.,
        MUTED,
    );
    if kind == ModelKind::Camera {
        label(
            world,
            properties,
            "Main Camera pose drives every live preview",
            9.,
            MUTED,
        );
    }
    wrapped_label(
        world,
        properties,
        "Enter / blur to commit · Esc to cancel",
        9.,
        MUTED,
    );
    wrapped_label(
        world,
        properties,
        "Drag X / Y / Z to adjust numeric values",
        9.,
        MUTED,
    );
}
fn section(world: &mut World, parent: Entity, symbol: Icon, title: &str) {
    let row = node(world, parent, row_style(36.), Color::srgb_u8(42, 42, 42));
    icon_label(world, row, &[symbol], title, 11., INK);
}
fn assets_tree(world: &mut World, parent: Entity, window: Entity, pane: &Pane) {
    let toolbar = node(world, parent, row_style(36.), PANEL);
    search(world, toolbar, window, pane, "Search local files");
    compact_icon(world, toolbar, window, Icon::Refresh, Action::RefreshAssets);
    let area = scroll_area(world, parent);
    assets_tree_contents(world, area, pane);
}
fn assets_tree_contents(world: &mut World, area: Entity, pane: &Pane) {
    let entries = world.resource::<AssetIndex>().entries.clone();
    let root = node(world, area, row_style(26.), Color::NONE);
    icon_label(world, root, &[Icon::FolderOpen], "assets/", 11., INK);
    for entry in entries {
        if !pane.query.is_empty()
            && !entry
                .path
                .to_string_lossy()
                .to_lowercase()
                .contains(&pane.query.to_lowercase())
        {
            continue;
        }
        let row = node(
            world,
            area,
            Node {
                padding: UiRect {
                    left: px(PANEL_INSET + entry.depth as f32 * TREE_INDENT),
                    right: px(PANEL_INSET),
                    ..default()
                },
                ..row_style(24.)
            },
            Color::NONE,
        );
        icons::spawn(
            world,
            row,
            if entry.directory {
                Icon::Folder
            } else {
                file_icon(&entry.path)
            },
            12.,
            MUTED,
        );
        label(world, row, &entry.name, 10., MUTED);
    }
}
fn gallery(world: &mut World, parent: Entity, window: Entity, pane: &Pane) {
    let toolbar = node(world, parent, row_style(36.), PANEL);
    icon_label(world, toolbar, &[Icon::FolderOpen], "Assets", 10., MUTED);
    search(world, toolbar, window, pane, "Filter assets");
    compact_icon(world, toolbar, window, Icon::Refresh, Action::RefreshAssets);
    let area = scroll_area(world, parent);
    gallery_contents(world, area, window, pane);
}
fn gallery_contents(world: &mut World, area: Entity, window: Entity, pane: &Pane) {
    let grid = node(
        world,
        area,
        Node {
            display: Display::Grid,
            width: percent(100),
            padding: UiRect::all(px(PANEL_INSET)),
            row_gap: px(PANEL_INSET),
            column_gap: px(PANEL_INSET),
            grid_template_columns: vec![RepeatedGridTrack::minmax(
                GridTrackRepetition::AutoFill,
                MinTrackSizingFunction::Px(103.),
                MaxTrackSizingFunction::Fraction(1.),
            )],
            align_items: AlignItems::Start,
            ..default()
        },
        PANEL,
    );
    let library = world.resource::<Library>().0.clone();
    let query = pane.query.to_lowercase();
    for asset in library {
        if !asset.kind.name().to_lowercase().contains(&query) {
            continue;
        }
        let card = button(
            world,
            grid,
            window,
            "",
            Action::Add(asset.kind),
            Node {
                padding: UiRect::all(px(4)),
                border_radius: BorderRadius::all(px(5)),
                row_gap: px(MICRO_GAP),
                ..column()
            },
            Color::NONE,
        );
        let image = node(
            world,
            card,
            Node {
                width: percent(100),
                aspect_ratio: Some(1.38),
                border_radius: BorderRadius::all(px(4)),
                overflow: Overflow::clip(),
                ..default()
            },
            Color::srgb_u8(51, 51, 51),
        );
        world
            .entity_mut(image)
            .insert((ImageNode::new(asset.thumbnail), PickingBehavior::IGNORE));
        label(world, card, asset.kind.name(), 10., INK);
        label(world, card, "BUILT-IN · click to add", 8., MUTED);
    }
    let entries = world.resource::<AssetIndex>().entries.clone();
    for entry in entries {
        if entry.directory || !entry.name.to_lowercase().contains(&query) {
            continue;
        }
        let card = node(
            world,
            grid,
            Node {
                padding: UiRect::all(px(MICRO_GAP)),
                row_gap: px(MICRO_GAP),
                ..column()
            },
            Color::NONE,
        );
        let thumb = node(
            world,
            card,
            Node {
                width: percent(100),
                aspect_ratio: Some(1.38),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(px(4)),
                overflow: Overflow::clip(),
                ..default()
            },
            Color::srgb_u8(51, 51, 51),
        );
        if let Some(image) = entry.image {
            world.entity_mut(thumb).insert(ImageNode::new(image));
        } else {
            icons::spawn(world, thumb, file_icon(&entry.path), 30., MUTED);
        }
        wrapped_label(world, card, &entry.name, 10., INK);
        wrapped_label(world, card, &entry.path.to_string_lossy(), 8., MUTED);
    }
}
fn viewport(world: &mut World, parent: Entity, window: Entity, pane: &Pane) {
    let preview = pane.kind == PaneType::Camera;
    let toolbar = node(
        world,
        parent,
        row_style(if preview { 31. } else { 37. }),
        Color::srgb_u8(40, 40, 40),
    );
    if preview {
        icon_label(world, toolbar, &[Icon::Camera], "Main Camera", 10., INK);
    } else {
        let (mode, space) = world
            .get_resource::<TransformGizmoSettings>()
            .map(|s| (s.mode, s.space))
            .unwrap_or_default();
        for (value, icon) in [
            (TransformGizmoMode::Translate, Icon::Move),
            (TransformGizmoMode::Rotate, Icon::Orbit),
            (TransformGizmoMode::Scale, Icon::Maximize),
        ] {
            let control = compact_icon(world, toolbar, window, icon, Action::GizmoMode(value));
            if mode == value {
                let color = Color::srgb_u8(64, 64, 64);
                world
                    .entity_mut(control)
                    .insert((BackgroundColor(color), BaseColor(color)));
            }
        }
        let control = compact(
            world,
            toolbar,
            window,
            if space == TransformGizmoSpace::World {
                "World"
            } else {
                "Local"
            },
            Action::GizmoSpace,
        );
        world
            .entity_mut(control)
            .insert(Tooltip(action_hint(&Action::GizmoSpace).unwrap().into()));
    }
    spacer(world, toolbar);
    if preview {
        icon_label(world, toolbar, &[Icon::Live], "LIVE", 9., MUTED);
    } else {
        icon_label(world, toolbar, &[Icon::Gallery], "Grid", 9., MUTED);
    }
    let stage = node(
        world,
        parent,
        Node {
            flex_grow: 1.,
            width: percent(100),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            overflow: Overflow::clip(),
            ..default()
        },
        Color::srgb_u8(25, 25, 25),
    );
    viewport_surface(world, stage, window, pane);
    if !preview {
        let overlay = node(
            world,
            stage,
            Node {
                position_type: PositionType::Absolute,
                left: px(PANEL_INSET),
                top: px(PANEL_INSET),
                row_gap: px(MICRO_GAP),
                ..column()
            },
            Color::NONE,
        );
        world.entity_mut(overlay).insert(PickingBehavior::IGNORE);
        icon_label(world, overlay, &[Icon::Globe], "QUIET COURTYARD", 10., INK);
        label(world, overlay, "Perspective · Editor camera", 9., MUTED);
        let hints = node(
            world,
            stage,
            Node {
                position_type: PositionType::Absolute,
                left: px(PANEL_INSET),
                right: px(PANEL_INSET),
                bottom: px(PANEL_INSET),
                ..default()
            },
            Color::NONE,
        );
        world.entity_mut(hints).insert(PickingBehavior::IGNORE);
        wrapped_label(
            world,
            hints,
            "RMB orbit · MMB pan · Scroll zoom · Drag handles · Esc cancel",
            9.,
            MUTED,
        );
    }
}
fn viewport_surface(world: &mut World, stage: Entity, window: Entity, pane: &Pane) {
    let preview = pane.kind == PaneType::Camera;
    let key = ViewportSurface {
        window,
        layout: world.get::<EditorWindow>(window).unwrap().document.active,
        pane: pane.id,
        preview,
    };
    let retained = world
        .query::<(Entity, &ViewportSurface, &ViewportNode)>()
        .iter(world)
        .find(|(_, identity, _)| **identity == key)
        .and_then(|(surface, _, node)| node.camera.map(|camera| (surface, camera)));
    let center = Vec3::from_array(pane.center);
    if let Some((surface, camera)) = retained {
        // Keep the node's derived picking pointer and the camera's render-world state too.
        world.entity_mut(surface).insert(ChildOf(stage));
        let mut view = world.get_mut::<SceneView>(camera).unwrap();
        if view.orbit != pane.orbit || view.center != center {
            view.orbit = pane.orbit;
            view.center = center;
            if !preview {
                world
                    .entity_mut(camera)
                    .insert(scene::orbit_transform(pane.orbit, center));
            }
        }
        return;
    }
    let image = world
        .resource_mut::<Assets<Image>>()
        .add(Image::new_target_texture(
            1,
            1,
            TextureFormat::Bgra8UnormSrgb,
            None,
        ));
    let transform = if preview {
        world
            .query_filtered::<&GlobalTransform, With<scene::MainGameCamera>>()
            .iter(world)
            .next()
            .map(GlobalTransform::compute_transform)
            .unwrap_or_else(|| scene::orbit_transform(pane.orbit, center))
    } else {
        scene::orbit_transform(pane.orbit, center)
    };
    let camera = world
        .spawn((
            Camera3d::default(),
            Camera {
                order: -1,
                clear_color: ClearColorConfig::Custom(Color::srgb_u8(38, 38, 38)),
                ..default()
            },
            RenderTarget::Image(image.clone().into()),
            if preview {
                RenderLayers::layer(0)
            } else {
                RenderLayers::from_layers(&[0, 1])
            },
            transform,
            SceneView {
                window,
                pane: pane.id,
                preview,
                target: image,
                orbit: pane.orbit,
                center,
            },
            UiOwned(window),
        ))
        .id();
    let surface = node(
        world,
        stage,
        Node {
            width: percent(100),
            height: if preview { auto() } else { percent(100) },
            max_height: percent(100),
            aspect_ratio: if preview { Some(16. / 9.) } else { None },
            ..default()
        },
        Color::NONE,
    );
    world.entity_mut(surface).insert((
        key,
        ViewportNode::new(camera),
        if preview {
            CursorRole::Default
        } else {
            CursorRole::Viewport(camera)
        },
    ));
    if !preview {
        world
            .entity_mut(surface)
            .insert(DragHandle::View { camera });
    }
}
// Presentation-only categories; PaneType and Layout JSON remain unchanged.
const PANE_CATEGORIES: [(&str, &[PaneType]); 3] = [
    ("Scene", &[PaneType::Viewport, PaneType::Camera]),
    ("Data", &[PaneType::World, PaneType::Inspector]),
    ("Assets", &[PaneType::AssetsTree, PaneType::AssetsGallery]),
];

fn menu_popup(
    world: &mut World,
    parent: Entity,
    window: Entity,
    origin: Vec2,
    size: Vec2,
    style: Node,
) -> Entity {
    let window_info = world.get::<Window>(window).unwrap();
    let bounds = Vec2::new(window_info.width(), window_info.height());
    let size = size.min((bounds - Vec2::splat(16.)).max(Vec2::ZERO));
    let origin = origin.clamp(
        Vec2::splat(8.),
        (bounds - size - Vec2::splat(8.)).max(Vec2::splat(8.)),
    );
    let container = node(
        world,
        parent,
        Node {
            position_type: PositionType::Absolute,
            left: px(origin.x),
            top: px(origin.y),
            width: px(size.x),
            max_height: px(size.y),
            border: UiRect::all(px(1)),
            border_radius: BorderRadius::all(px(6)),
            ..style
        },
        Color::srgb_u8(36, 36, 36),
    );
    world
        .entity_mut(container)
        .insert((
            GlobalZIndex(100),
            BorderColor::all(Color::srgb_u8(73, 73, 73)),
            EditorPopup(window),
        ))
        .observe(|mut click: On<PointerClick>| {
            // Heading/padding clicks are inside the popup, not outside dismissal.
            click.propagate(false);
        });
    container
}

fn pane_type_menu(
    world: &mut World,
    parent: Entity,
    window: Entity,
    workspace: &EditorWindow,
    group: Id,
    pane: Option<Id>,
) {
    let window_info = world.get::<Window>(window).unwrap();
    let origin = dock_rects(
        workspace.document.root(),
        Rect::from_corners(Vec2::ZERO, workspace_size(window_info)),
        visible_group(workspace, window_info.width()),
    )
    .into_iter()
    .find(|(id, _, splitter)| *id == group && !*splitter)
    .map_or(Vec2::new(180., 40.), |(_, rect, _)| {
        rect.min + Vec2::new(10., workspace_top(window_info.width()) + 39.)
    });
    let stacked = window_info.width() < 580.;
    let column_width = if stacked {
        (window_info.width() - 34.).min(240.)
    } else {
        170.
    };
    let column_gap = 16.;
    let padding = 8.;
    let heading_height = if stacked { 20. } else { 24. };
    let heading_gap = if stacked { MICRO_GAP } else { CONTROL_GAP };
    let row_height = if stacked { 24. } else { 28. };
    let row_gap = MICRO_GAP;
    let count = PANE_CATEGORIES.len();
    let rows = PANE_CATEGORIES
        .iter()
        .map(|(_, kinds)| kinds.len())
        .max()
        .unwrap();
    let content_width = if stacked {
        column_width
    } else {
        column_width * count as f32 + column_gap * (count - 1) as f32
    };
    let section_height = |rows: usize| {
        heading_height
            + heading_gap
            + row_height * rows as f32
            + row_gap * rows.saturating_sub(1) as f32
    };
    let size = Vec2::new(
        content_width + padding * 2. + 2.,
        padding * 2.
            + 2.
            + if stacked {
                PANE_CATEGORIES
                    .iter()
                    .map(|(_, kinds)| section_height(kinds.len()))
                    .sum::<f32>()
                    + CONTROL_GAP * count.saturating_sub(1) as f32
            } else {
                section_height(rows)
            },
    );
    let container = menu_popup(
        world,
        parent,
        window,
        origin,
        size,
        Node {
            padding: UiRect::all(px(padding)),
            overflow: Overflow::scroll_y(),
            ..column()
        },
    );
    // All categories remain discoverable without hidden horizontal menu scrolling.
    world.entity_mut(container).insert((
        ScrollPosition::default(),
        PopupAnchor {
            window,
            menu: Menu::PaneType { group, pane },
        },
    ));
    let columns = node(
        world,
        container,
        Node {
            width: px(content_width),
            column_gap: px(column_gap),
            row_gap: px(CONTROL_GAP),
            flex_direction: if stacked {
                FlexDirection::Column
            } else {
                FlexDirection::Row
            },
            align_items: AlignItems::Start,
            flex_shrink: 0.,
            ..default()
        },
        Color::NONE,
    );
    let current = pane
        .and_then(|id| workspace.document.root().pane(id))
        .map(|p| p.kind);
    for (title, kinds) in PANE_CATEGORIES {
        let section = node(
            world,
            columns,
            Node {
                width: px(column_width),
                flex_shrink: 0.,
                ..column()
            },
            Color::NONE,
        );
        let heading = node(
            world,
            section,
            Node {
                border: UiRect::bottom(px(1)),
                margin: UiRect::bottom(px(heading_gap)),
                ..row_style(heading_height)
            },
            Color::NONE,
        );
        world.entity_mut(heading).insert(BorderColor::all(LINE));
        label(world, heading, title, 11., MUTED);
        let list = node(
            world,
            section,
            Node {
                row_gap: px(row_gap),
                ..column()
            },
            Color::NONE,
        );
        for &kind in kinds {
            let selected = current == Some(kind);
            let item = button(
                world,
                list,
                window,
                "",
                Action::ChangeType { group, pane, kind },
                Node {
                    border_radius: BorderRadius::all(px(4)),
                    ..row_style(row_height)
                },
                if selected {
                    Color::srgb_u8(58, 58, 58)
                } else {
                    Color::NONE
                },
            );
            icons::spawn(world, item, pane_icon(kind), 14., INK);
            label(world, item, kind.title(), 11., INK);
            spacer(world, item);
            icons::spawn(
                world,
                item,
                Icon::Check,
                12.,
                if selected { INK } else { Color::NONE },
            );
        }
    }
}

fn render_menu(
    world: &mut World,
    parent: Entity,
    window: Entity,
    workspace: &EditorWindow,
    menu: &Menu,
) {
    if matches!(menu, Menu::Help) {
        let w = world.get::<Window>(window).unwrap();
        let container = menu_popup(
            world,
            parent,
            window,
            Vec2::new(12., header_height(w.width())),
            Vec2::new(360., 284.),
            Node {
                padding: UiRect::all(px(PANEL_INSET)),
                row_gap: px(CONTROL_GAP),
                overflow: Overflow::scroll_y(),
                ..column()
            },
        );
        world
            .entity_mut(container)
            .insert(ScrollPosition::default());
        for (text, size, color) in [
            ("Interaction help", 13., INK),
            ("Viewport", 11., INK),
            (
                "Right drag: orbit · Middle drag: pan · Wheel: zoom",
                10.,
                MUTED,
            ),
            (
                "Select a mesh; drag the Move / Rotate / Scale handles",
                10.,
                MUTED,
            ),
            (
                "World / Local axes · Escape cancels a handle drag",
                10.,
                MUTED,
            ),
            ("Inspector", 11., INK),
            ("Enter / blur: commit · Escape: discard draft", 10., MUTED),
            ("Drag the X / Y / Z labels to adjust values", 10., MUTED),
            ("Workspace", 11., INK),
            (
                "Drag tabs to dock · Pane corners: maximize / options",
                10.,
                MUTED,
            ),
            (
                "Ctrl+S saves layout only; scene edits are session-only",
                10.,
                MUTED,
            ),
        ] {
            wrapped_label(world, container, text, size, color);
        }
        return;
    }
    let mut items: Vec<(String, Action)> = Vec::new();
    let title;
    let mut origin = Vec2::new(180., 40.);
    let menu_width = 244.;
    match menu {
        Menu::Header(name) => {
            origin.x = 12.;
            title = *name;
            match *name {
                "File" => items.extend(
                    [
                        ("Save Layout JSON", Action::Save),
                        ("Load Layout JSON", Action::Load),
                        ("New Editor Window", Action::NewWindow),
                    ]
                    .map(|(t, a)| (t.into(), a)),
                ),
                "Edit" => items.push(("Delete selected entity".into(), Action::DeleteSelection)),
                "Add" => items.extend(
                    [
                        ModelKind::Cube,
                        ModelKind::Arch,
                        ModelKind::Tree,
                        ModelKind::Orb,
                    ]
                    .map(|k| (k.name().into(), Action::Add(k))),
                ),
                _ => items.extend(
                    [
                        (
                            "Add Pane",
                            Action::Menu(Menu::PaneType {
                                group: workspace.focused,
                                pane: None,
                            }),
                        ),
                        ("New Editor Window", Action::NewWindow),
                        ("Copy current Layout", Action::CopyLayout),
                        ("Reset current Layout", Action::ResetLayout),
                        ("Interaction help", Action::Help),
                    ]
                    .map(|(t, a)| (t.into(), a)),
                ),
            }
        }
        Menu::PaneType { group, pane } => {
            pane_type_menu(world, parent, window, workspace, *group, *pane);
            return;
        }
        Menu::Help => unreachable!(),
        Menu::Group(group) => {
            title = "PANE OPTIONS";
            items.extend(
                [
                    (
                        "Split left / right",
                        Action::Split {
                            group: *group,
                            axis: Axis::X,
                        },
                    ),
                    (
                        "Split top / bottom",
                        Action::Split {
                            group: *group,
                            axis: Axis::Y,
                        },
                    ),
                    ("Maximize / Restore", Action::Maximize(*group)),
                    (
                        "Add Pane",
                        Action::Menu(Menu::PaneType {
                            group: *group,
                            pane: None,
                        }),
                    ),
                ]
                .map(|(t, a)| (t.into(), a)),
            );
            if let Some(g) = workspace.document.root().group(*group) {
                items.push(("Close active tab".into(), Action::Close(g.active)));
            }
        }
    }
    let w = world.get::<Window>(window).unwrap();
    origin.y = header_height(w.width());
    if let Menu::Group(group) = menu {
        let rect = dock_rects(
            workspace.document.root(),
            Rect::from_corners(Vec2::ZERO, workspace_size(w)),
            visible_group(workspace, w.width()),
        )
        .into_iter()
        .find(|(id, _, split)| id == group && !split)
        .map(|(_, rect, _)| rect);
        if let Some(rect) = rect {
            origin = Vec2::new(
                rect.max.x + 5. - menu_width,
                rect.min.y + workspace_top(w.width()) + 39.,
            );
        }
    }
    let height = items.len() as f32 * 32. + 42.;
    let container = menu_popup(
        world,
        parent,
        window,
        origin,
        Vec2::new(menu_width, height),
        Node {
            padding: UiRect::all(px(CONTROL_GAP)),
            overflow: Overflow::scroll_y(),
            ..column()
        },
    );
    world.entity_mut(container).insert((
        ScrollPosition::default(),
        PopupAnchor {
            window,
            menu: menu.clone(),
        },
    ));
    let heading = node(
        world,
        container,
        Node {
            padding: UiRect::horizontal(px(CONTROL_GAP)),
            ..row_style(24.)
        },
        Color::NONE,
    );
    label(world, heading, title, 9., MUTED);
    for (title, action) in items {
        let symbol = match &action {
            Action::Save => Icon::Save,
            Action::Load => Icon::Load,
            Action::NewWindow => Icon::NewWindow,
            Action::CopyLayout => Icon::Copy,
            Action::ResetLayout => Icon::Refresh,
            Action::DeleteSelection | Action::Close(_) => Icon::Close,
            Action::Add(kind) => model_icon(*kind),
            Action::Split { axis: Axis::X, .. } => Icon::Columns,
            Action::Split { axis: Axis::Y, .. } => Icon::Rows,
            Action::Maximize(group) if workspace.maximized == Some(*group) => Icon::Minimize,
            Action::Maximize(_) => Icon::Maximize,
            Action::Help => Icon::Help,
            _ => Icon::Plus,
        };
        let item = button(
            world,
            container,
            window,
            "",
            action,
            Node {
                height: px(32),
                min_height: px(32),
                flex_shrink: 0.,
                padding: UiRect::horizontal(px(CONTROL_GAP)),
                column_gap: px(CONTROL_GAP),
                align_items: AlignItems::Center,
                border_radius: BorderRadius::all(px(4)),
                ..default()
            },
            Color::NONE,
        );
        icons::spawn(world, item, symbol, 14., MUTED);
        label(world, item, &title, 11., INK);
    }
}

/// Returns false only when this window actually needs a new shell.
pub(crate) fn refresh_window(
    world: &mut World,
    window: Entity,
    workspace: &EditorWindow,
    content_changed: ContentChanges,
) -> bool {
    let Some(root) = workspace.root else {
        return false;
    };
    let width = world.get::<Window>(window).unwrap().width();
    let shell = RenderedShell::capture(workspace, width);
    if world.get::<RenderedShell>(root) != Some(&shell) {
        return false;
    }
    world.get_mut::<ShellWidth>(root).unwrap().0 = width;
    refresh_menu(world, root, window, workspace, width);
    let selected = world.resource::<Selection>().0;
    let bodies: Vec<_> = world
        .query::<(Entity, &PaneBody)>()
        .iter(world)
        .filter(|(_, body)| body.window == window)
        .map(|(entity, body)| (entity, body.clone()))
        .collect();
    for (body, previous) in bodies {
        let Some(pane) = workspace.document.root().pane(previous.pane) else {
            continue;
        };
        let changed = previous.query != pane.query
            || previous.collapsed != pane.collapsed
            || match previous.kind {
                PaneType::World => {
                    content_changed.tree || (!pane.query.is_empty() && content_changed.names)
                }
                PaneType::AssetsTree | PaneType::AssetsGallery => content_changed.assets,
                PaneType::Inspector => previous.selected != selected,
                _ => false,
            };
        if changed {
            // Retain the scroll container, track and native thumb. Only its content changes.
            let scroll = world
                .query::<(Entity, &PaneScroll)>()
                .iter(world)
                .find(|(_, key)| {
                    key.window == window && key.layout == previous.layout && key.pane == pane.id
                })
                .map(|(entity, _)| entity);
            if let Some(scroll) = scroll {
                let children = world
                    .get::<Children>(scroll)
                    .map(|c| c.iter().collect::<Vec<_>>())
                    .unwrap_or_default();
                for child in children {
                    world.despawn(child);
                }
                match pane.kind {
                    PaneType::World => world_contents(world, scroll, window, pane),
                    PaneType::Inspector => inspector_contents(world, scroll, window, pane.id),
                    PaneType::AssetsTree => assets_tree_contents(world, scroll, pane),
                    PaneType::AssetsGallery => gallery_contents(world, scroll, window, pane),
                    _ => {}
                }
            }
        }
        world.entity_mut(body).insert(PaneBody::capture(
            window,
            workspace.document.active,
            pane,
            selected,
        ));
    }
    // Loading/resetting a layout may change just its camera poses, not its UI structure.
    let views: Vec<_> = world
        .query::<(Entity, &SceneView)>()
        .iter(world)
        .filter(|(_, view)| view.window == window)
        .map(|(entity, view)| (entity, view.pane, view.preview))
        .collect();
    for (camera, id, preview) in views {
        if let Some(pane) = workspace.document.root().pane(id) {
            let center = Vec3::from_array(pane.center);
            let mut view = world.get_mut::<SceneView>(camera).unwrap();
            if view.orbit != pane.orbit || view.center != center {
                view.orbit = pane.orbit;
                view.center = center;
                if !preview {
                    world
                        .entity_mut(camera)
                        .insert(scene::orbit_transform(pane.orbit, center));
                }
            }
        }
    }
    true
}
fn refresh_menu(
    world: &mut World,
    root: Entity,
    window: Entity,
    workspace: &EditorWindow,
    width: f32,
) {
    let changed = world
        .get::<RenderedMenu>(root)
        .is_none_or(|rendered| rendered.menu != workspace.menu || rendered.width != width);
    if !changed {
        return;
    }
    let popups: Vec<_> = world
        .query::<(Entity, &EditorPopup)>()
        .iter(world)
        .filter(|(_, popup)| popup.0 == window)
        .map(|(entity, _)| entity)
        .collect();
    for popup in popups {
        world.despawn(popup);
    }
    if let Some(menu) = &workspace.menu {
        render_menu(world, root, window, workspace, menu);
    }
    world.entity_mut(root).insert(RenderedMenu {
        menu: workspace.menu.clone(),
        width,
    });
}
fn set_text(world: &mut World, entity: Entity, value: String) {
    if let Some(mut text) = world.get_mut::<Text>(entity)
        && text.0 != value
    {
        text.0 = value;
    }
}
/// Synchronize retained presentation without destroying inputs or their native edit state.
pub(crate) fn sync_values(world: &mut World) {
    let focus = world.resource::<bevy::input_focus::InputFocus>().get();
    let inputs: Vec<_> = world
        .query::<(Entity, &InputBinding, Option<&FieldAxis>)>()
        .iter(world)
        .map(|(e, b, axis)| (e, b.clone(), axis.map_or(0, |a| a.0)))
        .collect();
    for (entity, binding, axis) in inputs {
        if Some(entity) == focus || world.get::<FieldError>(entity).is_some() {
            continue;
        }
        let value = match binding.field {
            Field::Query(pane) => world
                .get::<EditorWindow>(binding.window)
                .and_then(|ws| ws.document.layouts.get(binding.layout))
                .and_then(|l| l.root.pane(pane))
                .map(|p| p.query.clone()),
            Field::Name => binding
                .entity
                .and_then(|e| world.get::<Name>(e))
                .map(|n| n.as_str().to_owned()),
            field => binding
                .entity
                .and_then(|e| world.get::<Transform>(e))
                .map(|t| {
                    let values = match field {
                        Field::Position => t.translation,
                        Field::Scale => t.scale,
                        _ => {
                            let (x, y, z) = t.rotation.to_euler(EulerRot::XYZ);
                            Vec3::new(x, y, z) * (180. / std::f32::consts::PI)
                        }
                    };
                    format!("{:.2}", values[axis])
                }),
        };
        if let Some(value) = value {
            if world
                .get::<EditableText>(entity)
                .is_some_and(|text| text.value().to_string() != value)
            {
                world
                    .entity_mut(entity)
                    .insert(EditableText::new(value.clone()));
            }
            let mut binding = world.get_mut::<InputBinding>(entity).unwrap();
            if binding.committed != value {
                binding.committed = value;
            }
        }
    }
    let selected = world.resource::<Selection>().0;
    let names: Vec<_> = world
        .query::<(Entity, &EntityName)>()
        .iter(world)
        .map(|(text, name)| (text, name.0))
        .collect();
    for (text, object) in names {
        if let Some(name) = world.get::<Name>(object) {
            set_text(world, text, name.as_str().to_owned());
        }
        let color = if selected == Some(object) { INK } else { MUTED };
        if let Some(mut current) = world.get_mut::<TextColor>(text)
            && current.0 != color
        {
            current.0 = color;
        }
    }
    let statuses: Vec<_> = world
        .query::<(Entity, &StatusText)>()
        .iter(world)
        .map(|(entity, status)| (entity, status.0))
        .collect();
    for (text, window) in statuses {
        if let Some(ws) = world.get::<EditorWindow>(window) {
            set_text(world, text, ws.status.clone());
        }
    }
    let messages: Vec<_> = world
        .query::<(Entity, &StatusMessage)>()
        .iter(world)
        .map(|(entity, status)| (entity, status.0))
        .collect();
    for (entity, window) in messages {
        if let Some(value) = world
            .get::<EditorWindow>(window)
            .map(|ws| ws.status.clone())
        {
            let mut tooltip = world.get_mut::<Tooltip>(entity).unwrap();
            if tooltip.0 != value {
                tooltip.0 = value;
            }
        }
    }
    let regions: Vec<_> = world
        .query::<(Entity, &DockRegion)>()
        .iter(world)
        .filter(|(_, r)| !r.splitter)
        .map(|(e, r)| (e, r.window, r.id))
        .collect();
    for (entity, window, id) in regions {
        let focused = world
            .get::<EditorWindow>(window)
            .is_some_and(|ws| ws.focused == id);
        let intensity = if focused { 85 } else { 61 };
        let color = BorderColor::all(Color::srgb_u8(intensity, intensity, intensity));
        if let Some(mut border) = world.get_mut::<BorderColor>(entity)
            && *border != color
        {
            *border = color;
        }
    }
    let settings = world
        .get_resource::<TransformGizmoSettings>()
        .map(|s| (s.mode, s.space))
        .unwrap_or_default();
    let actions: Vec<_> = world
        .query::<(Entity, &UiAction)>()
        .iter(world)
        .map(|(entity, action)| (entity, action.action.clone()))
        .collect();
    for (entity, action) in actions {
        let color = match action {
            Action::Select(object) => {
                let active = selected == Some(object);
                if let Some(mut border) = world.get_mut::<BorderColor>(entity) {
                    let color = if active { ACCENT } else { Color::NONE };
                    if border.left != color {
                        border.left = color;
                    }
                }
                Some(if active {
                    Color::srgb_u8(56, 56, 56)
                } else {
                    Color::NONE
                })
            }
            Action::GizmoMode(mode) => Some(if mode == settings.0 {
                Color::srgb_u8(64, 64, 64)
            } else {
                Color::NONE
            }),
            Action::GizmoSpace => {
                let children = world
                    .get::<Children>(entity)
                    .map(|c| c.iter().collect::<Vec<_>>())
                    .unwrap_or_default();
                for text in children {
                    set_text(
                        world,
                        text,
                        if settings.1 == TransformGizmoSpace::World {
                            "World"
                        } else {
                            "Local"
                        }
                        .into(),
                    );
                }
                None
            }
            Action::Visible(object) => {
                let visible = world.get::<Visibility>(object) != Some(&Visibility::Hidden);
                let children = world
                    .get::<Children>(entity)
                    .map(|c| c.iter().collect::<Vec<_>>())
                    .unwrap_or_default();
                for child in children {
                    if let Some(mut icon) = world.get_mut::<icons::LucideIcon>(child) {
                        let desired = if visible {
                            Icon::SquareCheck
                        } else {
                            Icon::Square
                        };
                        if icon.0 != desired {
                            icon.0 = desired;
                        }
                    }
                }
                None
            }
            _ => None,
        };
        if let Some(color) = color
            && let Some(mut base) = world.get_mut::<BaseColor>(entity)
            && base.0 != color
        {
            base.0 = color;
            world.get_mut::<BackgroundColor>(entity).unwrap().0 = color;
        }
    }
}

fn rect_node(rect: Rect) -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: px(rect.min.x + 5.),
        top: px(rect.min.y + 5.),
        width: px(rect.width()),
        height: px(rect.height()),
        ..column()
    }
}
pub fn dock_rects(root: &DockNode, rect: Rect, maximized: Option<Id>) -> Vec<(Id, Rect, bool)> {
    if let Some(id) = maximized
        && root.group(id).is_some()
    {
        return vec![(id, rect, false)];
    }
    fn walk(node: &DockNode, rect: Rect, out: &mut Vec<(Id, Rect, bool)>) {
        match node {
            DockNode::Group(g) => out.push((g.id, rect, false)),
            DockNode::Split {
                id,
                axis,
                ratio,
                a,
                b,
            } => {
                let gap = 5.;
                let mut first = rect;
                let mut second = rect;
                let mut bar = rect;
                if *axis == Axis::X {
                    let x = rect.min.x + (rect.width() - gap).max(0.) * ratio;
                    first.max.x = x;
                    second.min.x = x + gap;
                    bar.min.x = x;
                    bar.max.x = x + gap;
                } else {
                    let y = rect.min.y + (rect.height() - gap).max(0.) * ratio;
                    first.max.y = y;
                    second.min.y = y + gap;
                    bar.min.y = y;
                    bar.max.y = y + gap;
                }
                out.push((*id, bar, true));
                walk(a, first, out);
                walk(b, second, out);
            }
        }
    }
    let mut out = Vec::new();
    walk(root, rect, &mut out);
    out
}
pub fn split_extent(root: &DockNode, id: Id, size: Vec2) -> Option<f32> {
    fn walk(node: &DockNode, id: Id, size: Vec2) -> Option<f32> {
        if let DockNode::Split {
            id: current,
            axis,
            ratio,
            a,
            b,
        } = node
        {
            let extent = if *axis == Axis::X { size.x } else { size.y };
            if *current == id {
                return Some((extent - 5.).max(1.));
            }
            let mut sa = size;
            let mut sb = size;
            let idx = if *axis == Axis::X { 0 } else { 1 };
            sa[idx] = (extent - 5.) * ratio;
            sb[idx] = (extent - 5.) * (1. - ratio);
            walk(a, id, sa).or_else(|| walk(b, id, sb))
        } else {
            None
        }
    }
    walk(root, id, size)
}
pub fn geometry(
    windows: Query<(&Window, &EditorWindow)>,
    mut regions: Query<(&DockRegion, &mut Node)>,
    mut previews: Query<(&DropPreview, &mut Node), Without<DockRegion>>,
    state: Res<DragState>,
) {
    for (region, mut node) in &mut regions {
        let Ok((window, workspace)) = windows.get(region.window) else {
            continue;
        };
        let rects = dock_rects(
            workspace.document.root(),
            Rect::from_corners(Vec2::ZERO, workspace_size(window)),
            visible_group(workspace, window.width()),
        );
        if let Some((_, rect, _)) = rects
            .iter()
            .find(|(id, _, splitter)| *id == region.id && *splitter == region.splitter)
        {
            let desired = rect_node(*rect);
            if node.left != desired.left
                || node.top != desired.top
                || node.width != desired.width
                || node.height != desired.height
            {
                node.left = desired.left;
                node.top = desired.top;
                node.width = desired.width;
                node.height = desired.height;
            }
        }
    }
    for (preview, mut node) in &mut previews {
        let Some((window, id, zone)) = state.drop.filter(|(w, _, _)| *w == preview.0) else {
            node.display = Display::None;
            continue;
        };
        let Ok((window_info, workspace)) = windows.get(window) else {
            continue;
        };
        if let Some((_, mut rect, _)) = dock_rects(
            workspace.document.root(),
            Rect::from_corners(Vec2::ZERO, workspace_size(window_info)),
            visible_group(workspace, window_info.width()),
        )
        .into_iter()
        .find(|(i, _, s)| *i == id && !*s)
        {
            match zone {
                Zone::Left => rect.max.x = rect.center().x,
                Zone::Right => rect.min.x = rect.center().x,
                Zone::Top => rect.max.y = rect.center().y,
                Zone::Bottom => rect.min.y = rect.center().y,
                Zone::Center => {}
            }
            let desired = rect_node(rect);
            node.display = Display::Flex;
            node.left = desired.left;
            node.top = desired.top;
            node.width = desired.width;
            node.height = desired.height;
        }
    }
}

pub(crate) fn field_feedback(
    focus: Res<bevy::input_focus::InputFocus>,
    mut fields: Query<(Entity, &InputBinding, Option<&FieldError>, &mut BorderColor)>,
    mut commands: Commands,
) {
    for (entity, binding, error, mut border) in &mut fields {
        let color = if error.is_some() {
            Color::srgb_u8(241, 161, 143)
        } else if focus.get() == Some(entity) {
            MUTED
        } else {
            INPUT_BORDER
        };
        let desired = BorderColor::all(color);
        if *border != desired {
            *border = desired;
        }
        let hint = error.map_or_else(
            || {
                if matches!(binding.field, Field::Query(_)) {
                    "Enter / blur to apply search · Esc to cancel"
                } else {
                    "Enter / blur to commit · Esc to cancel"
                }
            },
            |error| error.0,
        );
        commands.entity(entity).insert(Tooltip(hint.into()));
    }
}

pub(crate) fn popup_anchors(world: &mut World) {
    let popups: Vec<_> = world
        .query::<(Entity, &PopupAnchor)>()
        .iter(world)
        .map(|(e, a)| (e, a.window, a.menu.clone()))
        .collect();
    for (popup, window, menu) in popups {
        let origin = world
            .query::<(&UiAction, &ComputedNode, &bevy::ui::UiGlobalTransform)>()
            .iter(world)
            .find(|(a, n, _)| {
                a.window == window
                    && (matches!(&a.action, Action::Menu(m) if *m == menu)
                        || menu == Menu::Help && matches!(a.action, Action::Help))
                    && !n.is_empty()
            })
            .map(|(_, n, t)| {
                (t.translation + Vec2::new(-n.size().x / 2., n.size().y / 2.))
                    * n.inverse_scale_factor()
            });
        if let Some(origin) = origin {
            let bounds = world
                .get::<Window>(window)
                .map(|w| Vec2::new(w.width(), w.height()))
                .unwrap();
            let size = world
                .get::<ComputedNode>(popup)
                .map(|n| n.size() * n.inverse_scale_factor())
                .unwrap_or(Vec2::ZERO);
            let position = (origin + Vec2::new(0., 3.)).clamp(
                Vec2::splat(8.),
                (bounds - size - Vec2::splat(8.)).max(Vec2::splat(8.)),
            );
            let mut style = world.get_mut::<Node>(popup).unwrap();
            style.left = px(position.x);
            style.top = px(position.y);
        }
    }
}

pub(crate) fn tooltips(world: &mut World) {
    let now = world.resource::<Time>().elapsed_secs_f64();
    let mut state = world.remove_resource::<TooltipState>().unwrap_or_default();
    let target = world
        .get_resource::<bevy::picking::hover::HoverMap>()
        .and_then(|hover| {
            hover
                .iter()
                .filter(|(pointer, _)| **pointer == bevy::picking::pointer::PointerId::Mouse)
                .flat_map(|(_, hits)| hits.keys())
                .find_map(|hit| {
                    let mut entity = *hit;
                    loop {
                        if world.get::<Tooltip>(entity).is_some() {
                            return Some(entity);
                        }
                        entity = world.get::<ChildOf>(entity)?.parent();
                    }
                })
        });
    let menu_open = world
        .query::<&EditorWindow>()
        .iter(world)
        .any(|window| window.menu.is_some());
    let suppressed = menu_open
        || world
            .get_resource::<DragState>()
            .is_some_and(|drag| drag.tab.is_some())
        || world
            .get_resource::<ButtonInput<MouseButton>>()
            .is_some_and(|buttons| {
                buttons.any_pressed([MouseButton::Left, MouseButton::Right, MouseButton::Middle])
            });
    let focused = world
        .get_resource::<bevy::input_focus::InputFocus>()
        .and_then(|focus| focus.get());
    let editing = focused.is_some_and(|entity| world.get::<InputBinding>(entity).is_some())
        && !target.is_some_and(|entity| {
            focused == Some(entity) && world.get::<FieldError>(entity).is_some()
        });
    let target = if suppressed || editing { None } else { target };
    let message = target
        .and_then(|entity| world.get::<Tooltip>(entity))
        .map(|tip| tip.0.clone());
    if state.target != target
        || state.message != message
        || state.popup.is_some_and(|e| world.get_entity(e).is_err())
    {
        if let Some(popup) = state.popup.take() {
            let _ = world.despawn(popup);
        }
        state.target = target;
        state.message = message;
        state.since = if now < state.warm_until {
            now - 0.5
        } else {
            now
        };
    }
    if let Some(target) = target
        && state.popup.is_none()
        && now - state.since >= 0.5
    {
        let mut ancestor = target;
        let owner = loop {
            if let Some(owner) = world.get::<UiOwned>(ancestor) {
                break Some((ancestor, owner.0));
            }
            let Some(parent) = world.get::<ChildOf>(ancestor) else {
                break None;
            };
            ancestor = parent.parent();
        };
        if let Some((root, window)) = owner {
            let message = world.get::<Tooltip>(target).unwrap().0.clone();
            let bounds = world
                .get::<Window>(window)
                .map(|w| Vec2::new(w.width(), w.height()))
                .unwrap();
            let width = (message.chars().count() as f32 * 5.5 + 20.)
                .min(320.)
                .min(bounds.x - 16.);
            let origin = world
                .get::<bevy::ui::UiGlobalTransform>(target)
                .zip(world.get::<ComputedNode>(target))
                .map(|(t, n)| {
                    let top_left = (t.translation - n.size() * 0.5) * n.inverse_scale_factor();
                    Vec2::new(top_left.x, top_left.y - 38.)
                })
                .unwrap_or(Vec2::splat(8.));
            let position = origin.clamp(
                Vec2::splat(8.),
                (bounds - Vec2::new(width + 8., 64.)).max(Vec2::splat(8.)),
            );
            let popup = node(
                world,
                root,
                Node {
                    position_type: PositionType::Absolute,
                    left: px(position.x),
                    top: px(position.y),
                    width: px(width),
                    padding: UiRect::all(px(8)),
                    border: UiRect::all(px(1)),
                    border_radius: BorderRadius::all(px(4)),
                    ..default()
                },
                Color::srgb_u8(44, 44, 44),
            );
            world.entity_mut(popup).insert((
                GlobalZIndex(200),
                BorderColor::all(MUTED),
                PickingBehavior::IGNORE,
            ));
            wrapped_label(world, popup, &message, 10., INK);
            state.popup = Some(popup);
            state.warm_until = now + 1.;
        }
    }
    if let (Some(popup), Some(target)) = (state.popup, state.target) {
        let geometry = world
            .get::<ComputedNode>(popup)
            .zip(world.get::<ComputedNode>(target))
            .zip(world.get::<bevy::ui::UiGlobalTransform>(target))
            .map(|((popup, target), transform)| {
                (
                    popup.size() * popup.inverse_scale_factor(),
                    (transform.translation - target.size() * 0.5) * target.inverse_scale_factor(),
                    target.size() * target.inverse_scale_factor(),
                )
            });
        let bounds = world
            .get::<ChildOf>(popup)
            .and_then(|p| world.get::<UiOwned>(p.parent()))
            .and_then(|owner| world.get::<Window>(owner.0))
            .map(|w| Vec2::new(w.width(), w.height()));
        if let (Some((size, top_left, target_size)), Some(bounds)) = (geometry, bounds)
            && size.y > 0.
        {
            // Prefer above the control so header hints do not cover the first editable
            // row. Flip below only when there is no room, then clamp to the window.
            let mut position = top_left - Vec2::new(0., size.y + CONTROL_GAP);
            if position.y < 8. {
                position.y = top_left.y + target_size.y + CONTROL_GAP;
            }
            position = position.clamp(
                Vec2::splat(8.),
                (bounds - size - Vec2::splat(8.)).max(Vec2::splat(8.)),
            );
            let mut style = world.get_mut::<Node>(popup).unwrap();
            if style.left != px(position.x) || style.top != px(position.y) {
                style.left = px(position.x);
                style.top = px(position.y);
            }
        }
        state.warm_until = now + 1.;
    }
    world.insert_resource(state);
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::{
        asset::AssetPlugin,
        input::{
            ButtonState, InputPlugin,
            keyboard::{Key, KeyboardInput},
        },
        input_focus::{
            InputDispatchPlugin, InputFocus, InputFocusPlugin, pointer_focus::PointerFocusPlugin,
            tab_navigation::TabNavigationPlugin,
        },
        picking::{
            InteractionPlugin, PickingPlugin,
            backend::{HitData, PointerHits},
            pointer::{Location, PointerAction, PointerId, PointerInput},
        },
        text::{EditableTextSystems, TextLayoutInfo, TextPlugin},
        ui::widget::{update_editable_text_layout, update_editable_text_styles},
        ui_widgets::TextInputPlugin,
        window::{PrimaryWindow, WindowPlugin, WindowRef},
    };

    struct TextHarness {
        app: App,
        window: Entity,
        camera: Entity,
        object: Entity,
    }
    impl TextHarness {
        fn new() -> Self {
            let mut app = App::new();
            // CPU-only native focus/edit/layout pipeline: no winit, GPU, or GUI automation.
            app.add_plugins((
                MinimalPlugins,
                AssetPlugin::default(),
                bevy::scene::ScenePlugin,
                InputPlugin,
                WindowPlugin {
                    primary_window: None,
                    ..default()
                },
                PickingPlugin,
                InteractionPlugin,
                InputFocusPlugin,
                InputDispatchPlugin,
                PointerFocusPlugin,
                TabNavigationPlugin,
                TextPlugin,
                TextInputPlugin,
            ))
            .add_plugins((
                bevy::mesh::MeshPlugin,
                bevy::camera::visibility::VisibilityPlugin,
                EditorUiPlugin,
            ))
            .init_asset::<Image>()
            .init_resource::<UiScale>()
            .init_resource::<Selection>()
            .init_resource::<UiDirty>()
            .init_resource::<AssetIndex>()
            .init_resource::<Library>()
            .add_systems(
                PostUpdate,
                (
                    update_editable_text_styles.before(EditableTextSystems),
                    update_editable_text_layout.after(EditableTextSystems),
                ),
            );
            let window = app
                .world_mut()
                .spawn((Window::default(), PrimaryWindow))
                .id();
            let camera = app.world_mut().spawn_empty().id();
            let object = app
                .world_mut()
                .spawn((
                    Name::new("Cube"),
                    Transform::default(),
                    EditorObject {
                        key: 1,
                        kind: ModelKind::Cube,
                    },
                ))
                .id();
            app.world_mut().resource_mut::<Selection>().0 = Some(object);
            let workspace = EditorWindow {
                document: LayoutDocument::default(),
                camera,
                root: None,
                focused: 1,
                maximized: None,
                menu: None,
                status: String::new(),
            };
            app.world_mut().entity_mut(window).insert(workspace);
            app.world_mut().spawn(PointerId::Mouse);
            app.update();
            Self {
                app,
                window,
                camera,
                object,
            }
        }
        fn field(&mut self, field: Field, axis: usize) -> Entity {
            let world = self.app.world_mut();
            world
                .query::<(Entity, &InputBinding, Option<&FieldAxis>)>()
                .iter(world)
                .find(|(_, binding, a)| binding.field == field && a.map_or(0, |a| a.0) == axis)
                .unwrap()
                .0
        }
        fn click(&mut self, field: Entity) {
            // Supply backend hits, not focus or precomputed text edits.
            self.app
                .world_mut()
                .get_mut::<ComputedNode>(field)
                .unwrap()
                .size = Vec2::new(200., 25.);
            let location = Location {
                target: RenderTarget::Window(WindowRef::Entity(self.window))
                    .normalize(None)
                    .unwrap(),
                position: Vec2::ZERO,
            };
            for action in [
                PointerAction::Move { delta: Vec2::ZERO },
                PointerAction::Press(PointerButton::Primary),
                PointerAction::Release(PointerButton::Primary),
            ] {
                self.app.world_mut().write_message(PointerInput::new(
                    PointerId::Mouse,
                    location.clone(),
                    action,
                ));
            }
            self.app.world_mut().write_message(PointerHits::new(
                PointerId::Mouse,
                vec![(field, HitData::new(self.camera, 0., None, None))],
                10.,
            ));
            self.app.update();
        }
        fn key(&mut self, code: KeyCode, key: Key, text: Option<&str>) {
            for state in [ButtonState::Pressed, ButtonState::Released] {
                self.app.world_mut().write_message(KeyboardInput {
                    key_code: code,
                    logical_key: key.clone(),
                    text: text.map(Into::into),
                    state,
                    repeat: false,
                    window: self.window,
                });
            }
        }
        #[track_caller]
        fn text(&self, field: Entity) -> String {
            self.app
                .world()
                .get::<EditableText>(field)
                .expect("focused input must remain live after a UI rebuild")
                .value()
                .to_string()
        }
        #[track_caller]
        fn caret(&self, field: Entity) {
            let (visible, rect) = self
                .app
                .world()
                .get::<TextLayoutInfo>(field)
                .unwrap()
                .cursor
                .expect("focused input must prepare a caret for the native renderer");
            assert!(visible && rect.width() > 0. && rect.height() > 0.);
        }
    }

    #[test]
    fn retained_status_tooltip_updates_when_its_message_changes() {
        use std::time::Duration;
        let mut h = TextHarness::new();
        h.app
            .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
                Duration::from_millis(600),
            ));
        h.app
            .world_mut()
            .resource_mut::<Time<bevy::time::Virtual>>()
            .set_max_delta(Duration::from_secs(1));
        h.app
            .world_mut()
            .get_mut::<EditorWindow>(h.window)
            .unwrap()
            .status = "Before".into();
        h.app.world_mut().resource_mut::<UiDirty>().0 = true;
        h.app.update();
        let status = {
            let world = h.app.world_mut();
            world
                .query::<(Entity, &StatusMessage)>()
                .iter(world)
                .next()
                .unwrap()
                .0
        };
        for _ in 0..3 {
            h.click(status);
        }
        assert_eq!(
            h.app
                .world_mut()
                .query::<&Text>()
                .iter(h.app.world())
                .filter(|t| t.0 == "Before")
                .count(),
            2,
            "fixture must show both the retained status and its tooltip"
        );
        h.app
            .world_mut()
            .get_mut::<EditorWindow>(h.window)
            .unwrap()
            .status = "After".into();
        h.app.world_mut().resource_mut::<UiDirty>().0 = true;
        h.click(status);
        assert_eq!(
            h.app
                .world_mut()
                .query::<&Text>()
                .iter(h.app.world())
                .filter(|t| t.0 == "After")
                .count(),
            2,
            "a visible tooltip must not retain the previous status message"
        );
        assert!(
            !h.app
                .world_mut()
                .query::<&Text>()
                .iter(h.app.world())
                .any(|t| t.0 == "Before")
        );
    }

    #[test]
    fn dynamic_entity_and_layout_labels_preserve_icon_like_name_prefixes() {
        // User data remains literal text even when it starts with a former icon token.
        let mut h = TextHarness::new();
        for name in [
            "◇ Custom entity",
            "⌄ Folder name",
            "+ Literal plus",
            "● Status-like name",
        ] {
            h.app
                .world_mut()
                .get_mut::<Name>(h.object)
                .unwrap()
                .set(name);
            h.app
                .world_mut()
                .get_mut::<EditorWindow>(h.window)
                .unwrap()
                .document
                .layouts[0]
                .name = name.into();
            h.app.world_mut().resource_mut::<UiDirty>().0 = true;
            h.app.update();
            let world = h.app.world_mut();
            assert_eq!(
                world
                    .query::<&Text>()
                    .iter(world)
                    .filter(|text| text.0 == name)
                    .count(),
                2,
                "both the world tree and layout selector must preserve {name:?}",
            );
        }
    }

    struct LiveViewport {
        camera: Entity,
        surface: Entity,
        pointer: PointerId,
        target: Handle<Image>,
        pane: Id,
    }
    impl LiveViewport {
        fn capture(h: &mut TextHarness, window: Entity, preview: bool) -> Self {
            let world = h.app.world_mut();
            let (camera, target, pane) = world
                .query::<(Entity, &SceneView)>()
                .iter(world)
                .find(|(_, view)| view.window == window && view.preview == preview)
                .map(|(camera, view)| (camera, view.target.clone(), view.pane))
                .unwrap();
            let (surface, pointer) = world
                .query::<(Entity, &ViewportNode, &PointerId)>()
                .iter(world)
                .find(|(_, node, _)| node.camera == Some(camera))
                .map(|(surface, _, pointer)| (surface, *pointer))
                .unwrap();
            Self {
                camera,
                surface,
                pointer,
                target,
                pane,
            }
        }
        #[track_caller]
        fn assert_retained(&self, world: &World) {
            assert!(
                world.get::<SceneView>(self.camera).is_some(),
                "live camera was replaced"
            );
            assert_eq!(
                world
                    .get::<ViewportNode>(self.surface)
                    .and_then(|node| node.camera),
                Some(self.camera),
                "live viewport surface was replaced"
            );
            assert_eq!(world.get::<PointerId>(self.surface), Some(&self.pointer));
            assert_eq!(
                world.get::<SceneView>(self.camera).unwrap().target,
                self.target
            );
            assert!(world.resource::<Assets<Image>>().contains(self.target.id()));
            assert!(world.get::<ChildOf>(self.surface).is_some());
        }
        #[track_caller]
        fn assert_removed(&self, world: &World) {
            assert!(world.get_entity(self.camera).is_err());
            assert!(world.get_entity(self.surface).is_err());
            assert!(!world.resource::<Assets<Image>>().contains(self.target.id()));
        }
    }

    #[test]
    fn ui_actions_keep_live_viewport_surfaces_cameras_and_render_targets() {
        let mut h = TextHarness::new();
        let window = h.window;
        let editor = LiveViewport::capture(&mut h, window, false);
        let preview = LiveViewport::capture(&mut h, window, true);
        for view in [&editor, &preview] {
            h.app
                .world_mut()
                .resource_mut::<Assets<Image>>()
                .get_mut(&view.target)
                .unwrap()
                .resize(bevy::render::render_resource::Extent3d {
                    width: 320,
                    height: 180,
                    depth_or_array_layers: 1,
                });
        }
        for action in [
            Action::Menu(Menu::Header("File")),
            Action::Dismiss,
            Action::Select(h.object),
            Action::Help,
        ] {
            dispatch(h.app.world_mut(), h.window, action);
            h.app.update();
            for view in [&editor, &preview] {
                view.assert_retained(h.app.world());
                assert_eq!(
                    h.app
                        .world()
                        .resource::<Assets<Image>>()
                        .get(&view.target)
                        .unwrap()
                        .size(),
                    UVec2::new(320, 180)
                );
            }
        }
        let name = h.field(Field::Name, 0);
        h.click(name);
        h.key(KeyCode::KeyX, Key::Character("X".into()), Some("X"));
        h.key(KeyCode::Enter, Key::Enter, None);
        h.app.update();
        h.app.update();
        assert_eq!(
            h.app.world().get::<Name>(h.object).unwrap().as_str(),
            "CubeX"
        );
        editor.assert_retained(h.app.world());
        preview.assert_retained(h.app.world());
        {
            let mut workspace = h.app.world_mut().get_mut::<EditorWindow>(window).unwrap();
            let pane = workspace.document.root_mut().pane_mut(editor.pane).unwrap();
            pane.orbit[2] = 31.;
            pane.center[0] = 2.;
        }
        dispatch(h.app.world_mut(), window, Action::Help);
        h.app.update();
        editor.assert_retained(h.app.world());
        assert_eq!(
            h.app.world().get::<SceneView>(editor.camera).unwrap().orbit[2],
            31.
        );
        assert_eq!(
            h.app
                .world()
                .get::<SceneView>(editor.camera)
                .unwrap()
                .center
                .x,
            2.
        );
        dispatch(h.app.world_mut(), window, Action::ResetLayout);
        h.app.update();
        editor.assert_retained(h.app.world());
        preview.assert_retained(h.app.world());
        assert_eq!(
            h.app.world().get::<SceneView>(editor.camera).unwrap().orbit[2],
            14.5
        );
        assert_eq!(
            h.app
                .world()
                .get::<SceneView>(editor.camera)
                .unwrap()
                .center
                .x,
            0.
        );
    }

    #[test]
    fn viewport_resources_are_scoped_and_released_when_no_longer_visible() {
        let mut h = TextHarness::new();
        let window = h.window;
        let editor = LiveViewport::capture(&mut h, window, false);
        let preview = LiveViewport::capture(&mut h, window, true);
        dispatch(h.app.world_mut(), h.window, Action::NewWindow);
        h.app.update();
        let other = h
            .app
            .world_mut()
            .query::<(Entity, &EditorWindow)>()
            .iter(h.app.world())
            .find(|(window, _)| *window != h.window)
            .unwrap()
            .0;
        let linked = LiveViewport::capture(&mut h, other, false);
        assert_ne!(editor.camera, linked.camera);
        assert_ne!(editor.target, linked.target);
        editor.assert_retained(h.app.world());
        preview.assert_retained(h.app.world());
        // A copied layout reuses Pane IDs but must not inherit another layout's live views.
        dispatch(h.app.world_mut(), window, Action::CopyLayout);
        h.app.update();
        editor.assert_removed(h.app.world());
        preview.assert_removed(h.app.world());
        linked.assert_retained(h.app.world());
        let editor = LiveViewport::capture(&mut h, window, false);
        let preview = LiveViewport::capture(&mut h, window, true);
        let group = h
            .app
            .world()
            .get::<EditorWindow>(h.window)
            .unwrap()
            .document
            .root()
            .pane_group(editor.pane)
            .unwrap();
        dispatch(
            h.app.world_mut(),
            h.window,
            Action::ChangeType {
                group,
                pane: Some(editor.pane),
                kind: PaneType::Camera,
            },
        );
        h.app.update();
        editor.assert_removed(h.app.world());
        preview.assert_retained(h.app.world());
        linked.assert_retained(h.app.world());
        dispatch(h.app.world_mut(), h.window, Action::Close(preview.pane));
        h.app.update();
        preview.assert_removed(h.app.world());
        linked.assert_retained(h.app.world());
        h.app.world_mut().despawn(other);
        h.app.update();
        linked.assert_removed(h.app.world());
        let original = h
            .app
            .world_mut()
            .query::<(Entity, &SceneView)>()
            .iter(h.app.world())
            .map(|(camera, view)| (camera, view.target.clone()))
            .collect::<Vec<_>>();
        dispatch(h.app.world_mut(), h.window, Action::Layout(1));
        h.app.update();
        for (camera, target) in original {
            assert!(h.app.world().get_entity(camera).is_err());
            assert!(
                !h.app
                    .world()
                    .resource::<Assets<Image>>()
                    .contains(target.id())
            );
        }
    }

    #[test]
    fn native_input_click_prepares_caret_and_accepts_typing_backspace_and_tab_navigation() {
        let mut h = TextHarness::new();
        let name = h.field(Field::Name, 0);
        h.click(name);
        assert_eq!(h.app.world().resource::<InputFocus>().get(), Some(name));
        h.caret(name);
        h.key(KeyCode::KeyX, Key::Character("X".into()), Some("X"));
        h.app.update();
        assert_eq!(h.text(name), "CubeX");
        h.caret(name);
        h.key(KeyCode::Backspace, Key::Backspace, None);
        h.app.update();
        assert_eq!(h.text(name), "Cube");
        h.key(KeyCode::Home, Key::Home, None);
        h.key(KeyCode::KeyB, Key::Character("B".into()), Some("B"));
        h.app.update();
        assert_eq!(h.text(name), "BCube");
        let position = h.field(Field::Position, 0);
        h.key(KeyCode::Tab, Key::Tab, None);
        h.app.update();
        assert_eq!(h.app.world().resource::<InputFocus>().get(), Some(position));
        h.caret(position);
        assert_eq!(
            h.app.world().get::<Name>(h.object).unwrap().as_str(),
            "BCube"
        );
        h.app.world_mut().write_message(KeyboardInput {
            key_code: KeyCode::ShiftLeft,
            logical_key: Key::Shift,
            text: None,
            state: ButtonState::Pressed,
            repeat: false,
            window: h.window,
        });
        h.app.update();
        let name = h.field(Field::Name, 0);
        h.key(KeyCode::Tab, Key::Tab, None);
        h.app.update();
        assert_eq!(h.app.world().resource::<InputFocus>().get(), Some(name));
        h.caret(name);
        h.app.world_mut().write_message(KeyboardInput {
            key_code: KeyCode::ShiftLeft,
            logical_key: Key::Shift,
            text: None,
            state: ButtonState::Released,
            repeat: false,
            window: h.window,
        });
        h.app.update();
        let doc = &h
            .app
            .world()
            .get::<EditorWindow>(h.window)
            .unwrap()
            .document;
        let pane = doc.root().group(doc.root().first_group()).unwrap().active;
        let search = h.field(Field::Query(pane), 0);
        h.click(search);
        assert_eq!(h.app.world().resource::<InputFocus>().get(), Some(search));
        assert_eq!(h.text(search), "");
        h.caret(search);
        for (code, text) in [
            (KeyCode::KeyC, "C"),
            (KeyCode::KeyU, "u"),
            (KeyCode::KeyB, "b"),
            (KeyCode::KeyE, "e"),
        ] {
            h.key(code, Key::Character(text.into()), Some(text));
        }
        h.key(KeyCode::Enter, Key::Enter, None);
        h.app.update();
        assert_eq!(
            h.app
                .world()
                .get::<EditorWindow>(h.window)
                .unwrap()
                .document
                .root()
                .pane(pane)
                .unwrap()
                .query,
            "Cube"
        );
        h.app.update();
        let focused = h.app.world().resource::<InputFocus>().get().unwrap();
        assert_eq!(h.text(focused), "Cube");
        h.caret(focused);
        h.key(KeyCode::Home, Key::Home, None);
        h.key(KeyCode::KeyN, Key::Character("N".into()), Some("N"));
        h.app.update();
        let preset_button = h
            .app
            .world_mut()
            .query::<(Entity, &UiAction)>()
            .iter(h.app.world())
            .find(|(_, action)| matches!(action.action, Action::Layout(1)))
            .unwrap()
            .0;
        // Pane IDs repeat between presets: blur must commit to the originating layout.
        dispatch(h.app.world_mut(), h.window, Action::Layout(1));
        h.click(preset_button);
        let doc = &h
            .app
            .world()
            .get::<EditorWindow>(h.window)
            .unwrap()
            .document;
        assert_eq!(doc.layouts[0].root.pane(pane).unwrap().query, "NCube");
        assert_eq!(doc.layouts[1].root.pane(pane).unwrap().query, "");
    }

    #[test]
    fn native_submission_applies_last_keystroke_and_retains_draft_focus_and_caret_on_rebuild() {
        let mut h = TextHarness::new();
        let name = h.field(Field::Name, 0);
        h.click(name);
        // Both events in one frame: submission must happen after the queued native insert.
        h.key(KeyCode::KeyX, Key::Character("X".into()), Some("X"));
        h.key(KeyCode::Enter, Key::Enter, None);
        h.app.update();
        assert_eq!(
            h.app.world().get::<Name>(h.object).unwrap().as_str(),
            "CubeX"
        );
        h.app.update();
        let focused = h.app.world().resource::<InputFocus>().get().unwrap();
        assert_eq!(h.text(focused), "CubeX");
        h.caret(focused);
        h.key(KeyCode::Home, Key::Home, None);
        h.app.update();
        // Change shell structure while typing: a refresh notification alone is now incremental.
        h.app
            .world_mut()
            .get_mut::<EditorWindow>(h.window)
            .unwrap()
            .document
            .layouts[0]
            .name
            .push('!');
        h.app.world_mut().resource_mut::<UiDirty>().0 = true;
        h.key(KeyCode::KeyY, Key::Character("Y".into()), Some("Y"));
        h.app.update();
        let focused = h.app.world().resource::<InputFocus>().get().unwrap();
        assert_eq!(h.text(focused), "YCubeX");
        assert_eq!(
            h.app.world().get::<Name>(h.object).unwrap().as_str(),
            "CubeX"
        );
        h.caret(focused);
        let y = h.field(Field::Position, 1);
        // A structural UI action and blur in the same frame must not discard the previous draft.
        h.app
            .world_mut()
            .get_mut::<EditorWindow>(h.window)
            .unwrap()
            .document
            .layouts[0]
            .name
            .push('!');
        h.app.world_mut().resource_mut::<UiDirty>().0 = true;
        h.click(y);
        assert_eq!(
            h.app.world().get::<Name>(h.object).unwrap().as_str(),
            "YCubeX"
        );
        h.app.update();
        let focused = h.app.world().resource::<InputFocus>().get().unwrap();
        assert!(matches!(
            h.app.world().get::<InputBinding>(focused).unwrap().field,
            Field::Position
        ));
        assert_eq!(h.app.world().get::<FieldAxis>(focused).unwrap().0, 1);
        for _ in 0..4 {
            h.key(KeyCode::Backspace, Key::Backspace, None);
        }
        h.key(KeyCode::Digit2, Key::Character("2".into()), Some("2"));
        h.key(KeyCode::Enter, Key::Enter, None);
        h.app.update();
        assert_eq!(
            h.app
                .world()
                .get::<Transform>(h.object)
                .unwrap()
                .translation,
            Vec3::new(0., 2., 0.)
        );
        h.app.update();
        let focused = h.app.world().resource::<InputFocus>().get().unwrap();
        assert_eq!(h.text(focused), "2");
        h.caret(focused);
        h.key(KeyCode::Backspace, Key::Backspace, None);
        for (code, character) in [
            (KeyCode::KeyN, "N"),
            (KeyCode::KeyA, "a"),
            (KeyCode::KeyN, "N"),
        ] {
            h.key(code, Key::Character(character.into()), Some(character));
        }
        h.key(KeyCode::Enter, Key::Enter, None);
        h.app.update();
        h.app.update();
        assert_eq!(
            h.app
                .world()
                .get::<Transform>(h.object)
                .unwrap()
                .translation
                .y,
            2.
        );
        let focused = h.app.world().resource::<InputFocus>().get().unwrap();
        assert_eq!(h.text(focused), "NaN");
        assert!(
            h.app
                .world()
                .get::<EditorWindow>(h.window)
                .unwrap()
                .status
                .contains("Invalid number")
        );
        h.caret(focused);
        let name = h.field(Field::Name, 0);
        h.click(name);
        h.app.update();
        let focused = h.app.world().resource::<InputFocus>().get().unwrap();
        assert_eq!(
            h.text(focused),
            "NaN",
            "blur must retain and refocus the invalid draft"
        );
        assert!(h.app.world().get::<FieldError>(focused).is_some());
        // The native TextInput observer clears InputFocus before Update sees Escape.
        // Deliver the real keyboard event, not a direct keyboard-system invocation.
        h.key(KeyCode::Escape, Key::Escape, None);
        h.app.update();
        h.app.update();
        assert!(h.app.world().resource::<InputFocus>().get().is_none());
        let y = h.field(Field::Position, 1);
        assert_eq!(h.text(y), "2.00");
        assert!(h.app.world().get::<FieldError>(y).is_none());
        assert_eq!(
            h.app
                .world()
                .get::<Transform>(h.object)
                .unwrap()
                .translation,
            Vec3::new(0., 2., 0.)
        );
    }

    #[test]
    fn rebuilding_duplicate_inspectors_restores_the_same_pane_and_clears_hidden_fields() {
        let mut h = TextHarness::new();
        let other_pane = h
            .app
            .world_mut()
            .query::<&SceneView>()
            .iter(h.app.world())
            .find(|view| view.preview)
            .unwrap()
            .pane;
        h.app
            .world_mut()
            .get_mut::<EditorWindow>(h.window)
            .unwrap()
            .document
            .root_mut()
            .pane_mut(other_pane)
            .unwrap()
            .kind = PaneType::Inspector;
        h.app.world_mut().resource_mut::<UiDirty>().0 = true;
        h.app.update();
        let name = h
            .app
            .world_mut()
            .query::<(Entity, &InputBinding)>()
            .iter(h.app.world())
            .find(|(_, binding)| binding.field == Field::Name && binding.pane == other_pane)
            .unwrap()
            .0;
        h.click(name);
        h.key(KeyCode::Home, Key::Home, None);
        h.key(KeyCode::Digit9, Key::Character("9".into()), Some("9"));
        h.app.update();
        h.app
            .world_mut()
            .get_mut::<EditorWindow>(h.window)
            .unwrap()
            .document
            .layouts[0]
            .name
            .push('!');
        h.app.world_mut().resource_mut::<UiDirty>().0 = true;
        h.app.update();
        let focused = h.app.world().resource::<InputFocus>().get().unwrap();
        assert_eq!(
            h.app.world().get::<InputBinding>(focused).unwrap().pane,
            other_pane
        );
        assert_eq!(h.text(focused), "9Cube");
        h.key(KeyCode::KeyX, Key::Character("X".into()), Some("X"));
        h.app.update();
        assert_eq!(h.text(focused), "9XCube");
        h.app
            .world_mut()
            .get_mut::<EditorWindow>(h.window)
            .unwrap()
            .document
            .root_mut()
            .pane_mut(other_pane)
            .unwrap()
            .kind = PaneType::Viewport;
        h.app.world_mut().resource_mut::<UiDirty>().0 = true;
        h.app.update();
        assert_eq!(h.app.world().resource::<InputFocus>().get(), None);
        assert_eq!(
            h.app.world().get::<Name>(h.object).unwrap().as_str(),
            "Cube"
        );
    }
}
