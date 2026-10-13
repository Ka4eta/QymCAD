//! THE LIMITING CASES, every tool against every way of landing on the boundary: a blend reaching exactly to an edge,
//! a cut flush with a face, bodies only touching. OCCT degenerates exactly there - a chamfer of 4 on an edge 4 from a
//! step was refused while 3.99999 built - and a person drawing to a dimension lands there on purpose.
//!
//! Live OCCT, headless; failures accumulate and are told together. The oracle of a cell: the node is built, the
//! volume is the one computed by hand, no face of the result is a crack, and a next blend beside it builds.
use qymcad_core::feature::{ChamferMode, Reach};
use qymcad_core::geom::Point2;
use qymcad_core::model::{ChamferShape, Project};
use qymcad_core::refs::Ref;

/// THE STEPPED BAR: an L section extruded 20 along z. The upper step is 4 wide (x 0..4, y 6..10), the lower one runs
/// on to x = 30. The edge at (0, 10) along z is the one blended: a leg of 4 on its top face reaches the edge of the
/// step at (4, 10) exactly.
const LENGTH: f64 = 20.0;
const STEP: f64 = 4.0;
const BAR_VOLUME: f64 = (30.0 * 6.0 + STEP * 4.0) * LENGTH;
/// The faces of the bar: six sides of the section and two ends.
const BAR_FACES: u32 = 8;

fn stepped_bar() -> (Project, u64) {
    let mut p = Project::default();
    p.new_document();
    let pts = [(0.0, 0.0), (30.0, 0.0), (30.0, 6.0), (STEP, 6.0), (STEP, 10.0), (0.0, 10.0)].iter().map(|&(x, y)| Point2::new(x, y)).collect();
    let sid = p.add_line_sketch("Bar", pts, true);
    let si = p.sketch_index(sid).expect("the sketch");
    p.regen_sketch(si);
    p.add_sketch_node(sid, "Bar");
    let closed: Vec<u64> = p.sketches[si].contour_ids.iter().copied().filter(|c| p.contour_profile_xy(*c).is_some()).collect();
    let bar = p.add_extrude_multi(sid, closed, LENGTH, Reach::Forward, 0.0, vec![]);
    let (report, _) = qymcad_testkit::regenerate(&mut p);
    assert!(report.errors.is_empty(), "setup: the bar did not build: {:?}", report.errors);
    (p, bar)
}

/// The edge of `body` running along z through (x, y).
fn edge_along_z(p: &Project, body: u64, x: f64, y: f64) -> Option<u32> {
    p.regen_edges.get(&body)?.iter().find(|e| (e.mid[0] - x).abs() < 1e-3 && (e.mid[1] - y).abs() < 1e-3 && (e.a[2] - e.b[2]).abs() > 1.0).map(|e| e.id)
}

/// The faces of the body, all of them: the tessellated list leaves out a strip too narrow to draw.
fn face_count(shapes: &std::collections::HashMap<u64, qymcad_kernel::Shape>, body: u64) -> u32 {
    shapes.get(&body).and_then(|s| s.face_kinds()).map(|k| k.iter().sum()).unwrap_or(0)
}

/// A WAY OF BLENDING the edge: what the person picks in the tool, and the material it takes off a sharp edge of
/// the size `size` (both legs on the faces at 90 deg).
struct Blend {
    name: &'static str,
    apply: fn(&mut Project, u64, u32, f64) -> u64,
    removed: fn(f64) -> f64,
}

fn chamfer_area(d: f64) -> f64 {
    d * d / 2.0
}

fn fillet_area(r: f64) -> f64 {
    r * r * (1.0 - std::f64::consts::PI / 4.0)
}

/// An asymmetric chamfer by `mode`: two legs of `d1`, or the leg `d1` at 45 deg - both take `d1` off either face.
fn chamfer_ex(p: &mut Project, src: u64, edge: u32, d1: f64, mode: ChamferMode) -> u64 {
    let d2 = if mode == ChamferMode::DistAngle { 45.0 } else { d1 };
    p.add_chamfer_ex(src, d1, ChamferShape { mode, d2, flip: false, ref_face: 0 }, vec![edge])
}

const BLENDS: [Blend; 5] = [
    Blend { name: "symmetric chamfer", apply: |p, src, e, d| p.add_chamfer(src, d, vec![e]), removed: chamfer_area },
    Blend { name: "chamfer by two legs", apply: |p, src, e, d| chamfer_ex(p, src, e, d, ChamferMode::TwoDist), removed: chamfer_area },
    Blend { name: "chamfer by a leg and an angle", apply: |p, src, e, d| chamfer_ex(p, src, e, d, ChamferMode::DistAngle), removed: chamfer_area },
    Blend { name: "fillet of one radius", apply: |p, src, e, r| p.add_fillet(src, r, vec![e]), removed: fillet_area },
    Blend {
        name: "fillet with radii at the vertices",
        apply: |p, src, e, r| {
            let ends: Vec<(u32, f64)> = p.vertex_pool(src).into_iter().filter(|c| c.centroid[0].abs() < 1e-3 && (c.centroid[1] - 10.0).abs() < 1e-3).map(|c| (c.desc, r)).collect();
            p.add_fillet_at_vertices(src, r, Ref::picks(&[e]), ends)
        },
        removed: fillet_area,
    },
];

/// HOW THE BLEND LANDS on the face it runs across: its size, and how many faces the clean result has.
struct Landing {
    name: &'static str,
    size: f64,
    faces: u32,
}

const LANDINGS: [Landing; 3] = [
    // the control: well inside the face, one blend face added
    Landing { name: "inside the face", size: 3.0, faces: BAR_FACES + 1 },
    // the top face of the step is eaten whole and the blend face takes its place
    Landing { name: "exactly to the edge", size: STEP, faces: BAR_FACES },
    // a strip of 1e-6 stays - a real face, narrower than nothing a person draws, but geometry all the same
    Landing { name: "1e-6 short of the edge", size: STEP - 1e-6, faces: BAR_FACES + 1 },
];

#[test]
fn a_blend_lands_on_the_edge_of_a_step() {
    let mut fails: Vec<String> = Vec::new();
    for blend in &BLENDS {
        for landing in &LANDINGS {
            let cell = format!("{} {} ({})", blend.name, landing.size, landing.name);
            let (mut p, bar) = stepped_bar();
            let Some(edge) = edge_along_z(&p, bar, 0.0, 10.0) else {
                fails.push(format!("{cell}: setup - no edge at (0, 10)"));
                continue;
            };
            let node = (blend.apply)(&mut p, bar, edge, landing.size);
            let (report, shapes) = qymcad_testkit::regenerate(&mut p);
            if let Some((_, e)) = report.errors.iter().find(|(id, _)| *id == node) {
                fails.push(format!("{cell}: refused - {e:?}"));
                continue;
            }
            let Some(shape) = shapes.get(&node) else {
                fails.push(format!("{cell}: no body"));
                continue;
            };
            let want = BAR_VOLUME - (blend.removed)(landing.size) * LENGTH;
            if (shape.volume() - want).abs() > 1e-3 * want {
                fails.push(format!("{cell}: volume {:.4}, expected {want:.4}", shape.volume()));
            }
            let cracks = shape.sliver_faces();
            if cracks > 0 {
                fails.push(format!("{cell}: {cracks} crack face(s) left"));
            }
            let faces = face_count(&shapes, node);
            if landing.size == STEP && faces != landing.faces {
                fails.push(format!("{cell}: {faces} faces, the clean result has {}", landing.faces));
            }
            // THE NEXT BLEND BESIDE: the edge where the step goes down at x = 4, now one side of the first blend
            let Some(next) = edge_along_z(&p, node, STEP, 6.0) else {
                fails.push(format!("{cell}: no edge at ({STEP}, 6) to blend next"));
                continue;
            };
            let after = p.add_fillet(node, 1.0, vec![next]);
            let (report, _) = qymcad_testkit::regenerate(&mut p);
            if let Some((_, e)) = report.errors.iter().find(|(id, _)| *id == after) {
                fails.push(format!("{cell}: the fillet R1 beside it refused - {e:?}"));
            }
        }
    }
    assert!(fails.is_empty(), "\nBLENDS AT THE EDGE OF A STEP ({}):\n{}", fails.len(), fails.join("\n"));
}

/// THE BLOCK the cuts and bosses work on: 40 x 30 x 10, centred on the origin in plan, from z = 0 up.
const BLOCK_VOLUME: f64 = 40.0 * 30.0 * 10.0;

/// A CUT OR A BOSS FROM THE BOTTOM OF THE BLOCK: a rectangle on XY (z = 0), taken up into the block (a cut) or down
/// out of it (a boss), and what the clean result is.
struct FromTheBottom {
    name: &'static str,
    /// The rectangle, two corners.
    from: Point2,
    to: Point2,
    /// How far the profile goes: up into the block for a cut, down from it for a boss.
    height: f64,
    what: Combine,
    volume: f64,
    faces: u32,
}

/// What the profile does to the block.
#[derive(Clone, Copy, PartialEq)]
enum Combine {
    /// Taken up into the block and out of it.
    Cut,
    /// Taken down out of the block and joined to it.
    Boss,
}

impl Combine {
    /// The operation code of the combine node.
    fn code(self) -> u8 {
        match self {
            Combine::Cut => 0,
            Combine::Boss => 1,
        }
    }
}

const fn p2(x: f64, y: f64) -> Point2 {
    Point2 { x, y }
}

const CUTS: [FromTheBottom; 6] = [
    // as deep as the block is thick: a hole right through, its far end flush with the top
    FromTheBottom { name: "a pocket exactly as deep as the block", from: p2(-5.0, -5.0), to: p2(5.0, 5.0), height: 10.0, what: Combine::Cut, volume: BLOCK_VOLUME - 1000.0, faces: 10 },
    // one side of the slot on the front face of the block: the front split in two, three walls of the slot
    FromTheBottom { name: "a slot through, flush with the front", from: p2(-5.0, -15.0), to: p2(5.0, -5.0), height: 10.0, what: Combine::Cut, volume: BLOCK_VOLUME - 1000.0, faces: 10 },
    // two sides on the faces of the corner: a step 5 high in the corner
    FromTheBottom { name: "a step flush in the corner", from: p2(10.0, 5.0), to: p2(20.0, 15.0), height: 5.0, what: Combine::Cut, volume: BLOCK_VOLUME - 500.0, faces: 9 },
    // a boss standing on the bottom face: it touches the block along a face and joins it
    FromTheBottom { name: "a boss touching the bottom", from: p2(-5.0, -5.0), to: p2(5.0, 5.0), height: 5.0, what: Combine::Boss, volume: BLOCK_VOLUME + 500.0, faces: 11 },
    // a boss whose front is the front of the block carried down: the two fronts one face
    FromTheBottom { name: "a boss flush with the front", from: p2(-5.0, -15.0), to: p2(5.0, -5.0), height: 5.0, what: Combine::Boss, volume: BLOCK_VOLUME + 500.0, faces: 10 },
    // a boss 1e-6 short of the front: its front a face of its own, 1e-6 behind that of the block
    FromTheBottom { name: "a boss 1e-6 short of the front", from: p2(-5.0, -15.0 + 1e-6), to: p2(5.0, -5.0), height: 5.0, what: Combine::Boss, volume: BLOCK_VOLUME + 500.0, faces: 11 },
];

#[test]
fn a_cut_or_a_boss_lands_flush() {
    let mut fails: Vec<String> = Vec::new();
    for cell in &CUTS {
        let mut p = Project::default();
        p.new_document();
        let block = p.add_box(40.0, 30.0, 10.0);
        let si = p.new_sketch("Tool");
        let sid = p.sketches[si].id;
        p.add_sketch_node(sid, "Tool");
        p.add_rect_entity(si, cell.from.x, cell.from.y, cell.to.x, cell.to.y, qymcad_core::feature::Purpose::Real);
        p.regen_sketch(si);
        let c: Vec<u64> = p.sketches[si].contour_ids.iter().copied().filter(|c| p.contour_profile_xy(*c).is_some()).collect();
        let reach = if cell.what == Combine::Cut { Reach::Forward } else { Reach::Backward };
        let span = qymcad_core::model::CombineSpan { height: cell.height, down: 0.0, extent: qymcad_core::feature::Extent { reach, ..Default::default() }, fill: &[] };
        let node = p.add_combine_multi_op(block, sid, c, span, cell.what.code());
        let (report, shapes) = qymcad_testkit::regenerate(&mut p);
        if let Some((_, e)) = report.errors.iter().find(|(id, _)| *id == node) {
            fails.push(format!("{}: refused - {e:?}", cell.name));
            continue;
        }
        let Some(shape) = shapes.get(&node) else {
            fails.push(format!("{}: no body", cell.name));
            continue;
        };
        if (shape.volume() - cell.volume).abs() > 1e-3 {
            fails.push(format!("{}: volume {:.6}, expected {:.6}", cell.name, shape.volume(), cell.volume));
        }
        if shape.solid_count() != 1 {
            fails.push(format!("{}: {} solids, a part is one", cell.name, shape.solid_count()));
        }
        let cracks = shape.sliver_faces();
        if cracks > 0 {
            fails.push(format!("{}: {cracks} crack face(s) left", cell.name));
        }
        let faces = face_count(&shapes, node);
        if faces != cell.faces {
            fails.push(format!("{}: {faces} faces, the clean result has {}", cell.name, cell.faces));
        }
        // THE NEXT OPERATION: a fillet of the upright edge at the corner the cut leaves alone
        let Some(edge) = edge_along_z(&p, node, -20.0, -15.0) else {
            fails.push(format!("{}: no edge at (-20, -15) to round next", cell.name));
            continue;
        };
        let after = p.add_fillet(node, 1.0, vec![edge]);
        let (report, _) = qymcad_testkit::regenerate(&mut p);
        if let Some((_, e)) = report.errors.iter().find(|(id, _)| *id == after) {
            fails.push(format!("{}: the fillet R1 after it refused - {e:?}", cell.name));
        }
    }
    assert!(fails.is_empty(), "\nCUTS AND BOSSES FLUSH ({}):\n{}", fails.len(), fails.join("\n"));
}

/// WHAT A CELL ON THE BLOCK GIVES when it is right.
enum Expect {
    /// One solid of this volume; and, where it is known, this many faces.
    Builds { volume: f64, faces: Option<u32> },
    /// Nothing to build, said in words: an error of the node other than a broken body.
    Refused,
}

/// AN OPERATION ON THE BLOCK 40 x 30 x 10 landing on its boundary: `apply` lays the node and gives its id.
struct OnTheBlock {
    name: &'static str,
    apply: fn(&mut Project, u64) -> u64,
    expect: Expect,
}

/// The key of the face of `body` whose normal is `normal`.
fn face_key(p: &Project, body: u64, normal: [f64; 3]) -> qymcad_core::feature::FaceKey {
    let f = p.regen_faces.get(&body).and_then(|fs| fs.iter().find(|f| (0..3).all(|i| (f.normal[i] - normal[i]).abs() < 1e-6))).cloned().expect("the face of that normal");
    qymcad_core::feature::FaceKey { index: 0, centroid: [f.centroid.x, f.centroid.y, f.centroid.z], normal: f.normal, id: f.id }
}

/// The id of the face of `body` whose normal is `normal`.
fn face_id(p: &Project, body: u64, normal: [f64; 3]) -> u32 {
    face_key(p, body, normal).id
}

const UP: [f64; 3] = [0.0, 0.0, 1.0];

const ON_THE_BLOCK: [OnTheBlock; 5] = [
    // a hole 10 across exactly as deep as the block: through, no cap of the bottom left in it
    OnTheBlock {
        name: "a hole exactly as deep as the block",
        apply: |p, b| {
            let key = face_key(p, b, UP);
            p.add_hole(b, key, 10.0, 10.0)
        },
        expect: Expect::Builds { volume: BLOCK_VOLUME - std::f64::consts::PI * 25.0 * 10.0, faces: Some(7) },
    },
    // three copies of the block 40 apart: each touches the next along a face, and together they are one bar 120 long
    OnTheBlock { name: "copies touching face to face", apply: |p, b| p.add_linear_array(b, 40.0, 0.0, 0.0, 3), expect: Expect::Builds { volume: 3.0 * BLOCK_VOLUME, faces: Some(6) } },
    // walls 15 thick inside a block 30 wide meet in the middle: nothing is left hollow
    OnTheBlock {
        name: "a shell as thick as half the width",
        apply: |p, b| {
            let top = face_id(p, b, UP);
            p.add_shell(b, 15.0, vec![top], false)
        },
        expect: Expect::Refused,
    },
    // a floor as thick as the block is tall: nothing is left hollow
    OnTheBlock {
        name: "a shell as thick as the block is tall",
        apply: |p, b| {
            let top = face_id(p, b, UP);
            p.add_shell(b, 10.0, vec![top], false)
        },
        expect: Expect::Refused,
    },
    // the plane of the split lies on the bottom face: there is nothing on one side of it
    OnTheBlock { name: "a split on the plane of the bottom face", apply: |p, b| p.add_split_body(b, 0, 0, 0.0, 2)[0], expect: Expect::Refused },
];

#[test]
fn an_operation_lands_on_the_boundary_of_the_block() {
    let mut fails: Vec<String> = Vec::new();
    for cell in &ON_THE_BLOCK {
        let mut p = Project::default();
        p.new_document();
        let block = p.add_box(40.0, 30.0, 10.0);
        let _ = qymcad_testkit::regenerate(&mut p);
        let node = (cell.apply)(&mut p, block);
        let (report, shapes) = qymcad_testkit::regenerate(&mut p);
        let error = report.errors.iter().find(|(id, _)| *id == node).map(|(_, e)| e.clone());
        match (&cell.expect, error) {
            (Expect::Refused, Some(qymcad_core::errors::CoreError::BrokenSolid)) => fails.push(format!("{}: a broken body instead of a refusal in words", cell.name)),
            (Expect::Refused, Some(_)) => {}
            (Expect::Refused, None) => {
                let v = shapes.get(&node).map(|s| s.volume()).unwrap_or(0.0);
                fails.push(format!("{}: built ({v:.3} mm^3) where nothing can be", cell.name));
            }
            (Expect::Builds { .. }, Some(e)) => fails.push(format!("{}: refused - {e:?}", cell.name)),
            (Expect::Builds { volume, faces }, None) => {
                let Some(shape) = shapes.get(&node) else {
                    fails.push(format!("{}: no body", cell.name));
                    continue;
                };
                if (shape.volume() - volume).abs() > 1e-3 * volume {
                    fails.push(format!("{}: volume {:.4}, expected {volume:.4}", cell.name, shape.volume()));
                }
                if shape.solid_count() != 1 {
                    fails.push(format!("{}: {} solids, a part is one", cell.name, shape.solid_count()));
                }
                let cracks = shape.sliver_faces();
                if cracks > 0 {
                    fails.push(format!("{}: {cracks} crack face(s) left", cell.name));
                }
                let got = face_count(&shapes, node);
                if faces.is_some_and(|f| f != got) {
                    fails.push(format!("{}: {got} faces, the clean result has {}", cell.name, faces.unwrap_or(0)));
                }
            }
        }
    }
    assert!(fails.is_empty(), "\nOPERATIONS ON THE BOUNDARY OF THE BLOCK ({}):\n{}", fails.len(), fails.join("\n"));
}
