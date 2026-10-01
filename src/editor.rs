use crate::{
    assets::AssetIndex,
    layout::{Axis, *},
    scene::{self, EditorObject, ModelKind, ObjectCounter, SceneView},
};
use bevy::{
    camera::{NormalizedRenderTarget, RenderTarget, visibility::RenderLayers},
    gizmos::transform_gizmo::{
        TransformGizmoMode, TransformGizmoSettings, TransformGizmoSpace, TransformGizmoState,
    },
    input::{
        keyboard::KeyCode,
        mouse::{MouseScrollUnit, MouseWheel},
    },
    input_focus::{InputFocus, tab_navigation::TabNavigationPlugin},
    picking::hover::{HoverMap, Hovered},
    prelude::*,
    text::EditableText,
    ui::Pressed,
    ui_widgets::Activate,
    window::{PrimaryWindow, WindowRef},
};
use std::{fs, path::Path};

pub struct EditorPlugin;
impl Plugin for EditorPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Selection>()
            .init_resource::<ObjectCounter>()
            .add_plugins((
                TabNavigationPlugin,
                MeshPickingPlugin,
                crate::transform_gizmo::EditorTransformGizmoPlugin,
                crate::cursor::EditorCursorPlugin,
                EditorUiPlugin,
            ))
            .add_systems(Startup, initialize)
            .add_systems(
                Update,
                (
                    scroll,
                    scene::sync_previews,
                    scene::gizmos,
                    crate::ui::geometry,
                    hover_style,
                    crate::ui::placeholders,
                )
                    .chain(),
            )
            .add_observer(activate)
            .add_observer(click_tab)
            .add_observer(cancel_drag)
            .add_observer(begin_drag)
            .add_observer(drag)
            .add_observer(end_drag)
            .add_observer(scene::pick_object);
    }
}
/// Retained UI lifecycle and native text submission, also usable without the demo's 3D setup.
pub struct EditorUiPlugin;
impl Plugin for EditorUiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(crate::icons::LucideIconsPlugin)
            .init_resource::<crate::ui::TooltipState>()
            .init_resource::<UiDirty>()
            .init_resource::<LastFocus>()
            .init_resource::<DragState>()
            .add_systems(Update, keyboard.before(scroll).before(refresh_ui))
            .add_systems(
                Update,
                refresh_ui
                    .after(crate::ui::geometry)
                    .before(hover_style)
                    .before(crate::ui::placeholders),
            )
            .add_systems(
                PostUpdate,
                (
                    commit_fields.after(bevy::text::EditableTextSystems),
                    crate::ui::popup_anchors.after(bevy::ui::UiSystems::Layout),
                    crate::ui::tooltips.after(bevy::ui::UiSystems::Layout),
                    crate::ui::scrollbar_visibility.after(bevy::ui::UiSystems::Layout),
                    crate::ui::field_feedback
                        .after(bevy::text::EditableTextSystems)
                        .after(commit_fields),
                ),
            );
    }
}
#[derive(Resource, Default)]
pub struct Selection(pub Option<Entity>);
/// A coalesced refresh notification. Only changed window structure recreates a shell.
#[derive(Resource)]
pub struct UiDirty(pub bool);
impl Default for UiDirty {
    fn default() -> Self {
        Self(true)
    }
}
#[derive(Component, Clone)]
pub struct EditorWindow {
    pub document: LayoutDocument,
    pub camera: Entity,
    pub root: Option<Entity>,
    pub focused: Id,
    pub maximized: Option<Id>,
    pub menu: Option<Menu>,
    pub status: String,
}
#[derive(Clone, PartialEq, Eq)]
pub enum Menu {
    Header(&'static str),
    PaneType { group: Id, pane: Option<Id> },
    Group(Id),
    Help,
}
#[derive(Clone)]
pub enum Action {
    Menu(Menu),
    Dismiss,
    Layout(usize),
    CopyLayout,
    ResetLayout,
    Save,
    Load,
    NewWindow,
    Tab {
        group: Id,
        pane: Id,
    },
    Close(Id),
    ChangeType {
        group: Id,
        pane: Option<Id>,
        kind: PaneType,
    },
    Split {
        group: Id,
        axis: Axis,
    },
    Maximize(Id),
    FocusGroup(Id),
    Select(Entity),
    Collapse {
        pane: Id,
        key: u64,
    },
    Visible(Entity),
    Add(ModelKind),
    RefreshAssets,
    DeleteSelection,
    Help,
    GizmoMode(TransformGizmoMode),
    GizmoSpace,
}
#[derive(Component, Clone)]
pub struct UiAction {
    pub window: Entity,
    pub action: Action,
}
#[derive(Component, Clone)]
pub enum DragHandle {
    Tab {
        window: Entity,
        pane: Id,
    },
    Splitter {
        window: Entity,
        id: Id,
        axis: Axis,
    },
    Axis {
        entity: Entity,
        field: Field,
        axis: usize,
    },
    View {
        camera: Entity,
    },
}
#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    Name,
    Position,
    Rotation,
    Scale,
    Query(Id),
}
#[derive(Component, Clone)]
pub struct InputBinding {
    pub window: Entity,
    pub layout: usize,
    pub pane: Id,
    pub entity: Option<Entity>,
    pub field: Field,
    pub committed: String,
}
impl InputBinding {
    fn key(&self, axis: usize) -> (Entity, usize, Id, Option<Entity>, Field, usize) {
        (
            self.window,
            self.layout,
            self.pane,
            self.entity,
            self.field,
            axis,
        )
    }
}
#[derive(Component, Clone, Copy)]
pub(crate) struct FieldError(pub &'static str);
#[derive(Resource, Default)]
struct LastFocus(Option<Entity>);
#[derive(Resource, Default)]
pub struct DragState {
    pub tab: Option<(Entity, Id)>,
    pub drop: Option<(Entity, Id, Zone)>,
}
#[derive(Component)]
pub struct UiOwned(pub Entity);
#[derive(Component)]
pub struct DockRegion {
    pub window: Entity,
    pub id: Id,
    pub splitter: bool,
}
#[derive(Component)]
pub struct DropPreview(pub Entity);
#[derive(Component)]
pub struct BaseColor(pub Color);

fn initialize(world: &mut World) {
    let window = world
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(world)
        .expect("primary window");
    let loaded = fs::read_to_string("editor-layout.json");
    let (document, warning) = match loaded {
        Ok(json) => match LayoutDocument::from_json(&json) {
            Ok(document) => (document, None),
            Err(e) => (
                LayoutDocument::default(),
                Some(format!("Invalid saved layout; using defaults: {e}")),
            ),
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => (LayoutDocument::default(), None),
        Err(e) => (
            LayoutDocument::default(),
            Some(format!("Cannot read saved layout: {e}")),
        ),
    };
    install_window(world, window, document);
    if let Some(message) = warning {
        world.get_mut::<EditorWindow>(window).unwrap().status = message;
    }
    let index = AssetIndex::scan(world.resource::<AssetServer>());
    world.insert_resource(index);
    scene::setup(world);
    let mut configs = world.resource_mut::<GizmoConfigStore>();
    let (config, _) = configs.config_mut::<DefaultGizmoConfigGroup>();
    config.render_layers = RenderLayers::layer(1);
}
fn install_window(world: &mut World, window: Entity, document: LayoutDocument) {
    let camera = world
        .spawn((
            Camera2d,
            Camera {
                order: 10,
                ..default()
            },
            RenderTarget::Window(WindowRef::Entity(window)),
            UiOwned(window),
        ))
        .id();
    let focused = document.root().first_group();
    world.entity_mut(window).insert(EditorWindow {
        document,
        camera,
        root: None,
        focused,
        maximized: None,
        menu: None,
        status: "Ready · layout changes are saved explicitly".into(),
    });
}
pub fn dispatch(world: &mut World, window: Entity, action: Action) {
    let Some(mut workspace) = world.get::<EditorWindow>(window).cloned() else {
        return;
    };
    let previous_menu = workspace.menu.take();
    match action {
        Action::Menu(menu) => {
            if previous_menu.as_ref() != Some(&menu) {
                workspace.menu = Some(menu);
            }
        }
        Action::Dismiss => {}
        Action::Layout(i) => {
            if i < workspace.document.layouts.len() {
                workspace.document.active = i;
                workspace.maximized = None;
                workspace.focused = workspace.document.root().first_group();
            }
        }
        Action::CopyLayout => {
            if workspace.document.layouts.len() < 20 {
                let mut layout = workspace.document.layouts[workspace.document.active].clone();
                layout.name = format!("Custom {}", workspace.document.layouts.len() - 2);
                workspace.document.layouts.push(layout);
                workspace.document.active = workspace.document.layouts.len() - 1;
            }
        }
        Action::ResetLayout => {
            let name = workspace.document.layouts[workspace.document.active]
                .name
                .clone();
            workspace.document.layouts[workspace.document.active] = preset(&name);
            workspace.maximized = None;
            workspace.focused = workspace.document.root().first_group();
        }
        Action::Save => {
            workspace.status =
                match save_document(&workspace.document, Path::new("editor-layout.json")) {
                    Ok(()) => "Layout saved · editor-layout.json".into(),
                    Err(e) => format!("Save failed: {e}"),
                };
        }
        Action::Load => {
            let loaded = fs::read_to_string("editor-layout.json")
                .map_err(|e| e.to_string())
                .and_then(|json| LayoutDocument::from_json(&json));
            match loaded {
                Ok(document) => {
                    workspace.document = document;
                    workspace.maximized = None;
                    workspace.focused = workspace.document.root().first_group();
                    workspace.status = "Layout loaded".into();
                }
                Err(e) => workspace.status = format!("Load failed: {e}"),
            }
        }
        Action::NewWindow => {
            let window = world
                .spawn(Window {
                    title: "Koiro Editor · linked world".into(),
                    resolution: (1200, 800).into(),
                    ..default()
                })
                .id();
            install_window(world, window, workspace.document.clone());
        }
        Action::Tab { group, pane } => {
            if let Some(g) = workspace.document.root_mut().group_mut(group)
                && g.tabs.iter().any(|p| p.id == pane)
            {
                g.active = pane;
                workspace.focused = group;
            }
        }
        Action::Close(pane) => {
            workspace.document.root_mut().close(pane);
            workspace.maximized = None;
            workspace.focused = workspace.document.root().first_group();
        }
        Action::ChangeType { group, pane, kind } => {
            if let Some(id) = pane {
                if let Some(p) = workspace.document.root_mut().pane_mut(id) {
                    p.kind = kind;
                    p.query.clear();
                    p.collapsed.clear();
                }
            } else if workspace.document.root_mut().add(group, kind).is_none() {
                workspace.status = "Cannot add a tab: invalid group or layout limit reached".into();
            }
        }
        Action::Split { group, axis } => {
            let mut next = workspace.document.root().clone();
            let zone = if axis == Axis::X {
                Zone::Right
            } else {
                Zone::Bottom
            };
            if let Some(id) = next.add(group, PaneType::Viewport)
                && next.dock(id, group, zone).is_ok()
            {
                *workspace.document.root_mut() = next;
            } else {
                workspace.status = "Cannot split: layout limit reached".into();
            }
            workspace.maximized = None;
        }
        Action::Maximize(group) => {
            workspace.maximized = if workspace.maximized == Some(group) {
                None
            } else {
                Some(group)
            };
        }
        Action::FocusGroup(group) => {
            if workspace.document.root().group(group).is_some() {
                workspace.focused = group;
                workspace.maximized = None;
            }
        }
        Action::Select(entity) => {
            if world.get::<EditorObject>(entity).is_some() {
                world.resource_mut::<Selection>().0 = Some(entity);
            }
        }
        Action::Collapse { pane, key } => {
            if let Some(p) = workspace.document.root_mut().pane_mut(pane) {
                if let Some(i) = p.collapsed.iter().position(|k| *k == key) {
                    p.collapsed.remove(i);
                } else {
                    p.collapsed.push(key);
                }
            }
        }
        Action::Visible(entity) => {
            if let Some(mut v) = world.get_mut::<Visibility>(entity) {
                *v = if *v == Visibility::Hidden {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                };
            }
        }
        Action::Add(kind) => {
            let entity = scene::spawn_model(world, kind, Vec3::ZERO, None, Some(0));
            world.resource_mut::<Selection>().0 = Some(entity);
            workspace.status = format!("Added {}", kind.name());
        }
        Action::RefreshAssets => {
            let index = AssetIndex::scan(world.resource::<AssetServer>());
            workspace.status = index.message.clone();
            world.insert_resource(index);
        }
        Action::DeleteSelection => {
            if let Some(entity) = world.resource::<Selection>().0
                && let Some(object) = world.get::<EditorObject>(entity)
            {
                if matches!(
                    object.kind,
                    ModelKind::Group | ModelKind::Camera | ModelKind::Light
                ) {
                    workspace.status = "Group / camera / light deletion is not supported".into();
                } else {
                    world.despawn(entity);
                    world.resource_mut::<Selection>().0 = None;
                }
            }
        }
        Action::GizmoMode(mode) => {
            if !world.resource::<TransformGizmoState>().active {
                world.resource_mut::<TransformGizmoSettings>().mode = mode;
            }
        }
        Action::GizmoSpace => {
            if !world.resource::<TransformGizmoState>().active {
                let mut settings = world.resource_mut::<TransformGizmoSettings>();
                settings.space = match settings.space {
                    TransformGizmoSpace::World => TransformGizmoSpace::Local,
                    TransformGizmoSpace::Local => TransformGizmoSpace::World,
                };
            }
        }
        Action::Help => {
            workspace.menu = if previous_menu == Some(Menu::Help) {
                None
            } else {
                Some(Menu::Help)
            }
        }
    }
    world.entity_mut(window).insert(workspace);
    world.resource_mut::<UiDirty>().0 = true;
}
pub fn save_document(doc: &LayoutDocument, path: &Path) -> Result<(), String> {
    doc.validate()?;
    let json = serde_json::to_string_pretty(doc).map_err(|e| e.to_string())?;
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, json).map_err(|e| e.to_string())?;
    fs::rename(&temporary, path).map_err(|e| e.to_string())
}
fn activate(event: On<Activate>, actions: Query<&UiAction>, mut commands: Commands) {
    if let Ok(action) = actions.get(event.entity) {
        let action = action.clone();
        commands.queue(move |world: &mut World| dispatch(world, action.window, action.action));
    }
}
fn click_tab(
    event: On<PointerClick>,
    actions: Query<&UiAction, Without<bevy::ui_widgets::Button>>,
    state: Res<DragState>,
    mut commands: Commands,
) {
    if event.button == PointerButton::Primary
        && state.tab.is_none()
        && let Ok(action) = actions.get(event.entity)
    {
        let action = action.clone();
        commands.queue(move |world: &mut World| dispatch(world, action.window, action.action));
    }
}
fn cancel_drag(
    event: On<PointerCancel>,
    handles: Query<&DragHandle>,
    mut state: ResMut<DragState>,
) {
    // A ViewportNode owns another pointer. Its cancellation must not cancel a UI tab.
    if matches!(handles.get(event.entity), Ok(DragHandle::Tab { .. })) {
        *state = DragState::default();
    }
}
fn begin_drag(
    event: On<PointerDragStart>,
    handles: Query<&DragHandle>,
    mut state: ResMut<DragState>,
) {
    if let Ok(DragHandle::Tab { window, pane }) = handles.get(event.entity)
        && event.button == PointerButton::Primary
    {
        state.tab = Some((*window, *pane));
        state.drop = None;
    }
}
fn drag(event: On<PointerDrag>, handles: Query<&DragHandle>, mut commands: Commands) {
    if let Ok(handle) = handles.get(event.entity) {
        let handle = handle.clone();
        let delta = event.delta;
        let position = event.pointer.position;
        let pointer_window = match event.pointer.target {
            NormalizedRenderTarget::Window(window) => Some(window.entity()),
            _ => None,
        };
        let button = event.button;
        commands.queue(move |world: &mut World| match handle {
            DragHandle::Tab { window, .. } => {
                if button != PointerButton::Primary {
                    return;
                }
                if pointer_window != Some(window) {
                    world.resource_mut::<DragState>().drop = None;
                    return;
                }
                let size = world
                    .get::<Window>(window)
                    .map(crate::ui::workspace_size)
                    .unwrap_or(Vec2::ZERO);
                let Some(workspace) = world.get::<EditorWindow>(window) else {
                    return;
                };
                let rects = crate::ui::dock_rects(
                    workspace.document.root(),
                    Rect::from_corners(Vec2::ZERO, size),
                    crate::ui::visible_group(
                        workspace,
                        world.get::<Window>(window).map_or(1440., Window::width),
                    ),
                );
                let top = world
                    .get::<Window>(window)
                    .map_or(40., |w| crate::ui::workspace_top(w.width()));
                let cursor = position - Vec2::new(5., top + 5.);
                let mut drop = None;
                for (id, rect, splitter) in rects {
                    if !splitter && rect.contains(cursor) {
                        let local = cursor - rect.min;
                        let relative = local / rect.size();
                        // The entire tab strip merges, including labels near side edges.
                        let zone = if local.y
                            < crate::ui::PANE_BORDER_WIDTH + crate::ui::TAB_STRIP_HEIGHT
                        {
                            Zone::Center
                        } else if relative.x < 0.22 {
                            Zone::Left
                        } else if relative.x > 0.78 {
                            Zone::Right
                        } else if relative.y < 0.22 {
                            Zone::Top
                        } else if relative.y > 0.78 {
                            Zone::Bottom
                        } else {
                            Zone::Center
                        };
                        drop = Some((window, id, zone));
                        break;
                    }
                }
                let mut state = world.resource_mut::<DragState>();
                state.drop = drop;
            }
            DragHandle::Splitter { window, id, axis } => {
                if button != PointerButton::Primary {
                    return;
                }
                let size = world
                    .get::<Window>(window)
                    .map(crate::ui::workspace_size)
                    .unwrap_or(Vec2::ONE);
                let doc = &world.get::<EditorWindow>(window).unwrap().document;
                let extent = crate::ui::split_extent(doc.root(), id, size).unwrap_or(1.);
                if let Some(mut ws) = world.get_mut::<EditorWindow>(window)
                    && let Some(DockNode::Split { ratio, .. }) = ws.document.root_mut().find_mut(id)
                {
                    *ratio = (*ratio
                        + (if axis == Axis::X { delta.x } else { delta.y }) / extent.max(1.))
                    .clamp(0.08, 0.92);
                }
            }
            DragHandle::Axis {
                entity,
                field,
                axis,
            } => {
                if button == PointerButton::Primary {
                    edit_axis(
                        world,
                        entity,
                        field,
                        axis,
                        delta.x * if field == Field::Rotation { 0.5 } else { 0.01 },
                    );
                }
            }
            DragHandle::View { camera } => {
                if let Some(mut view) = world.get_mut::<SceneView>(camera) {
                    if view.preview {
                        return;
                    }
                    if button == PointerButton::Secondary {
                        view.orbit[0] -= delta.x * 0.01;
                        view.orbit[1] = (view.orbit[1] + delta.y * 0.01).clamp(0.05, 1.5);
                    } else if button == PointerButton::Middle {
                        let transform = scene::orbit_transform(view.orbit, view.center);
                        view.center +=
                            (-*transform.right() * delta.x + *transform.up() * delta.y) * 0.015;
                    }
                    let (window, pane, orbit, center) =
                        (view.window, view.pane, view.orbit, view.center);
                    let transform = scene::orbit_transform(orbit, center);
                    world.entity_mut(camera).insert(transform);
                    // A shell refresh during orbit/pan must not restore an older document pose.
                    if let Some(mut ws) = world.get_mut::<EditorWindow>(window)
                        && let Some(p) = ws.document.root_mut().pane_mut(pane)
                    {
                        p.orbit = orbit;
                        p.center = center.to_array();
                    }
                }
            }
        });
    }
}
fn end_drag(event: On<PointerDragEnd>, handles: Query<&DragHandle>, mut commands: Commands) {
    if let Ok(handle) = handles.get(event.entity) {
        let handle = handle.clone();
        let pointer_window = match event.pointer.target {
            NormalizedRenderTarget::Window(window) => Some(window.entity()),
            _ => None,
        };
        let button = event.button;
        commands.queue(move |world: &mut World| {
            match handle {
                DragHandle::Tab { window, pane } => {
                    if button != PointerButton::Primary {
                        return;
                    }
                    let drop = world.resource::<DragState>().drop;
                    *world.resource_mut::<DragState>() = DragState::default();
                    if let Some((target_window, group, zone)) = drop
                        && window == target_window
                        && pointer_window == Some(window)
                        && let Some(mut ws) = world.get_mut::<EditorWindow>(window)
                    {
                        let _ = ws.document.root_mut().dock(pane, group, zone);
                        ws.focused = group;
                        ws.maximized = None;
                    }
                }
                DragHandle::View { .. } | DragHandle::Splitter { .. } => {
                    // Camera poses and split geometry already update during the drag.
                    return;
                }
                _ => {}
            }
            world.resource_mut::<UiDirty>().0 = true;
        });
    }
}
fn edit_axis(world: &mut World, entity: Entity, field: Field, axis: usize, delta: f32) {
    if let Some(mut t) = world.get_mut::<Transform>(entity) {
        match field {
            Field::Position => t.translation[axis] += delta,
            Field::Scale => t.scale[axis] = (t.scale[axis] + delta).max(0.001),
            Field::Rotation => {
                let (x, y, z) = t.rotation.to_euler(EulerRot::XYZ);
                let mut v = Vec3::new(x, y, z);
                v[axis] += delta.to_radians();
                t.rotation = Quat::from_euler(EulerRot::XYZ, v.x, v.y, v.z);
            }
            _ => {}
        }
    }
}
fn keyboard(world: &mut World) {
    let key = world.resource::<ButtonInput<KeyCode>>();
    let escape = key.just_pressed(KeyCode::Escape);
    let save = key.just_pressed(KeyCode::KeyS)
        && key.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]);
    let delete = key.just_pressed(KeyCode::Delete);
    let focused_input = world
        .resource::<InputFocus>()
        .get()
        .filter(|e| world.get::<InputBinding>(*e).is_some());
    let typing = focused_input.is_some();
    // Native TextInput clears InputFocus while dispatching Escape in PreUpdate.
    // LastFocus still identifies that draft until commit_fields runs in PostUpdate.
    let cancelled_input = focused_input
        .or_else(|| escape.then_some(world.resource::<LastFocus>().0).flatten())
        .filter(|e| world.get::<InputBinding>(*e).is_some());
    if escape && let Some(input) = cancelled_input {
        let binding = world.get::<InputBinding>(input).unwrap().clone();
        let value = binding.committed;
        if let Some(mut workspace) = world.get_mut::<EditorWindow>(binding.window) {
            workspace.status = "Edit cancelled · scene unchanged".into();
        }
        world
            .entity_mut(input)
            .insert(EditableText::new(value))
            .remove::<FieldError>();
        world.resource_mut::<InputFocus>().clear();
        world.resource_mut::<LastFocus>().0 = None;
    }
    if !(escape || save || delete && !typing) {
        return;
    }
    let window = world
        .query::<(Entity, &Window, &EditorWindow)>()
        .iter(world)
        .find(|(_, w, _)| w.focused)
        .map(|(e, _, _)| e);
    if let Some(window) = window {
        if escape {
            *world.resource_mut::<DragState>() = DragState::default();
        }
        dispatch(
            world,
            window,
            if escape {
                Action::Dismiss
            } else if save {
                Action::Save
            } else {
                Action::DeleteSelection
            },
        );
    }
}
pub fn commit_fields(world: &mut World) {
    let focus = world.resource::<InputFocus>().get();
    let last = world.resource::<LastFocus>().0;
    let enter = world
        .resource::<ButtonInput<KeyCode>>()
        .just_pressed(KeyCode::Enter);
    let mut targets = Vec::new();
    if last != focus
        && let Some(e) = last
    {
        targets.push(e);
    }
    if enter && let Some(e) = focus {
        targets.push(e);
    }
    world.resource_mut::<LastFocus>().0 = focus;
    for e in targets {
        let Some(binding) = world.get::<InputBinding>(e).cloned() else {
            continue;
        };
        let Some(text) = world.get::<EditableText>(e) else {
            continue;
        };
        let value = text.value().to_string();
        if value == binding.committed {
            continue;
        }
        let mut error = None;
        if binding
            .entity
            .is_some_and(|entity| world.get_entity(entity).is_err())
        {
            error = Some("Selected entity no longer exists");
        } else {
            match binding.field {
                Field::Query(pane) => {
                    if value.len() <= 200 {
                        if let Some(mut ws) = world.get_mut::<EditorWindow>(binding.window)
                            && let Some(p) = ws
                                .document
                                .layouts
                                .get_mut(binding.layout)
                                .and_then(|layout| layout.root.pane_mut(pane))
                        {
                            p.query = value.clone();
                        } else {
                            error = Some("Search target no longer exists");
                        }
                    } else {
                        error = Some("Search is limited to 200 bytes");
                    }
                }
                Field::Name => {
                    if value.trim().is_empty() {
                        error = Some("Name must not be empty");
                    } else if let Some(entity) = binding.entity {
                        world.entity_mut(entity).insert(Name::new(value.clone()));
                    }
                }
                field => {
                    let axis = world.get::<crate::ui::FieldAxis>(e).map_or(0, |a| a.0);
                    match value.trim().parse::<f32>() {
                        Ok(number)
                            if number.is_finite() && (field != Field::Scale || number > 0.) =>
                        {
                            if let Some(entity) = binding.entity
                                && let Some(mut t) = world.get_mut::<Transform>(entity)
                            {
                                // Absolute input must not be converted to a delta: two finite,
                                // opposite large values can have an infinite difference.
                                match field {
                                    Field::Position => t.translation[axis] = number,
                                    Field::Scale => t.scale[axis] = number,
                                    _ => {
                                        let (x, y, z) = t.rotation.to_euler(EulerRot::XYZ);
                                        let mut angles = [x, y, z];
                                        angles[axis] = number.to_radians();
                                        t.rotation = Quat::from_euler(
                                            EulerRot::XYZ,
                                            angles[0],
                                            angles[1],
                                            angles[2],
                                        );
                                    }
                                }
                            }
                        }
                        _ => {
                            error = Some(if field == Field::Scale {
                                "Invalid number: scale must be finite and > 0"
                            } else {
                                "Invalid number: enter a finite value"
                            })
                        }
                    }
                }
            }
        }
        if error.is_none()
            && let Some(mut binding) = world.get_mut::<InputBinding>(e)
        {
            binding.committed = value;
        }
        if let Some(message) = error {
            world.entity_mut(e).insert(FieldError(message));
            // Keep the invalid draft visible on blur as well as Enter; let Esc discard it.
            world
                .resource_mut::<InputFocus>()
                .set(e, bevy::input_focus::FocusCause::Navigated);
            world.resource_mut::<LastFocus>().0 = Some(e);
        } else {
            world.entity_mut(e).remove::<FieldError>();
        }
        if let Some(mut ws) = world.get_mut::<EditorWindow>(binding.window) {
            ws.status = error.unwrap_or("Property updated · session only").into();
        }
        world.resource_mut::<UiDirty>().0 = true;
    }
}
fn scroll(
    mut wheel: MessageReader<MouseWheel>,
    hover: Res<HoverMap>,
    mut views: Query<(&mut SceneView, &mut Transform)>,
    viewport_nodes: Query<&bevy::ui::widget::ViewportNode>,
    mut scrolls: Query<(&mut ScrollPosition, &ComputedNode, &Node)>,
    mut workspaces: Query<&mut EditorWindow>,
    parents: Query<&ChildOf>,
) {
    for event in wheel.read() {
        let amount = if event.unit == MouseScrollUnit::Line {
            event.y * 24.
        } else {
            event.y
        };
        let mut applied = false;
        for map in hover.values() {
            for hit in map.keys() {
                let mut entity = *hit;
                loop {
                    if let Ok(node) = viewport_nodes.get(entity)
                        && let Some(camera) = node.camera
                        && let Ok((mut view, mut transform)) = views.get_mut(camera)
                        && view.window == event.window
                        && !view.preview
                    {
                        view.orbit[2] = (view.orbit[2] - amount * 0.05).clamp(2., 80.);
                        *transform = scene::orbit_transform(view.orbit, view.center);
                        if let Ok(mut ws) = workspaces.get_mut(view.window)
                            && let Some(p) = ws.document.root_mut().pane_mut(view.pane)
                        {
                            p.orbit = view.orbit;
                            p.center = view.center.to_array();
                        }
                        applied = true;
                        break;
                    }
                    if let Ok((mut position, computed, node)) = scrolls.get_mut(entity) {
                        let extent = (computed.content_size() - computed.size())
                            * computed.inverse_scale_factor();
                        if node.overflow.y == OverflowAxis::Scroll {
                            position.y = (position.y - amount).clamp(0., extent.y.max(0.));
                            applied = true;
                            break;
                        }
                        if node.overflow.x == OverflowAxis::Scroll {
                            position.x = (position.x - amount).clamp(0., extent.x.max(0.));
                            applied = true;
                            break;
                        }
                    }
                    let Ok(parent) = parents.get(entity) else {
                        break;
                    };
                    entity = parent.parent();
                }
                if applied {
                    break;
                }
            }
            if applied {
                break;
            }
        }
    }
}
fn hover_style(mut buttons: Query<(&Hovered, Has<Pressed>, &BaseColor, &mut BackgroundColor)>) {
    for (hover, pressed, base, mut color) in &mut buttons {
        let new = if pressed {
            Color::srgb_u8(66, 66, 66)
        } else if hover.get() {
            Color::srgb_u8(48, 48, 48)
        } else {
            base.0
        };
        if color.0 != new {
            color.0 = new;
        }
    }
}
struct FocusedField {
    source: Entity,
    binding: InputBinding,
    axis: usize,
    text: EditableText,
    error: Option<FieldError>,
}
impl FocusedField {
    fn capture(world: &World) -> Option<Self> {
        let entity = world.resource::<InputFocus>().get()?;
        Some(Self {
            source: entity,
            binding: world.get::<InputBinding>(entity)?.clone(),
            axis: world
                .get::<crate::ui::FieldAxis>(entity)
                .map_or(0, |axis| axis.0),
            text: world.get::<EditableText>(entity)?.clone(),
            error: world.get::<FieldError>(entity).copied(),
        })
    }
    fn restore(self, world: &mut World) {
        // Incremental refreshes leave the real native editor untouched.
        if world.get_entity(self.source).is_ok() {
            return;
        }
        let key = self.binding.key(self.axis);
        let replacement = world
            .query::<(Entity, &InputBinding, Option<&crate::ui::FieldAxis>)>()
            .iter(world)
            .find(|(_, binding, axis)| binding.key(axis.map_or(0, |axis| axis.0)) == key)
            .map(|(entity, _, _)| entity);
        if let Some(entity) = replacement {
            // Preserve the native editor (selection/caret, draft, composition, queued edits),
            // not just its string. Pane identity keeps duplicate Inspectors from stealing focus.
            world.entity_mut(entity).insert(self.text);
            if let Some(error) = self.error {
                world.entity_mut(entity).insert(error);
            }
            world.get_mut::<InputBinding>(entity).unwrap().committed = self.binding.committed;
            world
                .resource_mut::<InputFocus>()
                .set(entity, bevy::input_focus::FocusCause::Navigated);
            world.resource_mut::<LastFocus>().0 = Some(entity);
        } else {
            world.resource_mut::<InputFocus>().clear();
            world.resource_mut::<LastFocus>().0 = None;
        }
    }
}
fn refresh_ui(world: &mut World) {
    // Remove windows' UI cameras and render targets after the OS window has closed.
    let orphaned: Vec<_> = world
        .query::<(Entity, &UiOwned)>()
        .iter(world)
        .filter(|(_, owner)| world.get::<Window>(owner.0).is_none())
        .map(|(e, _)| e)
        .collect();
    for entity in orphaned {
        if let Some(view) = world.get::<SceneView>(entity) {
            let image = view.target.id();
            world.resource_mut::<Assets<Image>>().remove(image);
        }
        world.despawn(entity);
    }
    if let Some(focus) = world.resource::<InputFocus>().get()
        && world.get_entity(focus).is_err()
    {
        world.resource_mut::<InputFocus>().clear();
    }
    let resized = world
        .query::<(&Window, &EditorWindow)>()
        .iter(world)
        .any(|(window, ws)| {
            ws.root
                .and_then(|root| world.get::<crate::ui::ShellWidth>(root))
                .is_some_and(|shell| shell.0 != window.width())
        });
    if resized {
        world.resource_mut::<UiDirty>().0 = true;
    }
    if !world.resource::<UiDirty>().0 {
        return;
    }
    // A blurred field must survive until PostUpdate applies pending native text edits
    // and commits it. Otherwise a menu/selection action can destroy an unsubmitted draft.
    if let Some(previous) = world.resource::<LastFocus>().0
        && Some(previous) != world.resource::<InputFocus>().get()
        && world.get::<InputBinding>(previous).is_some()
    {
        return;
    }
    let focused = FocusedField::capture(world);
    let scrolls: Vec<_> = world
        .query::<(&crate::ui::PaneScroll, &ScrollPosition)>()
        .iter(world)
        .map(|(key, position)| (key.clone(), position.clone()))
        .collect();
    world.resource_mut::<UiDirty>().0 = false;
    let content_changed = crate::ui::content_changes(world);
    let windows: Vec<_> = world
        .query::<(Entity, &EditorWindow)>()
        .iter(world)
        .map(|(e, w)| (e, w.clone()))
        .collect();
    for (window, workspace) in windows {
        if crate::ui::refresh_window(world, window, &workspace, content_changed) {
            continue;
        }
        // Detach live surfaces before the shell's recursive despawn. render_window reparents
        // matching window/layout/pane/kind identities without replacing cameras or textures.
        let retained: Vec<_> = world
            .query::<(Entity, &crate::ui::ViewportSurface)>()
            .iter(world)
            .filter(|(_, surface)| surface.window == window)
            .map(|(entity, _)| entity)
            .collect();
        for surface in retained {
            world.entity_mut(surface).remove::<ChildOf>();
        }
        if let Some(root) = workspace.root {
            world.despawn(root);
        }
        let root = crate::ui::render_window(world, window, &workspace);
        if let Some(mut workspace) = world.get_mut::<EditorWindow>(window) {
            workspace.root = Some(root);
        }
        // Unclaimed surfaces belong to closed/hidden panes, another layout, or a changed type.
        let unused: Vec<_> = world
            .query_filtered::<(
                Entity,
                &crate::ui::ViewportSurface,
                &bevy::ui::widget::ViewportNode,
            ), Without<ChildOf>>()
            .iter(world)
            .filter(|(_, surface, _)| surface.window == window)
            .map(|(surface, _, node)| (surface, node.camera))
            .collect();
        for (surface, camera) in unused {
            if let Some(camera) = camera {
                if let Some(view) = world.get::<SceneView>(camera) {
                    let image = view.target.id();
                    world.resource_mut::<Assets<Image>>().remove(image);
                }
                world.despawn(camera);
            }
            world.despawn(surface);
        }
    }
    for (key, mut position) in world
        .query::<(&crate::ui::PaneScroll, &mut ScrollPosition)>()
        .iter_mut(world)
    {
        if let Some((_, previous)) = scrolls.iter().find(|(previous, _)| previous == key)
            && (position.x != previous.x || position.y != previous.y)
        {
            *position = previous.clone();
        }
    }
    crate::ui::sync_values(world);
    if let Some(focused) = focused {
        focused.restore(world);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn saving_and_loading_layout_uses_real_files_and_preserves_state() {
        // Protect atomic replacement and validation-before-write at the real persistence boundary.
        let temp = std::env::temp_dir().join(format!("koiro-layout-test-{}", std::process::id()));
        fs::create_dir_all(&temp).unwrap();
        let mut doc = LayoutDocument::default();
        let path = temp.join("layout.json");
        save_document(&doc, &path).unwrap();
        doc.active = 2;
        save_document(&doc, &path).unwrap();
        let bytes = fs::read_to_string(&path).unwrap();
        assert_eq!(LayoutDocument::from_json(&bytes).unwrap().active, 2);
        assert!(!path.with_extension("json.tmp").exists());
        doc.version = 99;
        assert!(save_document(&doc, &path).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), bytes);
        assert!(
            save_document(
                &LayoutDocument::default(),
                &temp.join("missing/layout.json")
            )
            .is_err()
        );
        fs::remove_dir_all(temp).unwrap();
    }
    fn workspace_world() -> (World, Entity) {
        let mut world = World::new();
        world.init_resource::<Selection>();
        world.init_resource::<UiDirty>();
        let window = world.spawn(Window::default()).id();
        install_window(&mut world, window, LayoutDocument::default());
        (world, window)
    }
    #[test]
    fn new_windows_share_selection_but_have_independent_layouts() {
        let (mut world, first) = workspace_world();
        let object = world
            .spawn((
                EditorObject {
                    key: 1,
                    kind: ModelKind::Cube,
                },
                Transform::default(),
            ))
            .id();
        dispatch(&mut world, first, Action::NewWindow);
        let second = world
            .query::<(Entity, &EditorWindow)>()
            .iter(&world)
            .find(|(e, _)| *e != first)
            .unwrap()
            .0;
        dispatch(&mut world, second, Action::Layout(2));
        dispatch(&mut world, second, Action::Select(object));
        assert_eq!(world.get::<EditorWindow>(first).unwrap().document.active, 0);
        assert_eq!(
            world.get::<EditorWindow>(second).unwrap().document.active,
            2
        );
        assert_eq!(world.resource::<Selection>().0, Some(object));
        assert!(world.get::<Transform>(object).is_some());
    }
    fn native_ui_app() -> (App, Entity, Entity) {
        use bevy::{
            asset::AssetPlugin,
            camera::RenderTargetInfo,
            input::InputPlugin,
            input_focus::{
                InputDispatchPlugin, InputFocusPlugin, pointer_focus::PointerFocusPlugin,
            },
            picking::{InteractionPlugin, PickingPlugin, pointer::PointerId},
            text::TextPlugin,
            ui::UiPlugin,
            ui_widgets::{ButtonPlugin, ScrollbarPlugin, TextInputPlugin},
            window::WindowPlugin,
        };
        let mut app = App::new();
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
        ))
        .add_plugins((
            TransformPlugin,
            bevy::mesh::MeshPlugin,
            bevy::camera::visibility::VisibilityPlugin,
            UiPlugin,
            ButtonPlugin,
            ScrollbarPlugin,
            TextInputPlugin,
            EditorUiPlugin,
        ))
        .init_asset::<Image>()
        .init_asset::<TextureAtlasLayout>()
        .init_resource::<Selection>()
        .init_resource::<DragState>()
        .init_resource::<AssetIndex>()
        .init_resource::<scene::Library>()
        .add_observer(activate)
        .add_observer(click_tab)
        .add_observer(drag)
        .add_observer(end_drag)
        .add_systems(Update, crate::ui::geometry.before(refresh_ui));
        let window = app
            .world_mut()
            .spawn((
                Window {
                    resolution: (1440, 900).into(),
                    ..default()
                },
                PrimaryWindow,
            ))
            .id();
        install_window(app.world_mut(), window, LayoutDocument::default());
        let camera = app.world().get::<EditorWindow>(window).unwrap().camera;
        app.world_mut()
            .get_mut::<Camera>(camera)
            .unwrap()
            .computed
            .target_info = Some(RenderTargetInfo {
            physical_size: UVec2::new(1440, 900),
            scale_factor: 1.,
        });
        app.world_mut().spawn(PointerId::Mouse);
        let object = app
            .world_mut()
            .spawn((
                Name::new("Cube"),
                Transform::default(),
                Visibility::Inherited,
                EditorObject {
                    key: 1,
                    kind: ModelKind::Cube,
                },
            ))
            .id();
        app.world_mut().resource_mut::<Selection>().0 = Some(object);
        app.update();
        app.update();
        (app, window, camera)
    }
    #[test]
    fn open_menus_do_not_swallow_other_controls_or_outside_clicks() {
        use bevy::{
            camera::RenderTargetInfo,
            picking::pointer::{Location, PointerAction, PointerId, PointerInput},
            ui::UiGlobalTransform,
        };
        fn action_position(app: &mut App, predicate: impl Fn(&Action) -> bool) -> Vec2 {
            let world = app.world_mut();
            world
                .query::<(&UiAction, &UiGlobalTransform)>()
                .iter(world)
                .find(|(action, _)| predicate(&action.action))
                .expect("visible action")
                .1
                .translation
        }
        fn click(app: &mut App, window: Entity, position: Vec2) {
            let location = Location {
                target: RenderTarget::Window(WindowRef::Entity(window))
                    .normalize(None)
                    .unwrap(),
                position,
            };
            // Real layout/stack/UI hit testing, with press and release on separate frames.
            // Injecting PointerHits would bypass the transparent overlay regression.
            for action in [
                PointerAction::Move { delta: Vec2::ZERO },
                PointerAction::Press(PointerButton::Primary),
                PointerAction::Release(PointerButton::Primary),
            ] {
                app.world_mut().write_message(PointerInput::new(
                    PointerId::Mouse,
                    location.clone(),
                    action,
                ));
                app.update();
            }
            app.update();
        }
        let (mut app, window, camera) = native_ui_app();

        let file = action_position(&mut app, |a| {
            matches!(a, Action::Menu(Menu::Header("File")))
        });
        click(&mut app, window, file);
        assert!(matches!(
            app.world().get::<EditorWindow>(window).unwrap().menu,
            Some(Menu::Header("File"))
        ));
        let edit = action_position(&mut app, |a| {
            matches!(a, Action::Menu(Menu::Header("Edit")))
        });
        click(&mut app, window, edit);
        assert!(
            matches!(
                app.world().get::<EditorWindow>(window).unwrap().menu,
                Some(Menu::Header("Edit"))
            ),
            "one click must switch menus, not hit an invisible dismiss button"
        );
        click(&mut app, window, edit);
        assert!(
            app.world()
                .get::<EditorWindow>(window)
                .unwrap()
                .menu
                .is_none(),
            "clicking the open menu's trigger must toggle it closed"
        );

        for layout in [1, 0] {
            click(&mut app, window, file);
            let preset =
                action_position(&mut app, |a| matches!(a, Action::Layout(i) if *i == layout));
            click(&mut app, window, preset);
            let workspace = app.world().get::<EditorWindow>(window).unwrap();
            assert_eq!(
                workspace.document.active, layout,
                "outside buttons must act on the first click"
            );
            assert!(workspace.menu.is_none());
        }
        let window_menu = action_position(&mut app, |a| {
            matches!(a, Action::Menu(Menu::Header("Window")))
        });
        click(&mut app, window, window_menu);
        let help = {
            let world = app.world_mut();
            world
                .query::<(&UiAction, &UiGlobalTransform)>()
                .iter(world)
                .find(|(a, t)| matches!(a.action, Action::Help) && t.translation.y > 40.)
                .unwrap()
                .1
                .translation
        };
        click(&mut app, window, help);
        assert!(matches!(
            app.world().get::<EditorWindow>(window).unwrap().menu,
            Some(Menu::Help)
        ));
        assert!(
            app.world_mut()
                .query::<&Text>()
                .iter(app.world())
                .any(|text| text.0.contains("Ctrl+S saves layout only"))
        );
        click(&mut app, window, file);
        click(&mut app, window, Vec2::new(30., 60.));
        assert!(
            matches!(
                app.world().get::<EditorWindow>(window).unwrap().menu,
                Some(Menu::Header("File"))
            ),
            "popup heading clicks are not outside clicks"
        );
        let name_position = {
            let world = app.world_mut();
            world
                .query::<(&InputBinding, &UiGlobalTransform)>()
                .iter(world)
                .find(|(binding, _)| binding.field == Field::Name)
                .unwrap()
                .1
                .translation
        };
        click(&mut app, window, name_position);
        assert!(
            app.world()
                .get::<EditorWindow>(window)
                .unwrap()
                .menu
                .is_none()
        );
        let focus = app
            .world()
            .resource::<InputFocus>()
            .get()
            .expect("outside input must focus on the first click");
        assert!(matches!(
            app.world().get::<InputBinding>(focus).unwrap().field,
            Field::Name
        ));
        click(&mut app, window, file);
        click(&mut app, window, Vec2::new(700., 886.));
        assert!(
            app.world()
                .get::<EditorWindow>(window)
                .unwrap()
                .menu
                .is_none(),
            "blank UI outside the popup must still dismiss it"
        );

        // The bottom-right pane exercises popup clamping as well as cross-column picking.
        let (group, pane) = {
            let world = app.world_mut();
            let workspace = world.get::<EditorWindow>(window).unwrap().clone();
            world
                .query::<&UiAction>()
                .iter(world)
                .find_map(|action| match action.action {
                    Action::Menu(Menu::PaneType {
                        group,
                        pane: Some(pane),
                    }) if workspace.document.root().pane(pane).unwrap().kind
                        == PaneType::Camera =>
                    {
                        Some((group, pane))
                    }
                    _ => None,
                })
                .unwrap()
        };
        let trigger = action_position(
            &mut app,
            |a| matches!(a, Action::Menu(Menu::PaneType { pane: Some(id), .. }) if *id == pane),
        );
        click(&mut app, window, trigger);
        let positions = [
            PaneType::Viewport,
            PaneType::Camera,
            PaneType::World,
            PaneType::Inspector,
            PaneType::AssetsTree,
            PaneType::AssetsGallery,
        ]
        .map(|kind| {
            action_position(
                &mut app,
                |a| matches!(a, Action::ChangeType { kind: k, .. } if *k == kind),
            )
        });
        for pair in positions.chunks_exact(2) {
            assert!((pair[0].x - pair[1].x).abs() < 0.1);
            assert!(pair[0].y < pair[1].y, "each category is a vertical list");
        }
        assert!(
            positions[0].x < positions[2].x && positions[2].x < positions[4].x,
            "Scene, Data and Assets must occupy separate left-to-right columns"
        );
        {
            let world = app.world_mut();
            for (action, transform, node) in world
                .query::<(&UiAction, &UiGlobalTransform, &ComputedNode)>()
                .iter(world)
                .filter(|(action, _, _)| matches!(action.action, Action::ChangeType { .. }))
            {
                assert_eq!(action.window, window);
                let half = node.size() * node.inverse_scale_factor() * 0.5;
                let min = transform.translation - half;
                let max = transform.translation + half;
                assert!(min.x >= 0. && min.y >= 0. && max.x <= 1440. && max.y <= 900.);
            }
            let highlighted = world
                .query::<(&UiAction, &BaseColor)>()
                .iter(world)
                .filter_map(|(action, color)| match action.action {
                    Action::ChangeType { kind, .. } if color.0 != Color::NONE => Some(kind),
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert_eq!(highlighted, [PaneType::Camera]);
        }
        click(&mut app, window, positions[2]);
        let workspace = app.world().get::<EditorWindow>(window).unwrap();
        assert_eq!(
            workspace.document.root().pane(pane).unwrap().kind,
            PaneType::World
        );
        assert!(workspace.menu.is_none());
        let count = workspace.document.root().group(group).unwrap().tabs.len();

        let plus = action_position(
            &mut app,
            |a| matches!(a, Action::Menu(Menu::PaneType { group: id, pane: None }) if *id == group),
        );
        click(&mut app, window, plus);
        {
            let world = app.world_mut();
            assert!(
                world
                    .query::<(&UiAction, &BaseColor)>()
                    .iter(world)
                    .filter(|(action, _)| matches!(action.action, Action::ChangeType { .. }))
                    .all(|(_, color)| color.0 == Color::NONE),
                "adding a pane must not mark an existing type as selected"
            );
        }
        let assets = action_position(&mut app, |a| {
            matches!(
                a,
                Action::ChangeType {
                    kind: PaneType::AssetsTree,
                    ..
                }
            )
        });
        click(&mut app, window, assets);
        let workspace = app.world().get::<EditorWindow>(window).unwrap();
        let tabs = workspace.document.root().group(group).unwrap();
        assert_eq!(tabs.tabs.len(), count + 1);
        assert_eq!(
            workspace.document.root().pane(tabs.active).unwrap().kind,
            PaneType::AssetsTree
        );
        assert_eq!(
            workspace.document.root().pane(pane).unwrap().kind,
            PaneType::World
        );
        assert!(workspace.menu.is_none());

        // Resize through the actual native UI layout and hit-testing boundary. Compact
        // mode must keep controls reachable without rewriting the persisted dock tree.
        let document = serde_json::to_value(&workspace.document).unwrap();
        for width in [390., 760., 1024., 1440.] {
            app.world_mut()
                .get_mut::<Window>(window)
                .unwrap()
                .resolution
                .set(width, 700.);
            app.world_mut()
                .get_mut::<Camera>(camera)
                .unwrap()
                .computed
                .target_info = Some(RenderTargetInfo {
                physical_size: UVec2::new(width as u32, 700),
                scale_factor: 1.,
            });
            for _ in 0..3 {
                app.update();
            }
            let world = app.world_mut();
            let regions = world
                .query::<&DockRegion>()
                .iter(world)
                .filter(|r| !r.splitter)
                .count();
            assert_eq!(
                regions,
                if width < 900. { 1 } else { 5 },
                "usable pane count at {width}px"
            );
            for (action, transform, node) in world
                .query::<(&UiAction, &UiGlobalTransform, &ComputedNode)>()
                .iter(world)
                .filter(|(a, _, _)| {
                    matches!(
                        a.action,
                        Action::Layout(_)
                            | Action::CopyLayout
                            | Action::Help
                            | Action::FocusGroup(_)
                            | Action::Menu(Menu::Header(_))
                    )
                })
            {
                let center = transform.translation * node.inverse_scale_factor();
                let half = node.size() * node.inverse_scale_factor() * 0.5;
                assert!(
                    center.x - half.x >= 0.
                        && center.x + half.x <= width
                        && center.y - half.y >= 0.
                        && center.y + half.y <= 700.,
                    "essential control cropped at {width}px"
                );
                assert_eq!(action.window, window);
            }
            if width < 900. {
                let inspector = action_position(&mut app, |a| {
                    if let Action::FocusGroup(id) = a {
                        let doc: LayoutDocument = serde_json::from_value(document.clone()).unwrap();
                        let group = doc.root().group(*id).unwrap();
                        doc.root().pane(group.active).unwrap().kind == PaneType::Inspector
                    } else {
                        false
                    }
                });
                click(&mut app, window, inspector);
                assert!(
                    app.world_mut()
                        .query::<&InputBinding>()
                        .iter(app.world())
                        .any(|binding| binding.field == Field::Name)
                );
            }
            assert_eq!(
                serde_json::to_value(&app.world().get::<EditorWindow>(window).unwrap().document)
                    .unwrap(),
                document,
                "responsive presentation must not mutate saved docking geometry"
            );
        }
    }
    #[test]
    fn nonstructural_refreshes_retain_shell_fields_and_scrollbars() {
        use bevy::{
            input_focus::FocusCause,
            ui_widgets::{Scrollbar, ScrollbarThumb},
        };
        let (mut app, window, _) = native_ui_app();
        let object = app.world().resource::<Selection>().0.unwrap();
        let root = app
            .world()
            .get::<EditorWindow>(window)
            .unwrap()
            .root
            .unwrap();
        let controls = {
            let world = app.world_mut();
            world.query_filtered::<Entity, Or<(With<Scrollbar>, With<ScrollbarThumb>, With<InputBinding>)>>()
                .iter(world).collect::<Vec<_>>()
        };
        app.init_resource::<TransformGizmoSettings>()
            .init_resource::<TransformGizmoState>();
        for action in [
            Action::Menu(Menu::Header("File")),
            Action::Menu(Menu::Header("Edit")),
            Action::Dismiss,
            Action::Dismiss,
            Action::Help,
            Action::Dismiss,
            Action::Select(object),
            Action::Visible(object),
            Action::GizmoMode(TransformGizmoMode::Rotate),
            Action::GizmoSpace,
            Action::RefreshAssets,
            Action::Layout(usize::MAX),
            Action::Split {
                group: Id::MAX,
                axis: Axis::X,
            },
        ] {
            dispatch(app.world_mut(), window, action);
            app.update();
            assert_eq!(
                app.world().get::<EditorWindow>(window).unwrap().root,
                Some(root),
                "menus, status and property changes must not rebuild the shell"
            );
            for entity in &controls {
                assert!(
                    app.world().get_entity(*entity).is_ok(),
                    "unaffected controls were replaced"
                );
            }
        }
        {
            let world = app.world_mut();
            assert!(
                world
                    .query::<&Text>()
                    .iter(world)
                    .any(|t| t.0.starts_with("Cannot split:"))
            );
            let (control, _) = world
                .query::<(Entity, &UiAction)>()
                .iter(world)
                .find(|(_, a)| matches!(a.action, Action::Visible(e) if e == object))
                .unwrap();
            let icon = world
                .get::<Children>(control)
                .unwrap()
                .iter()
                .find_map(|e| world.get::<crate::icons::LucideIcon>(e))
                .unwrap();
            assert_eq!(icon.0, crate::icons::Icon::Square);
            assert!(
                world
                    .query::<(&UiAction, &BaseColor)>()
                    .iter(world)
                    .any(|(a, c)| {
                        matches!(a.action, Action::GizmoMode(TransformGizmoMode::Rotate))
                            && c.0 != Color::NONE
                    })
            );
            assert!(world.query::<&Text>().iter(world).any(|t| t.0 == "Local"));
        }
        let position = {
            let world = app.world_mut();
            world
                .query::<(Entity, &InputBinding, &crate::ui::FieldAxis)>()
                .iter(world)
                .find(|(_, b, a)| b.field == Field::Position && a.0 == 0)
                .unwrap()
                .0
        };
        app.world_mut()
            .entity_mut(position)
            .insert(EditableText::new("3.25"));
        app.world_mut()
            .resource_mut::<InputFocus>()
            .set(position, FocusCause::Navigated);
        for state in [
            bevy::input::ButtonState::Pressed,
            bevy::input::ButtonState::Released,
        ] {
            app.world_mut()
                .write_message(bevy::input::keyboard::KeyboardInput {
                    key_code: KeyCode::Enter,
                    logical_key: bevy::input::keyboard::Key::Enter,
                    text: None,
                    state,
                    repeat: false,
                    window,
                });
        }
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .clear();
        app.update();
        assert_eq!(
            app.world().get::<Transform>(object).unwrap().translation.x,
            3.25
        );
        assert_eq!(app.world().resource::<InputFocus>().get(), Some(position));
        assert_eq!(
            app.world().get::<EditorWindow>(window).unwrap().root,
            Some(root)
        );
        assert_eq!(
            app.world()
                .get::<EditableText>(position)
                .unwrap()
                .value()
                .to_string(),
            "3.25"
        );

        // Numeric scrubbing and mesh/gizmo completion must synchronize existing fields.
        app.world_mut().resource_mut::<InputFocus>().clear();
        app.update();
        edit_axis(app.world_mut(), object, Field::Position, 0, 1.);
        let axis = {
            let world = app.world_mut();
            world
                .query::<(Entity, &DragHandle)>()
                .iter(world)
                .find(|(_, h)| {
                    matches!(
                        h,
                        DragHandle::Axis {
                            field: Field::Position,
                            axis: 0,
                            ..
                        }
                    )
                })
                .unwrap()
                .0
        };
        app.world_mut().trigger(PointerDragEnd {
            entity: axis,
            pointer: bevy::picking::events::Pointer::new(
                bevy::picking::pointer::PointerId::Mouse,
                bevy::picking::pointer::Location {
                    target: RenderTarget::Window(WindowRef::Entity(window))
                        .normalize(None)
                        .unwrap(),
                    position: Vec2::ZERO,
                },
            ),
            button: PointerButton::Primary,
            distance: Vec2::X * 100.,
        });
        app.update();
        assert_eq!(
            app.world()
                .get::<EditableText>(position)
                .unwrap()
                .value()
                .to_string(),
            "4.25"
        );
        // A rename updates the tree label in place as well as other Inspector fields.
        let name = {
            let world = app.world_mut();
            world
                .query::<(Entity, &InputBinding)>()
                .iter(world)
                .find(|(_, b)| b.field == Field::Name)
                .unwrap()
                .0
        };
        app.world_mut()
            .entity_mut(name)
            .insert(EditableText::new("Renamed"));
        app.world_mut()
            .resource_mut::<InputFocus>()
            .set(name, FocusCause::Navigated);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Enter);
        commit_fields(app.world_mut());
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .clear();
        app.update();
        assert_eq!(app.world().get::<Name>(object).unwrap().as_str(), "Renamed");
        assert!(
            app.world_mut()
                .query::<&Text>()
                .iter(app.world())
                .any(|text| text.0 == "Renamed")
        );
        for entity in &controls {
            assert!(app.world().get_entity(*entity).is_ok());
        }
    }

    #[test]
    fn local_content_and_layout_refreshes_do_not_replace_unaffected_windows() {
        use bevy::ui_widgets::Scrollbar;
        let (mut app, first, camera) = native_ui_app();
        let parent = app.world().resource::<Selection>().0.unwrap();
        let child = app
            .world_mut()
            .spawn((
                Name::new("Nested"),
                Transform::default(),
                ChildOf(parent),
                EditorObject {
                    key: 2,
                    kind: ModelKind::Cube,
                },
            ))
            .id();
        let original_root = app.world().get::<EditorWindow>(first).unwrap().root;
        dispatch(app.world_mut(), first, Action::NewWindow);
        app.update();
        assert_eq!(
            app.world().get::<EditorWindow>(first).unwrap().root,
            original_root,
            "creating another window must retain the existing shell"
        );
        let second = {
            let world = app.world_mut();
            world
                .query::<(Entity, &EditorWindow)>()
                .iter(world)
                .find(|(w, _)| *w != first)
                .unwrap()
                .0
        };
        let second_root = app
            .world()
            .get::<EditorWindow>(second)
            .unwrap()
            .root
            .unwrap();
        let first_root = app
            .world()
            .get::<EditorWindow>(first)
            .unwrap()
            .root
            .unwrap();
        let tracks = {
            let world = app.world_mut();
            world
                .query::<(Entity, &Scrollbar)>()
                .iter(world)
                .map(|(e, b)| (e, b.target))
                .collect::<Vec<_>>()
        };
        let pane = app
            .world()
            .get::<EditorWindow>(first)
            .unwrap()
            .document
            .root()
            .first_group();
        let pane = app
            .world()
            .get::<EditorWindow>(first)
            .unwrap()
            .document
            .root()
            .group(pane)
            .unwrap()
            .active;
        dispatch(app.world_mut(), first, Action::Collapse { pane, key: 1 });
        app.update();
        {
            let world = app.world_mut();
            let child_windows: Vec<_> = world
                .query::<&UiAction>()
                .iter(world)
                .filter(|a| matches!(a.action, Action::Select(e) if e == child))
                .map(|a| a.window)
                .collect();
            assert_eq!(
                child_windows,
                [second],
                "collapse must only hide rows in its own pane"
            );
        }
        assert_eq!(
            app.world().get::<EditorWindow>(first).unwrap().root,
            Some(first_root)
        );
        assert_eq!(
            app.world().get::<EditorWindow>(second).unwrap().root,
            Some(second_root)
        );
        for (track, target) in &tracks {
            assert!(app.world().get::<Scrollbar>(*track).is_some());
            assert!(app.world().get::<ScrollPosition>(*target).is_some());
        }
        // Search commits update that pane's rows without replacing its input or tracks.
        let search = {
            let world = app.world_mut();
            world
                .query::<(Entity, &InputBinding)>()
                .iter(world)
                .find(|(_, b)| b.window == first && b.field == Field::Query(pane))
                .unwrap()
                .0
        };
        app.world_mut()
            .entity_mut(search)
            .insert(EditableText::new("no match"));
        app.world_mut()
            .resource_mut::<InputFocus>()
            .set(search, bevy::input_focus::FocusCause::Navigated);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Enter);
        commit_fields(app.world_mut());
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .clear();
        app.update();
        {
            let world = app.world_mut();
            assert!(
                !world
                    .query::<&UiAction>()
                    .iter(world)
                    .any(|a| { a.window == first && matches!(a.action, Action::Select(_)) })
            );
        }
        assert_eq!(app.world().resource::<InputFocus>().get(), Some(search));
        for (track, target) in &tracks {
            assert!(app.world().get::<Scrollbar>(*track).is_some());
            assert!(app.world().get::<ScrollPosition>(*target).is_some());
        }
        app.world_mut().resource_mut::<InputFocus>().clear();
        app.update();
        app.world_mut()
            .get_mut::<Window>(first)
            .unwrap()
            .resolution
            .set(1400., 900.);
        app.world_mut()
            .get_mut::<Camera>(camera)
            .unwrap()
            .computed
            .target_info
            .as_mut()
            .unwrap()
            .physical_size
            .x = 1400;
        app.update();
        assert_eq!(
            app.world().get::<EditorWindow>(first).unwrap().root,
            Some(first_root),
            "resizing within a responsive mode must only change geometry"
        );
        dispatch(app.world_mut(), first, Action::Layout(1));
        app.update();
        assert_ne!(
            app.world().get::<EditorWindow>(first).unwrap().root,
            Some(first_root)
        );
        assert_eq!(
            app.world().get::<EditorWindow>(second).unwrap().root,
            Some(second_root),
            "a layout change must not rebuild another window"
        );
        let layout_root = app.world().get::<EditorWindow>(first).unwrap().root;
        dispatch(app.world_mut(), first, Action::Select(child));
        app.update();
        assert_eq!(
            app.world().get::<EditorWindow>(first).unwrap().root,
            layout_root
        );
        assert_eq!(
            app.world().get::<EditorWindow>(second).unwrap().root,
            Some(second_root)
        );
        {
            let world = app.world_mut();
            let fields: Vec<_> = world
                .query::<(&InputBinding, &EditableText)>()
                .iter(world)
                .filter(|(b, _)| b.field == Field::Name)
                .collect();
            assert_eq!(
                fields.len(),
                2,
                "both windows must refresh their Inspectors"
            );
            assert!(
                fields
                    .iter()
                    .all(|(b, t)| b.entity == Some(child) && t.value() == "Nested")
            );
        }
        app.world_mut()
            .get_mut::<Window>(first)
            .unwrap()
            .resolution
            .set(760., 900.);
        app.world_mut()
            .get_mut::<Camera>(camera)
            .unwrap()
            .computed
            .target_info
            .as_mut()
            .unwrap()
            .physical_size
            .x = 760;
        app.update();
        assert_ne!(
            app.world().get::<EditorWindow>(first).unwrap().root,
            layout_root,
            "crossing a responsive breakpoint must still rebuild the affected shell"
        );
        assert_eq!(
            app.world().get::<EditorWindow>(second).unwrap().root,
            Some(second_root)
        );
    }

    #[test]
    fn viewport_orbit_and_pan_release_keep_pane_scrollbars_stable() {
        use bevy::{
            picking::pointer::{Location, PointerAction, PointerId, PointerInput},
            ui::{UiGlobalTransform, widget::ViewportNode},
            ui_widgets::{Scrollbar, ScrollbarThumb},
        };
        let (mut app, window, _) = native_ui_app();
        // Real overflow and native thumb layout, not hand-authored ComputedNode values.
        for key in 2..50 {
            app.world_mut().spawn((
                Name::new(format!("Object {key}")),
                Transform::default(),
                EditorObject {
                    key,
                    kind: ModelKind::Cube,
                },
            ));
        }
        app.world_mut().resource_mut::<UiDirty>().0 = true;
        for _ in 0..4 {
            app.update();
        }
        let (surface, camera, start) = {
            let world = app.world_mut();
            world
                .query::<(Entity, &ViewportNode, &UiGlobalTransform)>()
                .iter(world)
                .find_map(|(surface, node, transform)| {
                    let camera = node.camera?;
                    (!world.get::<SceneView>(camera)?.preview).then_some((
                        surface,
                        camera,
                        transform.translation,
                    ))
                })
                .unwrap()
        };
        let bars = {
            let world = app.world_mut();
            world
                .query::<(Entity, &Scrollbar, &Node)>()
                .iter(world)
                .filter(|(_, _, style)| style.display != Display::None)
                .map(|(track, bar, _)| (track, bar.target))
                .collect::<Vec<_>>()
        };
        assert!(
            !bars.is_empty(),
            "fixture must have visible overflow scrollbars"
        );
        for (_, target) in &bars {
            app.world_mut()
                .get_mut::<ScrollPosition>(*target)
                .unwrap()
                .y = 48.;
        }
        app.update();
        let thumbs = {
            let world = app.world_mut();
            world.query_filtered::<(Entity, &ComputedNode, &UiGlobalTransform), With<ScrollbarThumb>>()
                .iter(world)
                .filter(|(_, node, _)| node.size().y > 0.)
                .map(|(entity, node, pose)| (entity, node.size(), pose.translation))
                .collect::<Vec<_>>()
        };
        assert!(
            !thumbs.is_empty(),
            "native scrollbar thumbs must be laid out"
        );
        let positions = bars
            .iter()
            .map(|(_, target)| {
                (
                    *target,
                    app.world().get::<ScrollPosition>(*target).unwrap().y,
                )
            })
            .collect::<Vec<_>>();
        let location = |position| Location {
            target: RenderTarget::Window(WindowRef::Entity(window))
                .normalize(None)
                .unwrap(),
            position,
        };
        for button in [PointerButton::Secondary, PointerButton::Middle] {
            let view = app.world().get::<SceneView>(camera).unwrap();
            let (before_orbit, before_center) = (view.orbit, view.center);
            let delta = Vec2::new(20., 10.);
            for (position, action) in [
                (start, PointerAction::Move { delta: Vec2::ZERO }),
                (start, PointerAction::Press(button)),
                (start + delta, PointerAction::Move { delta }),
                (start + delta * 2., PointerAction::Move { delta }),
                (start + delta * 2., PointerAction::Release(button)),
            ] {
                app.world_mut().write_message(PointerInput::new(
                    PointerId::Mouse,
                    location(position),
                    action,
                ));
                app.update();
                for (track, _) in &bars {
                    assert_eq!(
                        app.world().get::<Node>(*track).map(|n| n.display),
                        Some(Display::Flex),
                        "camera input must not replace or hide other panes' scrollbars"
                    );
                }
                for (entity, size, translation) in &thumbs {
                    assert_eq!(
                        app.world()
                            .get::<ComputedNode>(*entity)
                            .map(ComputedNode::size),
                        Some(*size),
                        "camera input must not reset scrollbar thumb layout"
                    );
                    assert_eq!(
                        app.world()
                            .get::<UiGlobalTransform>(*entity)
                            .map(|t| t.translation),
                        Some(*translation)
                    );
                }
                for (target, y) in &positions {
                    assert_eq!(
                        app.world().get::<ScrollPosition>(*target).map(|p| p.y),
                        Some(*y)
                    );
                }
            }
            let view = app.world().get::<SceneView>(camera).unwrap();
            if button == PointerButton::Secondary {
                assert_ne!(
                    view.orbit, before_orbit,
                    "real pointer input must orbit the camera"
                );
            } else {
                assert_ne!(
                    view.center, before_center,
                    "real pointer input must pan the camera"
                );
            }
            let pane = app
                .world()
                .get::<EditorWindow>(window)
                .unwrap()
                .document
                .root()
                .pane(view.pane)
                .unwrap();
            assert_eq!(pane.orbit, view.orbit);
            assert_eq!(pane.center, view.center.to_array());
            assert!(app.world().get::<ViewportNode>(surface).is_some());
        }
        // Splitter release used the same unconditional rebuild path as camera release.
        let (id, start) = {
            let world = app.world_mut();
            world
                .query::<(&DragHandle, &UiGlobalTransform)>()
                .iter(world)
                .find_map(|(handle, pose)| match handle {
                    DragHandle::Splitter {
                        id, axis: Axis::X, ..
                    } => Some((*id, pose.translation)),
                    _ => None,
                })
                .unwrap()
        };
        let ratio = |app: &App| {
            let DockNode::Split { ratio, .. } = app
                .world()
                .get::<EditorWindow>(window)
                .unwrap()
                .document
                .root()
                .find(id)
                .unwrap()
            else {
                unreachable!()
            };
            *ratio
        };
        let before = ratio(&app);
        let root = app.world().get::<EditorWindow>(window).unwrap().root;
        let delta = Vec2::new(20., 0.);
        for (position, action) in [
            (start, PointerAction::Move { delta: Vec2::ZERO }),
            (start, PointerAction::Press(PointerButton::Primary)),
            (start + delta, PointerAction::Move { delta }),
            (start + delta * 2., PointerAction::Move { delta }),
            (
                start + delta * 2.,
                PointerAction::Release(PointerButton::Primary),
            ),
        ] {
            app.world_mut().write_message(PointerInput::new(
                PointerId::Mouse,
                location(position),
                action,
            ));
            app.update();
            assert_eq!(app.world().get::<EditorWindow>(window).unwrap().root, root);
            for (track, target) in &bars {
                assert!(app.world().get::<Scrollbar>(*track).is_some());
                assert!(app.world().get::<ScrollPosition>(*target).is_some());
            }
            for (thumb, _, _) in &thumbs {
                assert!(app.world().get::<ComputedNode>(*thumb).is_some());
            }
        }
        assert_ne!(
            ratio(&app),
            before,
            "real pointer input must resize the dock split"
        );
    }
    #[test]
    fn tab_strip_drops_merge_while_body_edges_still_split() {
        use bevy::picking::{
            backend::HitData,
            events::Pointer,
            pointer::{Location, PointerId},
        };
        // Wide docking mode: a 1200x600 window puts the right group at (602.5, 45).
        // Its tab strip ends at y=80, including the group's 1px top border.
        for (name, position, zone) in [
            ("tab strip left", Vec2::new(610., 60.), Zone::Center),
            ("tab strip middle", Vec2::new(900., 60.), Zone::Center),
            ("tab strip right", Vec2::new(1180., 60.), Zone::Center),
            ("tab strip bottom", Vec2::new(900., 79.5), Zone::Center),
            ("body center", Vec2::new(900., 307.), Zone::Center),
            ("body left", Vec2::new(610., 307.), Zone::Left),
            ("body right", Vec2::new(1180., 307.), Zone::Right),
            ("body top", Vec2::new(900., 90.), Zone::Top),
            ("body bottom", Vec2::new(900., 560.), Zone::Bottom),
        ] {
            let (mut world, window) = workspace_world();
            world
                .get_mut::<Window>(window)
                .unwrap()
                .resolution
                .set(1200., 600.);
            world.get_mut::<EditorWindow>(window).unwrap().document = LayoutDocument::from_json(
                r#"{"version":1,"active":0,"layouts":[{"name":"Drag test","root":{
                        "node":"Split","id":1,"axis":"X","ratio":0.5,
                        "a":{"node":"Group","id":2,"active":3,
                             "tabs":[{"id":3,"kind":"World"}]},
                        "b":{"node":"Group","id":4,"active":5,
                             "tabs":[{"id":5,"kind":"Inspector"}]}
                    }}]}"#,
            )
            .unwrap();
            world.init_resource::<DragState>();
            world.add_observer(begin_drag);
            world.add_observer(drag);
            world.add_observer(end_drag);
            let camera = world.get::<EditorWindow>(window).unwrap().camera;
            let handle = world.spawn(DragHandle::Tab { window, pane: 3 }).id();
            let pointer = |position| {
                Pointer::new(
                    PointerId::Mouse,
                    Location {
                        target: RenderTarget::Window(WindowRef::Entity(window))
                            .normalize(None)
                            .unwrap(),
                        position,
                    },
                )
            };
            let start = Vec2::new(60., 60.);
            world.trigger(PointerDragStart {
                entity: handle,
                pointer: pointer(start),
                button: PointerButton::Primary,
                hit: HitData::new(camera, 0., None, None),
            });
            world.trigger(PointerDrag {
                entity: handle,
                pointer: pointer(position),
                button: PointerButton::Primary,
                distance: position - start,
                delta: position - start,
            });
            world.flush();
            assert_eq!(
                world.resource::<DragState>().drop,
                Some((window, 4, zone)),
                "{name}"
            );
            world.trigger(PointerDragEnd {
                entity: handle,
                pointer: pointer(position),
                button: PointerButton::Primary,
                distance: position - start,
            });
            world.flush();
            let doc = &world.get::<EditorWindow>(window).unwrap().document;
            doc.validate().unwrap();
            if zone == Zone::Center {
                let DockNode::Group(group) = doc.root() else {
                    panic!("{name}: drop should merge, not split");
                };
                assert_eq!(group.id, 4, "{name}");
                assert_eq!(
                    group.tabs.iter().map(|pane| pane.id).collect::<Vec<_>>(),
                    [5, 3],
                    "{name}"
                );
                assert_eq!(group.active, 3, "{name}");
            } else {
                let DockNode::Split {
                    axis, ratio, a, b, ..
                } = doc.root()
                else {
                    panic!("{name}: body edge should split");
                };
                let horizontal = matches!(zone, Zone::Left | Zone::Right);
                assert_eq!(*axis, if horizontal { Axis::X } else { Axis::Y }, "{name}");
                assert_eq!(*ratio, 0.5, "{name}");
                let (moved, target) = if matches!(zone, Zone::Left | Zone::Top) {
                    (a, b)
                } else {
                    (b, a)
                };
                assert!(moved.pane(3).is_some(), "{name}");
                assert!(target.group(4).is_some(), "{name}");
            }
            assert!(world.resource::<DragState>().tab.is_none(), "{name}");
            assert!(world.resource::<DragState>().drop.is_none(), "{name}");
        }
    }
    #[test]
    fn tab_release_in_another_window_rejects_a_stale_drop_target() {
        use bevy::picking::{
            events::Pointer,
            pointer::{Location, PointerId},
        };
        let (mut world, window) = workspace_world();
        world.init_resource::<DragState>();
        world.add_observer(end_drag);
        let doc = &world.get::<EditorWindow>(window).unwrap().document;
        let group = doc.root().first_group();
        let pane = doc.root().group(group).unwrap().active;
        let target = crate::ui::dock_rects(
            doc.root(),
            Rect::from_corners(Vec2::ZERO, Vec2::splat(1000.)),
            None,
        )
        .into_iter()
        .find(|(id, _, splitter)| !*splitter && *id != group)
        .unwrap()
        .0;
        let before = serde_json::to_string(doc).unwrap();
        let handle = world.spawn(DragHandle::Tab { window, pane }).id();
        let other = world.spawn(Window::default()).id();
        for release_window in [other, window] {
            *world.resource_mut::<DragState>() = DragState {
                tab: Some((window, pane)),
                drop: Some((window, target, Zone::Center)),
            };
            world.trigger(PointerDragEnd {
                entity: handle,
                button: PointerButton::Primary,
                distance: Vec2::splat(100.),
                pointer: Pointer::new(
                    PointerId::Mouse,
                    Location {
                        target: RenderTarget::Window(WindowRef::Entity(release_window))
                            .normalize(None)
                            .unwrap(),
                        position: Vec2::splat(200.),
                    },
                ),
            });
            world.flush();
            let doc = &world.get::<EditorWindow>(window).unwrap().document;
            if release_window == other {
                assert_eq!(serde_json::to_string(doc).unwrap(), before);
            } else {
                // Positive control: the exact same source/target is a valid same-window drop.
                assert!(
                    doc.root()
                        .group(target)
                        .unwrap()
                        .tabs
                        .iter()
                        .any(|tab| tab.id == pane)
                );
            }
            assert!(world.resource::<DragState>().tab.is_none());
        }
    }
    #[test]
    fn inspector_commits_on_enter_or_blur_and_rejects_invalid_numbers() {
        use bevy::input_focus::FocusCause;
        let (mut world, window) = workspace_world();
        world.init_resource::<InputFocus>();
        world.init_resource::<LastFocus>();
        world.init_resource::<ButtonInput<KeyCode>>();
        let object = world.spawn(Transform::default()).id();
        let input = world
            .spawn((
                EditableText::new("3.25"),
                InputBinding {
                    window,
                    layout: 0,
                    pane: 10,
                    entity: Some(object),
                    field: Field::Position,
                    committed: "0.00".into(),
                },
                crate::ui::FieldAxis(0),
            ))
            .id();
        world
            .resource_mut::<InputFocus>()
            .set(input, FocusCause::Navigated);
        world
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Enter);
        commit_fields(&mut world);
        assert_eq!(world.get::<Transform>(object).unwrap().translation.x, 3.25);
        for number in [3.0e38_f32, -3.0e38_f32, 3.25] {
            world
                .entity_mut(input)
                .insert(EditableText::new(number.to_string()));
            commit_fields(&mut world);
            assert_eq!(
                world.get::<Transform>(object).unwrap().translation.x,
                number
            );
        }
        for invalid in ["NaN", "inf", "not a number"] {
            world.entity_mut(input).insert(EditableText::new(invalid));
            commit_fields(&mut world);
            assert_eq!(world.get::<Transform>(object).unwrap().translation.x, 3.25);
            assert!(
                world
                    .get::<EditorWindow>(window)
                    .unwrap()
                    .status
                    .contains("Invalid number")
            );
        }
        world.entity_mut(input).insert(EditableText::new("-2.5"));
        world.resource_mut::<ButtonInput<KeyCode>>().clear();
        world.resource_mut::<InputFocus>().clear();
        commit_fields(&mut world);
        assert_eq!(world.get::<Transform>(object).unwrap().translation.x, -2.5);
    }
}
