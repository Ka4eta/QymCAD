//! THE FLOOR AND THE AXES REACH THE HORIZON, in any position of the camera.
//!
//! Reported behaviour: "in the 3D view the table, the grid and the axes are not endless - one sees the plane end as the
//! view moves". The grid was a square of lines round the middle of the view: tilted 10 deg over the floor and taken out
//! to 20 m, it stopped 87 px under the horizon in perspective, and in orthographic its far edge crossed the frame with
//! the background past it.
use crate::gui::App;

/// The frame of the view, in points.
const SIZE: [usize; 2] = [900, 700];

/// HOW FAR UNDER WHERE THE FLOOR BEGINS ITS FIRST LINE MAY LIE: a level of the grid shows once its lines are 18 points
/// apart and gives way to the next one at 9, so the lines nearest the horizon can stand up to 45 points apart. The grid
/// that ended stopped 117 to 276 rows short.
const SPACED: usize = 48;

/// A block of 50 standing at the origin, the camera 10 deg over the floor and 20 m across.
fn far_low_view(projection: qymcad_ui_state::Projection) -> App {
    let mut app = App::default();
    let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
    app.project.add_rect_entity(si, 0.0, 0.0, 50.0, 50.0, qymcad_core::feature::Purpose::Real);
    app.project.regen_sketch(si);
    app.finish_sketch_edit();
    app.chosen.sel = qymcad_ui_state::Sel::Sketch(si);
    app.start_feat_cmd(1);
    app.apply_feat_cmd();
    qymcad_ui_state::rebuild_if_dirty(&mut app.rebuild_ctx());
    app.chosen.sel = qymcad_ui_state::Sel::None;
    app.set.projection = projection;
    app.viewing.mode_3d = true;
    app.viewing.cam.init = true;
    app.viewing.cam.target = [0.0; 3];
    app.viewing.cam.yaw = -0.7;
    app.viewing.cam.pitch = 10f64.to_radians();
    app.viewing.cam.scale = SIZE[0] as f32 / 20_000.0;
    app
}

/// The view as the window draws it - the bodies, the floor and whatever lies over them - taken without a screen.
fn picture(app: &mut App) -> egui::ColorImage {
    let bg = app.scheme.pal.viewport_bg();
    crate::gui::help_raster::shot_ui(SIZE, bg, |ui| app.viewport(ui))
}

/// The row of the horizon in perspective: where a ray of sight runs level with the floor.
fn horizon_row(app: &App) -> f32 {
    let rect = app.viewing.view_rect;
    let (_, up, fwd) = app.viewing.cam.basis();
    let d_eye = 1.0 / qymcad_ui_state::persp_inv_d_eye(&app.viewing.cam, &app.set, rect.height() * 0.5);
    rect.center().y - (-fwd[2] / up[2] * d_eye) as f32 * app.viewing.cam.scale
}

/// Whether a pixel shows something over the background: a line of the grid, an axis, the block.
fn drawn(img: &egui::ColorImage, bg: egui::Color32, at: egui::Pos2) -> bool {
    let p = img.pixels[at.y as usize * img.size[0] + at.x as usize];
    (p.r() as i32 - bg.r() as i32).abs().max((p.g() as i32 - bg.g() as i32).abs()).max((p.b() as i32 - bg.b() as i32).abs()) >= 4
}

/// The highest row of a band of columns that shows the floor, from `top` down.
fn floor_top(img: &egui::ColorImage, bg: egui::Color32, columns: std::ops::Range<usize>, top: usize) -> Option<usize> {
    (top..img.size[1]).find(|&y| columns.clone().any(|x| drawn(img, bg, egui::pos2(x as f32, y as f32))))
}

/// Where the floor begins on screen, and how many rows under it the grid must show.
struct Reach {
    from_row: usize,
    rows: usize,
}

/// Every band of 60 columns shows the floor within `reach`.
fn bands_short_of(img: &egui::ColorImage, bg: egui::Color32, reach: Reach) -> Vec<String> {
    let Reach { from_row, rows: reach } = reach;
    (0..img.size[0] / 60)
        .filter_map(|b| {
            let top = floor_top(img, bg, b * 60..(b + 1) * 60, from_row);
            match top {
                Some(y) if y <= from_row + reach => None,
                Some(y) => Some(format!("columns {}..{}: the floor begins {} rows down", b * 60, (b + 1) * 60, y - from_row)),
                None => Some(format!("columns {}..{}: no floor at all", b * 60, (b + 1) * 60)),
            }
        })
        .collect()
}

fn keep(img: &egui::ColorImage, name: &str) {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/look");
    std::fs::create_dir_all(&dir).ok();
    if let Some(png) = crate::gui::color_image_to_png(img) {
        std::fs::write(dir.join(format!("{name}.png")), png).ok();
    }
}

/// IN PERSPECTIVE THE FLOOR GOES TO THE HORIZON: in every band of columns the grid shows within a line's spacing of it.
#[test]
fn in_perspective_the_floor_goes_to_the_horizon() {
    let mut app = far_low_view(qymcad_ui_state::Projection::Perspective);
    app.set.gpu_viewport = false;
    let img = picture(&mut app);
    keep(&img, "floor-perspective-raster");
    let bg = app.scheme.pal.viewport_bg();
    let horizon = horizon_row(&app).round().max(0.0) as usize;
    assert!(horizon > 40 && horizon < SIZE[1] / 2, "setup: the horizon stands at row {horizon}");
    let short = bands_short_of(&img, bg, Reach { from_row: horizon, rows: SPACED });
    assert!(short.is_empty(), "the floor stops short of the horizon at row {horizon}:\n{}", short.join("\n"));
}

/// IN ORTHOGRAPHIC THE FLOOR FILLS THE FRAME: it has no horizon, and the grid shows within a line's spacing of the top in
/// every band of columns.
#[test]
fn in_orthographic_the_floor_fills_the_frame() {
    let mut app = far_low_view(qymcad_ui_state::Projection::Ortho);
    app.set.gpu_viewport = false;
    let img = picture(&mut app);
    keep(&img, "floor-ortho-raster");
    let bg = app.scheme.pal.viewport_bg();
    let short = bands_short_of(&img, bg, Reach { from_row: 0, rows: SPACED });
    assert!(short.is_empty(), "the floor ends inside the frame:\n{}", short.join("\n"));
}

/// THE CARD DRAWS THE SAME FLOOR: the picture taken off it, under the same oracle, in both projections.
#[test]
fn the_card_draws_the_floor_to_the_horizon() {
    let mut wrong = Vec::new();
    for projection in [qymcad_ui_state::Projection::Perspective, qymcad_ui_state::Projection::Ortho] {
        let mut app = far_low_view(projection);
        let _ = picture(&mut app); // the window lays the canvas out
        let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), app.viewing.view_rect.size());
        let Some(card) = crate::gui::gpu_shot::eyes::shot(&app.painting(), rect) else {
            eprintln!("PASSED OVER: there is no graphics device here - the card cannot be asked");
            return;
        };
        keep(&card, &format!("floor-{projection:?}-card"));
        // the card's picture is transparent where nothing is drawn
        let img = egui::ColorImage { pixels: card.pixels.iter().map(|p| if p.a() >= 4 { egui::Color32::WHITE } else { egui::Color32::BLACK }).collect(), ..card };
        let from = if projection == qymcad_ui_state::Projection::Perspective { horizon_row(&app).round().max(0.0) as usize } else { 0 };
        for line in bands_short_of(&img, egui::Color32::BLACK, Reach { from_row: from, rows: SPACED }) {
            wrong.push(format!("{projection:?}: {line}"));
        }
    }
    assert!(wrong.is_empty(), "the card's floor ends inside the frame:\n{}", wrong.join("\n"));
}
