//! Embedded Lucide SVGs rasterized on demand into tintable native UI images.
use bevy::{
    asset::RenderAssetUsages,
    image::ImageSampler,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
    ui::UiSystems,
};
use resvg::{tiny_skia, usvg};
use std::collections::{HashMap, HashSet};

macro_rules! lucide_icons {
    ($($variant:ident => $file:literal),+ $(,)?) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum Icon { $($variant),+ }
        impl Icon {
            fn svg(self) -> &'static str {
                match self {
                    $(Self::$variant => include_str!(concat!("lucide_icons/", $file, ".svg"))),+
                }
            }
            #[cfg(test)]
            const ALL: &'static [Self] = &[$(Self::$variant),+];
        }
    };
}
lucide_icons! {
    World => "network",
    Inspector => "sliders-horizontal",
    Folder => "folder",
    FolderOpen => "folder-open",
    Gallery => "layout-grid",
    Viewport => "monitor",
    Camera => "camera",
    ChevronDown => "chevron-down",
    ChevronRight => "chevron-right",
    Search => "search",
    Refresh => "refresh-cw",
    SquareCheck => "square-check",
    Square => "square",
    Select => "mouse-pointer-2",
    Move => "move",
    Globe => "globe",
    Check => "check",
    Close => "x",
    Plus => "plus",
    Help => "circle-question-mark",
    More => "ellipsis",
    File => "file",
    FileText => "file-text",
    FileImage => "file-image",
    FileCode => "file-code",
    Ready => "circle-check",
    Live => "circle",
    Box => "box",
    Sun => "sun",
    Trees => "trees",
    Orbit => "orbit",
    Columns => "columns-2",
    Rows => "rows-2",
    Maximize => "maximize",
    Minimize => "minimize",
    Save => "save",
    Load => "folder-input",
    NewWindow => "app-window",
    Copy => "copy",
}

#[derive(Component)]
#[require(ImageNode)]
pub struct LucideIcon(pub Icon);

pub struct LucideIconsPlugin;
impl Plugin for LucideIconsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<IconCache>().add_systems(
            PostUpdate,
            update_images
                .after(UiSystems::Layout)
                .before(bevy::asset::AssetEventSystems),
        );
    }
}

#[derive(Resource, Default)]
struct IconCache {
    trees: HashMap<Icon, usvg::Tree>,
    images: HashMap<(Icon, UVec2), Handle<Image>>,
}
impl IconCache {
    fn image(&mut self, icon: Icon, size: UVec2, images: &mut Assets<Image>) -> Handle<Image> {
        self.images
            .entry((icon, size))
            .or_insert_with(|| {
                let tree = self.trees.entry(icon).or_insert_with(|| {
                    usvg::Tree::from_str(icon.svg(), &usvg::Options::default())
                        .expect("bundled Lucide SVG must be valid")
                });
                let mut pixmap = tiny_skia::Pixmap::new(size.x, size.y)
                    .expect("bounded, nonzero icon dimensions");
                // Contain the square SVG even when a caller supplies a rectangular node.
                let scale =
                    (size.x as f32 / tree.size().width()).min(size.y as f32 / tree.size().height());
                let transform = tiny_skia::Transform::from_scale(scale, scale).post_translate(
                    (size.x as f32 - tree.size().width() * scale) * 0.5,
                    (size.y as f32 - tree.size().height() * scale) * 0.5,
                );
                resvg::render(tree, transform, &mut pixmap.as_mut());
                let mut pixels = pixmap.take();
                // Lucide is monochrome: keep coverage as alpha and make RGB straight white.
                // Copying tiny-skia's premultiplied pixels would darken antialiased edges
                // with Bevy UI's straight-alpha blending, and black currentColor can't tint.
                for pixel in pixels.chunks_exact_mut(4) {
                    pixel[..3].fill(255);
                }
                let mut image = Image::new(
                    Extent3d {
                        width: size.x,
                        height: size.y,
                        depth_or_array_layers: 1,
                    },
                    TextureDimension::D2,
                    pixels,
                    TextureFormat::Rgba8UnormSrgb,
                    RenderAssetUsages::default(),
                );
                image.sampler = ImageSampler::linear();
                images.add(image)
            })
            .clone()
    }
}

fn update_images(
    mut icons: Query<(&LucideIcon, &ComputedNode, &mut ImageNode)>,
    mut cache: ResMut<IconCache>,
    mut images: ResMut<Assets<Image>>,
) {
    let mut live = HashSet::new();
    for (icon, node, mut image) in &mut icons {
        // ComputedNode is already in physical pixels: this includes UiScale and
        // the particular target window's DPI, not just the primary window's DPI.
        let size = node.size();
        if !size.is_finite() || size.x <= 0. || size.y <= 0. {
            continue;
        }
        let size = size.ceil().clamp(Vec2::ONE, Vec2::splat(512.)).as_uvec2();
        live.insert((icon.0, size));
        let handle = cache.image(icon.0, size, &mut images);
        if image.image != handle {
            image.image = handle;
        }
    }
    // Drop unused sizes after a resize/window close; native asset lifetime follows
    // the remaining ImageNode handles. Same-frame shell rebuilds reuse live sizes.
    cache.images.retain(|key, _| live.contains(key));
}

pub fn spawn(world: &mut World, parent: Entity, icon: Icon, size: f32, color: Color) -> Entity {
    world
        .spawn((
            LucideIcon(icon),
            Node {
                width: px(size),
                height: px(size),
                flex_shrink: 0.,
                ..default()
            },
            ImageNode { color, ..default() },
            Pickable::IGNORE,
            ChildOf(parent),
        ))
        .id()
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::{
        asset::AssetPlugin,
        camera::{RenderTarget, RenderTargetInfo, visibility::VisibilityPlugin},
        input::InputPlugin,
        input_focus::InputFocusPlugin,
        picking::{InteractionPlugin, PickingPlugin},
        text::TextPlugin,
        ui::{UiPlugin, UiScale},
        window::{WindowPlugin, WindowRef},
    };

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            InputPlugin,
            WindowPlugin {
                primary_window: None,
                ..default()
            },
            PickingPlugin,
            InteractionPlugin,
            InputFocusPlugin,
            TextPlugin,
            TransformPlugin,
            bevy::mesh::MeshPlugin,
            VisibilityPlugin,
            UiPlugin,
            LucideIconsPlugin,
        ))
        .init_asset::<Image>()
        .init_asset::<TextureAtlasLayout>();
        app
    }
    fn root(app: &mut App, scale: f32) -> (Entity, Entity) {
        let window = app.world_mut().spawn(Window::default()).id();
        let mut camera = Camera::default();
        camera.computed.target_info = Some(RenderTargetInfo {
            physical_size: UVec2::splat(1000),
            scale_factor: scale,
        });
        let camera = app
            .world_mut()
            .spawn((
                Camera2d,
                camera,
                RenderTarget::Window(WindowRef::Entity(window)),
            ))
            .id();
        let root = app
            .world_mut()
            .spawn((
                Node {
                    width: percent(100),
                    height: percent(100),
                    flex_wrap: FlexWrap::Wrap,
                    ..default()
                },
                UiTargetCamera(camera),
            ))
            .id();
        (root, camera)
    }
    fn handle(app: &App, icon: Entity) -> Handle<Image> {
        app.world().get::<ImageNode>(icon).unwrap().image.clone()
    }
    fn size(app: &App, icon: Entity) -> UVec2 {
        app.world()
            .resource::<Assets<Image>>()
            .get(&handle(app, icon))
            .unwrap()
            .size()
    }

    #[test]
    fn bundled_icons_deliver_visible_white_alpha_masks_for_native_tinting() {
        // Guards the actual bundled assets and the resvg -> Bevy alpha boundary.
        // Black currentColor or copied premultiplied edges can't be tinted correctly.
        let mut app = app();
        let (root, _) = root(&mut app, 1.);
        let icons: Vec<_> = Icon::ALL
            .iter()
            .map(|&icon| {
                (
                    icon,
                    spawn(app.world_mut(), root, icon, 24., Color::srgb(0.2, 0.6, 1.)),
                )
            })
            .collect();
        app.update();
        for (kind, icon) in icons {
            let images = app.world().resource::<Assets<Image>>();
            let image = images
                .get(&handle(&app, icon))
                .expect("icon was rasterized after layout");
            assert_eq!(image.size(), UVec2::splat(24), "{kind:?}");
            let pixels: Vec<_> = image.data.as_ref().unwrap().chunks_exact(4).collect();
            assert!(pixels.iter().any(|p| p[3] > 0), "{kind:?}: missing strokes");
            assert!(
                pixels.iter().any(|p| p[3] == 0),
                "{kind:?}: opaque background"
            );
            assert!(
                pixels.iter().any(|p| p[3] > 0 && p[3] < 255),
                "{kind:?}: no antialiasing"
            );
            assert!(
                pixels.iter().all(|p| p[..3] == [255, 255, 255]),
                "{kind:?}: dark tint/edges"
            );
            assert_eq!(
                app.world().get::<ImageNode>(icon).unwrap().color,
                Color::srgb(0.2, 0.6, 1.)
            );
            assert_eq!(
                *app.world().get::<Pickable>(icon).unwrap(),
                Pickable::IGNORE
            );
        }
    }

    #[test]
    fn native_layout_drives_dpi_size_and_reuses_textures_across_tints_and_rebuilds() {
        // Real UI layout owns physical dimensions; no fabricated ComputedNode sizes.
        // Detect primary-window DPI assumptions, resize lag and redundant rasterization.
        let mut app = app();
        let (first_root, first_camera) = root(&mut app, 1.);
        let (second_root, _) = root(&mut app, 2.);
        let first = spawn(app.world_mut(), first_root, Icon::Search, 16., Color::WHITE);
        let second = spawn(app.world_mut(), first_root, Icon::Search, 16., Color::BLACK);
        let high_dpi = spawn(
            app.world_mut(),
            second_root,
            Icon::Search,
            16.,
            Color::WHITE,
        );
        app.update();
        assert_eq!(size(&app, first), UVec2::splat(16));
        assert_eq!(size(&app, high_dpi), UVec2::splat(32));
        assert_eq!(handle(&app, first), handle(&app, second));
        assert_ne!(handle(&app, first), handle(&app, high_dpi));
        let original = handle(&app, first);
        app.world_mut().get_mut::<ImageNode>(first).unwrap().color = Color::srgb(0.8, 0.3, 0.1);
        app.update();
        app.update();
        assert_eq!(
            handle(&app, first),
            original,
            "tint/idle frames must not re-render"
        );
        app.world_mut().despawn(first);
        app.world_mut().despawn(second);
        let rebuilt = spawn(app.world_mut(), first_root, Icon::Search, 16., Color::WHITE);
        app.update();
        assert_eq!(
            handle(&app, rebuilt),
            original,
            "same-frame rebuild must reuse the texture"
        );
        app.world_mut()
            .get_mut::<Camera>(first_camera)
            .unwrap()
            .computed
            .target_info
            .as_mut()
            .unwrap()
            .scale_factor = 2.;
        app.update();
        assert_eq!(
            handle(&app, rebuilt),
            handle(&app, high_dpi),
            "DPI change must use the shared 32px texture"
        );
        app.world_mut().get_mut::<Node>(rebuilt).unwrap().width = px(24);
        app.world_mut().get_mut::<Node>(rebuilt).unwrap().height = px(24);
        app.update();
        assert_eq!(size(&app, rebuilt), UVec2::splat(48));
        app.world_mut().resource_mut::<UiScale>().0 = 1.5;
        app.update();
        assert_eq!(size(&app, rebuilt), UVec2::splat(72));
        assert_eq!(size(&app, high_dpi), UVec2::splat(48));
    }
}
