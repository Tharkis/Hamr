//! Undo and redo, by keeping snapshots of the whole scene.
//!
//! Scenes are small (a few hundred prims at most), so whole snapshots are simpler and safer than
//! recording individual operations.

use crate::Scene;

/// How many steps can be undone.
const LIMIT: usize = 200;

#[derive(Debug, Default)]
pub struct History {
    /// The scene as of the last commit; edits are measured against it.
    baseline: Scene,
    undo: Vec<Scene>,
    redo: Vec<Scene>,
}

impl History {
    pub fn new(scene: &Scene) -> Self {
        Self {
            baseline: scene.clone(),
            undo: Vec::new(),
            redo: Vec::new(),
        }
    }

    /// Records `scene` as one undoable step if it differs from the last commit. Call it when an
    /// edit is finished (for example when a drag ends), so a whole drag undoes in one step.
    /// Returns whether a step was recorded.
    pub fn commit(&mut self, scene: &Scene) -> bool {
        if *scene == self.baseline {
            return false;
        }
        let previous = std::mem::replace(&mut self.baseline, scene.clone());
        self.undo.push(previous);
        if self.undo.len() > LIMIT {
            self.undo.remove(0);
        }
        self.redo.clear();
        true
    }

    /// Returns the scene to restore, if there is anything to undo.
    pub fn undo(&mut self) -> Option<Scene> {
        let previous = self.undo.pop()?;
        let current = std::mem::replace(&mut self.baseline, previous.clone());
        self.redo.push(current);
        Some(previous)
    }

    /// Returns the scene to restore, if there is anything to redo.
    pub fn redo(&mut self) -> Option<Scene> {
        let next = self.redo.pop()?;
        let current = std::mem::replace(&mut self.baseline, next.clone());
        self.undo.push(current);
        Some(next)
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// True when `scene` has changes that are not in the last commit.
    pub fn is_pending(&self, scene: &Scene) -> bool {
        *scene != self.baseline
    }
}

#[cfg(test)]
mod tests {
    use glam::Vec3;

    use super::*;
    use crate::PrimKind;

    #[test]
    fn undo_and_redo_walk_through_commits() {
        let mut scene = Scene::default();
        let mut history = History::new(&scene);
        scene.add_prim(PrimKind::Box, Vec3::ZERO);
        assert!(history.commit(&scene));
        let one = scene.clone();
        scene.add_prim(PrimKind::Sphere, Vec3::ZERO);
        history.commit(&scene);
        let two = scene.clone();

        assert_eq!(history.undo(), Some(one.clone()));
        assert_eq!(history.undo(), Some(Scene::default()));
        assert_eq!(history.undo(), None);
        assert_eq!(history.redo(), Some(one));
        assert_eq!(history.redo(), Some(two));
        assert_eq!(history.redo(), None);
    }

    #[test]
    fn unchanged_scene_records_nothing() {
        let scene = Scene::default();
        let mut history = History::new(&scene);
        assert!(!history.commit(&scene));
        assert!(!history.can_undo());
    }

    #[test]
    fn new_edit_after_undo_drops_redo() {
        let mut scene = Scene::default();
        let mut history = History::new(&scene);
        scene.add_prim(PrimKind::Box, Vec3::ZERO);
        history.commit(&scene);
        scene = history.undo().unwrap();
        scene.add_prim(PrimKind::Torus, Vec3::ZERO);
        history.commit(&scene);
        assert!(!history.can_redo());
    }
}
