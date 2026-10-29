//! Writing a merged [`Model`] as STL, OBJ or glTF.

use std::fmt::Write as _;

use glam::Vec3;

use crate::merge::Model;
use crate::prim::CREASE_DEGREES;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    /// Binary STL, in millimetres. The usual format for slicers.
    Stl,
    /// Wavefront OBJ, in millimetres, with vertex colours.
    Obj,
    /// Binary glTF (`.glb`), in metres as glTF requires, with vertex colours and normals.
    Glb,
}

impl Format {
    pub const ALL: [Format; 3] = [Format::Stl, Format::Obj, Format::Glb];

    pub fn extension(self) -> &'static str {
        match self {
            Format::Stl => "stl",
            Format::Obj => "obj",
            Format::Glb => "glb",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Format::Stl => "STL",
            Format::Obj => "OBJ",
            Format::Glb => "glTF binary",
        }
    }
}

pub fn write(model: &Model, format: Format, name: &str) -> Vec<u8> {
    match format {
        Format::Stl => stl(model, name),
        Format::Obj => obj(model, name).into_bytes(),
        Format::Glb => glb(model, name),
    }
}

fn triangle(model: &Model, tri: &[u32; 3]) -> [Vec3; 3] {
    tri.map(|i| model.solid.positions[i as usize])
}

/// Binary STL: an 80-byte header, the triangle count, then a normal and three corners per
/// triangle.
pub fn stl(model: &Model, name: &str) -> Vec<u8> {
    let tris = &model.solid.triangles;
    let mut out = Vec::with_capacity(84 + tris.len() * 50);
    let mut header = [b' '; 80];
    let text = format!("Hamr model {name}");
    // A header starting with "solid" makes some readers think the file is ASCII STL.
    let text = text.trim_start_matches("solid");
    let len = text.len().min(80);
    header[..len].copy_from_slice(&text.as_bytes()[..len]);
    out.extend_from_slice(&header);
    out.extend_from_slice(&(tris.len() as u32).to_le_bytes());
    for tri in tris {
        let [a, b, c] = triangle(model, tri);
        let normal = (b - a).cross(c - a).normalize_or_zero();
        for v in [normal, a, b, c] {
            for x in v.to_array() {
                out.extend_from_slice(&x.to_le_bytes());
            }
        }
        out.extend_from_slice(&0u16.to_le_bytes());
    }
    out
}

/// OBJ with shared vertices. Colours use the common `v x y z r g b` extension, in sRGB, taken
/// from the first corner that uses each vertex.
pub fn obj(model: &Model, name: &str) -> String {
    let solid = &model.solid;
    let mut colors = vec![[0.8f32; 3]; solid.positions.len()];
    let mut seen = vec![false; solid.positions.len()];
    for (corner, &v) in solid.triangles.iter().flatten().enumerate() {
        if !seen[v as usize] {
            seen[v as usize] = true;
            colors[v as usize] = model.corner_colors[corner].map(linear_to_srgb);
        }
    }
    let mut out = String::new();
    let _ = writeln!(out, "# Hamr model, millimetres, Z up");
    let _ = writeln!(out, "o {}", name.replace(char::is_whitespace, "_"));
    for (p, c) in solid.positions.iter().zip(&colors) {
        let _ = writeln!(
            out,
            "v {} {} {} {:.4} {:.4} {:.4}",
            p.x, p.y, p.z, c[0], c[1], c[2]
        );
    }
    for [a, b, c] in &solid.triangles {
        let _ = writeln!(out, "f {} {} {}", a + 1, b + 1, c + 1);
    }
    out
}

fn linear_to_srgb(c: f32) -> f32 {
    if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

/// Binary glTF with one mesh. glTF is Y up and in metres, so the model is turned and scaled.
/// Vertices are split per corner for shading and colour, like the viewport mesh.
pub fn glb(model: &Model, name: &str) -> Vec<u8> {
    let mesh = model.solid.to_mesh(CREASE_DEGREES);
    // Z up (mm) to Y up (m): (x, y, z) -> (x, z, -y).
    let convert = |v: [f32; 3]| [v[0], v[2], -v[1]];
    let positions: Vec<[f32; 3]> = mesh
        .vertices
        .iter()
        .map(|v| convert(v.position).map(|x| x * 0.001))
        .collect();
    let normals: Vec<[f32; 3]> = mesh.vertices.iter().map(|v| convert(v.normal)).collect();
    // `to_mesh` makes one vertex per triangle corner, in order, so corner colours line up.
    let colors = &model.corner_colors;
    let indices = &mesh.indices;

    let (min, max) = positions
        .iter()
        .fold(([f32::MAX; 3], [f32::MIN; 3]), |(lo, hi), p| {
            (
                std::array::from_fn(|i| lo[i].min(p[i])),
                std::array::from_fn(|i| hi[i].max(p[i])),
            )
        });

    let mut bin: Vec<u8> = Vec::new();
    let mut views = Vec::new();
    let mut push_view = |bytes: &[u8], target: u32| {
        while bin.len() % 4 != 0 {
            bin.push(0);
        }
        views.push(format!(
            r#"{{"buffer":0,"byteOffset":{},"byteLength":{},"target":{target}}}"#,
            bin.len(),
            bytes.len()
        ));
        bin.extend_from_slice(bytes);
    };
    push_view(bytemuck::cast_slice(&positions), 34962);
    push_view(bytemuck::cast_slice(&normals), 34962);
    push_view(bytemuck::cast_slice(colors), 34962);
    push_view(bytemuck::cast_slice(indices), 34963);
    while bin.len() % 4 != 0 {
        bin.push(0);
    }

    let count = positions.len();
    let json = format!(
        r#"{{"asset":{{"version":"2.0","generator":"Hamr"}},"scene":0,"scenes":[{{"nodes":[0]}}],"nodes":[{{"mesh":0,"name":{name}}}],"meshes":[{{"name":{name},"primitives":[{{"attributes":{{"POSITION":0,"NORMAL":1,"COLOR_0":2}},"indices":3,"material":0}}]}}],"materials":[{{"pbrMetallicRoughness":{{"baseColorFactor":[1,1,1,1],"metallicFactor":0,"roughnessFactor":0.8}}}}],"accessors":[{{"bufferView":0,"componentType":5126,"count":{count},"type":"VEC3","min":[{},{},{}],"max":[{},{},{}]}},{{"bufferView":1,"componentType":5126,"count":{count},"type":"VEC3"}},{{"bufferView":2,"componentType":5126,"count":{count},"type":"VEC3"}},{{"bufferView":3,"componentType":5125,"count":{},"type":"SCALAR"}}],"bufferViews":[{}],"buffers":[{{"byteLength":{}}}]}}"#,
        min[0],
        min[1],
        min[2],
        max[0],
        max[1],
        max[2],
        indices.len(),
        views.join(","),
        bin.len(),
        name = serde_json::to_string(name).expect("strings serialize"),
    );
    let mut json = json.into_bytes();
    while json.len() % 4 != 0 {
        json.push(b' ');
    }

    let total = 12 + 8 + json.len() + 8 + bin.len();
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(b"glTF");
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&(total as u32).to_le_bytes());
    out.extend_from_slice(&(json.len() as u32).to_le_bytes());
    out.extend_from_slice(b"JSON");
    out.extend_from_slice(&json);
    out.extend_from_slice(&(bin.len() as u32).to_le_bytes());
    out.extend_from_slice(b"BIN\0");
    out.extend_from_slice(&bin);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::{Mesh, Vertex};
    use crate::{merge, PrimKind, Scene};

    fn model() -> Model {
        let mut scene = Scene::default();
        scene.add_prim(PrimKind::Box, Vec3::ZERO);
        let id = scene.add_prim(PrimKind::Sphere, Vec3::new(30.0, 0.0, 0.0));
        scene.get_mut(id).unwrap().color = [0.2, 0.4, 1.0, 1.0];
        merge::merge(&scene.objects).unwrap()
    }

    fn f32_at(bytes: &[u8], at: usize) -> f32 {
        f32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
    }

    #[test]
    fn stl_reads_back_watertight() {
        // Reading the STL's triangles back must give a closed, outward-facing surface of the
        // same volume, which catches wrong byte layout or winding.
        let model = model();
        let bytes = stl(&model, "test");
        assert!(!bytes.starts_with(b"solid"));
        let count = u32::from_le_bytes(bytes[80..84].try_into().unwrap()) as usize;
        assert_eq!(count, model.solid.triangles.len());
        assert_eq!(bytes.len(), 84 + count * 50);
        let mut mesh = Mesh::default();
        for t in 0..count {
            let base = 84 + t * 50 + 12;
            for k in 0..3 {
                let p = std::array::from_fn(|i| f32_at(&bytes, base + k * 12 + i * 4));
                mesh.vertices.push(Vertex {
                    position: p,
                    normal: [0.0; 3],
                });
                mesh.indices.push(mesh.indices.len() as u32);
            }
        }
        mesh.check_watertight().unwrap();
        assert!((mesh.signed_volume() - model.solid.signed_volume()).abs() < 1.0);
    }

    #[test]
    fn obj_has_every_vertex_and_face() {
        let model = model();
        let text = obj(&model, "my model");
        assert!(text.contains("o my_model"));
        let v = text.lines().filter(|l| l.starts_with("v ")).count();
        let f = text.lines().filter(|l| l.starts_with("f ")).count();
        assert_eq!(v, model.solid.positions.len());
        assert_eq!(f, model.solid.triangles.len());
        // Faces are 1-based and never point past the last vertex.
        let max = text
            .lines()
            .filter_map(|l| l.strip_prefix("f "))
            .flat_map(|l| l.split(' ').map(|i| i.parse::<usize>().unwrap()))
            .max()
            .unwrap();
        assert_eq!(max, v);
    }

    #[test]
    fn glb_has_valid_chunks_and_json() {
        // The container lengths and alignment must be right, and the JSON must parse and point
        // inside the binary chunk, or viewers refuse the file.
        let model = model();
        let bytes = glb(&model, "a \"quoted\" name");
        assert_eq!(&bytes[0..4], b"glTF");
        let u32_at = |at: usize| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()) as usize;
        assert_eq!(u32_at(8), bytes.len());
        let json_len = u32_at(12);
        assert_eq!(&bytes[16..20], b"JSON");
        assert_eq!(json_len % 4, 0);
        let json: serde_json::Value = serde_json::from_slice(&bytes[20..20 + json_len]).unwrap();
        let bin_at = 20 + json_len;
        let bin_len = u32_at(bin_at);
        assert_eq!(&bytes[bin_at + 4..bin_at + 8], b"BIN\0");
        assert_eq!(bin_at + 8 + bin_len, bytes.len());
        assert_eq!(json["buffers"][0]["byteLength"], bin_len);
        for view in json["bufferViews"].as_array().unwrap() {
            let end = view["byteOffset"].as_u64().unwrap() + view["byteLength"].as_u64().unwrap();
            assert!(end as usize <= bin_len);
        }
        // Metres, Y up: the 50 mm tall box reaches 0.05 up.
        let max_y = json["accessors"][0]["max"][1].as_f64().unwrap();
        assert!((max_y - 0.05).abs() < 1e-4, "{max_y}");
        let tris = json["accessors"][3]["count"].as_u64().unwrap() as usize;
        assert_eq!(tris, model.solid.triangles.len() * 3);
    }
}
