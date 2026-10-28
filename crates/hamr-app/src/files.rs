//! Open and save dialogs, and reading and writing project files.

use std::path::{Path, PathBuf};

use hamr_core::project::{self, EXTENSION};
use hamr_core::Scene;

fn dialog() -> rfd::FileDialog {
    rfd::FileDialog::new().add_filter("Hamr project", &[EXTENSION])
}

/// Asks for a project to open. `None` if the user cancelled.
pub fn pick_open() -> Option<PathBuf> {
    dialog().pick_file()
}

/// Asks where to save, suggesting `current`. Adds the extension if the user left it off.
pub fn pick_save(current: Option<&Path>) -> Option<PathBuf> {
    let mut d = dialog();
    match current {
        Some(path) => {
            if let Some(dir) = path.parent() {
                d = d.set_directory(dir);
            }
            if let Some(name) = path.file_name() {
                d = d.set_file_name(name.to_string_lossy());
            }
        }
        None => d = d.set_file_name(format!("Untitled.{EXTENSION}")),
    }
    let mut path = d.save_file()?;
    if path.extension().is_none() {
        path.set_extension(EXTENSION);
    }
    Some(path)
}

pub fn load(path: &Path) -> Result<Scene, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("Couldn't open {}: {e}", path.display()))?;
    project::from_json(&text).map_err(|e| format!("Couldn't open {}: {e}", path.display()))
}

pub fn save(path: &Path, scene: &Scene) -> Result<(), String> {
    // Write to a temporary file first so a failed save never destroys the previous version.
    let tmp = path.with_extension(format!("{EXTENSION}.tmp"));
    std::fs::write(&tmp, project::to_json(scene))
        .and_then(|()| std::fs::rename(&tmp, path))
        .map_err(|e| format!("Couldn't save {}: {e}", path.display()))
}
