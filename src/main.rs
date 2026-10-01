use bevy::prelude::*;
use bevy_koiro_editor::{editor::EditorPlugin, ui::BG};
fn main() {
    App::new()
        .insert_resource(ClearColor(BG))
        .add_plugins(
            DefaultPlugins
                .set(AssetPlugin {
                    file_path: std::env::current_dir()
                        .expect("project working directory")
                        .join("assets")
                        .to_string_lossy()
                        .into_owned(),
                    ..default()
                })
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Koiro · World Editor".into(),
                        resolution: (1440, 900).into(),
                        ..default()
                    }),
                    ..default()
                }),
        )
        .add_plugins(EditorPlugin)
        .run();
}
