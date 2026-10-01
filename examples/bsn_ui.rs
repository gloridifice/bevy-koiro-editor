//! Small API probe: Bevy 0.20 BSN composition and native headless button.
//! Run: cargo run --example bsn_ui
use bevy::{prelude::*, ui_widgets::Button};
fn button(label: &str) -> impl Scene {
    bsn! {
        Button
        Node { padding: px(12), border_radius: BorderRadius::all(px(4)), }
        BackgroundColor(Color::srgb(0.22, 0.22, 0.22))
        Children [ Text(label) TextFont { font_size: px(13), } ]
    }
}
fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .run();
}
fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);
    commands.spawn_scene(bsn! {
        Node { width: percent(100), height: percent(100), align_items: AlignItems::Center, justify_content: JustifyContent::Center, }
        Children [ @button("Native BSN editor control") on(|_: On<PointerClick>| info!("clicked")) ]
    });
}
