//! Reading and writing project files (`.hamr`), which are JSON.

use serde::{Deserialize, Serialize};

use crate::{Object, Scene};

/// File extension of project files, without the dot.
pub const EXTENSION: &str = "hamr";
/// Bump when the format changes in a way older versions can't read, and add a migration.
const VERSION: u32 = 1;

#[derive(Serialize, Deserialize)]
struct ProjectFile {
    format: String,
    version: u32,
    objects: Vec<Object>,
}

#[derive(Debug)]
pub enum ProjectError {
    /// The file is not valid JSON or does not have the expected fields.
    Invalid(serde_json::Error),
    /// The file is valid JSON but not a Hamr project.
    NotAProject,
    /// The file was written by a newer Hamr.
    TooNew(u32),
}

impl std::fmt::Display for ProjectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProjectError::Invalid(e) => write!(f, "not a readable project file: {e}"),
            ProjectError::NotAProject => write!(f, "this file is not a Hamr project"),
            ProjectError::TooNew(v) => write!(
                f,
                "this project was saved by a newer version of Hamr (format {v}); please update Hamr"
            ),
        }
    }
}

impl std::error::Error for ProjectError {}

pub fn to_json(scene: &Scene) -> String {
    let file = ProjectFile {
        format: "hamr".into(),
        version: VERSION,
        objects: scene.objects.clone(),
    };
    serde_json::to_string_pretty(&file).expect("scene types always serialize")
}

pub fn from_json(text: &str) -> Result<Scene, ProjectError> {
    // Check the header first so a newer file gets a clear message instead of a field error.
    #[derive(Deserialize)]
    struct Header {
        format: String,
        version: u32,
    }
    let header: Header = serde_json::from_str(text).map_err(ProjectError::Invalid)?;
    if header.format != "hamr" {
        return Err(ProjectError::NotAProject);
    }
    if header.version > VERSION {
        return Err(ProjectError::TooNew(header.version));
    }
    let file: ProjectFile = serde_json::from_str(text).map_err(ProjectError::Invalid)?;
    let objects = file
        .objects
        .into_iter()
        .map(|mut o| {
            // Files may be edited by hand; never trust shape values to be in range.
            o.shape = o.shape.clamped();
            o
        })
        .collect();
    Ok(Scene::from_objects(objects))
}

#[cfg(test)]
mod tests {
    use glam::{Quat, Vec3};

    use super::*;
    use crate::PrimKind;

    #[test]
    fn round_trip_keeps_every_field() {
        let mut scene = Scene::default();
        let id = scene.add_prim(PrimKind::Torus, Vec3::new(1.0, 2.0, 3.0));
        let o = scene.get_mut(id).unwrap();
        o.shape.hollow = 0.4;
        o.shape.twist = [10.0, -90.0];
        o.rotation = Quat::from_rotation_z(0.5);
        o.negative = true;
        o.name = "Ring \"one\"".into();
        let loaded = from_json(&to_json(&scene)).unwrap();
        assert_eq!(loaded.objects, scene.objects);
    }

    #[test]
    fn newer_files_are_refused_with_a_clear_error() {
        let text = r#"{"format": "hamr", "version": 99, "objects": []}"#;
        assert!(matches!(from_json(text), Err(ProjectError::TooNew(99))));
    }

    #[test]
    fn other_json_is_not_a_project() {
        let text = r#"{"format": "something", "version": 1, "objects": []}"#;
        assert!(matches!(from_json(text), Err(ProjectError::NotAProject)));
    }
}
