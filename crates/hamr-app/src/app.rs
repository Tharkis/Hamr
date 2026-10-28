//! The main window: menu bar, build panel, object panel, status bar and the 3D viewport.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use eframe::egui::{self, Key, KeyboardShortcut, Modifiers, PointerButton, Sense};
use eframe::egui_wgpu;
use glam::{Vec2, Vec3};
use hamr_core::camera::ray_ground;
use hamr_core::gizmo::{self, Drag, Handle, Snap, Tool};
use hamr_core::grid::{Grid, LineVertex};
use hamr_core::{History, Mesh, Object, OrbitCamera, PrimKind, PrimShape, Scene, Unit};

use crate::renderer::{MeshVertex, SceneGeometry, ViewportCallback, ViewportRenderer};
use crate::{files, object_panel};

/// Radians of orbit per pixel of mouse movement.
const ORBIT_SPEED: f32 = 0.008;
/// Zoom factor per pixel of vertical drag when Alt-dragging.
const DRAG_ZOOM_SPEED: f32 = 0.005;
/// Colour of negative prims, which are drawn see-through.
const NEGATIVE_COLOR: [f32; 4] = [0.9, 0.2, 0.15, 0.35];
const SELECTION_COLOR: [f32; 4] = [1.0, 0.85, 0.2, 1.0];
/// Colour of the handle under the cursor or being dragged.
const ACTIVE_HANDLE_COLOR: [f32; 4] = [1.0, 1.0, 0.4, 1.0];
/// Rotation snaps to multiples of this many degrees.
const ANGLE_SNAP_DEGREES: f32 = 15.0;

const UNDO: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Z);
const REDO: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Y);
const REDO_SHIFT: KeyboardShortcut =
    KeyboardShortcut::new(Modifiers::COMMAND.plus(Modifiers::SHIFT), Key::Z);
const NEW: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::N);
const OPEN: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::O);
const SAVE: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::S);
const SAVE_AS: KeyboardShortcut =
    KeyboardShortcut::new(Modifiers::COMMAND.plus(Modifiers::SHIFT), Key::S);
const DUPLICATE: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::D);

/// Something that would throw away unsaved work, waiting for the user to confirm.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Discard {
    New,
    Open,
    Quit,
}

pub struct HamrApp {
    camera: OrbitCamera,
    grid: Grid,
    show_grid: bool,
    unit: Unit,
    scene: Scene,
    selected: Option<u64>,
    /// Generated meshes per object id, with the shape they were generated from.
    meshes: HashMap<u64, (PrimShape, Mesh)>,
    geometry: Arc<SceneGeometry>,
    dirty: bool,
    history: History,
    /// Where the project was last saved or opened from.
    path: Option<PathBuf>,
    /// The scene as last saved, to tell whether there are unsaved changes.
    saved: Scene,
    tool: Tool,
    snap: bool,
    stretch_both_sides: bool,
    drag: Option<Drag>,
    hovered_handle: Option<Handle>,
    /// An action waiting for "discard unsaved changes?" to be answered.
    confirm: Option<Discard>,
    /// Set once the user has agreed to quit, so the close request is let through.
    quitting: bool,
    /// The last error, shown in the status bar until the next action.
    message: Option<String>,
    title: String,
}

impl HamrApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let render_state = cc
            .wgpu_render_state
            .as_ref()
            .expect("Hamr needs the wgpu renderer");
        ViewportRenderer::install(render_state);

        Self {
            camera: OrbitCamera {
                focus: Vec3::new(0.0, 0.0, 25.0),
                ..Default::default()
            },
            grid: Grid::default(),
            show_grid: true,
            unit: Unit::default(),
            scene: Scene::default(),
            selected: None,
            meshes: HashMap::new(),
            geometry: Arc::default(),
            dirty: true,
            history: History::default(),
            path: None,
            saved: Scene::default(),
            tool: Tool::default(),
            snap: true,
            stretch_both_sides: false,
            drag: None,
            hovered_handle: None,
            confirm: None,
            quitting: false,
            message: None,
            title: String::new(),
        }
    }

    fn has_unsaved_changes(&self) -> bool {
        self.scene != self.saved
    }

    /// Replaces the whole scene, as for undo, redo, new and open.
    fn set_scene(&mut self, scene: Scene) {
        self.scene = scene;
        self.drag = None;
        if self.selected.is_some_and(|id| self.scene.get(id).is_none()) {
            self.selected = None;
        }
        self.dirty = true;
    }

    fn undo(&mut self) {
        // Edits still in progress become their own step first, so undo never loses them.
        self.history.commit(&self.scene);
        if let Some(scene) = self.history.undo() {
            self.set_scene(scene);
        }
    }

    fn redo(&mut self) {
        if self.history.is_pending(&self.scene) {
            return;
        }
        if let Some(scene) = self.history.redo() {
            self.set_scene(scene);
        }
    }

    fn new_project(&mut self) {
        self.set_scene(Scene::default());
        self.history = History::new(&self.scene);
        self.saved = self.scene.clone();
        self.path = None;
    }

    fn open_project(&mut self) {
        let Some(path) = files::pick_open() else {
            return;
        };
        match files::load(&path) {
            Ok(scene) => {
                self.set_scene(scene);
                self.history = History::new(&self.scene);
                self.saved = self.scene.clone();
                self.path = Some(path);
                self.frame_all();
            }
            Err(e) => self.message = Some(e),
        }
    }

    /// Saves to the current file, or asks for one. Returns whether the project was saved.
    fn save_project(&mut self, ask: bool) -> bool {
        let path = match (&self.path, ask) {
            (Some(path), false) => path.clone(),
            _ => match files::pick_save(self.path.as_deref()) {
                Some(path) => path,
                None => return false,
            },
        };
        match files::save(&path, &self.scene) {
            Ok(()) => {
                self.saved = self.scene.clone();
                self.path = Some(path);
                true
            }
            Err(e) => {
                self.message = Some(e);
                false
            }
        }
    }

    /// Runs `action` now, or asks first if it would throw away unsaved changes.
    fn request(&mut self, ctx: &egui::Context, action: Discard) {
        if self.has_unsaved_changes() {
            self.confirm = Some(action);
        } else {
            self.run(ctx, action);
        }
    }

    fn run(&mut self, ctx: &egui::Context, action: Discard) {
        match action {
            Discard::New => self.new_project(),
            Discard::Open => self.open_project(),
            Discard::Quit => {
                self.quitting = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }

    fn duplicate_selected(&mut self) {
        let Some(id) = self.selected else { return };
        // Offset the copy by one grid step so it doesn't hide the original.
        if let Some(copy) = self.scene.duplicate(id, Vec3::X * self.grid.spacing) {
            self.selected = Some(copy);
            self.dirty = true;
        }
    }

    fn snap(&self) -> Option<Snap> {
        self.snap.then_some(Snap {
            grid: self.grid.spacing,
            angle_degrees: ANGLE_SNAP_DEGREES,
        })
    }

    /// The object's unit-box mesh, regenerated only when its shape has changed.
    fn mesh_for<'a>(meshes: &'a mut HashMap<u64, (PrimShape, Mesh)>, object: &Object) -> &'a Mesh {
        let entry = meshes
            .entry(object.id)
            .or_insert_with(|| (object.shape.clone(), object.shape.mesh()));
        if entry.0 != object.shape {
            *entry = (object.shape.clone(), object.shape.mesh());
        }
        &entry.1
    }

    /// Rebuilds the GPU-ready geometry when the scene or grid settings have changed.
    fn refresh_geometry(&mut self) {
        if !self.dirty {
            return;
        }
        self.dirty = false;
        self.meshes
            .retain(|id, _| self.scene.objects.iter().any(|o| o.id == *id));

        let mut geometry = SceneGeometry {
            revision: self.geometry.revision + 1,
            ..Default::default()
        };
        if self.show_grid {
            geometry.lines = self.grid.lines();
        }
        // Opaque prims first, then negative ones, so the renderer can draw them in two ranges.
        for negative_pass in [false, true] {
            for object in self
                .scene
                .objects
                .iter()
                .filter(|o| o.negative == negative_pass)
            {
                let mesh = Self::mesh_for(&mut self.meshes, object);
                let transform = object.transform();
                let normal_matrix = transform.inverse().transpose();
                let color = if object.negative {
                    NEGATIVE_COLOR
                } else {
                    object.color
                };
                let base = geometry.mesh_vertices.len() as u32;
                geometry.mesh_vertices.extend(mesh.vertices.iter().map(|v| {
                    MeshVertex {
                        position: transform.transform_point3(v.position.into()).into(),
                        normal: normal_matrix
                            .transform_vector3(v.normal.into())
                            .normalize_or_zero()
                            .into(),
                        color,
                    }
                }));
                geometry
                    .mesh_indices
                    .extend(mesh.indices.iter().map(|i| base + i));
            }
            if !negative_pass {
                geometry.opaque_index_count = geometry.mesh_indices.len() as u32;
            }
        }
        self.geometry = Arc::new(geometry);
    }

    /// Finds what is under the cursor: the nearest object and the point hit, or failing that the
    /// point on the ground.
    fn pick(&mut self, origin: Vec3, dir: Vec3) -> (Option<u64>, Option<Vec3>) {
        let mut best: Option<(f32, u64)> = None;
        for object in &self.scene.objects {
            let inv = object.transform().inverse();
            let local_origin = inv.transform_point3(origin);
            let local_dir = inv.transform_vector3(dir);
            let scale = local_dir.length();
            let mesh = Self::mesh_for(&mut self.meshes, object);
            if let Some(t) = mesh.ray_hit(local_origin, local_dir / scale) {
                let t = t / scale;
                if best.is_none_or(|(b, _)| t < b) {
                    best = Some((t, object.id));
                }
            }
        }
        match best {
            Some((t, id)) => (Some(id), Some(origin + dir * t)),
            None => (None, ray_ground(origin, dir)),
        }
    }

    fn frame_all(&mut self) {
        let corners = self.scene.objects.iter().flat_map(|o| {
            let m = o.transform();
            unit_box_corners().map(move |c| m.transform_point3(c))
        });
        let Some((lo, hi)) = corners.fold(None, |acc: Option<(Vec3, Vec3)>, p| {
            Some(acc.map_or((p, p), |(lo, hi)| (lo.min(p), hi.max(p))))
        }) else {
            self.camera = OrbitCamera::default();
            return;
        };
        let radius = (hi - lo).length() * 0.5;
        self.camera.focus = (lo + hi) * 0.5;
        self.camera.distance = (radius / (self.camera.fov_y * 0.5).sin()).max(10.0);
    }

    fn delete_selected(&mut self) {
        if let Some(id) = self.selected.take() {
            self.scene.remove(id);
            self.dirty = true;
        }
    }

    fn menu_bar(&mut self, ui: &mut egui::Ui) {
        egui::MenuBar::new().ui(ui, |ui| {
            let ctx = ui.ctx().clone();
            let shortcut = |s: &KeyboardShortcut| ctx.format_shortcut(s);
            ui.menu_button("File", |ui| {
                if ui
                    .add(egui::Button::new("New").shortcut_text(shortcut(&NEW)))
                    .clicked()
                {
                    self.request(&ctx, Discard::New);
                }
                if ui
                    .add(egui::Button::new("Open…").shortcut_text(shortcut(&OPEN)))
                    .clicked()
                {
                    self.request(&ctx, Discard::Open);
                }
                if ui
                    .add(egui::Button::new("Save").shortcut_text(shortcut(&SAVE)))
                    .clicked()
                {
                    self.save_project(false);
                }
                if ui
                    .add(egui::Button::new("Save as…").shortcut_text(shortcut(&SAVE_AS)))
                    .clicked()
                {
                    self.save_project(true);
                }
                ui.separator();
                if ui.button("Quit").clicked() {
                    self.request(&ctx, Discard::Quit);
                }
            });
            ui.menu_button("Edit", |ui| {
                let undo = egui::Button::new("Undo").shortcut_text(shortcut(&UNDO));
                let can_undo = self.history.can_undo() || self.history.is_pending(&self.scene);
                if ui.add_enabled(can_undo, undo).clicked() {
                    self.undo();
                }
                let redo = egui::Button::new("Redo").shortcut_text(shortcut(&REDO));
                if ui.add_enabled(self.history.can_redo(), redo).clicked() {
                    self.redo();
                }
                ui.separator();
                let selected = self.selected.is_some();
                let dup = egui::Button::new("Duplicate").shortcut_text(shortcut(&DUPLICATE));
                if ui.add_enabled(selected, dup).clicked() {
                    self.duplicate_selected();
                }
                let delete = egui::Button::new("Delete").shortcut_text("Delete");
                if ui.add_enabled(selected, delete).clicked() {
                    self.delete_selected();
                }
            });
            ui.menu_button("View", |ui| {
                if ui.button("Frame all (F)").clicked() {
                    self.frame_all();
                }
                if ui.button("Reset camera (Home)").clicked() {
                    self.camera = OrbitCamera::default();
                }
                if ui.checkbox(&mut self.show_grid, "Show grid").changed() {
                    self.dirty = true;
                }
            });
        });
    }

    fn build_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("Build");
        ui.separator();
        ui.label("Edit");
        ui.horizontal(|ui| {
            for (tool, name, key) in [
                (Tool::Move, "Move", "W"),
                (Tool::Rotate, "Rotate", "E"),
                (Tool::Stretch, "Stretch", "R"),
            ] {
                ui.selectable_value(&mut self.tool, tool, name)
                    .on_hover_text(format!("Shortcut: {key}"));
            }
        });
        ui.checkbox(&mut self.snap, "Snap to grid")
            .on_hover_text(format!(
                "Moves and sizes snap to the grid spacing, rotations to {ANGLE_SNAP_DEGREES}°."
            ));
        ui.checkbox(&mut self.stretch_both_sides, "Stretch both sides")
            .on_hover_text(
                "Stretch symmetrically around the centre instead of from the opposite face.",
            );
        ui.weak("Shift-drag an arrow to leave a copy behind.");
        ui.separator();
        ui.label("Create");
        ui.horizontal_wrapped(|ui| {
            for kind in PrimKind::ALL {
                if ui
                    .button(kind.name())
                    .on_hover_text("Add at the camera focus")
                    .clicked()
                {
                    let id = self.scene.add_prim(kind, self.camera.focus);
                    self.selected = Some(id);
                    self.dirty = true;
                }
            }
        });
        ui.separator();

        ui.label("Units");
        egui::ComboBox::from_id_salt("unit")
            .selected_text(self.unit.suffix())
            .show_ui(ui, |ui| {
                for unit in Unit::ALL {
                    ui.selectable_value(&mut self.unit, unit, unit.suffix());
                }
            });

        ui.label("Grid spacing");
        let mut spacing = self.unit.from_mm(self.grid.spacing);
        let drag = egui::DragValue::new(&mut spacing)
            .speed(0.1)
            .range(self.unit.from_mm(0.1)..=self.unit.from_mm(1000.0))
            .suffix(format!(" {}", self.unit.suffix()));
        if ui.add(drag).changed() {
            self.grid.spacing = self.unit.to_mm(spacing);
            self.dirty = true;
        }

        ui.separator();
        ui.label(format!("Objects ({})", self.scene.objects.len()));
        egui::ScrollArea::vertical().show(ui, |ui| {
            for object in &self.scene.objects {
                let selected = self.selected == Some(object.id);
                let label = if object.negative {
                    format!("{} (negative)", object.name)
                } else {
                    object.name.clone()
                };
                if ui.selectable_label(selected, label).clicked() {
                    self.selected = Some(object.id);
                    self.dirty = true;
                }
            }
        });
    }

    fn object_panel(&mut self, ui: &mut egui::Ui) {
        let Some(object) = self.selected.and_then(|id| self.scene.get_mut(id)) else {
            return;
        };
        match object_panel::show(ui, object, self.unit) {
            object_panel::Outcome::Unchanged => {}
            object_panel::Outcome::Changed => self.dirty = true,
            object_panel::Outcome::Delete => self.delete_selected(),
        }
    }

    fn status_bar(&self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if let Some(message) = &self.message {
                ui.colored_label(ui.visuals().error_fg_color, message);
                ui.separator();
            }
            let f = self.camera.focus;
            ui.label(format!(
                "Focus {} , {} , {}   Distance {}",
                self.unit.format(f.x),
                self.unit.format(f.y),
                self.unit.format(f.z),
                self.unit.format(self.camera.distance),
            ));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.weak("Click: select · Right-drag or Ctrl+Alt-drag: orbit · Middle-drag or Ctrl+Alt+Shift-drag: pan · Wheel: zoom · Alt-click: focus");
            });
        });
    }

    /// Lines drawn on top of the scene: the selection box and the active tool's handles.
    fn overlay(&self, eye: Vec3) -> Vec<LineVertex> {
        let Some(object) = self.selected.and_then(|id| self.scene.get(id)) else {
            return Vec::new();
        };
        let mut lines = box_outline(object);
        let active = self.drag.as_ref().map(|d| d.handle).or(self.hovered_handle);
        for h in gizmo::handles(self.tool, object, eye) {
            let color = if Some(h.handle) == active {
                ACTIVE_HANDLE_COLOR
            } else {
                match h.handle {
                    Handle::Axis(a) | Handle::Ring(a) | Handle::Face { axis: a, .. } => {
                        gizmo::axis_color(a)
                    }
                }
            };
            for (a, b) in h.segments {
                lines.push(LineVertex {
                    position: a.into(),
                    color,
                });
                lines.push(LineVertex {
                    position: b.into(),
                    color,
                });
            }
        }
        lines
    }

    fn viewport(&mut self, ui: &mut egui::Ui) {
        let (rect, response) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
        let ppp = ui.pixels_per_point();
        let size_px = [
            (rect.width() * ppp).round() as u32,
            (rect.height() * ppp).round() as u32,
        ];
        let aspect = rect.width() / rect.height().max(1.0);

        let (mods, scroll, delta) =
            ui.input(|i| (i.modifiers, i.smooth_scroll_delta.y, i.pointer.delta()));
        let delta = Vec2::new(delta.x, delta.y);

        // Second Life style: Alt-drag orbits horizontally and zooms vertically, Ctrl+Alt-drag
        // orbits freely and Ctrl+Alt+Shift-drag pans. Mouse-button shortcuts work too.
        let left = response.dragged_by(PointerButton::Primary);
        if response.dragged_by(PointerButton::Middle)
            || (left && mods.alt && mods.ctrl && mods.shift)
        {
            self.camera.pan(delta * ppp, rect.height() * ppp);
        } else if response.dragged_by(PointerButton::Secondary) || (left && mods.alt && mods.ctrl) {
            self.camera.orbit(Vec2::new(delta.x, delta.y) * ORBIT_SPEED);
        } else if left && mods.alt {
            self.camera.orbit(Vec2::new(delta.x * ORBIT_SPEED, 0.0));
            self.camera.zoom((delta.y * DRAG_ZOOM_SPEED).exp());
        }
        if response.hovered() && scroll != 0.0 {
            self.camera.zoom((-scroll * 0.002).exp());
        }

        let view_proj = self.camera.view_projection(aspect);
        let viewport_pt = Vec2::new(rect.width(), rect.height());
        let ray_at = |pos: egui::Pos2| {
            let ndc = Vec2::new(
                (pos.x - rect.left()) / rect.width() * 2.0 - 1.0,
                1.0 - (pos.y - rect.top()) / rect.height() * 2.0,
            );
            self.camera.ray(ndc, aspect)
        };

        // Handles of the selected object, hit-tested in points relative to the viewport.
        let eye = self.camera.eye();
        let handles = self
            .selected
            .and_then(|id| self.scene.get(id))
            .map(|o| gizmo::handles(self.tool, o, eye))
            .unwrap_or_default();
        self.hovered_handle = match (self.drag.as_ref(), response.hover_pos()) {
            (Some(_), _) => None,
            (None, Some(pos)) if !mods.alt => gizmo::hit_test(
                &handles,
                view_proj,
                viewport_pt,
                Vec2::new(pos.x - rect.left(), pos.y - rect.top()),
            ),
            _ => None,
        };

        if response.drag_started_by(PointerButton::Primary) && !mods.alt {
            if let (Some(handle), Some(id), Some(pos)) = (
                self.hovered_handle,
                self.selected,
                response.interact_pointer_pos(),
            ) {
                // Shift-dragging an arrow leaves a copy where the object was, as in Second Life.
                if mods.shift && matches!(handle, Handle::Axis(_)) {
                    self.scene.duplicate(id, Vec3::ZERO);
                    self.dirty = true;
                }
                let (origin, dir) = ray_at(pos);
                self.drag = self
                    .scene
                    .get(id)
                    .and_then(|o| Drag::start(handle, o, origin, dir));
            }
        }
        if let Some(drag) = &self.drag {
            if response.dragged_by(PointerButton::Primary) {
                if let (Some(pos), Some(id)) = (response.interact_pointer_pos(), self.selected) {
                    let (origin, dir) = ray_at(pos);
                    let updated = drag.update(origin, dir, self.snap(), self.stretch_both_sides);
                    if let (Some(updated), Some(object)) = (updated, self.scene.get_mut(id)) {
                        if *object != updated {
                            *object = updated;
                            self.dirty = true;
                        }
                    }
                }
            } else {
                self.drag = None;
            }
        }

        if response.clicked() {
            if let Some(pos) = response.interact_pointer_pos() {
                let (origin, dir) = ray_at(pos);
                let (object, point) = self.pick(origin, dir);
                if mods.alt {
                    if let Some(point) = point {
                        self.camera.focus_on(point);
                    }
                } else {
                    self.selected = object;
                    self.dirty = true;
                }
            }
        }

        if response.hovered() || response.has_focus() {
            let (frame, home, delete) = ui.input_mut(|i| {
                (
                    i.consume_key(Modifiers::NONE, Key::F),
                    i.consume_key(Modifiers::NONE, Key::Home),
                    i.consume_key(Modifiers::NONE, Key::Delete),
                )
            });
            if frame {
                self.frame_all();
            }
            if home {
                self.camera = OrbitCamera::default();
            }
            if delete {
                self.delete_selected();
            }
        }

        self.refresh_geometry();
        let overlay = self.overlay(eye);
        ui.painter().add(egui_wgpu::Callback::new_paint_callback(
            rect,
            ViewportCallback {
                view_proj,
                overlay,
                grid_fade_center: self.camera.focus,
                grid_fade_radius: self.camera.distance * 2.0,
                size_px,
                geometry: self.geometry.clone(),
            },
        ));
    }
}

/// The eight corners of the unit box that every prim is built in.
fn unit_box_corners() -> [Vec3; 8] {
    std::array::from_fn(|i| {
        Vec3::new(
            if i & 1 == 0 { -0.5 } else { 0.5 },
            if i & 2 == 0 { -0.5 } else { 0.5 },
            if i & 4 == 0 { -0.5 } else { 0.5 },
        )
    })
}

/// The twelve edges of an object's bounding box, for the selection outline.
fn box_outline(object: &Object) -> Vec<LineVertex> {
    let m = object.transform();
    let corners = unit_box_corners().map(|c| m.transform_point3(c));
    let mut out = Vec::with_capacity(24);
    for a in 0..8usize {
        for bit in [1, 2, 4] {
            let b = a | bit;
            if b != a {
                for i in [a, b] {
                    out.push(LineVertex {
                        position: corners[i].into(),
                        color: SELECTION_COLOR,
                    });
                }
            }
        }
    }
    out
}

impl HamrApp {
    /// Keyboard shortcuts that work anywhere except while typing in a text field.
    fn shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        // Check the more specific shortcuts first, since consuming Ctrl+Z would also match
        // Ctrl+Shift+Z.
        let pressed = ctx.input_mut(|i| {
            [REDO_SHIFT, REDO, UNDO, SAVE_AS, SAVE, OPEN, NEW, DUPLICATE]
                .into_iter()
                .find(|s| i.consume_shortcut(s))
        });
        match pressed {
            Some(s) if s == REDO || s == REDO_SHIFT => self.redo(),
            Some(s) if s == UNDO => self.undo(),
            Some(s) if s == SAVE_AS => {
                self.save_project(true);
            }
            Some(s) if s == SAVE => {
                self.save_project(false);
            }
            Some(s) if s == OPEN => self.request(ctx, Discard::Open),
            Some(s) if s == NEW => self.request(ctx, Discard::New),
            Some(s) if s == DUPLICATE => self.duplicate_selected(),
            _ => {}
        }
        if self.drag.is_none() {
            let tool = ctx.input_mut(|i| {
                [
                    (Key::W, Tool::Move),
                    (Key::E, Tool::Rotate),
                    (Key::R, Tool::Stretch),
                ]
                .into_iter()
                .find(|(key, _)| i.consume_key(Modifiers::NONE, *key))
            });
            if let Some((_, tool)) = tool {
                self.tool = tool;
            }
        }
    }

    /// Asks before throwing away unsaved changes.
    fn confirm_dialog(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        let Some(action) = self.confirm else { return };
        let mut answer = None;
        egui::Modal::new(egui::Id::unique("confirm discard")).show(ui.ctx(), |ui| {
            ui.heading("Unsaved changes");
            ui.label("Save your changes to this project first?");
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("Save").clicked() {
                    answer = Some(true);
                }
                if ui.button("Don't save").clicked() {
                    answer = Some(false);
                }
                if ui.button("Cancel").clicked() || ui.input(|i| i.key_pressed(Key::Escape)) {
                    self.confirm = None;
                }
            });
        });
        if let Some(save) = answer {
            self.confirm = None;
            if !save || self.save_project(false) {
                self.run(ctx, action);
            }
        }
    }

    fn update_title(&mut self, ctx: &egui::Context) {
        let name = self
            .path
            .as_ref()
            .and_then(|p| p.file_name())
            .map_or("Untitled".into(), |n| n.to_string_lossy().into_owned());
        let star = if self.has_unsaved_changes() { "*" } else { "" };
        let title = format!("{name}{star} - Hamr");
        if title != self.title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.title = title;
        }
    }
}

impl eframe::App for HamrApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if ctx.input(|i| i.viewport().close_requested())
            && !self.quitting
            && self.has_unsaved_changes()
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.confirm = Some(Discard::Quit);
        }
        if self.confirm.is_none() {
            self.shortcuts(&ctx);
        }
        egui::Panel::top("menu").show(ui, |ui| self.menu_bar(ui));
        egui::Panel::bottom("status").show(ui, |ui| self.status_bar(ui));
        egui::Panel::left("build")
            .default_size(220.0)
            .show(ui, |ui| self.build_panel(ui));
        if self.selected.is_some() {
            egui::Panel::right("object")
                .default_size(280.0)
                .show(ui, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| self.object_panel(ui));
                });
        }
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| self.viewport(ui));
        self.confirm_dialog(&ctx, ui);

        // Gather edits into one undo step once the user lets go: after a drag, or when they
        // leave a text field or slider.
        let busy = self.drag.is_some()
            || ctx.input(|i| i.pointer.any_down())
            || ctx.memory(|m| m.focused().is_some());
        if !busy && self.history.commit(&self.scene) {
            self.message = None;
        }
        self.update_title(&ctx);
    }
}
