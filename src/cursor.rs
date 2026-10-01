//! Per-window OS cursors from the mouse's real picking state, including viewport pointers.
use crate::{
    editor::{DragState, EditorWindow, UiOwned},
    layout::Axis,
    scene::{EditorObject, SceneView},
};
use bevy::{
    camera::NormalizedRenderTarget,
    picking::{
        events::PointerState,
        hover::PointerCaptureMap,
        pointer::{PointerId, PointerInteraction, PointerLocation, PointerMap},
    },
    prelude::*,
    ui::{InteractionDisabled, widget::ViewportNode},
    window::{CursorIcon, SystemCursorIcon},
};

/// A control's affordance; decorative children inherit the nearest control's role.
#[derive(Component, Clone, Copy)]
pub enum CursorRole {
    Default,
    Click,
    Text,
    Tab,
    Resize(Axis),
    Scrub,
    Viewport(Entity),
}
impl CursorRole {
    fn hover(self) -> SystemCursorIcon {
        match self {
            Self::Default | Self::Viewport(_) => SystemCursorIcon::Default,
            Self::Click | Self::Tab => SystemCursorIcon::Pointer,
            Self::Text => SystemCursorIcon::Text,
            Self::Resize(Axis::X) => SystemCursorIcon::ColResize,
            Self::Resize(Axis::Y) => SystemCursorIcon::RowResize,
            Self::Scrub => SystemCursorIcon::EwResize,
        }
    }
}

pub struct EditorCursorPlugin;
impl Plugin for EditorCursorPlugin {
    fn build(&self, app: &mut App) {
        // After picking observers and UI rebuilds, before winit's Last cursor update.
        app.add_systems(PostUpdate, update);
    }
}

fn owner(world: &World, mut entity: Entity) -> Option<Entity> {
    loop {
        if let Some(owner) = world.get::<UiOwned>(entity) {
            return Some(owner.0);
        }
        if world.get::<EditorWindow>(entity).is_some() {
            return Some(entity);
        }
        entity = world.get::<ChildOf>(entity)?.parent();
    }
}
fn role(world: &World, mut entity: Entity) -> Option<(Entity, CursorRole)> {
    let mut nearest = None;
    loop {
        if world.get::<InteractionDisabled>(entity).is_some() {
            return Some((entity, CursorRole::Default));
        }
        if nearest.is_none()
            && let Some(role) = world.get::<CursorRole>(entity)
        {
            nearest = Some((entity, *role));
        }
        let Some(parent) = world.get::<ChildOf>(entity) else {
            return nearest;
        };
        entity = parent.parent();
    }
}
fn nearest_hit(world: &World, pointer: PointerId) -> Option<(Entity, Entity)> {
    let entity = world.resource::<PointerMap>().get_entity(pointer)?;
    // A viewport pointer may have old hits after lifting; never use those.
    world.get::<PointerLocation>(entity)?.location()?;
    world
        .get::<PointerInteraction>(entity)?
        .iter()
        .find(|(entity, _)| world.get_entity(*entity).is_ok())
        .map(|(entity, hit)| (*entity, hit.camera))
}
fn selectable(world: &World, mut entity: Entity) -> bool {
    loop {
        if world.get::<EditorObject>(entity).is_some() {
            return true;
        }
        let Some(parent) = world.get::<ChildOf>(entity) else {
            return false;
        };
        entity = parent.parent();
    }
}
fn hover_cursor(world: &World, window: Entity) -> SystemCursorIcon {
    if let Some(route) = world.get_resource::<crate::transform_gizmo::ViewportGizmo>()
        && route.window == Some(window)
        && world
            .get_resource::<bevy::gizmos::transform_gizmo::TransformGizmoState>()
            .is_some_and(|state| state.active || state.hovered_axis.is_some())
    {
        return if world
            .resource::<bevy::gizmos::transform_gizmo::TransformGizmoState>()
            .active
        {
            SystemCursorIcon::Grabbing
        } else {
            SystemCursorIcon::Pointer
        };
    }
    let Some((hit, _)) = nearest_hit(world, PointerId::Mouse) else {
        return SystemCursorIcon::Default;
    };
    if owner(world, hit) != Some(window) {
        return SystemCursorIcon::Default;
    }
    let Some((surface, role)) = role(world, hit) else {
        return SystemCursorIcon::Default;
    };
    if let CursorRole::Viewport(camera) = role
        && world
            .get::<SceneView>(camera)
            .is_some_and(|view| !view.preview && view.window == window)
        && let Some(pointer) = world.get::<PointerId>(surface)
        && let Some((mesh, hit_camera)) = nearest_hit(world, *pointer)
        && hit_camera == camera
        && selectable(world, mesh)
    {
        return SystemCursorIcon::Pointer;
    }
    role.hover()
}
fn drag_cursor(world: &World, window: Entity) -> Option<SystemCursorIcon> {
    for button in PointerButton::iter() {
        let Some(state) = world
            .resource::<PointerState>()
            .get(PointerId::Mouse, button)
        else {
            continue;
        };
        // Stable ordering for overlapping non-blocking targets.
        let mut sources: Vec<_> = state.dragging.keys().copied().collect();
        sources.sort_by_key(|entity| entity.to_bits());
        for source in sources {
            let Some((control, role)) = role(world, source) else {
                continue;
            };
            let source_window = owner(world, control)?;
            if source_window != window {
                return Some(if matches!(role, CursorRole::Tab) {
                    SystemCursorIcon::NoDrop
                } else {
                    SystemCursorIcon::Default
                });
            }
            let icon = match (role, button) {
                (CursorRole::Tab, PointerButton::Primary) => {
                    let valid = world.get_resource::<DragState>().is_some_and(|state| {
                        state.drop.is_some_and(|(target, group, zone)| {
                            target == window
                                && state.tab.is_some_and(|(source, pane)| {
                                    source == window
                                        && world.get::<EditorWindow>(window).is_some_and(|ws| {
                                            ws.document
                                                .root()
                                                .clone()
                                                .dock(pane, group, zone)
                                                .is_ok()
                                        })
                                })
                        })
                    });
                    if valid {
                        SystemCursorIcon::Grabbing
                    } else {
                        SystemCursorIcon::NoDrop
                    }
                }
                (CursorRole::Text, PointerButton::Primary) => SystemCursorIcon::Text,
                (CursorRole::Resize(_) | CursorRole::Scrub, PointerButton::Primary) => role.hover(),
                (CursorRole::Viewport(camera), button) => {
                    let view = world.get::<SceneView>(camera)?;
                    if view.preview {
                        continue;
                    }
                    match button {
                        PointerButton::Secondary => SystemCursorIcon::Grabbing,
                        PointerButton::Middle => SystemCursorIcon::Move,
                        PointerButton::Primary => continue,
                    }
                }
                _ => continue,
            };
            return Some(icon);
        }
    }
    None
}

fn clean_sessions(world: &mut World) {
    let escape = world
        .get_resource::<ButtonInput<KeyCode>>()
        .is_some_and(|keys| keys.just_pressed(KeyCode::Escape));
    let viewport_owners: Vec<_> = world
        .query_filtered::<(Entity, &PointerId), With<ViewportNode>>()
        .iter(world)
        .filter_map(|(entity, pointer)| owner(world, entity).map(|window| (*pointer, window)))
        .collect();
    let mut cancelled = Vec::new();
    world.resource_scope(|world, mut state: Mut<PointerState>| {
        for ((pointer, _), button) in &mut state.pointer_buttons {
            let window = viewport_owners
                .iter()
                .find(|(id, _)| id == pointer)
                .map(|(_, window)| *window);
            let invalid = window
                .is_some_and(|window| !world.get::<Window>(window).is_some_and(|w| w.focused))
                || button
                    .pressing
                    .keys()
                    .chain(button.dragging.keys())
                    .any(|entity| {
                        world.get_entity(*entity).is_err()
                            || owner(world, *entity).is_some_and(|window| {
                                !world.get::<Window>(window).is_some_and(|w| w.focused)
                            })
                    });
            if escape || invalid {
                button.clear();
                cancelled.push(*pointer);
            }
        }
    });
    if let Some(mut captures) = world.get_resource_mut::<PointerCaptureMap>() {
        for pointer in cancelled {
            captures.release(pointer);
        }
    }
    if let Some((window, _)) = world.get_resource::<DragState>().and_then(|tabs| tabs.tab) {
        let tab_dragging =
            world
                .resource::<PointerState>()
                .pointer_buttons
                .iter()
                .any(|((_, button), state)| {
                    *button == PointerButton::Primary
                        && state.dragging.keys().any(|source| {
                            role(world, *source).is_some_and(|(control, role)| {
                                matches!(role, CursorRole::Tab)
                                    && owner(world, control) == Some(window)
                            })
                        })
                });
        if !tab_dragging {
            *world.resource_mut::<DragState>() = DragState::default();
        }
    }
}
fn update(world: &mut World) {
    clean_sessions(world);
    let mouse_window = world
        .resource::<PointerMap>()
        .get_entity(PointerId::Mouse)
        .and_then(|mouse| world.get::<PointerLocation>(mouse))
        .and_then(PointerLocation::location)
        .and_then(|location| match location.target {
            NormalizedRenderTarget::Window(window) => Some(window.entity()),
            _ => None,
        });
    let windows: Vec<_> = world
        .query_filtered::<(Entity, &Window), With<EditorWindow>>()
        .iter(world)
        .map(|(entity, window)| (entity, window.focused, window.cursor_position().is_some()))
        .collect();
    for (window, focused, inside) in windows {
        let icon = if inside && mouse_window == Some(window) {
            drag_cursor(world, window).unwrap_or_else(|| {
                if focused {
                    hover_cursor(world, window)
                } else {
                    SystemCursorIcon::Default
                }
            })
        } else {
            SystemCursorIcon::Default
        };
        let cursor = CursorIcon::System(icon);
        if world.get::<CursorIcon>(window) != Some(&cursor) {
            world.entity_mut(window).insert(cursor);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        layout::{LayoutDocument, Zone},
        scene::ModelKind,
    };
    use bevy::{
        camera::RenderTarget,
        picking::{
            InteractionPlugin, PickingPlugin,
            backend::{HitData, PointerHits},
            pointer::{Location, PointerAction, PointerInput},
        },
        window::WindowRef,
    };

    // Drive the real picking pipeline rather than manufacture hover/drag state.
    struct Harness {
        app: App,
        window: Entity,
        camera: Entity,
        position: Vec2,
    }
    impl Harness {
        fn new() -> Self {
            let mut app = App::new();
            app.add_plugins((
                MinimalPlugins,
                PickingPlugin,
                InteractionPlugin,
                EditorCursorPlugin,
            ))
            .init_resource::<DragState>()
            .init_resource::<ButtonInput<KeyCode>>();
            let camera = app.world_mut().spawn_empty().id();
            let mut harness = Self {
                app,
                window: Entity::PLACEHOLDER,
                camera,
                position: Vec2::splat(10.),
            };
            harness.window = harness.add_window();
            harness.app.world_mut().spawn(PointerId::Mouse);
            harness
        }
        fn add_window(&mut self) -> Entity {
            let mut window = Window {
                focused: true,
                ..default()
            };
            window.set_cursor_position(Some(Vec2::splat(10.)));
            self.app
                .world_mut()
                .spawn((
                    window,
                    CursorIcon::default(),
                    EditorWindow {
                        document: LayoutDocument::default(),
                        camera: self.camera,
                        root: None,
                        focused: 1,
                        maximized: None,
                        menu: None,
                        status: String::new(),
                    },
                ))
                .id()
        }
        fn control(&mut self, role: CursorRole) -> Entity {
            self.app
                .world_mut()
                .spawn((role, UiOwned(self.window)))
                .id()
        }
        fn hit(&mut self, pointer: PointerId, entity: Entity, camera: Entity) {
            self.app.world_mut().write_message(PointerHits::new(
                pointer,
                vec![(entity, HitData::new(camera, 0., None, None))],
                10.,
            ));
        }
        fn input(
            &mut self,
            pointer: PointerId,
            target: NormalizedRenderTarget,
            action: PointerAction,
        ) {
            self.position += Vec2::ONE;
            self.app.world_mut().write_message(PointerInput::new(
                pointer,
                Location {
                    target,
                    position: self.position,
                },
                action,
            ));
        }
        fn mouse(&mut self, window: Entity, entity: Entity, action: PointerAction) {
            self.hit(PointerId::Mouse, entity, self.camera);
            self.input(
                PointerId::Mouse,
                RenderTarget::Window(WindowRef::Entity(window))
                    .normalize(None)
                    .unwrap(),
                action,
            );
        }
        fn step(&mut self, entity: Entity, action: PointerAction) {
            self.mouse(self.window, entity, action);
            self.app.update();
        }
        fn icon(&self, window: Entity) -> &CursorIcon {
            self.app.world().get::<CursorIcon>(window).unwrap()
        }
        #[track_caller]
        fn assert_icon(&self, icon: SystemCursorIcon) {
            assert_eq!(self.icon(self.window), &CursorIcon::System(icon));
        }
        fn start(&mut self, entity: Entity, button: PointerButton) {
            self.step(entity, PointerAction::Press(button));
            self.step(entity, PointerAction::Move { delta: Vec2::ONE });
        }
        fn viewport(&mut self, preview: bool) -> (Entity, Entity, PointerId) {
            let camera = self
                .app
                .world_mut()
                .spawn(SceneView {
                    window: self.window,
                    pane: 1,
                    preview,
                    target: Handle::default(),
                    orbit: [0., 0.5, 10.],
                    center: Vec3::ZERO,
                })
                .id();
            let surface = self
                .app
                .world_mut()
                .spawn((
                    ViewportNode::new(camera),
                    CursorRole::Viewport(camera),
                    UiOwned(self.window),
                ))
                .id();
            let pointer = *self.app.world().get::<PointerId>(surface).unwrap();
            (surface, camera, pointer)
        }
        fn viewport_step(
            &mut self,
            surface: Entity,
            camera: Entity,
            pointer: PointerId,
            mesh: Entity,
            action: PointerAction,
        ) {
            self.mouse(self.window, surface, action);
            self.hit(pointer, mesh, camera);
            self.input(
                pointer,
                RenderTarget::Image(Handle::<Image>::default().into())
                    .normalize(None)
                    .unwrap(),
                action,
            );
            self.app.update();
        }
    }
    const MOVE: PointerAction = PointerAction::Move { delta: Vec2::ONE };

    #[test]
    fn hover_feedback_respects_nested_controls_disabled_state_and_mouse_window() {
        let mut h = Harness::new();
        for (role, icon) in [
            (CursorRole::Click, SystemCursorIcon::Pointer),
            (CursorRole::Text, SystemCursorIcon::Text),
            (CursorRole::Tab, SystemCursorIcon::Pointer),
            (CursorRole::Resize(Axis::X), SystemCursorIcon::ColResize),
            (CursorRole::Resize(Axis::Y), SystemCursorIcon::RowResize),
            (CursorRole::Scrub, SystemCursorIcon::EwResize),
            (CursorRole::Default, SystemCursorIcon::Default),
        ] {
            let control = h.control(role);
            let decoration = h.app.world_mut().spawn(ChildOf(control)).id();
            h.step(decoration, MOVE);
            h.assert_icon(icon);
        }
        let tab = h.control(CursorRole::Tab);
        let input = h
            .app
            .world_mut()
            .spawn((CursorRole::Text, ChildOf(tab)))
            .id();
        let close = h
            .app
            .world_mut()
            .spawn((CursorRole::Click, ChildOf(tab)))
            .id();
        h.step(input, MOVE);
        h.assert_icon(SystemCursorIcon::Text);
        h.step(close, MOVE);
        h.assert_icon(SystemCursorIcon::Pointer);
        h.app
            .world_mut()
            .entity_mut(tab)
            .insert(InteractionDisabled);
        h.step(close, MOVE);
        h.assert_icon(SystemCursorIcon::Default);
        h.app
            .world_mut()
            .entity_mut(tab)
            .remove::<InteractionDisabled>();

        let other_window = h.add_window();
        let other_input = h
            .app
            .world_mut()
            .spawn((CursorRole::Text, UiOwned(other_window)))
            .id();
        h.app.world_mut().spawn(PointerId::Touch(7));
        h.mouse(h.window, close, MOVE);
        h.hit(PointerId::Touch(7), other_input, h.camera);
        h.input(
            PointerId::Touch(7),
            RenderTarget::Window(WindowRef::Entity(other_window))
                .normalize(None)
                .unwrap(),
            MOVE,
        );
        h.app.update();
        h.assert_icon(SystemCursorIcon::Pointer);
        assert_eq!(
            h.icon(other_window),
            &CursorIcon::System(SystemCursorIcon::Default)
        );
        h.mouse(other_window, other_input, MOVE);
        h.app.update();
        h.assert_icon(SystemCursorIcon::Default);
        assert_eq!(
            h.icon(other_window),
            &CursorIcon::System(SystemCursorIcon::Text)
        );
        // Touch dragging remains a real editing session but cannot change the mouse cursor.
        h.mouse(h.window, close, MOVE);
        h.hit(PointerId::Touch(7), tab, h.camera);
        let target = RenderTarget::Window(WindowRef::Entity(h.window))
            .normalize(None)
            .unwrap();
        h.input(
            PointerId::Touch(7),
            target.clone(),
            PointerAction::Press(PointerButton::Primary),
        );
        h.app.update();
        h.app.world_mut().resource_mut::<DragState>().tab = Some((h.window, 2));
        h.mouse(h.window, close, MOVE);
        h.hit(PointerId::Touch(7), tab, h.camera);
        h.input(PointerId::Touch(7), target, MOVE);
        h.app.update();
        h.assert_icon(SystemCursorIcon::Pointer);
        assert_eq!(
            h.app.world().resource::<DragState>().tab,
            Some((h.window, 2))
        );
    }

    #[test]
    fn drag_feedback_overrides_hover_and_recovers_after_release_cancel_or_lifecycle_change() {
        let mut h = Harness::new();
        let button = h.control(CursorRole::Click);
        for (role, icon) in [
            (CursorRole::Resize(Axis::X), SystemCursorIcon::ColResize),
            (CursorRole::Resize(Axis::Y), SystemCursorIcon::RowResize),
            (CursorRole::Scrub, SystemCursorIcon::EwResize),
            (CursorRole::Text, SystemCursorIcon::Text),
            (CursorRole::Tab, SystemCursorIcon::Grabbing),
        ] {
            let control = h.control(role);
            if matches!(role, CursorRole::Tab) {
                let ws = h.app.world().get::<EditorWindow>(h.window).unwrap();
                let group = ws.document.root().first_group();
                let pane = ws.document.root().group(group).unwrap().active;
                // Drop geometry is owned by editor, not by the cursor plugin.
                h.step(control, PointerAction::Press(PointerButton::Primary));
                *h.app.world_mut().resource_mut::<DragState>() = DragState {
                    tab: Some((h.window, pane)),
                    drop: Some((h.window, group, Zone::Center)),
                };
                h.step(control, MOVE);
            } else {
                h.start(control, PointerButton::Primary);
            }
            h.assert_icon(icon);
            h.step(button, MOVE);
            h.assert_icon(icon);
            if matches!(role, CursorRole::Tab) {
                h.app.world_mut().resource_mut::<DragState>().drop = None;
                h.step(button, MOVE);
                h.assert_icon(SystemCursorIcon::NoDrop);
                let other = h.add_window();
                h.app.world_mut().get_mut::<Window>(other).unwrap().focused = false;
                h.mouse(other, other, MOVE);
                h.app.update();
                assert_eq!(h.icon(other), &CursorIcon::System(SystemCursorIcon::NoDrop));
                h.assert_icon(SystemCursorIcon::Default);
                h.step(button, MOVE);
            }
            h.step(button, PointerAction::Release(PointerButton::Primary));
            h.assert_icon(SystemCursorIcon::Pointer);
        }
        // These exits must not leave a resize cursor or revive it on focus regain.
        for exit in ["cancel", "escape", "focus", "despawn", "leave"] {
            let control = h.control(CursorRole::Scrub);
            h.start(control, PointerButton::Primary);
            h.assert_icon(SystemCursorIcon::EwResize);
            match exit {
                "cancel" => h.step(button, PointerAction::Cancel),
                "escape" => {
                    h.app
                        .world_mut()
                        .resource_mut::<ButtonInput<KeyCode>>()
                        .press(KeyCode::Escape);
                    h.step(button, MOVE);
                    h.app
                        .world_mut()
                        .resource_mut::<ButtonInput<KeyCode>>()
                        .clear();
                }
                "focus" => {
                    h.app
                        .world_mut()
                        .get_mut::<Window>(h.window)
                        .unwrap()
                        .focused = false;
                    h.step(button, MOVE);
                    h.assert_icon(SystemCursorIcon::Default);
                    h.app
                        .world_mut()
                        .get_mut::<Window>(h.window)
                        .unwrap()
                        .focused = true;
                    h.step(button, MOVE);
                }
                "despawn" => {
                    h.app.world_mut().despawn(control);
                    h.step(button, MOVE);
                }
                _ => {
                    h.app
                        .world_mut()
                        .get_mut::<Window>(h.window)
                        .unwrap()
                        .set_cursor_position(None);
                    h.step(button, MOVE);
                    h.assert_icon(SystemCursorIcon::Default);
                    h.step(button, PointerAction::Release(PointerButton::Primary));
                    h.app
                        .world_mut()
                        .get_mut::<Window>(h.window)
                        .unwrap()
                        .set_cursor_position(Some(Vec2::splat(10.)));
                    h.step(button, MOVE);
                }
            }
            if exit == "cancel" {
                // Cancellation suppresses this frame's hover; next movement restores it.
                h.assert_icon(SystemCursorIcon::Default);
                h.step(button, MOVE);
            }
            h.assert_icon(SystemCursorIcon::Pointer);
        }
    }

    #[test]
    fn viewport_feedback_uses_only_the_mouse_owned_surface_and_excludes_camera_preview() {
        let mut h = Harness::new();
        let (surface, camera, pointer) = h.viewport(false);
        let (preview, preview_camera, preview_pointer) = h.viewport(true);
        let object = h
            .app
            .world_mut()
            .spawn(EditorObject {
                key: 1,
                kind: ModelKind::Cube,
            })
            .id();
        let mesh = h.app.world_mut().spawn(ChildOf(object)).id();
        let button = h.control(CursorRole::Click);
        h.viewport_step(surface, camera, pointer, mesh, MOVE);
        h.assert_icon(SystemCursorIcon::Pointer);
        h.viewport_step(preview, preview_camera, preview_pointer, mesh, MOVE);
        h.assert_icon(SystemCursorIcon::Default);
        for (button_type, icon) in [
            (PointerButton::Secondary, SystemCursorIcon::Grabbing),
            (PointerButton::Middle, SystemCursorIcon::Move),
        ] {
            h.start(surface, button_type);
            h.assert_icon(icon);
            h.step(button, MOVE);
            h.assert_icon(icon);
            h.step(button, PointerAction::Release(button_type));
            h.assert_icon(SystemCursorIcon::Pointer);
            h.start(preview, button_type);
            h.assert_icon(SystemCursorIcon::Default);
            h.step(preview, PointerAction::Release(button_type));
        }
        h.viewport_step(
            surface,
            camera,
            pointer,
            mesh,
            PointerAction::Press(PointerButton::Primary),
        );
        h.viewport_step(surface, camera, pointer, mesh, MOVE);
        h.assert_icon(SystemCursorIcon::Pointer);
        h.mouse(h.window, button, MOVE);
        h.hit(pointer, mesh, camera);
        h.input(
            pointer,
            RenderTarget::Image(Handle::<Image>::default().into())
                .normalize(None)
                .unwrap(),
            MOVE,
        );
        h.app.update();
        h.assert_icon(SystemCursorIcon::Pointer);
        h.viewport_step(
            surface,
            camera,
            pointer,
            mesh,
            PointerAction::Release(PointerButton::Primary),
        );
        h.assert_icon(SystemCursorIcon::Pointer);

        // An unrelated synthetic pointer hitting the same mesh cannot hijack a UI cursor.
        h.mouse(h.window, button, MOVE);
        h.hit(preview_pointer, mesh, camera);
        h.input(
            preview_pointer,
            RenderTarget::Image(Handle::<Image>::default().into())
                .normalize(None)
                .unwrap(),
            MOVE,
        );
        h.app.update();
        h.assert_icon(SystemCursorIcon::Pointer);
    }
}
