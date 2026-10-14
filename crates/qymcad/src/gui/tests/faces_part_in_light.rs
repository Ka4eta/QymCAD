//! A FORM READS BY ITS LIGHT: neighbouring faces of a part come out in different tones.
//!
//! Reported behaviour: "add a second light, so the 3D models and their structure look more solid." One light taken
//! by its absolute value lit a face and the face opposite it alike, and faces at one angle to it alike: on a block
//! with a chamfer round the top the chamfer on the right came out within 2 levels of the top it leads into, and the
//! edge between them was not there to see.
use crate::gui::App;

fn rect() -> egui::Rect {
    egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(700.0, 500.0))
}

/// A block 40 x 30 x 12, extruded from a sketch, with a chamfer of 3 round every edge.
fn chamfered_block() -> App {
    let mut app = App::default();
    let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
    app.project.add_rect_entity(si, 0.0, 0.0, 40.0, 30.0, qymcad_core::feature::Purpose::Real);
    app.project.regen_sketch(si);
    app.finish_sketch_edit();
    app.chosen.sel = qymcad_ui_state::Sel::Sketch(si);
    app.start_feat_cmd(1);
    if let Some(p) = app.tools.cmd.params.iter_mut().find(|p| p.key == "height") {
        p.val = 12.0;
        p.txt = "12".into();
    }
    app.apply_feat_cmd();
    qymcad_ui_state::rebuild_if_dirty(&mut app.rebuild_ctx());
    let body = app.project.timeline.iter().rev().find_map(|n| n.kind.body()).expect("setup: the block was not built");
    let edges: Vec<u32> = app.project.regen_edges.get(&body).map(|es| es.iter().map(|e| e.id).collect()).unwrap_or_default();
    app.project.add_chamfer(body, 3.0, edges);
    qymcad_ui_state::rebuild_if_dirty(&mut app.rebuild_ctx());
    app.chosen.sel = qymcad_ui_state::Sel::None;
    app.viewing.mode_3d = true;
    app.viewing.cam.init = true;
    app.viewing.cam.target = [20.0, 15.0, 6.0];
    app.viewing.cam.scale = 7.0;
    app
}

/// A face as the picture shows it: where its middle lands and the colour drawn there.
struct SeenFace {
    normal: [f64; 3],
    corners: Vec<[f64; 3]>,
    colour: egui::Color32,
}

/// The faces of the body turned to the eye and wide enough to sample, with the colour at the middle of each. Where the
/// floor alone draws something (`floor`, the same view with the block hidden) the face is not sampled: seen from below,
/// the floor lies between the eye and the block.
fn seen_faces(app: &App, shot: &Shot) -> Vec<SeenFace> {
    let Shot { img, floor } = shot;
    let basis = app.viewing.cam.basis();
    let scr = qymcad_ui_state::Screen { cam: &app.viewing.cam, set: &app.set, rect: rect(), basis: &basis };
    let mi = app.project.bodies.iter().rposition(|b| b.visible).expect("setup: no body");
    let body = &app.project.bodies[mi];
    let mut out = Vec::new();
    for f in &body.faces {
        let c = [f.centroid.x, f.centroid.y, f.centroid.z];
        if qymcad_ui_state::v_dot(f.normal, basis.2) > -0.15 || f.area < 30.0 {
            continue; // turned away, edge-on, or a corner too small to sample
        }
        let corners: Vec<[f64; 3]> = f.triangles.iter().flat_map(|&t| body.mesh.triangle(t as usize)).map(|p| [p.x, p.y, p.z]).collect();
        // the middle of the face, or a point a third of the way to one of its corners: the first the floor leaves bare
        let mid = scr.at(c).0;
        let bare = std::iter::once(mid).chain(corners.iter().map(|k| mid + (scr.at(*k).0 - mid) * 0.3)).find(|p| {
            let (x, y) = (p.x.round() as usize, p.y.round() as usize);
            x < img.size[0] && y < img.size[1] && floor.pixels[y * img.size[0] + x].a() == 0
        });
        let Some(at) = bare else { continue };
        let (x, y) = (at.x.round() as usize, at.y.round() as usize);
        out.push(SeenFace { normal: f.normal, corners, colour: img.pixels[y * img.size[0] + x] });
    }
    out
}

impl SeenFace {
    /// Whether the two faces meet along an edge: two corners of one lie on corners of the other. Two chamfers that only
    /// touch at the corner of the block, with the little face of the corner between them, are not neighbours.
    fn touches(&self, other: &SeenFace) -> bool {
        let mut shared: Vec<[f64; 3]> = Vec::new();
        for p in self.corners.iter().filter(|p| other.corners.iter().any(|q| (0..3).all(|k| (p[k] - q[k]).abs() < 1e-4))) {
            if !shared.iter().any(|q| (0..3).all(|k| (p[k] - q[k]).abs() < 1e-4)) {
                shared.push(*p);
            }
        }
        shared.len() >= 2
    }

    /// How far apart the two tones are, in levels of the channel that differs most.
    fn levels_from(&self, other: &SeenFace) -> i32 {
        let (a, b) = (self.colour, other.colour);
        (a.r() as i32 - b.r() as i32).abs().max((a.g() as i32 - b.g() as i32).abs()).max((a.b() as i32 - b.b() as i32).abs())
    }
}

/// A picture of the view, and the same view with the block hidden - what the floor alone draws.
struct Shot {
    img: egui::ColorImage,
    floor: egui::ColorImage,
}

/// The picture by `draw`, with and without the block.
fn shot(app: &mut App, draw: impl Fn(&App) -> Option<egui::ColorImage>) -> Option<Shot> {
    let img = draw(app)?;
    for b in &mut app.project.bodies {
        b.visible = false;
    }
    let floor = draw(app);
    for b in &mut app.project.bodies {
        b.visible = true;
    }
    Some(Shot { img, floor: floor? })
}

/// Every two faces that meet, seen from where the camera stands now: how far apart their tones are.
fn pairs_too_close(app: &App, shot: &Shot, view: &str, wrong: &mut Vec<String>) {
    let faces = seen_faces(app, shot);
    assert!(faces.len() >= 5, "{view}: setup: {} faces of the block seen", faces.len());
    for (i, a) in faces.iter().enumerate() {
        for b in &faces[i + 1..] {
            if !a.touches(b) || qymcad_ui_state::v_dot(a.normal, b.normal) > 0.999 {
                continue;
            }
            let d = a.levels_from(b);
            if d < 8 {
                wrong.push(format!(
                    "{view}: faces {:?} and {:?} are {d} levels apart ({:?} / {:?})",
                    a.normal.map(|v| (v * 100.0).round() / 100.0),
                    b.normal.map(|v| (v * 100.0).round() / 100.0),
                    a.colour,
                    b.colour
                ));
            }
        }
    }
}

/// Where the camera stands: its name in a message, and its turn and tilt in radians.
struct View {
    name: &'static str,
    yaw: f64,
    pitch: f64,
}

/// The views a person turns a part to: the usual three-quarter one, from behind, from below.
const VIEWS: [View; 3] = [View { name: "three-quarter", yaw: -0.7, pitch: 0.6 }, View { name: "from behind", yaw: 2.3, pitch: 0.5 }, View { name: "from below", yaw: -0.7, pitch: -0.6 }];

/// NEIGHBOURING FACES OF A CHAMFERED BLOCK PART IN TONE, in the software raster, from every side.
#[test]
fn neighbouring_faces_part_in_tone_in_the_raster() {
    let mut app = chamfered_block();
    app.set.gpu_viewport = false;
    let mut wrong = Vec::new();
    for View { name: view, yaw, pitch } in VIEWS {
        app.viewing.cam.yaw = yaw;
        app.viewing.cam.pitch = pitch;
        let pictured = shot(&mut app, |app| qymcad_render::rasterize_3d(&app.painting(), rect(), &app.viewing.cam.basis(), 1.0, 1.0)).expect("the raster drew nothing");
        keep(&pictured.img, &format!("lit-raster-{view}"));
        pairs_too_close(&app, &pictured, view, &mut wrong);
    }
    assert!(wrong.is_empty(), "faces that meet merge into one tone ({}):\n{}", wrong.len(), wrong.join("\n"));
}

/// THE SAME ON THE CARD: the shader lights by the same description as the raster.
#[test]
fn neighbouring_faces_part_in_tone_on_the_card() {
    let mut app = chamfered_block();
    let mut wrong = Vec::new();
    for View { name: view, yaw, pitch } in VIEWS {
        app.viewing.cam.yaw = yaw;
        app.viewing.cam.pitch = pitch;
        let Some(pictured) = shot(&mut app, |app| crate::gui::gpu_shot::eyes::shot(&app.painting(), rect())) else {
            eprintln!("PASSED OVER: there is no graphics device here - the card cannot be asked");
            return;
        };
        keep(&pictured.img, &format!("lit-card-{view}"));
        pairs_too_close(&app, &pictured, view, &mut wrong);
    }
    assert!(wrong.is_empty(), "faces that meet merge into one tone on the card ({}):\n{}", wrong.len(), wrong.join("\n"));
}

/// The picture kept beside the numbers, for the eye.
fn keep(img: &egui::ColorImage, name: &str) {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/look");
    std::fs::create_dir_all(&dir).ok();
    if let Some(png) = crate::gui::color_image_to_png(img) {
        std::fs::write(dir.join(format!("{}.png", name.replace(' ', "-"))), png).ok();
    }
}
