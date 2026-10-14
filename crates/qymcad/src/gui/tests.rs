//! THE GUI TESTS, lifted out of `gui.rs`.
//!
//! This is a CHILD module of `gui` rather than a neighbouring file: `App`'s private fields are visible to the
//! owning module and its descendants, so the headless flows keep working with the internal state directly.
// DO NOT REMOVE: rustc calls this import unused, yet without it the file does not compile - it is what lets the
// nested test modules see the `gui` names by the `super::` path (a private import is available to descendants).
#[allow(unused_imports)]
use super::*;

#[cfg(test)]
mod command_flow_tests;

#[cfg(test)]
mod section_clip_tests;

#[cfg(test)]
mod gizmo_ring_sign_tests;

#[cfg(test)]
mod section_drag_tests;

#[cfg(test)]
mod pick_depth_tests;

#[cfg(test)]
mod gizmo_math_tests;

#[cfg(test)]
mod sketch_conflict_ui_tests;

#[cfg(test)]
mod brep_warmup_tests;

#[cfg(test)]
mod sketch_plane_pick_frames_tests;

#[cfg(test)]
mod faces_part_in_light;

#[cfg(test)]
mod the_floor_reaches_the_horizon;

#[cfg(test)]
mod parts_far_apart_draw_clean;
