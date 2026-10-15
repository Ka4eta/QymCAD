//! A BLEND IS NAMED AFTER THE EDGE IT WAS PICKED ON, as the node holds the pick: an edit above that renames the edge -
//! a face of its pair merged into another - but moves neither the edge nor the blend leaves the surface of the blend
//! its name. Reported behaviour: on the case of the examples a dimension of a sketch by +10% renamed eight faces of a
//! chamfer that did not move, and everything standing on them with them.
use qymcad_core::model::{DimTarget, Id, Project};

/// A FACE OF A BLEND by its name and where it stands.
struct Stand {
    name: String,
    at: [f64; 3],
    area: f64,
}

/// The faces node `node` made on its own body - a blend's body is the node - by where they stand.
fn blends(p: &Project, node: Id) -> Vec<Stand> {
    p.regen_faces[&node]
        .iter()
        .filter(|f| p.names.get(f.id).is_some_and(|n| n.feature == node))
        .map(|f| Stand { name: p.names.describe(f.id), at: [f.centroid.x, f.centroid.y, f.centroid.z], area: f.area })
        .collect()
}

#[test]
fn the_faces_of_a_chamfer_that_did_not_move_keep_their_names() {
    let path = format!("{}/../../examples/QymBoxPc-Case.qcad", env!("CARGO_MANIFEST_DIR"));
    let mut p = qymcad_io::load_project(&path).expect("the example opens");
    let shapes = qymcad_testkit::restore_import_shapes(&p);
    let (_, shapes) = qymcad_testkit::regenerate_with_shapes(&mut p, shapes);
    let before = blends(&p, 5341);
    assert!(!before.is_empty(), "GUARD: the chamfer 5341 of the case has faces of its own");
    p.set_dim_target_value(&DimTarget::Sketch { sketch: 5182, refs: vec![5328, 5329] }, 68.2);
    let si = p.sketch_index(5182).expect("sketch 5182");
    p.solve_sketch(si);
    p.mark_sketch_dirty(5182);
    let _ = qymcad_testkit::regenerate_dirty_with_shapes(&mut p, shapes);
    let after = blends(&p, 5341);
    let still = |a: &Stand, b: &Stand| (0..3).all(|k| (a.at[k] - b.at[k]).abs() < 1e-6) && (a.area - b.area).abs() < 1e-6 * a.area.max(1.0);
    let renamed: Vec<String> = after.iter().filter_map(|a| before.iter().find(|b| still(a, b)).filter(|b| b.name != a.name).map(|b| format!("{} -> {}", b.name, a.name))).collect();
    assert!(renamed.is_empty(), "faces of the chamfer that did not move were renamed:\n  {}", renamed.join("\n  "));
}
