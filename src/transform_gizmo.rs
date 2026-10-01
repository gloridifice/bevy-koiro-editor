//! Bevy's native handles with input routed through editor Viewport surfaces.
//! The RC's built-in input reads PrimaryWindow coordinates, not ViewportNode pointers.
use crate::{
    editor::{Selection, UiDirty},
    scene::{EditorObject, SceneView},
};
use bevy::{
    camera::{NormalizedRenderTarget, RenderTarget, visibility::RenderLayers},
    gizmos::transform_gizmo::*,
    picking::pointer::{
        PointerAction, PointerId, PointerInput, PointerInteraction, PointerLocation, PointerMap,
    },
    prelude::*,
    ui::widget::ViewportNode,
};

pub struct EditorTransformGizmoPlugin;
impl Plugin for EditorTransformGizmoPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(TransformGizmoPlugin)
            // Native interaction assumes a single window-rendering camera. Keep its
            // settings/state/rendering, but replace that input path for offscreen views.
            .configure_sets(PostUpdate, TransformGizmoSystems.run_if(|| false))
            .init_resource::<ViewportGizmo>()
            .insert_resource(TransformGizmoSettings {
                confine_cursor: false,
                axis_hit_distance: 10.,
                ..default()
            })
            .add_systems(PostStartup, configure_native_rendering)
            .add_systems(
                PreUpdate,
                input
                    .after(bevy::picking::hover::update_interactions)
                    .before(bevy::picking::events::pointer_events),
            )
            .add_systems(
                PostUpdate,
                sync_render_camera.before(bevy::camera::CameraUpdateSystems),
            );
    }
}

/// One native gizmo, shared selection; its camera follows the last used Viewport.
#[derive(Resource, Default)]
pub struct ViewportGizmo {
    pub camera: Option<Entity>,
    pub window: Option<Entity>,
    surface: Option<Entity>,
    plane_normal: Vec3,
    direction: Vec3,
    start_cursor: Vec2,
    suppress_pick: bool,
}
impl ViewportGizmo {
    pub fn captures(&self, camera: Entity) -> bool {
        self.camera == Some(camera) && self.suppress_pick
    }
}
#[derive(Component)]
struct NativeOverlay;

fn configure_native_rendering(world: &mut World) {
    // The RC render plugin creates one private overlay on layer 15. It does not
    // copy RenderTarget/Projection from the marked camera, so bind those here.
    // Layer 15 also belongs to a library thumbnail: only the native window-target
    // camera is the overlay. Move its meshes to editor-only layer 2 to avoid leaks.
    let overlays: Vec<_> = world
        .query_filtered::<(Entity, &RenderLayers, &RenderTarget), With<Camera3d>>()
        .iter(world)
        .filter(|(_, layers, target)| {
            **layers == RenderLayers::layer(15) && matches!(target, RenderTarget::Window(_))
        })
        .map(|(entity, _, _)| entity)
        .collect();
    for entity in overlays {
        world.entity_mut(entity).insert((
            NativeOverlay,
            RenderLayers::layer(2),
            Camera {
                is_active: false,
                clear_color: ClearColorConfig::None,
                ..default()
            },
        ));
    }
    let meshes: Vec<_> = world
        .query_filtered::<Entity, With<TransformGizmoMeshMarker>>()
        .iter(world)
        .collect();
    for entity in meshes {
        world
            .entity_mut(entity)
            .insert((Pickable::IGNORE, RenderLayers::layer(2)));
    }
}

fn sync_render_camera(world: &mut World) {
    let selected = world
        .resource::<Selection>()
        .0
        .filter(|e| world.get::<EditorObject>(*e).is_some());
    let focused: Vec<_> = world
        .query_filtered::<Entity, With<TransformGizmoFocus>>()
        .iter(world)
        .collect();
    for entity in focused {
        if Some(entity) != selected {
            world.entity_mut(entity).remove::<TransformGizmoFocus>();
        }
    }
    if let Some(entity) = selected {
        world.entity_mut(entity).insert(TransformGizmoFocus);
    }
    let mut camera = world
        .resource::<ViewportGizmo>()
        .camera
        .filter(|e| world.get::<SceneView>(*e).is_some_and(|v| !v.preview));
    if camera.is_none() {
        camera = world
            .query::<(Entity, &SceneView)>()
            .iter(world)
            .find(|(_, v)| !v.preview)
            .map(|(e, _)| e);
        world.resource_mut::<ViewportGizmo>().camera = camera;
    }
    let marked: Vec<_> = world
        .query_filtered::<Entity, With<TransformGizmoCamera>>()
        .iter(world)
        .collect();
    for entity in marked {
        if Some(entity) != camera {
            world.entity_mut(entity).remove::<TransformGizmoCamera>();
        }
    }
    let source = camera.and_then(|entity| {
        Some((
            entity,
            world.get::<Camera>(entity)?.clone(),
            world.get::<Transform>(entity)?.to_owned(),
            world.get::<Projection>(entity)?.clone(),
            world.get::<RenderTarget>(entity)?.clone(),
        ))
    });
    let overlays: Vec<_> = world
        .query_filtered::<Entity, With<NativeOverlay>>()
        .iter(world)
        .collect();
    if let Some((entity, camera, pose, projection, target)) = source {
        world.entity_mut(entity).insert(TransformGizmoCamera);
        for overlay in overlays {
            world
                .entity_mut(overlay)
                .insert((pose, projection.clone(), target.clone()));
            let mut overlay = world.get_mut::<Camera>(overlay).unwrap();
            overlay.is_active = selected.is_some();
            overlay.order = camera.order + 1;
            overlay.viewport = camera.viewport.clone();
            overlay.clear_color = ClearColorConfig::None;
        }
    } else {
        for overlay in overlays {
            world.get_mut::<Camera>(overlay).unwrap().is_active = false;
        }
    }
}

fn mouse_on_surface(world: &World, surface: Entity) -> Option<(Entity, Entity, Vec2)> {
    let camera = world.get::<ViewportNode>(surface)?.camera?;
    let view = world.get::<SceneView>(camera)?;
    if view.preview || !world.get::<Window>(view.window)?.focused {
        return None;
    }
    let mouse = world
        .resource::<PointerMap>()
        .get_entity(PointerId::Mouse)?;
    let location = world.get::<PointerLocation>(mouse)?.location()?;
    if !matches!(location.target, NormalizedRenderTarget::Window(w) if w.entity() == view.window) {
        return None;
    }
    let node = world.get::<ComputedNode>(surface)?;
    let pose = world.get::<UiGlobalTransform>(surface)?;
    let rect = Rect::from_center_size(pose.translation.trunc(), node.size());
    let top_left = rect.min * node.inverse_scale_factor();
    let size = node.size() * node.inverse_scale_factor();
    if size.min_element() <= 0. {
        return None;
    }
    let viewport_size = world.get::<Camera>(camera)?.logical_viewport_size()?;
    Some((
        camera,
        view.window,
        (location.position - top_left) / size * viewport_size,
    ))
}

fn hover(
    world: &World,
    camera: Entity,
    cursor: Vec2,
    entity: Entity,
) -> Option<TransformGizmoAxis> {
    let camera_pose = world.get::<GlobalTransform>(camera)?;
    let camera = world.get::<Camera>(camera)?;
    let pose = world.get::<GlobalTransform>(entity)?;
    let settings = world.resource::<TransformGizmoSettings>();
    let origin = pose.translation();
    let rotation = gizmo_rotation(pose, effective_space(settings));
    let scale = handle_scale(settings, camera_pose, origin);
    let mut best = (settings.axis_hit_distance, None);
    for axis in [
        TransformGizmoAxis::X,
        TransformGizmoAxis::Y,
        TransformGizmoAxis::Z,
        TransformGizmoAxis::View,
    ] {
        let direction = axis_direction(axis, rotation, camera_pose);
        let distance = match (settings.mode, axis) {
            (TransformGizmoMode::Rotate, _) => point_to_ring_screen_dist(
                cursor,
                camera,
                camera_pose,
                origin,
                direction,
                scale
                    * if axis == TransformGizmoAxis::View {
                        VIEW_RING_MAJOR
                    } else {
                        settings.rotate_ring_radius
                    },
            ),
            (TransformGizmoMode::Translate, TransformGizmoAxis::View) => {
                let center = camera.world_to_viewport(camera_pose, origin).ok()?;
                let edge = camera
                    .world_to_viewport(
                        camera_pose,
                        origin + camera_pose.right() * VIEW_CIRCLE_MAJOR * scale,
                    )
                    .ok()?;
                ((cursor - center).length() - (edge - center).length()).abs()
            }
            (TransformGizmoMode::Scale, TransformGizmoAxis::View) => {
                (cursor - camera.world_to_viewport(camera_pose, origin).ok()?).length()
            }
            _ => {
                let Ok(start) = camera
                    .world_to_viewport(camera_pose, origin + direction * AXIS_START_OFFSET * scale)
                else {
                    continue;
                };
                let Ok(end) = camera.world_to_viewport(
                    camera_pose,
                    origin + direction * settings.axis_length * scale,
                ) else {
                    continue;
                };
                point_to_segment_dist(cursor, start, end)
            }
        };
        if distance < best.0 {
            best = (distance, Some(axis));
        }
    }
    best.1
}
fn handle_scale(settings: &TransformGizmoSettings, camera: &GlobalTransform, origin: Vec3) -> f32 {
    if settings.screen_scale_factor > 0. {
        (camera.translation() - origin).length() * settings.screen_scale_factor
    } else {
        1.
    }
}
fn finish(world: &mut World, rollback: bool) {
    let state = world.resource::<TransformGizmoState>();
    let active = state.active;
    let restore = state.entity.zip(Some(state.start_transform));
    if rollback
        && active
        && let Some((entity, pose)) = restore
        && let Some(mut transform) = world.get_mut::<Transform>(entity)
    {
        *transform = pose;
    }
    *world.resource_mut::<TransformGizmoState>() = TransformGizmoState::default();
    if active {
        world.resource_mut::<UiDirty>().0 = true;
    }
}

fn input(world: &mut World, mut events: Local<bevy::ecs::message::MessageCursor<PointerInput>>) {
    let cancelled = events
        .read(world.resource::<Messages<PointerInput>>())
        .any(|event| {
            event.pointer_id == PointerId::Mouse && matches!(event.action, PointerAction::Cancel)
        });
    let active = world.resource::<TransformGizmoState>().active;
    world.resource_mut::<ViewportGizmo>().suppress_pick = active;
    let escape = world
        .resource::<ButtonInput<KeyCode>>()
        .just_pressed(KeyCode::Escape);
    let selected = world.resource::<Selection>().0;
    if escape || cancelled || (active && selected != world.resource::<TransformGizmoState>().entity)
    {
        finish(world, escape);
        return;
    }
    let Some(entity) = selected.filter(|e| world.get::<EditorObject>(*e).is_some()) else {
        finish(world, false);
        return;
    };
    let pressed = world
        .resource::<ButtonInput<MouseButton>>()
        .pressed(MouseButton::Left);
    if active && !pressed {
        finish(world, false);
        return;
    }
    let mouse = world.resource::<PointerMap>().get_entity(PointerId::Mouse);
    let surface = if active {
        world.resource::<ViewportGizmo>().surface
    } else {
        mouse
            .and_then(|e| world.get::<PointerInteraction>(e))
            .and_then(|hits| {
                hits.iter()
                    .find(|(e, _)| world.get::<ViewportNode>(*e).is_some())
                    .map(|(e, _)| *e)
            })
    };
    let Some((surface, (camera, window, cursor))) =
        surface.and_then(|s| mouse_on_surface(world, s).map(|v| (s, v)))
    else {
        finish(world, false);
        return;
    };
    {
        let mut route = world.resource_mut::<ViewportGizmo>();
        route.camera = Some(camera);
        route.window = Some(window);
        route.surface = Some(surface);
    }
    if !active {
        let axis = hover(world, camera, cursor, entity);
        world.resource_mut::<TransformGizmoState>().hovered_axis = axis;
        if axis.is_some() {
            world.resource_mut::<ViewportGizmo>().suppress_pick = true;
        }
        if !world
            .resource::<ButtonInput<MouseButton>>()
            .just_pressed(MouseButton::Left)
        {
            return;
        }
        let Some(axis) = axis else {
            return;
        };
        let Some(pose) = world.get::<Transform>(entity).copied() else {
            return;
        };
        let global = world.get::<GlobalTransform>(entity).unwrap();
        let camera_pose = world.get::<GlobalTransform>(camera).unwrap();
        let settings = world.resource::<TransformGizmoSettings>();
        let origin = global.translation();
        let direction = axis_direction(
            axis,
            gizmo_rotation(global, effective_space(settings)),
            camera_pose,
        );
        let Ok(ray) = world
            .get::<Camera>(camera)
            .unwrap()
            .viewport_to_world(camera_pose, cursor)
        else {
            return;
        };
        let normal = if settings.mode == TransformGizmoMode::Rotate {
            direction
        } else if axis == TransformGizmoAxis::View {
            camera_pose.forward().as_vec3()
        } else {
            translation_plane_normal(ray, direction)
        };
        let Some(start) = intersect_plane(ray, normal, origin) else {
            return;
        };
        if settings.mode == TransformGizmoMode::Rotate && (start - origin).length_squared() < 1e-8 {
            return;
        }
        let mut state = world.resource_mut::<TransformGizmoState>();
        state.active = true;
        state.axis = Some(axis);
        state.entity = Some(entity);
        state.start_transform = pose;
        state.drag_start_world = start;
        state.gizmo_origin = origin;
        let mut route = world.resource_mut::<ViewportGizmo>();
        route.plane_normal = normal;
        route.direction = direction;
        route.start_cursor = cursor;
        return;
    }
    apply_drag(world, entity, camera, cursor);
}

fn apply_drag(world: &mut World, entity: Entity, camera: Entity, cursor: Vec2) {
    let route = world.resource::<ViewportGizmo>();
    let state = world.resource::<TransformGizmoState>();
    let settings = world.resource::<TransformGizmoSettings>();
    let axis = state.axis.unwrap();
    let Some(camera_pose) = world.get::<GlobalTransform>(camera) else {
        return;
    };
    let Ok(ray) = world
        .get::<Camera>(camera)
        .unwrap()
        .viewport_to_world(camera_pose, cursor)
    else {
        return;
    };
    let Some(point) = intersect_plane(ray, route.plane_normal, state.gizmo_origin) else {
        return;
    };
    let parent = world
        .get::<ChildOf>(entity)
        .and_then(|p| world.get::<GlobalTransform>(p.parent()))
        .copied()
        .unwrap_or(GlobalTransform::IDENTITY);
    if parent.affine().matrix3.determinant().abs() < 1e-8 {
        return;
    }
    let mut pose = state.start_transform;
    let component = match axis {
        TransformGizmoAxis::X => 0,
        TransformGizmoAxis::Y => 1,
        _ => 2,
    };
    match settings.mode {
        TransformGizmoMode::Translate => {
            let mut delta = point - state.drag_start_world;
            if axis != TransformGizmoAxis::View {
                delta = route.direction * snap(delta.dot(route.direction), settings.snap_translate);
            } else if let Some(inc) = settings.snap_translate {
                delta = delta.map(|v| snap(v, Some(inc)));
            }
            pose.translation += parent.affine().inverse().transform_vector3(delta);
        }
        TransformGizmoMode::Rotate => {
            let Some(start) = (state.drag_start_world - state.gizmo_origin).try_normalize() else {
                return;
            };
            let Some(end) = (point - state.gizmo_origin).try_normalize() else {
                return;
            };
            let angle = snap(
                route.direction.dot(start.cross(end)).atan2(start.dot(end)),
                settings.snap_rotate,
            );
            let rotation = parent.to_scale_rotation_translation().1;
            pose.rotation = rotation.inverse()
                * Quat::from_axis_angle(route.direction, angle)
                * rotation
                * pose.rotation;
        }
        TransformGizmoMode::Scale => {
            let factor = if axis == TransformGizmoAxis::View {
                ((cursor.x - route.start_cursor.x) * 0.01 * settings.scale_sensitivity).exp()
            } else {
                let start = (state.drag_start_world - state.gizmo_origin).dot(route.direction);
                if start.abs() < 1e-8 {
                    return;
                }
                1. + ((point - state.gizmo_origin).dot(route.direction) / start - 1.)
                    * settings.scale_sensitivity
            };
            if axis == TransformGizmoAxis::View {
                pose.scale = (pose.scale * factor).map(|v| snap(v, settings.snap_scale).max(0.01));
            } else {
                pose.scale[component] =
                    snap(pose.scale[component] * factor, settings.snap_scale).max(0.01);
            }
        }
    }
    if pose.translation.is_finite()
        && pose.rotation.is_finite()
        && pose.scale.is_finite()
        && let Some(mut transform) = world.get_mut::<Transform>(entity)
    {
        *transform = pose;
    }
}
fn snap(value: f32, increment: Option<f32>) -> f32 {
    increment
        .filter(|v| v.is_finite() && *v > 0.)
        .map_or(value, |v| (value / v).round() * v)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::{
        asset::{AssetApp, AssetPlugin},
        camera::{CameraProjection, ComputedCameraValues, RenderTargetInfo, ScalingMode},
        math::Affine2,
        picking::{
            InteractionPlugin, PickingPlugin,
            backend::{HitData, PointerHits},
            pointer::{Location, PointerAction, PointerInput},
        },
        window::{PrimaryWindow, WindowRef},
    };

    struct Harness {
        app: App,
        window: Entity,
        camera: Entity,
        surface: Entity,
        object: Entity,
        behind: Entity,
        position: Vec2,
    }
    impl Harness {
        fn new() -> Self {
            let mut app = App::new();
            app.add_plugins((
                MinimalPlugins,
                AssetPlugin::default(),
                TransformPlugin,
                PickingPlugin,
                InteractionPlugin,
                EditorTransformGizmoPlugin,
                crate::cursor::EditorCursorPlugin,
                bevy::gizmos_render::transform_gizmo_render::TransformGizmoRenderPlugin,
            ))
            .init_asset::<Mesh>()
            .init_asset::<StandardMaterial>()
            .init_asset::<Image>()
            .init_resource::<Selection>()
            .init_resource::<UiDirty>()
            .init_resource::<crate::editor::DragState>()
            .init_resource::<crate::scene::ObjectCounter>()
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<ButtonInput<KeyCode>>()
            .add_observer(crate::scene::pick_object);
            // Exercise a non-primary window, a UI offset and 200% DPI. Input is
            // supplied at the picking backend boundary, never as gizmo hover state.
            crate::scene::setup(app.world_mut());
            app.world_mut().spawn((Window::default(), PrimaryWindow));
            let window = app
                .world_mut()
                .spawn(Window {
                    focused: true,
                    ..default()
                })
                .id();
            let image =
                app.world_mut()
                    .resource_mut::<Assets<Image>>()
                    .add(Image::new_target_texture(
                        800,
                        600,
                        bevy::render::render_resource::TextureFormat::Bgra8UnormSrgb,
                        None,
                    ));
            let mut projection = OrthographicProjection {
                scaling_mode: ScalingMode::Fixed {
                    width: 8.,
                    height: 6.,
                },
                ..OrthographicProjection::default_3d()
            };
            projection.update(800., 600.);
            let camera = app
                .world_mut()
                .spawn((
                    Camera3d::default(),
                    Camera {
                        computed: ComputedCameraValues {
                            clip_from_view: projection.get_clip_from_view(),
                            target_info: Some(RenderTargetInfo {
                                physical_size: UVec2::new(800, 600),
                                scale_factor: 1.,
                            }),
                            ..default()
                        },
                        ..default()
                    },
                    Projection::Orthographic(projection),
                    Transform::from_xyz(0., 0., 10.),
                    RenderTarget::Image(image.clone().into()),
                    SceneView {
                        window,
                        pane: 1,
                        preview: false,
                        target: image,
                        orbit: [0., 0., 10.],
                        center: Vec3::ZERO,
                    },
                ))
                .id();
            let surface = app
                .world_mut()
                .spawn((
                    ViewportNode::new(camera),
                    ComputedNode {
                        size: Vec2::new(800., 600.),
                        inverse_scale_factor: 0.5,
                        ..default()
                    },
                    UiGlobalTransform::from(Affine2::from_translation(Vec2::new(640., 460.))),
                ))
                .id();
            app.world_mut().entity_mut(surface).insert((
                crate::editor::UiOwned(window),
                crate::cursor::CursorRole::Viewport(camera),
            ));
            app.world_mut()
                .entity_mut(window)
                .insert(crate::editor::EditorWindow {
                    document: crate::layout::LayoutDocument::default(),
                    camera,
                    root: None,
                    focused: 1,
                    maximized: None,
                    menu: None,
                    status: String::new(),
                });
            let parent = app
                .world_mut()
                .spawn(
                    Transform::from_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2))
                        .with_scale(Vec3::splat(2.)),
                )
                .id();
            let object = app
                .world_mut()
                .spawn((
                    Transform::default(),
                    ChildOf(parent),
                    EditorObject {
                        key: 1,
                        kind: crate::scene::ModelKind::Cube,
                    },
                ))
                .id();
            let behind = app
                .world_mut()
                .spawn((
                    Transform::default(),
                    EditorObject {
                        key: 2,
                        kind: crate::scene::ModelKind::Cube,
                    },
                ))
                .id();
            app.world_mut().resource_mut::<Selection>().0 = Some(object);
            app.world_mut().spawn(PointerId::Mouse);
            app.update();
            Self {
                app,
                window,
                camera,
                surface,
                object,
                behind,
                position: Vec2::ZERO,
            }
        }
        fn step(&mut self, cursor: Vec2, action: PointerAction) {
            let world = self.app.world_mut();
            let screen = Vec2::new(120., 80.) + cursor * 0.5;
            world
                .get_mut::<Window>(self.window)
                .unwrap()
                .set_cursor_position(Some(screen));
            let mouse = world.resource_mut::<ButtonInput<MouseButton>>();
            match action {
                PointerAction::Press(_) => mouse.into_inner().press(MouseButton::Left),
                PointerAction::Release(_) => mouse.into_inner().release(MouseButton::Left),
                _ => {}
            }
            world.write_message(PointerInput::new(
                PointerId::Mouse,
                Location {
                    target: RenderTarget::Window(WindowRef::Entity(self.window))
                        .normalize(None)
                        .unwrap(),
                    position: screen,
                },
                action,
            ));
            world.write_message(PointerHits::new(
                PointerId::Mouse,
                vec![(self.surface, HitData::new(self.camera, 0., None, None))],
                10.,
            ));
            let pointer = *world.get::<PointerId>(self.surface).unwrap();
            world.write_message(PointerInput::new(
                pointer,
                Location {
                    target: world
                        .get::<RenderTarget>(self.camera)
                        .unwrap()
                        .normalize(None)
                        .unwrap(),
                    position: cursor,
                },
                action,
            ));
            world.write_message(PointerHits::new(
                pointer,
                vec![(self.behind, HitData::new(self.camera, 0., None, None))],
                10.,
            ));
            self.app.update();
            self.app
                .world_mut()
                .resource_mut::<ButtonInput<MouseButton>>()
                .clear();
            self.position = cursor;
        }
        fn move_to(&mut self, cursor: Vec2) {
            self.step(
                cursor,
                PointerAction::Move {
                    delta: (cursor - self.position) * 0.5,
                },
            );
        }
        fn press(&mut self, cursor: Vec2) {
            self.move_to(cursor);
            self.step(cursor, PointerAction::Press(PointerButton::Primary));
            assert!(self.app.world().resource::<TransformGizmoState>().active);
        }
        fn release(&mut self) {
            self.step(
                self.position,
                PointerAction::Release(PointerButton::Primary),
            );
            assert!(!self.app.world().resource::<TransformGizmoState>().active);
        }
        fn pose(&self) -> Transform {
            *self.app.world().get::<Transform>(self.object).unwrap()
        }
    }

    #[test]
    fn native_handles_edit_parented_objects_from_offset_secondary_viewport_and_cancel() {
        let mut h = Harness::new();
        h.move_to(Vec2::new(470., 300.));
        assert_eq!(
            h.app.world().get::<bevy::window::CursorIcon>(h.window),
            Some(&bevy::window::CursorIcon::System(
                bevy::window::SystemCursorIcon::Pointer
            ))
        );
        h.press(Vec2::new(470., 300.));
        assert_eq!(
            h.app.world().resource::<TransformGizmoState>().axis,
            Some(TransformGizmoAxis::X)
        );
        h.move_to(Vec2::new(570., 300.));
        assert!((h.pose().translation - Vec3::new(0., -0.5, 0.)).length() < 1e-4);
        assert_eq!(h.pose().scale, Vec3::ONE);
        h.app
            .world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Escape);
        h.move_to(Vec2::new(570., 300.));
        assert_eq!(h.pose(), Transform::default());
        assert!(!h.app.world().resource::<TransformGizmoState>().active);
        h.app
            .world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .clear();
        h.release();

        h.app
            .world_mut()
            .resource_mut::<TransformGizmoSettings>()
            .mode = TransformGizmoMode::Rotate;
        h.press(Vec2::new(470.710_7, 229.28932));
        assert_eq!(
            h.app.world().resource::<TransformGizmoState>().axis,
            Some(TransformGizmoAxis::Z)
        );
        h.move_to(Vec2::new(329.289_3, 229.28932));
        assert!((h.pose().rotation * Vec3::X - Vec3::Y).length() < 1e-4);
        assert_eq!(h.pose().translation, Vec3::ZERO);
        h.release();

        h.app
            .world_mut()
            .resource_mut::<TransformGizmoSettings>()
            .mode = TransformGizmoMode::Scale;
        h.press(Vec2::new(400., 300.));
        h.move_to(Vec2::new(450., 300.));
        assert!((h.pose().scale - Vec3::splat(0.5_f32.exp())).length() < 1e-4);
        h.release();
        let uniform = h.pose();
        h.press(Vec2::new(330., 300.));
        assert_eq!(
            h.app.world().resource::<TransformGizmoState>().axis,
            Some(TransformGizmoAxis::X)
        );
        h.move_to(Vec2::new(280., 300.));
        assert!((h.pose().scale.x - uniform.scale.x * 12. / 7.).abs() < 1e-4);
        assert_eq!(h.pose().scale.y, uniform.scale.y);
        assert_eq!(h.pose().scale.z, uniform.scale.z);
        assert_eq!(h.pose().rotation, uniform.rotation);
        h.release();
        assert!(h.app.world().resource::<UiDirty>().0);
        assert_eq!(
            h.app.world().resource::<Selection>().0,
            Some(h.object),
            "mesh clicks beneath the handles must not replace the edited selection"
        );
    }

    #[test]
    fn overlay_targets_only_editor_view_and_invalid_sources_end_drag() {
        let mut h = Harness::new();
        let world = h.app.world_mut();
        let target = world.get::<RenderTarget>(h.camera).unwrap().clone();
        let overlay = world
            .query_filtered::<Entity, With<NativeOverlay>>()
            .single(world)
            .unwrap();
        assert_eq!(
            world.get::<RenderTarget>(overlay).unwrap().as_image(),
            target.as_image()
        );
        assert!(matches!(
            world.get::<Camera>(overlay).unwrap().clear_color,
            ClearColorConfig::None
        ));
        h.press(Vec2::new(470., 300.));
        h.move_to(Vec2::new(570., 300.));
        let moved = h.pose();
        h.app
            .world_mut()
            .get_mut::<Window>(h.window)
            .unwrap()
            .focused = false;
        h.move_to(Vec2::new(650., 300.));
        assert!(!h.app.world().resource::<TransformGizmoState>().active);
        assert_eq!(h.pose(), moved);
        h.release();
        h.app
            .world_mut()
            .get_mut::<Window>(h.window)
            .unwrap()
            .focused = true;
        h.press(Vec2::new(570., 300.));
        h.move_to(Vec2::new(650., 300.));
        let cancelled = h.pose();
        h.step(Vec2::new(650., 300.), PointerAction::Cancel);
        assert!(!h.app.world().resource::<TransformGizmoState>().active);
        assert_eq!(h.pose(), cancelled);
        h.release();
        h.app
            .world_mut()
            .get_mut::<SceneView>(h.camera)
            .unwrap()
            .preview = true;
        h.step(
            Vec2::new(570., 300.),
            PointerAction::Press(PointerButton::Primary),
        );
        assert!(!h.app.world().resource::<TransformGizmoState>().active);
        assert_eq!(h.pose(), cancelled);
        assert!(!h.app.world().get::<Camera>(overlay).unwrap().is_active);
        h.app.world_mut().despawn(h.camera);
        h.app.update();
        assert!(!h.app.world().get::<Camera>(overlay).unwrap().is_active);
    }
}
