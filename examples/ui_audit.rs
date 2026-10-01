//! Native layout evidence for `cutty` screenshots (no browser/DOM approximation).
//! Run `cargo run --example ui_audit`; snapshots go to the OS temp directory.
use bevy::{prelude::*, text::EditableText, ui::UiGlobalTransform};
use bevy_koiro_editor::{editor::*, ui::BG};
use serde_json::json;

fn main() {
    App::new()
        .insert_resource(ClearColor(BG))
        .add_plugins(
            DefaultPlugins
                .set(AssetPlugin {
                    file_path: std::env::current_dir()
                        .unwrap()
                        .join("assets")
                        .to_string_lossy()
                        .into_owned(),
                    ..default()
                })
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Koiro · UI Audit".into(),
                        resolution: (1440, 900).into(),
                        ..default()
                    }),
                    ..default()
                }),
        )
        .add_plugins(EditorPlugin)
        .add_systems(Last, snapshot)
        .run();
}

fn snapshot(world: &mut World, mut next: Local<f32>) {
    let elapsed = world.resource::<Time>().elapsed_secs();
    if elapsed < *next {
        return;
    }
    *next = elapsed + 1.;
    let windows: Vec<_> = world.query::<(Entity, &Window, &EditorWindow)>().iter(world).map(|(e, w, ws)| {
        json!({"id": e.to_bits(), "width": w.width(), "height": w.height(), "scale": w.scale_factor(),
            "layout": ws.document.active, "focused": ws.focused, "maximized": ws.maximized,
            "menu": ws.menu.is_some(), "status": ws.status})
    }).collect();
    let nodes: Vec<_> = world.query::<(Entity, &ComputedNode, &UiGlobalTransform)>().iter(world).filter(|(e, n, _)| !n.is_empty() && world.get::<InheritedVisibility>(*e).is_none_or(|v| v.get())).map(|(e, n, t)| {
        let size = n.size() * n.inverse_scale_factor();
        let center = t.translation * n.inverse_scale_factor();
        let style = world.get::<Node>(e);
        let color = |c: Color| { let c = c.to_srgba(); [c.red, c.green, c.blue, c.alpha] };
        let pixels = |v: Val| if let Val::Px(p) = v { Some(p) } else { None };
        let padding = n.padding * n.inverse_scale_factor();
        json!({"id": e.to_bits(), "parent": world.get::<ChildOf>(e).map(|p| p.parent().to_bits()),
            "rect": [center.x-size.x/2., center.y-size.y/2., size.x, size.y],
            "content": (n.content_size() * n.inverse_scale_factor()).to_array(),
            "padding": [padding.min_inset.x, padding.min_inset.y, padding.max_inset.x, padding.max_inset.y],
            "absolute": style.is_some_and(|s| s.position_type == PositionType::Absolute),
            "flow": style.map(|s| format!("{:?}", s.flex_direction)),
            "gap_px": style.map(|s| [pixels(s.column_gap), pixels(s.row_gap)]),
            "margin_px": style.map(|s| [pixels(s.margin.left), pixels(s.margin.top), pixels(s.margin.right), pixels(s.margin.bottom)]),
            "scroll": world.get::<ScrollPosition>(e).map(|p| [p.x,p.y]),
            "scroll_x": style.is_some_and(|s| s.overflow.x == OverflowAxis::Scroll),
            "scroll_y": style.is_some_and(|s| s.overflow.y == OverflowAxis::Scroll),
            "clip_x": style.is_some_and(|s| s.overflow.x != OverflowAxis::Visible),
            "clip_y": style.is_some_and(|s| s.overflow.y != OverflowAxis::Visible),
            "text": world.get::<Text>(e).map(|t| t.0.as_str()),
            "input": world.get::<EditableText>(e).map(|t| t.value().to_string()),
            "ink": world.get::<TextColor>(e).map(|c| color(c.0)),
            "background": world.get::<BackgroundColor>(e).map(|c| color(c.0)),
            "interactive": world.get::<UiAction>(e).is_some() || world.get::<InputBinding>(e).is_some(),
            "pane": world.get::<DockRegion>(e).map(|r| r.id),
            "splitter": world.get::<DockRegion>(e).is_some_and(|r| r.splitter),
            "z": world.get::<GlobalZIndex>(e).map_or(0, |z| z.0),
            "popup": world.get::<GlobalZIndex>(e).is_some_and(|z| z.0 == 100)
        })
    }).collect();
    let output = json!({"windows": windows, "nodes": nodes});
    let path = std::env::temp_dir().join("koiro-ui-audit.json");
    let temporary = path.with_extension("json.tmp");
    if std::fs::write(&temporary, serde_json::to_vec_pretty(&output).unwrap()).is_ok() {
        let _ = std::fs::rename(temporary, path);
    }
}
