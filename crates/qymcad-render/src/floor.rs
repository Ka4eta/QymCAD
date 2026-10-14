//! THE FLOOR IN THE SOFTWARE RASTER: the plane Z = 0 under its grid and the world axes, pixel by pixel
//! (`qymcad_ui_state::floor_pixel`), drawn only where no solid body is - the floor lies behind every body, seen from
//! above or from below. It comes after the solid bodies and before the ghosts, as on the card.

use super::*;

/// The floor of one frame: its rays, its look, and the size of the picture in pixels.
pub struct FloorFrame<'a> {
    pub rays: &'a ViewRays,
    pub look: &'a FloorLook,
    pub size: [usize; 2],
}

impl FloorFrame<'_> {
    /// Draw the floor into a band of the picture starting at row `first_row`, `color` and `zbuf` holding exactly that
    /// band: where the buffer is still empty.
    pub fn draw(&self, color: &mut [Color32], zbuf: &[f32], first_row: usize) {
        let [w, h] = self.size;
        for (i, px) in color.iter_mut().enumerate() {
            let (x, y) = (i % w, first_row + i / w);
            let ndc = (((x as f64 + 0.5) / w as f64) * 2.0 - 1.0, 1.0 - ((y as f64 + 0.5) / h as f64) * 2.0);
            if zbuf[i] != f32::INFINITY {
                continue; // a body is drawn here
            }
            let Some(f) = floor_pixel(self.rays, self.look, ndc) else { continue };
            let under = px.to_array();
            let ch = |k: usize| ((f.colour[k] * 255.0) + under[k] as f32 * (1.0 - f.colour[3])).round().clamp(0.0, 255.0) as u8;
            *px = Color32::from_rgba_premultiplied(ch(0), ch(1), ch(2), ch(3));
        }
    }
}
