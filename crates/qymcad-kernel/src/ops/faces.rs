//! THE NAMES OF THE FACES THE OPERATIONS ON FACES MAKE: a copy or an offset of a face, a patch, the inner walls of a
//! shell, the plate and walls of a thickening, the side faces of a draft. The model says which node; the rule is here.

use crate::door::Roles;
use crate::ImageOf;
use qymcad_core::model::Id;
use qymcad_core::names::{GeoName, NameTable, Role};

/// The face a copy or an offset makes of each of `faces`, named after the face it was taken from.
pub fn instances(feature: Id, faces: &[u32], roles: &mut Roles) -> Vec<u32> {
    faces.iter().map(|f| roles.give(GeoName::new(feature, Role::Instance, *f as Id))).collect()
}

/// The one face of a patch, named after the node: there is one patch a node and its surface has no other source.
pub fn patch(feature: Id, roles: &mut Roles) -> u32 {
    roles.give(GeoName::new(feature, Role::Patch, 0))
}

/// THE INNER WALL A SHELL MAKES FROM EVERY FACE OF THE BODY, named after that face. Without its own name an inner wall
/// took the name of its outer face and could not be told from it. Only a face named by recipe gives a wall a name: a
/// positional number is nothing to derive one from.
pub fn shell_walls(feature: Id, faces: &[u32], roles: &mut Roles) -> Vec<ImageOf> {
    faces.iter().filter(|f| NameTable::is_named(**f)).map(|f| ImageOf { source: *f, name: roles.give(GeoName::new(feature, Role::ShellWall, *f as Id)) }).collect()
}

/// THE NAMES OF A THICKENED PLATE, as the C++ side reads them - pairs laid flat: the offset side of every face after
/// that face, and the wall every edge sweeps after that edge.
pub struct Thickened {
    pub faces: Vec<u32>,
    pub edges: Vec<u32>,
}

pub fn thickened(feature: Id, faces: &[u32], edges: &[u32], roles: &mut Roles) -> Thickened {
    let mut out = Thickened { faces: Vec::with_capacity(faces.len() * 2), edges: Vec::with_capacity(edges.len() * 2) };
    for f in faces {
        out.faces.push(*f);
        out.faces.push(roles.give(GeoName::new(feature, Role::Thickened, *f as Id)));
    }
    for e in edges {
        out.edges.push(*e);
        out.edges.push(roles.give(GeoName::new(feature, Role::ThickenWall, *e as Id)));
    }
    out
}

/// THE SIDE FACE A DRAFT MAKES next to every face it tilts, named after that face, as pairs laid flat: without a name
/// of its own it took a positional number.
pub fn draft_sides(feature: Id, faces: &[u32], roles: &mut Roles) -> Vec<u32> {
    faces.iter().filter(|f| NameTable::is_named(**f)).flat_map(|f| [*f, roles.give(GeoName::new(feature, Role::DraftSide, *f as Id))]).collect()
}
