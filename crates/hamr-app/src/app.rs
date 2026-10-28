//! The main window: menu bar, build panel, object panel, status bar and the 3D viewport.

use std::collections::HashMap;
use std::sync::Arc;

use eframe::egui::{self, Key, Modifiers, PointerButton, Sense};
use eframe::egui_wgpu;
use glam::{Vec2, Vec3};
use hamr_core::camera::ray_ground;
use hamr_core::grid::{Grid, LineVertex};
use hamr_core::{Mesh, Object, OrbitCamera, PrimKind, PrimShape, Scene, Unit};

use crate::object_panel;
use crate::renderer::{MeshVertex, SceneGeometry, ViewportCallback, ViewportRenderer};

/// Radians of orbit per pixel of mouse movement.
const ORBIT_SPEED: f32 = 0.008;
/// Zoom factor per pixel of vertical drag when Alt-dragging.
const DRAG_ZOOM_SPEED: f32 = 0.005;
/// Colour of negative prims, which are drawn see-through.
const NEGATIVE_COLOR: [f32; 4] = [0.9, 0.2, 0.15, 0.35];
const SELECTION_COLOR: [f32; 4] = [1.0, 0.85, 0.2, 1.0];

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
        }
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
        if let Some(object) = self.selected.and_then(|id| self.scene.get(id)) {
            geometry.overlay_lines = box_outline(object);
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
            ui.menu_button("File", |ui| {
                if ui.button("Quit").clicked() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
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

        if response.clicked() {
            if let Some(pos) = response.interact_pointer_pos() {
                let ndc = Vec2::new(
                    (pos.x - rect.left()) / rect.width() * 2.0 - 1.0,
                    1.0 - (pos.y - rect.top()) / rect.height() * 2.0,
                );
                let (origin, dir) = self.camera.ray(ndc, aspect);
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
        ui.painter().add(egui_wgpu::Callback::new_paint_callback(
            rect,
            ViewportCallback {
                view_proj: self.camera.view_projection(aspect),
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

impl eframe::App for HamrApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
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
    }
}
