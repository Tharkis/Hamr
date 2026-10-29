//! The Custom prims section of the Build panel and the library folder behind it.

use std::path::{Path, PathBuf};

use eframe::egui::{self, Ui};
use hamr_core::library::{CustomPrim, EXTENSION};

pub enum Action {
    /// Add the custom prim at this index to the scene.
    Place(usize),
    /// Save the selection as a custom prim with this name.
    Save(String),
}

/// How a selection is saved as a custom prim.
#[derive(Clone, Copy, PartialEq, Eq)]
enum SaveAs {
    Group,
    Baked,
}

pub struct Library {
    /// Where custom prims are kept; `None` if no user data folder could be found.
    dir: Option<PathBuf>,
    items: Vec<(PathBuf, CustomPrim)>,
    /// The name being typed while saving, if the save form is open.
    naming: Option<String>,
    save_as: SaveAs,
    message: Option<String>,
}

impl Library {
    /// Reads every custom prim in the library folder. Files that can't be read are skipped.
    pub fn load() -> Self {
        let dir = library_dir();
        let mut items: Vec<(PathBuf, CustomPrim)> = dir
            .as_deref()
            .and_then(|d| std::fs::read_dir(d).ok())
            .into_iter()
            .flatten()
            .filter_map(|entry| {
                let path = entry.ok()?.path();
                if path.extension()? != EXTENSION {
                    return None;
                }
                let text = std::fs::read_to_string(&path).ok()?;
                match CustomPrim::from_json(&text) {
                    Ok(prim) => Some((path, prim)),
                    Err(e) => {
                        log::warn!("skipping {}: {e}", path.display());
                        None
                    }
                }
            })
            .collect();
        items.sort_by_key(|(_, p)| p.name.to_lowercase());
        Self {
            dir,
            items,
            naming: None,
            save_as: SaveAs::Group,
            message: None,
        }
    }

    pub fn get(&self, index: usize) -> Option<&CustomPrim> {
        self.items.get(index).map(|(_, p)| p)
    }

    /// Writes `prim` to the library folder under a file name made from its name.
    pub fn save(&mut self, prim: CustomPrim) {
        let Some(dir) = &self.dir else {
            self.message = Some("No folder to keep custom prims in was found.".into());
            return;
        };
        let result = std::fs::create_dir_all(dir)
            .and_then(|()| write_new(&free_path(dir, &prim.name), &prim.to_json()));
        match result {
            Ok(path) => {
                self.message = None;
                self.items.push((path, prim));
                self.items.sort_by_key(|(_, p)| p.name.to_lowercase());
            }
            Err(e) => self.message = Some(format!("Couldn't save the custom prim: {e}")),
        }
    }

    fn delete(&mut self, index: usize) {
        let (path, _) = &self.items[index];
        match std::fs::remove_file(path) {
            Ok(()) => {
                self.items.remove(index);
            }
            Err(e) => self.message = Some(format!("Couldn't delete {}: {e}", path.display())),
        }
    }

    pub fn show(&mut self, ui: &mut Ui, has_selection: bool) -> Option<Action> {
        let mut action = None;
        ui.label("Custom prims");
        if self.items.is_empty() {
            ui.weak("Save a selection to add it again later like a prim.");
        }
        let mut delete = None;
        ui.horizontal_wrapped(|ui| {
            for (i, (path, prim)) in self.items.iter().enumerate() {
                let parts = prim.objects.len();
                let button = ui.button(&prim.name).on_hover_text(format!(
                    "Add at the camera focus ({parts} prim{}). Right-click to delete.",
                    if parts == 1 { "" } else { "s" }
                ));
                if button.clicked() {
                    action = Some(Action::Place(i));
                }
                button.context_menu(|ui| {
                    ui.weak(path.display().to_string());
                    if ui.button("Delete from library").clicked() {
                        delete = Some(i);
                        ui.close();
                    }
                });
            }
        });
        if let Some(i) = delete {
            self.delete(i);
        }

        match &mut self.naming {
            None => {
                if ui
                    .add_enabled(has_selection, egui::Button::new("Save selection…"))
                    .clicked()
                {
                    self.naming = Some(String::new());
                }
            }
            Some(name) => {
                ui.horizontal(|ui| {
                    ui.label("Name");
                    ui.text_edit_singleline(name);
                });
                ui.radio_value(&mut self.save_as, SaveAs::Group, "Group of prims")
                    .on_hover_text("Each prim stays editable after it is added.");
                ui.add_enabled_ui(false, |ui| {
                    ui.radio_value(&mut self.save_as, SaveAs::Baked, "Baked mesh")
                        .on_disabled_hover_text(
                            "Merging prims into one solid mesh arrives with the export tools.",
                        );
                });
                let name = name.trim().to_owned();
                let mut close = false;
                ui.horizontal(|ui| {
                    let can_save = has_selection && !name.is_empty();
                    if ui
                        .add_enabled(can_save, egui::Button::new("Save"))
                        .clicked()
                    {
                        action = Some(Action::Save(name));
                        close = true;
                    }
                    close |= ui.button("Cancel").clicked();
                });
                if close {
                    self.naming = None;
                }
            }
        }
        if let Some(message) = &self.message {
            ui.colored_label(ui.visuals().error_fg_color, message);
        }
        action
    }
}

/// The per-user folder custom prims are kept in.
fn library_dir() -> Option<PathBuf> {
    let base = if cfg!(windows) {
        std::env::var_os("APPDATA").map(|d| PathBuf::from(d).join("Hamr"))
    } else {
        std::env::var_os("XDG_DATA_HOME")
            .filter(|d| !d.is_empty())
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
            .map(|d| d.join("hamr"))
    };
    base.map(|b| b.join("prims"))
}

/// A path in `dir` for a custom prim called `name` that isn't taken yet.
fn free_path(dir: &Path, name: &str) -> PathBuf {
    let stem: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || " -_".contains(c) {
                c
            } else {
                '_'
            }
        })
        .collect();
    let stem = stem.trim();
    let stem = if stem.is_empty() { "Custom prim" } else { stem };
    (1..)
        .map(|n| match n {
            1 => dir.join(format!("{stem}.{EXTENSION}")),
            n => dir.join(format!("{stem} {n}.{EXTENSION}")),
        })
        .find(|p| !p.exists())
        .expect("some numbered name is free")
}

/// Writes `text` to `path` through a temporary file, so a failed write leaves nothing half done.
fn write_new(path: &Path, text: &str) -> std::io::Result<PathBuf> {
    let tmp = path.with_extension(format!("{EXTENSION}.tmp"));
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, path)?;
    Ok(path.to_owned())
}
