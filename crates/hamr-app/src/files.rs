//! Open, save and export dialogs, and reading and writing project and export files.

use std::path::{Path, PathBuf};

use hamr_core::export::Format;
use hamr_core::merge;
use hamr_core::project::{self, EXTENSION};
use hamr_core::{Object, Scene};

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

/// Asks where to export, offering every format. The format follows the chosen extension, and is
/// STL if the user left the extension off.
pub fn pick_export(project: Option<&Path>) -> Option<PathBuf> {
    let mut d = rfd::FileDialog::new();
    for format in Format::ALL {
        d = d.add_filter(format.name(), &[format.extension()]);
    }
    let stem = project
        .and_then(|p| p.file_stem())
        .map_or("Untitled".into(), |s| s.to_string_lossy().into_owned());
    if let Some(dir) = project.and_then(Path::parent) {
        d = d.set_directory(dir);
    }
    let mut path = d.set_file_name(format!("{stem}.stl")).save_file()?;
    if format_of(&path).is_none() {
        path.set_extension(Format::Stl.extension());
    }
    Some(path)
}

fn format_of(path: &Path) -> Option<Format> {
    let ext = path.extension()?.to_string_lossy().to_lowercase();
    Format::ALL.into_iter().find(|f| f.extension() == ext)
}

/// Merges `objects` into one solid and writes it to `path` in the format its extension names.
pub fn export<'a>(
    path: &Path,
    objects: impl IntoIterator<Item = &'a Object>,
) -> Result<(), String> {
    let format = format_of(path).unwrap_or(Format::Stl);
    let fail = |e: &dyn std::fmt::Display| format!("Couldn't export {}: {e}", path.display());
    let model = merge::merge(objects).map_err(|e| fail(&e))?;
    // Never write a file a slicer would reject; this would be a bug in the merge.
    model
        .solid
        .check_closed()
        .map_err(|e| fail(&format!("the merged model is not watertight ({e:?})")))?;
    let name = path
        .file_stem()
        .map_or("model".into(), |s| s.to_string_lossy().into_owned());
    let bytes = hamr_core::export::write(&model, format, &name);
    let tmp = path.with_extension("export.tmp");
    std::fs::write(&tmp, bytes)
        .and_then(|()| std::fs::rename(&tmp, path))
        .map_err(|e| fail(&e))
}
