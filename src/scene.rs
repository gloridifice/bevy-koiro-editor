use crate::editor::{Selection, UiDirty};
use bevy::{
    camera::{RenderTarget, visibility::RenderLayers},
    prelude::*,
    render::render_resource::TextureFormat,
};

#[derive(Component, Clone)]
pub struct EditorObject {
    pub key: u64,
    pub kind: ModelKind,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModelKind {
    Group,
    Ground,
    Arch,
    Tree,
    Orb,
    Steps,
    Rock,
    Bench,
    Cypress,
    Planter,
    Camera,
    Light,
    Cube,
}
impl ModelKind {
    pub fn name(self) -> &'static str {
        match self {
            Self::Group => "Group",
            Self::Ground => "Courtyard",
            Self::Arch => "Arc Pavilion",
            Self::Tree => "Ginkgo",
            Self::Orb => "Orbit Sculpture",
            Self::Steps => "Stone Steps",
            Self::Rock => "River Stones",
            Self::Bench => "Timber Bench",
            Self::Cypress => "Cypress",
            Self::Planter => "Fern Planter",
            Self::Camera => "Main Camera",
            Self::Light => "Sun",
            Self::Cube => "Cube",
        }
    }
}
#[derive(Resource, Default)]
pub struct ObjectCounter(pub u64);
#[derive(Component)]
pub struct MainGameCamera;
#[derive(Component)]
pub struct SceneView {
    pub window: Entity,
    pub pane: u64,
    pub preview: bool,
    pub target: Handle<Image>,
    pub orbit: [f32; 3],
    pub center: Vec3,
}
#[derive(Clone)]
pub struct LibraryAsset {
    pub kind: ModelKind,
    pub thumbnail: Handle<Image>,
}
#[derive(Resource, Default)]
pub struct Library(pub Vec<LibraryAsset>);

pub fn setup(world: &mut World) {
    world.insert_resource(GlobalAmbientLight {
        color: Color::WHITE,
        brightness: 600.0,
        ..default()
    });
    let env = object(world, "Environment", ModelKind::Group, Vec3::ZERO, None);
    let light = object(world, "Sun", ModelKind::Light, Vec3::ZERO, Some(env));
    world.entity_mut(light).insert((
        DirectionalLight {
            illuminance: 12000.0,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_xyz(5., 10., 6.).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    let cam = object(
        world,
        "Main Camera",
        ModelKind::Camera,
        Vec3::ZERO,
        Some(env),
    );
    // Pose entity, not a window-rendering camera; every preview renders this pose independently.
    world.entity_mut(cam).insert((
        MainGameCamera,
        Transform::from_xyz(8., 6., 9.).looking_at(Vec3::new(0., 1., 0.), Vec3::Y),
    ));
    let architecture = object(world, "Architecture", ModelKind::Group, Vec3::ZERO, None);
    spawn_model(
        world,
        ModelKind::Ground,
        Vec3::ZERO,
        Some(architecture),
        Some(0),
    );
    let arch = spawn_model(
        world,
        ModelKind::Arch,
        Vec3::new(-1.12, 0., -0.9),
        Some(architecture),
        Some(0),
    );
    spawn_model(
        world,
        ModelKind::Steps,
        Vec3::new(-1.12, 0., 1.2),
        Some(architecture),
        Some(0),
    );
    let garden = object(world, "Garden", ModelKind::Group, Vec3::ZERO, None);
    let tree = spawn_model(
        world,
        ModelKind::Tree,
        Vec3::new(2.5, 0., -1.8),
        Some(garden),
        Some(0),
    );
    world.entity_mut(tree).insert(Name::new("Ginkgo · 01"));
    let tree = spawn_model(
        world,
        ModelKind::Tree,
        Vec3::new(-3., 0., 0.6),
        Some(garden),
        Some(0),
    );
    world.entity_mut(tree).insert((
        Name::new("Ginkgo · 02"),
        Transform::from_xyz(-3., 0., 0.6).with_scale(Vec3::splat(0.75)),
    ));
    spawn_model(
        world,
        ModelKind::Cypress,
        Vec3::new(2.9, 0., 0.5),
        Some(garden),
        Some(0),
    );
    spawn_model(
        world,
        ModelKind::Planter,
        Vec3::new(1.5, 0., 2.25),
        Some(garden),
        Some(0),
    );
    spawn_model(
        world,
        ModelKind::Rock,
        Vec3::new(-2.95, 0., 2.17),
        Some(garden),
        Some(0),
    );
    let props = object(world, "Props", ModelKind::Group, Vec3::ZERO, None);
    spawn_model(
        world,
        ModelKind::Orb,
        Vec3::new(-1.12, 0., -0.8),
        Some(props),
        Some(0),
    );
    spawn_model(
        world,
        ModelKind::Bench,
        Vec3::new(1.25, 0., 1.05),
        Some(props),
        Some(0),
    );
    world.resource_mut::<Selection>().0 = Some(arch);
    let mut library = Vec::new();
    for (i, kind) in [
        ModelKind::Arch,
        ModelKind::Tree,
        ModelKind::Orb,
        ModelKind::Steps,
        ModelKind::Rock,
        ModelKind::Bench,
        ModelKind::Cypress,
        ModelKind::Planter,
        ModelKind::Cube,
    ]
    .into_iter()
    .enumerate()
    {
        let layer = 10 + i;
        spawn_model(world, kind, Vec3::ZERO, None, Some(layer));
        // Library roots must not appear in the editable world tree.
        let image = world
            .resource_mut::<Assets<Image>>()
            .add(Image::new_target_texture(
                160,
                116,
                TextureFormat::Bgra8UnormSrgb,
                None,
            ));
        world.spawn((
            Camera3d::default(),
            Camera {
                order: -2,
                clear_color: ClearColorConfig::Custom(Color::srgb_u8(51, 51, 51)),
                ..default()
            },
            RenderTarget::Image(image.clone().into()),
            RenderLayers::layer(layer),
            Transform::from_xyz(3.0, 2.5, 4.0).looking_at(Vec3::new(0., 1., 0.), Vec3::Y),
        ));
        library.push(LibraryAsset {
            kind,
            thumbnail: image,
        });
    }
    world.spawn((
        DirectionalLight {
            illuminance: 10000.,
            ..default()
        },
        RenderLayers::from_layers(&(10..19).collect::<Vec<_>>()),
        Transform::from_xyz(4., 7., 5.).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    world.insert_resource(Library(library));
}
fn object(
    world: &mut World,
    name: &str,
    kind: ModelKind,
    pos: Vec3,
    parent: Option<Entity>,
) -> Entity {
    let key = {
        let mut counter = world.resource_mut::<ObjectCounter>();
        counter.0 += 1;
        counter.0
    };
    let e = world
        .spawn((
            Name::new(name.to_owned()),
            EditorObject { key, kind },
            Transform::from_translation(pos),
            Visibility::default(),
        ))
        .id();
    if let Some(parent) = parent {
        world.entity_mut(e).insert(ChildOf(parent));
    }
    e
}
pub fn spawn_model(
    world: &mut World,
    kind: ModelKind,
    pos: Vec3,
    parent: Option<Entity>,
    layer: Option<usize>,
) -> Entity {
    let layer = layer.unwrap_or(0);
    let root = if layer == 0 {
        object(world, kind.name(), kind, pos, parent)
    } else {
        world
            .spawn((Transform::from_translation(pos), Visibility::default()))
            .id()
    };
    let terra = Color::srgb_u8(223, 148, 121);
    let sage = Color::srgb_u8(168, 187, 129);
    let stone = Color::srgb_u8(213, 198, 164);
    let wood = Color::srgb_u8(155, 121, 83);
    match kind {
        ModelKind::Ground => {
            part(
                world,
                root,
                Cuboid::new(8., 0.22, 7.),
                Vec3::new(0., -0.12, 0.),
                Color::srgb_u8(188, 200, 173),
                layer,
            );
            for i in -4..=4 {
                part(
                    world,
                    root,
                    Cuboid::new(0.012, 0.008, 7.),
                    Vec3::new(i as f32, 0., 0.),
                    Color::srgb_u8(139, 153, 129),
                    layer,
                );
            }
            for i in -3..=3 {
                part(
                    world,
                    root,
                    Cuboid::new(8., 0.008, 0.012),
                    Vec3::new(0., 0., i as f32),
                    Color::srgb_u8(139, 153, 129),
                    layer,
                );
            }
        }
        ModelKind::Arch => {
            part(
                world,
                root,
                Cuboid::new(2.8, 0.15, 1.0),
                Vec3::new(0., 0.075, 0.),
                terra,
                layer,
            );
            for x in [-0.95, 0.95] {
                part(
                    world,
                    root,
                    Cuboid::new(0.46, 1.55, 0.55),
                    Vec3::new(x, 0.9, 0.),
                    terra,
                    layer,
                );
            }
            for i in 0..12 {
                let angle = (i as f32 + 0.5) * std::f32::consts::PI / 12.;
                let e = part(
                    world,
                    root,
                    Cuboid::new(0.30, 0.46, 0.55),
                    Vec3::new(angle.cos() * 0.95, 1.62 + angle.sin() * 0.95, 0.),
                    terra,
                    layer,
                );
                world.entity_mut(e).get_mut::<Transform>().unwrap().rotation =
                    Quat::from_rotation_z(angle - std::f32::consts::FRAC_PI_2);
            }
        }
        ModelKind::Tree => {
            part(
                world,
                root,
                Cylinder::new(0.085, 1.8),
                Vec3::new(0., 0.9, 0.),
                wood,
                layer,
            );
            for (p, r) in [
                (Vec3::new(0., 2., 0.), 0.67),
                (Vec3::new(0.25, 2.5, 0.), 0.6),
                (Vec3::new(-0.2, 2.3, 0.1), 0.58),
            ] {
                let mesh = Sphere::new(r).mesh().ico(1).unwrap();
                part(world, root, mesh, p, sage, layer);
            }
        }
        ModelKind::Orb => {
            part(
                world,
                root,
                Cylinder::new(0.25, 0.75),
                Vec3::new(0., 0.375, 0.),
                stone,
                layer,
            );
            part(
                world,
                root,
                Sphere::new(0.48).mesh().ico(2).unwrap(),
                Vec3::new(0., 1.23, 0.),
                Color::srgb_u8(229, 201, 132),
                layer,
            );
            let e = part(
                world,
                root,
                Torus::new(0.55, 0.58),
                Vec3::new(0., 1.23, 0.),
                Color::srgb_u8(200, 169, 96),
                layer,
            );
            world.entity_mut(e).get_mut::<Transform>().unwrap().rotation =
                Quat::from_rotation_z(0.3);
        }
        ModelKind::Steps => {
            for i in 0..3 {
                part(
                    world,
                    root,
                    Cuboid::new(2.25, 0.15, 0.65),
                    Vec3::new(0., 0.075 + i as f32 * 0.15, -i as f32 * 0.6),
                    stone,
                    layer,
                );
            }
        }
        ModelKind::Rock => {
            for (p, r) in [
                (Vec3::new(-0.2, 0.2, 0.), 0.35),
                (Vec3::new(0.25, 0.15, 0.15), 0.25),
                (Vec3::new(0., 0.1, -0.3), 0.20),
            ] {
                part(
                    world,
                    root,
                    Sphere::new(r).mesh().ico(0).unwrap(),
                    p,
                    Color::srgb_u8(166, 178, 161),
                    layer,
                );
            }
        }
        ModelKind::Bench => {
            part(
                world,
                root,
                Cuboid::new(1.5, 0.12, 0.5),
                Vec3::new(0., 0.6, 0.),
                wood,
                layer,
            );
            part(
                world,
                root,
                Cuboid::new(1.5, 0.35, 0.10),
                Vec3::new(0., 0.92, -0.2),
                wood,
                layer,
            );
            for x in [-0.55, 0.55] {
                part(
                    world,
                    root,
                    Cuboid::new(0.12, 0.6, 0.4),
                    Vec3::new(x, 0.3, 0.),
                    wood,
                    layer,
                );
            }
        }
        ModelKind::Cypress => {
            part(
                world,
                root,
                Cone::new(0.55, 2.7),
                Vec3::new(0., 1.55, 0.),
                Color::srgb_u8(113, 153, 118),
                layer,
            );
            part(
                world,
                root,
                Cylinder::new(0.08, 0.35),
                Vec3::new(0., 0.175, 0.),
                wood,
                layer,
            );
        }
        ModelKind::Planter => {
            part(
                world,
                root,
                Cuboid::new(1.2, 0.4, 0.55),
                Vec3::new(0., 0.2, 0.),
                wood,
                layer,
            );
            for x in [-0.35, 0., 0.35] {
                part(
                    world,
                    root,
                    Cone::new(0.18, 0.7),
                    Vec3::new(x, 0.65, 0.),
                    sage,
                    layer,
                );
            }
        }
        _ => {
            part(
                world,
                root,
                Cuboid::new(1., 1., 1.),
                Vec3::Y * 0.5,
                sage,
                layer,
            );
        }
    }
    root
}
fn part(
    world: &mut World,
    parent: Entity,
    mesh: impl Into<Mesh>,
    pos: Vec3,
    color: Color,
    layer: usize,
) -> Entity {
    let mesh = world.resource_mut::<Assets<Mesh>>().add(mesh.into());
    let material = world
        .resource_mut::<Assets<StandardMaterial>>()
        .add(StandardMaterial {
            base_color: color,
            perceptual_roughness: 0.68,
            ..default()
        });
    world
        .spawn((
            Mesh3d(mesh),
            MeshMaterial3d(material),
            Transform::from_translation(pos),
            RenderLayers::layer(layer),
            ChildOf(parent),
        ))
        .id()
}
pub fn orbit_transform(orbit: [f32; 3], center: Vec3) -> Transform {
    let [yaw, pitch, d] = orbit;
    Transform::from_translation(
        center
            + Vec3::new(
                yaw.sin() * pitch.cos(),
                pitch.sin(),
                yaw.cos() * pitch.cos(),
            ) * d,
    )
    .looking_at(center, Vec3::Y)
}
pub fn sync_previews(
    poses: Query<&GlobalTransform, With<MainGameCamera>>,
    mut cameras: Query<(&SceneView, &mut Transform), Without<MainGameCamera>>,
) {
    let Some(pose) = poses.iter().next() else {
        return;
    };
    for (view, mut transform) in &mut cameras {
        if view.preview {
            *transform = pose.compute_transform();
        }
    }
}
pub fn gizmos(
    selection: Res<Selection>,
    objects: Query<&GlobalTransform, With<EditorObject>>,
    mut gizmos: Gizmos,
) {
    let grid = Color::srgb_u8(57, 57, 57);
    for i in -20..=20 {
        let f = i as f32;
        gizmos.line(Vec3::new(f, -0.15, -20.), Vec3::new(f, -0.15, 20.), grid);
        gizmos.line(Vec3::new(-20., -0.15, f), Vec3::new(20., -0.15, f), grid);
    }
    if let Some(entity) = selection.0
        && let Ok(transform) = objects.get(entity)
    {
        gizmos.cube(
            Transform::from_translation(transform.translation() + Vec3::Y * 1.4)
                .with_scale(Vec3::splat(2.8)),
            Color::srgba(0.85, 0.85, 0.85, 0.35),
        );
    }
}
pub fn pick_object(
    event: On<PointerClick>,
    mut selection: ResMut<Selection>,
    mut dirty: ResMut<UiDirty>,
    objects: Query<&EditorObject>,
    parents: Query<&ChildOf>,
    views: Query<&SceneView>,
    gizmo: Option<Res<crate::transform_gizmo::ViewportGizmo>>,
) {
    if gizmo.is_some_and(|g| g.captures(event.hit.camera)) {
        return;
    }
    if event.button != PointerButton::Primary
        || !views.get(event.hit.camera).is_ok_and(|v| !v.preview)
    {
        return;
    }
    let mut entity = event.entity;
    loop {
        if objects.contains(entity) {
            selection.0 = Some(entity);
            dirty.0 = true;
            return;
        }
        let Ok(parent) = parents.get(entity) else {
            return;
        };
        entity = parent.parent();
    }
}
