//! The main window: menu bar, build panel, status bar and the 3D viewport.

use std::sync::Arc;

use eframe::egui::{self, Key, Modifiers, PointerButton, Sense};
use eframe::egui_wgpu;
use glam::{Mat4, Vec2, Vec3};
use hamr_core::camera::ray_ground;
use hamr_core::grid::Grid;
use hamr_core::{Mesh, OrbitCamera, Unit};

use crate::renderer::{MeshVertex, SceneGeometry, ViewportCallback, ViewportRenderer};

/// Radians of orbit per pixel of mouse movement.
const ORBIT_SPEED: f32 = 0.008;
/// Zoom factor per pixel of vertical drag when Alt-dragging.
const DRAG_ZOOM_SPEED: f32 = 0.005;

/// A placeholder object so the viewport has something to look at until prims arrive in step 2.
struct Placeholder {
    mesh: Mesh,
    transform: Mat4,
    color: [f32; 4],
}

pub struct HamrApp {
    camera: OrbitCamera,
    grid: Grid,
    show_grid: bool,
    unit: Unit,
    objects: Vec<Placeholder>,
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

        let cube = Placeholder {
            mesh: Mesh::cuboid(Vec3::splat(50.0)),
            transform: Mat4::from_translation(Vec3::new(0.0, 0.0, 25.0)),
            color: [0.85, 0.62, 0.25, 1.0],
        };
        Self {
            camera: OrbitCamera {
                focus: Vec3::new(0.0, 0.0, 25.0),
                ..Default::default()
            },
            grid: Grid::default(),
            show_grid: true,
            unit: Unit::default(),
            objects: vec![cube],
            geometry: Arc::default(),
            dirty: true,
        }
    }

    /// Rebuilds the GPU-ready geometry when the scene or grid settings have changed.
    fn refresh_geometry(&mut self) {
        if !self.dirty {
            return;
        }
        self.dirty = false;
        let mut geometry = SceneGeometry {
            revision: self.geometry.revision + 1,
            ..Default::default()
        };
        if self.show_grid {
            geometry.lines = self.grid.lines();
        }
        for object in &self.objects {
            let base = geometry.mesh_vertices.len() as u32;
            let normal_matrix = object.transform.inverse().transpose();
            geometry
                .mesh_vertices
                .extend(object.mesh.vertices.iter().map(|v| {
                    MeshVertex {
                        position: object.transform.transform_point3(v.position.into()).into(),
                        normal: normal_matrix
                            .transform_vector3(v.normal.into())
                            .normalize()
                            .into(),
                        color: object.color,
                    }
                }));
            geometry
                .mesh_indices
                .extend(object.mesh.indices.iter().map(|i| base + i));
        }
        self.geometry = Arc::new(geometry);
    }

    /// Finds the nearest point under the cursor on an object or, failing that, the ground.
    fn pick(&self, origin: Vec3, dir: Vec3) -> Option<Vec3> {
        let object_hit = self
            .objects
            .iter()
            .filter_map(|o| {
                let inv = o.transform.inverse();
                let local_origin = inv.transform_point3(origin);
                let local_dir = inv.transform_vector3(dir);
                let scale = local_dir.length();
                let t = o.mesh.ray_hit(local_origin, local_dir / scale)? / scale;
                Some(t)
            })
            .min_by(f32::total_cmp)
            .map(|t| origin + dir * t);
        object_hit.or_else(|| ray_ground(origin, dir))
    }

    fn frame_all(&mut self) {
        let bounds = self.objects.iter().filter_map(|o| {
            let (lo, hi) = o.mesh.bounds()?;
            Some((
                o.transform.transform_point3(lo),
                o.transform.transform_point3(hi),
            ))
        });
        let Some((lo, hi)) = bounds.reduce(|(a, b), (c, d)| (a.min(c), b.max(d))) else {
            self.camera = OrbitCamera::default();
            return;
        };
        let radius = (hi - lo).length() * 0.5;
        self.camera.focus = (lo + hi) * 0.5;
        self.camera.distance = (radius / (self.camera.fov_y * 0.5).sin()).max(10.0);
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
        ui.add_enabled_ui(false, |ui| {
            ui.horizontal_wrapped(|ui| {
                for name in [
                    "Box", "Cylinder", "Prism", "Sphere", "Torus", "Tube", "Ring",
                ] {
                    let _ = ui.button(name);
                }
            });
        })
        .response
        .on_disabled_hover_text("Prims arrive in the next milestone.");
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
                ui.weak("Right-drag or Ctrl+Alt-drag: orbit · Middle-drag or Ctrl+Alt+Shift-drag: pan · Wheel: zoom · Alt-click: focus");
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

        if response.clicked() && mods.alt {
            if let Some(pos) = response.interact_pointer_pos() {
                let ndc = Vec2::new(
                    (pos.x - rect.left()) / rect.width() * 2.0 - 1.0,
                    1.0 - (pos.y - rect.top()) / rect.height() * 2.0,
                );
                let (origin, dir) = self.camera.ray(ndc, aspect);
                if let Some(point) = self.pick(origin, dir) {
                    self.camera.focus_on(point);
                }
            }
        }

        if response.hovered() || response.has_focus() {
            let (frame, home) = ui.input_mut(|i| {
                (
                    i.consume_key(Modifiers::NONE, Key::F),
                    i.consume_key(Modifiers::NONE, Key::Home),
                )
            });
            if frame {
                self.frame_all();
            }
            if home {
                self.camera = OrbitCamera::default();
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

impl eframe::App for HamrApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::Panel::top("menu").show(ui, |ui| self.menu_bar(ui));
        egui::Panel::bottom("status").show(ui, |ui| self.status_bar(ui));
        egui::Panel::left("build")
            .default_size(220.0)
            .show(ui, |ui| self.build_panel(ui));
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| self.viewport(ui));
    }
}
