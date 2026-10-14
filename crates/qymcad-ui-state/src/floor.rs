//! THE FLOOR OF THE 3D VIEW: the plane Z = 0 under a grid, and the world axes, to the horizon.
//!
//! Reported behaviour: "in the 3D view the table, the grid and the axes are not endless - one sees the plane end as the
//! view moves". The grid was a square of egui lines round the middle of the view, at most 160 lines each way: tilted
//! 10 deg over the floor and taken out to 20 m it stopped 117 to 151 rows under the horizon in perspective, and its far
//! edge crossed the frame in orthographic. The axes were segments of the same square, the Z axis four steps high.
//!
//! Now every pixel casts its ray of sight onto the plane. The grid is drawn where the point met lies near a line, a
//! line's width is measured in pixels from the size of a pixel on the plane there, and a level of the grid fades out as
//! its lines close in under 18 points - towards the horizon, and at a grazing look - with the next level, five times
//! coarser, taking its place. There is no last line to reach. The card does the same in `fs_floor`; this is the
//! arithmetic the software raster runs, and the description both take.

use super::*;
use egui::Color32;

/// WHAT THE FLOOR IS DRAWN WITH: the step of its finest lines and the colours, all from the scheme.
#[derive(Clone, Copy, Debug)]
pub struct FloorLook {
    /// The finest step of the grid, mm: its lines 9 to 45 points apart at the camera's target, as before.
    pub step: f64,
    /// A line of the finest step, and a line of every fifth.
    pub minor: Color32,
    pub major: Color32,
    /// The world axes: X and Y lie on the floor, Z stands up from the origin.
    pub axis_x: Color32,
    pub axis_y: Color32,
    pub axis_z: Color32,
}

/// The widths of the lines, in points, as the egui strokes drew them.
pub const FLOOR_LINE: f64 = 1.0;
pub const FLOOR_AXIS: f64 = 1.6;
pub const FLOOR_AXIS_Z: f64 = 1.8;
/// A level of the grid is gone at 9 points between its lines and whole at 18: the fade the minor lines had at the
/// switch of the step.
pub const FLOOR_FADE_GONE: f64 = 9.0;
pub const FLOOR_FADE_WHOLE: f64 = 18.0;
/// How many levels of the grid, each five times coarser: six reach from the finest step to 3125 of them.
pub const FLOOR_LEVELS: usize = 6;
/// The grid fades out as the ray of sight comes within this sine of level (0.7 deg): there a pixel spans more of the
/// plane than any level can show.
pub const FLOOR_GRAZING: f64 = 0.012;
/// THE FLOOR STANDS A HAIR BEHIND WHAT LIES ON IT: its depth is taken as if it were this much further along the ray,
/// and further still by as much as its depth changes over `FLOOR_SLOPE` pixels. The bottom of a part standing on the
/// floor is the floor's own plane, and seen from below the grid came through the face - 6 levels of it on the face of a
/// chamfered block. A hundred-thousandth of the distance was not enough: the card tests the face's depth at every sample
/// of a pixel and the floor's once at its middle, and across half a pixel of a face seen aslant the depth changes by
/// 0.2 %; the raster's interpolation is good to about a hundred-thousandth. A ten-thousandth plus a pixel's slope covers
/// both and is far below anything a person could lift a part by.
pub const FLOOR_BEHIND: f64 = 1.0 + 1e-4;
pub const FLOOR_SLOPE: f64 = 1.0;

/// The floor for the camera and the scheme of `pn`.
pub fn floor_look(pn: &Painting) -> FloorLook {
    let sc = pn.cam.scale as f64;
    let mut step = 10.0_f64;
    while step * sc < 9.0 {
        step *= 5.0;
    }
    let pal = &pn.scheme.pal;
    FloorLook { step, minor: qymcad_scheme::a(pal.grid_minor(), 170), major: qymcad_scheme::a(pal.grid(), 230), axis_x: pal.grid_axis_x(), axis_y: pal.grid_axis_y(), axis_z: pal.grid_axis_z() }
}

/// A ray of sight: where it starts and where it runs. In perspective it starts at the eye and runs one unit along the
/// line of sight per unit of `along`'s length on it, so the distance it has gone is the depth.
#[derive(Clone, Copy, Debug)]
pub struct Ray {
    pub from: [f64; 3],
    pub along: [f64; 3],
}

/// THE RAYS OF SIGHT OF A FRAME: the camera, the frame in points and in pixels, and its depth.
#[derive(Clone, Copy, Debug)]
pub struct ViewRays {
    pub target: [f64; 3],
    /// The camera's own directions.
    pub right: [f64; 3],
    pub up: [f64; 3],
    pub fwd: [f64; 3],
    /// Points per mm at the target.
    pub scale: f64,
    /// Half the frame, in points.
    pub half: egui::Vec2,
    /// The size of one pixel in the measure of `through`: 2 across the frame.
    pub pixel: egui::Vec2,
    /// Pixels per point.
    pub ppp: f64,
    pub proj: ProjParams,
}

/// Where a ray meets the floor: how far along it, and the point.
struct FloorHit {
    t: f64,
    at: [f64; 2],
}

impl ViewRays {
    /// The rays of a frame of `rect` points drawn at `ppp` pixels a point.
    pub fn new(cam: &Cam3, rect: Rect, ppp: f32, proj: ProjParams) -> Self {
        let (right, up, fwd) = cam.basis();
        let px = rect.size() * ppp;
        ViewRays {
            target: cam.target,
            right,
            up,
            fwd,
            scale: (cam.scale as f64).max(1e-9),
            half: rect.size() * 0.5,
            pixel: egui::vec2(2.0 / px.x.max(1.0), 2.0 / px.y.max(1.0)),
            ppp: ppp as f64,
            proj,
        }
    }

    /// The ray through a point of the frame given from its middle, x to the right and y up, -1..1 across.
    pub fn through(&self, ndc: (f64, f64)) -> Ray {
        let (right, up, fwd) = (self.right, self.up, self.fwd);
        let s = (ndc.0 * self.half.x as f64 / self.scale, ndc.1 * self.half.y as f64 / self.scale);
        let side: [f64; 3] = std::array::from_fn(|a| right[a] * s.0 + up[a] * s.1);
        let inv_d = self.proj.inv_d_eye;
        if inv_d > 0.0 {
            Ray { from: std::array::from_fn(|a| self.target[a] - fwd[a] / inv_d), along: std::array::from_fn(|a| fwd[a] + side[a] * inv_d) }
        } else {
            Ray { from: std::array::from_fn(|a| self.target[a] + side[a]), along: fwd }
        }
    }

    /// The number the depth buffer holds for a point `t` along a ray (`depth_ndc`): in perspective `t` is the distance
    /// from the eye, in orthographic from the plane of the target.
    pub fn depth(&self, t: f64) -> f32 {
        if self.proj.inv_d_eye > 0.0 {
            (self.proj.z_near / t) as f32
        } else {
            (0.5 - t / (2.0 * self.proj.depth_half)).clamp(0.0, 1.0) as f32
        }
    }

    /// Where the ray meets the floor, and how far along it: none if it runs level or meets it behind the eye.
    fn on_floor(&self, ray: &Ray) -> Option<FloorHit> {
        if ray.along[2].abs() < 1e-12 {
            return None;
        }
        let t = -ray.from[2] / ray.along[2];
        if self.proj.inv_d_eye > 0.0 && t <= self.proj.z_near {
            return None;
        }
        Some(FloorHit { t, at: [ray.from[0] + ray.along[0] * t, ray.from[1] + ray.along[1] * t] })
    }
}

/// What a pixel of the floor comes out as: its colour, premultiplied, 0..1, and the depth of what it shows.
#[derive(Clone, Copy, Debug)]
pub struct FloorPixel {
    pub colour: [f32; 4],
    pub depth: f32,
}

/// A line `width` pixels wide.
struct Line {
    width: f64,
}

impl Line {
    /// How much of the line a pixel `off` pixels from its middle covers.
    fn covers(&self, off: f64) -> f64 {
        (self.width * 0.5 + 0.5 - off).clamp(0.0, 1.0)
    }
}

/// A fade from nothing at `gone` to the whole at `whole`, smooth at both ends.
struct Fade {
    gone: f64,
    whole: f64,
}

impl Fade {
    fn at(&self, x: f64) -> f64 {
        let t = ((x - self.gone) / (self.whole - self.gone)).clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    }
}

/// A colour premultiplied by its alpha, 0..1.
#[derive(Clone, Copy)]
struct Paint {
    rgba: [f32; 4],
}

impl Paint {
    const NONE: Paint = Paint { rgba: [0.0; 4] };

    /// A colour of the scheme taken `k` of the way.
    fn of(c: Color32, k: f64) -> Paint {
        Paint { rgba: c.to_array().map(|v| (v as f64 / 255.0 * k) as f32) }
    }

    /// This laid over `under`.
    fn over(self, under: Paint) -> Paint {
        Paint { rgba: std::array::from_fn(|k| self.rgba[k] + under.rgba[k] * (1.0 - self.rgba[3])) }
    }
}

/// THE FLOOR AT THE PIXEL whose middle is `ndc` (`ViewRays::through`): the grid and the axes X and Y where its ray meets
/// the plane, the Z axis where the ray passes by it. None where neither is.
pub fn floor_pixel(rays: &ViewRays, look: &FloorLook, ndc: (f64, f64)) -> Option<FloorPixel> {
    let ray = rays.through(ndc);
    let ppp = rays.ppp;
    let pixel = (rays.pixel.x as f64, rays.pixel.y as f64);
    let mut colour = Paint::NONE;
    let mut depth = 0.0f32;
    if let Some(FloorHit { t, at: p }) = rays.on_floor(&ray) {
        // THE SIZE OF A PIXEL ON THE PLANE, from the points the next pixel to the right and the next one down meet - what
        // `fwidth` gives the card. A neighbour whose ray misses the plane is taken as a pixel without end.
        let next = |d: (f64, f64)| rays.on_floor(&rays.through((ndc.0 + d.0, ndc.1 + d.1)));
        let (right, below) = (next((pixel.0, 0.0)), next((0.0, -pixel.1)));
        let fw = match (&right, &below) {
            (Some(a), Some(b)) => [(a.at[0] - p[0]).abs() + (b.at[0] - p[0]).abs(), (a.at[1] - p[1]).abs() + (b.at[1] - p[1]).abs()].map(|v| v.max(1e-12)),
            _ => [f64::INFINITY; 2],
        };
        // how much the floor's depth changes over a pixel, as `fwidth` gives it the card
        let slope = [&right, &below].iter().map(|h| h.as_ref().map_or(0.0, |h| (rays.depth(h.t) - rays.depth(t)).abs())).sum::<f32>();
        let len = (ray.along.iter().map(|v| v * v).sum::<f64>()).sqrt();
        let level = (ray.along[2] / len).abs();
        // a ray running within 0.7 deg of level meets the plane where a pixel spans more than the grid can show
        let grazing = Fade { gone: 0.0, whole: FLOOR_GRAZING }.at(level);
        let lod = Fade { gone: FLOOR_FADE_GONE, whole: FLOOR_FADE_WHOLE };
        let line = Line { width: FLOOR_LINE * ppp };
        let mut grid = Paint::NONE;
        let mut spacing = look.step;
        for k in 0..FLOOR_LEVELS {
            // the lines across X and the lines across Y each fade by their own spacing on screen: looked at low, the lines
            // running away from the eye stay wide apart while the lines across the view close in
            let cov = (0..2).map(|a| line.covers((p[a] - (p[a] / spacing).round() * spacing).abs() / fw[a]) * lod.at(spacing / (fw[a] * ppp))).fold(0.0, f64::max);
            let c = Paint::of(if k == 0 { look.minor } else { look.major }, cov * grazing);
            if c.rgba[3] > grid.rgba[3] {
                grid = c;
            }
            spacing *= 5.0;
        }
        let near_level = Fade { gone: 0.0, whole: 0.01 }.at(level);
        let axis = Line { width: FLOOR_AXIS * ppp };
        let ax = Paint::of(look.axis_x, axis.covers(p[1].abs() / fw[1]) * near_level);
        let ay = Paint::of(look.axis_y, axis.covers(p[0].abs() / fw[0]) * near_level);
        colour = ay.over(ax.over(grid));
        depth = rays.depth(t * FLOOR_BEHIND) - slope * FLOOR_SLOPE as f32;
    }
    // THE Z AXIS: where the ray passes closest to the line standing up from the origin
    let (o, d) = (ray.from, ray.along);
    let aa = d.iter().map(|v| v * v).sum::<f64>();
    let b = d[2];
    let den = aa - b * b;
    if den > 1e-12 * aa {
        let dd = (0..3).map(|k| d[k] * o[k]).sum::<f64>();
        let e = o[2];
        let sc = (b * e - dd) / den; // along the ray
        let tc = (aa * e - b * dd) / den; // up the axis
        let persp = rays.proj.inv_d_eye > 0.0;
        if tc >= 0.0 && (!persp || sc > rays.proj.z_near) {
            let gap = ((o[0] + d[0] * sc).powi(2) + (o[1] + d[1] * sc).powi(2) + (o[2] + d[2] * sc - tc).powi(2)).sqrt();
            let per_point = if persp { sc * rays.proj.inv_d_eye / rays.scale } else { 1.0 / rays.scale };
            let az = Line { width: FLOOR_AXIS_Z * ppp }.covers(gap / per_point * ppp);
            if az > 0.0 {
                let z = Paint::of(look.axis_z, az);
                let dz = rays.depth(sc);
                // the nearer of the two lies over the other, and the depth is the nearer one's
                if colour.rgba[3] <= 0.0 || dz >= depth {
                    colour = z.over(colour);
                    depth = dz;
                } else {
                    colour = colour.over(z);
                }
            }
        }
    }
    (colour.rgba[3] >= 0.5 / 255.0).then_some(FloorPixel { colour: colour.rgba, depth })
}
