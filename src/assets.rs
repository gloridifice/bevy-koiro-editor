//! Read-only local asset index, bounded so a hostile directory cannot stall startup.
use bevy::prelude::*;
use std::{
    fs,
    path::{Path, PathBuf},
};
#[derive(Clone, PartialEq)]
pub struct AssetEntry {
    pub path: PathBuf,
    pub name: String,
    pub depth: usize,
    pub directory: bool,
    pub image: Option<Handle<Image>>,
}
#[derive(Resource, Default)]
pub struct AssetIndex {
    pub entries: Vec<AssetEntry>,
    pub message: String,
}
impl AssetIndex {
    pub fn scan(server: &AssetServer) -> Self {
        let mut index = Self::default();
        if !Path::new("assets").is_dir() {
            index.message = "No assets/ directory. Built-in library is available.".into();
            return index;
        }
        index.visit(Path::new("assets"), 0, server);
        if index.message.is_empty() {
            index.message = format!("{} local entries · read only", index.entries.len());
        }
        index
    }
    fn visit(&mut self, path: &Path, depth: usize, server: &AssetServer) {
        if depth > 12 || self.entries.len() >= 2000 {
            self.message = "Index limited to 2,000 entries / 12 levels".into();
            return;
        }
        let Ok(read) = fs::read_dir(path) else {
            self.message = format!("Cannot read {}", path.display());
            return;
        };
        let mut children: Vec<_> = read.filter_map(Result::ok).collect();
        children.sort_by_key(|e| {
            (
                !e.file_type().is_ok_and(|t| t.is_dir()),
                e.file_name().to_string_lossy().to_lowercase(),
            )
        });
        for child in children {
            if self.entries.len() >= 2000 {
                self.message = "Index limited to 2,000 entries".into();
                break;
            }
            let Ok(kind) = child.file_type() else {
                self.message = "Some entries could not be inspected".into();
                continue;
            };
            if kind.is_symlink() || !(kind.is_file() || kind.is_dir()) {
                continue;
            }
            let path = child.path();
            let relative = path.strip_prefix("assets").unwrap_or(&path).to_owned();
            let extension = path
                .extension()
                .unwrap_or_default()
                .to_string_lossy()
                .to_lowercase();
            let image = if kind.is_file() && matches!(extension.as_str(), "png" | "jpg" | "jpeg") {
                Some(server.load(relative.to_string_lossy().replace('\\', "/")))
            } else {
                None
            };
            self.entries.push(AssetEntry {
                path: relative,
                name: child.file_name().to_string_lossy().into(),
                depth,
                directory: kind.is_dir(),
                image,
            });
            if kind.is_dir() {
                self.visit(&path, depth + 1, server);
            }
        }
    }
}
