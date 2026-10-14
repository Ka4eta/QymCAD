//! THE NAMES ON REAL DOCUMENTS: what the matrices of names check on blocks and stairs, checked on the documents people
//! made - the examples the program ships and the private samples, where this tree has them.
//!
//! A document is opened as the window opens it (the bodies of imports restored, a full rebuild), and then:
//!
//! * rebuilt again with no edit - every name of a face, an edge and an end of an edge stands as it stood;
//! * edited one number at a time, as a person edits it: the height of an extrusion, a parameter of the document, a
//!   dimension of a sketch, by +10% or by -10%. After the edit no reference is lost, none is found by place,
//!   no node goes red that stood before, and an element that did not move kept its name. The edit is then undone as
//!   Ctrl+Z undoes it, and every name is the one the document opened with.
//!
//! The faults of every document and every edit are gathered and told together.
use qymcad_core::errors::CoreError;
use qymcad_core::model::{DimTarget, Id, Project};
use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::Ordering;

/// A DOCUMENT OF THE MATRIX: its file name and where it lies.
pub struct Doc {
    pub name: String,
    pub path: String,
}

/// The examples the program ships, always there, and the private samples that are documents of parts, where this tree
/// has them (a missing one is said on the output and passed over).
pub fn documents() -> Vec<Doc> {
    let root = format!("{}/../..", env!("CARGO_MANIFEST_DIR"));
    let mut docs: Vec<Doc> = ["Filter-v2.qcad", "QymBoxPc-Case.qcad"].iter().map(|n| Doc { name: n.to_string(), path: format!("{root}/examples/{n}") }).collect();
    for name in ["vacuumCleaner.qcad", "test_bridge_resize_body.qcad", "bugs_fillet_again.qcad", "pref_sketch.qcad", "sketch_line_test.qcad", "issue-4/project/project.qcad"] {
        let path = format!("{root}/samples/{name}");
        if std::path::Path::new(&path).exists() {
            docs.push(Doc { name: name.to_string(), path });
        } else {
            eprintln!("PASSED OVER: the private sample {name} is not in this tree - the check on it runs only where the samples are");
        }
    }
    docs
}

/// A FACE AS THE MATRIX SEES IT: its name and where it stands.
#[derive(Clone)]
struct FaceMark {
    id: u32,
    centroid: Place,
    normal: Place,
    area: f64,
}

/// AN EDGE AS THE MATRIX SEES IT: its name, the names of its ends, and where it stands.
#[derive(Clone)]
struct EdgeMark {
    id: u32,
    ends: [qymcad_core::names::VertexName; 2],
    a: Place,
    b: Place,
    mid: Place,
}

/// The faces and the edges of one body.
#[derive(Clone, Default)]
struct BodyMarks {
    faces: Vec<FaceMark>,
    edges: Vec<EdgeMark>,
}

/// EVERY NAME OF A DOCUMENT AT ONE MOMENT, by body.
#[derive(Clone, Default)]
pub struct Marks {
    bodies: BTreeMap<Id, BodyMarks>,
}

impl Marks {
    pub fn of(p: &Project) -> Marks {
        let mut bodies: BTreeMap<Id, BodyMarks> = BTreeMap::new();
        for (b, faces) in &p.regen_faces {
            let e = bodies.entry(*b).or_default();
            e.faces = faces.iter().map(|f| FaceMark { id: f.id, centroid: Place([f.centroid.x, f.centroid.y, f.centroid.z]), normal: Place(f.normal), area: f.area }).collect();
        }
        for (b, edges) in &p.regen_edges {
            let e = bodies.entry(*b).or_default();
            e.edges = edges.iter().map(|x| EdgeMark { id: x.id, ends: x.ends, a: Place(x.a), b: Place(x.b), mid: Place(x.mid) }).collect();
        }
        for m in bodies.values_mut() {
            m.faces.sort_by_key(|f| f.id);
            m.edges.sort_by_key(|e| e.id);
        }
        Marks { bodies }
    }

    pub fn faces(&self) -> usize {
        self.bodies.values().map(|b| b.faces.len()).sum()
    }

    pub fn edges(&self) -> usize {
        self.bodies.values().map(|b| b.edges.len()).sum()
    }
}

/// How far two places may stand apart and still be one place: the rebuild of an untouched node gives the same numbers
/// to the last bit, and 1e-7 mm leaves room only for the sums of a tessellation done in another order.
const SAME_PLACE: f64 = 1e-7;

/// A POINT OR A DIRECTION, compared to `SAME_PLACE`.
#[derive(Clone, Copy)]
struct Place([f64; 3]);

impl Place {
    fn is(&self, other: &Place) -> bool {
        (0..3).all(|i| (self.0[i] - other.0[i]).abs() < SAME_PLACE)
    }
}

impl FaceMark {
    /// Standing where `other` stood: the same centre, the same normal, the same area.
    fn stands_as(&self, other: &FaceMark) -> bool {
        self.centroid.is(&other.centroid) && self.normal.is(&other.normal) && (self.area - other.area).abs() < SAME_PLACE * self.area.max(1.0)
    }
}

impl EdgeMark {
    /// Standing where `other` stood: the same middle and the same ends, either way round.
    fn stands_as(&self, other: &EdgeMark) -> bool {
        self.mid.is(&other.mid) && ((self.a.is(&other.a) && self.b.is(&other.b)) || (self.a.is(&other.b) && self.b.is(&other.a)))
    }
}

/// TWO MOMENTS OF ONE DOCUMENT: the names before something happened to it and after.
pub struct Change<'a> {
    pub was: &'a Marks,
    pub now: &'a Marks,
}

/// WHAT DIFFERS BETWEEN TWO MOMENTS BY NAME: a body come or gone, a name come or gone, the ends of an edge renamed.
/// Empty when every name stands as it stood.
pub fn renamed(change: Change<'_>, p: &Project) -> Vec<String> {
    let Change { was, now } = change;
    let mut out = Vec::new();
    for (b, w) in &was.bodies {
        let Some(n) = now.bodies.get(b) else {
            out.push(format!("body {b} is gone"));
            continue;
        };
        let wf: Vec<u32> = w.faces.iter().map(|f| f.id).collect();
        let nf: Vec<u32> = n.faces.iter().map(|f| f.id).collect();
        for id in wf.iter().filter(|i| !nf.contains(i)) {
            out.push(format!("body {b}: face {id} = {} is gone", p.names.describe(*id)));
        }
        for id in nf.iter().filter(|i| !wf.contains(i)) {
            out.push(format!("body {b}: face {id} = {} is new", p.names.describe(*id)));
        }
        for we in &w.edges {
            match n.edges.iter().find(|e| e.id == we.id) {
                None => out.push(format!("body {b}: edge {} = {} is gone", we.id, p.names.describe(we.id))),
                Some(ne) if ne.ends != we.ends => out.push(format!("body {b}: the ends of edge {} were {:?}, are {:?}", we.id, we.ends, ne.ends)),
                Some(_) => {}
            }
        }
        for ne in n.edges.iter().filter(|e| !w.edges.iter().any(|x| x.id == e.id)) {
            out.push(format!("body {b}: edge {} = {} is new", ne.id, p.names.describe(ne.id)));
        }
    }
    for b in now.bodies.keys().filter(|b| !was.bodies.contains_key(b)) {
        out.push(format!("body {b} is new"));
    }
    out
}

/// AN ELEMENT THAT DID NOT MOVE KEEPS ITS NAME: every face of `now` standing exactly where one face of `was` stood, in
/// the same body, holds that face's name; and so does every edge standing still between faces that stood still. An
/// edge is the pair of its faces: one left in place while a face beside it came, went or moved - the cap of a boss
/// flush with the top of its base one moment and below it the next - is another edge, and its new name is right.
fn moved_names(change: Change<'_>, p: &Project) -> Vec<String> {
    let Change { was, now } = change;
    let mut out = Vec::new();
    for (b, n) in &now.bodies {
        let Some(w) = was.bodies.get(b) else { continue };
        let mut still: std::collections::HashSet<u32> = std::collections::HashSet::new();
        for f in &n.faces {
            let same: Vec<&FaceMark> = w.faces.iter().filter(|x| x.stands_as(f)).collect();
            if let [only] = same.as_slice() {
                if only.id == f.id {
                    still.insert(f.id);
                } else {
                    out.push(format!("body {b}: a face that did not move was {} = {}, is {} = {}", only.id, p.names.describe(only.id), f.id, p.names.describe(f.id)));
                }
            }
        }
        let between_still = |id: u32| p.names.edge(id).is_some_and(|e| e.faces.iter().all(|f| still.contains(f)));
        for e in &n.edges {
            let same: Vec<&EdgeMark> = w.edges.iter().filter(|x| x.stands_as(e)).collect();
            if let [only] = same.as_slice() {
                if only.id != e.id && between_still(only.id) && between_still(e.id) {
                    out.push(format!("body {b}: an edge that did not move was {} = {}, is {} = {}", only.id, p.names.describe(only.id), e.id, p.names.describe(e.id)));
                }
            }
        }
    }
    out
}

/// A REFERENCE NOT FOUND: the codes a node goes red or yellow with when a name it holds is not in its body.
fn is_lost_reference(e: &CoreError) -> bool {
    matches!(e, CoreError::FaceNotFound | CoreError::FacesNotFound | CoreError::EdgesNotFound { .. } | CoreError::DescribedEdgesNotFound | CoreError::EdgesDropped { .. })
}

/// A DOCUMENT OPENED, with what the edits are measured against.
pub struct Opened {
    /// Where the document lies: an edit that did not undo cleanly opens it afresh, so it does not spoil the next.
    path: String,
    pub project: Project,
    shapes: HashMap<Id, qymcad_kernel::Shape>,
    pub marks: Marks,
    errors: HashMap<Id, CoreError>,
    warnings: HashMap<Id, CoreError>,
}

/// Open `doc` as the window opens it.
pub fn open(doc: &Doc) -> Opened {
    let mut project = qymcad_io::load_project(&doc.path).unwrap_or_else(|e| panic!("{} opens: {e:?}", doc.name));
    let shapes = crate::restore_import_shapes(&project);
    let (_, shapes) = crate::regenerate_with_shapes(&mut project, shapes);
    let marks = Marks::of(&project);
    let errors = project.regen_errors.clone();
    let warnings = project.regen_warnings.clone();
    Opened { path: doc.path.clone(), project, shapes, marks, errors, warnings }
}

/// TWO REBUILDS IN A ROW GIVE THE SAME NAMES: `doc` opened, then rebuilt whole once more with nothing edited.
pub fn determinism(doc: &Doc) -> Vec<String> {
    let mut o = open(doc);
    let first = o.marks.clone();
    let shapes = crate::restore_import_shapes(&o.project);
    let _ = crate::regenerate_with_shapes(&mut o.project, shapes);
    let second = Marks::of(&o.project);
    eprintln!("{}: {} bodies, {} faces, {} edges", doc.name, first.bodies.len(), first.faces(), first.edges());
    renamed(Change { was: &first, now: &second }, &o.project).into_iter().map(|r| format!("{}: a second rebuild renamed: {r}", doc.name)).collect()
}

/// WHAT IS EDITED: one number a person types.
#[derive(Clone)]
pub enum Edit {
    /// A dimension of a node (`height`, `down`).
    Feature { node: Id, key: &'static str, value: f64 },
    /// A parameter of the document, by its place in the table.
    Param { index: usize, value: f64 },
    /// A dimension of a sketch, by the entities it is laid on.
    Sketch { sketch: Id, refs: Vec<Id>, value: f64 },
}

impl Edit {
    fn says(&self, p: &Project) -> String {
        match self {
            Edit::Feature { node, key, value, .. } => format!("{key} of node {node} = {value}"),
            Edit::Param { index, value } => format!("parameter {} = {value}", p.parameters[*index].name),
            Edit::Sketch { sketch, refs, value } => format!("dimension {refs:?} of sketch {sketch} = {value}"),
        }
    }
}

/// EVERY NUMBER OF A DOCUMENT A PERSON EDITS: the heights of extrusions, the parameters, the driving dimensions of the
/// sketches, one of each value in a sketch. A zero is left alone: a tenth of nothing is nothing.
pub fn edits(p: &Project) -> Vec<Edit> {
    use qymcad_core::feature::FeatureKind;
    let vars = p.param_map();
    let mut out = Vec::new();
    for n in &p.timeline {
        if !matches!(n.kind, FeatureKind::Extrude { .. } | FeatureKind::Combine { .. }) {
            continue;
        }
        for (key, stored) in n.kind.dims() {
            let value = p.feat_dim_value(n.id, key, stored, &vars);
            if value.abs() > 1e-9 {
                out.push(Edit::Feature { node: n.id, key, value });
            }
        }
    }
    for (index, par) in p.parameters.iter().enumerate() {
        if par.value.abs() > 1e-9 {
            out.push(Edit::Param { index, value: par.value });
        }
    }
    for s in &p.sketches {
        // one dimension of each value in a sketch: forty holes of one diameter are one edit forty times over, and the
        // case of the documents gives 154 numbers at 12 s each without this
        let mut seen: Vec<f64> = Vec::new();
        for c in &s.constraints {
            let Some(refs) = Project::dim_refs(c) else { continue };
            let Some(value) = Project::dim_value_of(c) else { continue };
            if c.is_driven() || value.abs() <= 1e-9 || seen.iter().any(|v| (v - value).abs() < 1e-9) {
                continue;
            }
            seen.push(value);
            out.push(Edit::Sketch { sketch: s.id, refs, value });
        }
    }
    out
}

/// The edit `e` set to `value`, as the window writes it, with the document marked for the rebuild.
fn apply(p: &mut Project, e: &Edit, value: f64) {
    match e {
        Edit::Feature { node, key, .. } => {
            p.set_dim_target_value(&DimTarget::Feature { node: *node, key: key.to_string() }, value);
        }
        Edit::Param { index, .. } => {
            let name = p.parameters[*index].name.clone();
            p.parameters[*index].expr = qymcad_core::expr::fmt_num(value);
            settle_params(p, &name);
        }
        Edit::Sketch { sketch, refs, .. } => {
            let index = p.sketch_index(*sketch).expect("the sketch of the dimension");
            p.set_dim_target_value(&DimTarget::Sketch { sketch: *sketch, refs: refs.clone() }, value);
            p.solve_sketch(index);
            p.mark_sketch_dirty(*sketch);
        }
    }
}

/// A parameter changed: the values evaluated, the sketches over formulas solved, and what mentions the name marked.
fn settle_params(p: &mut Project, name: &str) {
    p.eval_parameters();
    for si in 0..p.sketches.len() {
        if p.sketches[si].constraints.iter().any(|c| c.expr().is_some()) {
            p.solve_sketch(si);
            let sid = p.sketches[si].id;
            p.mark_sketch_dirty(sid);
        }
    }
    p.mark_param_dependents_dirty_for(name);
}

/// THE EDIT UNDONE as Ctrl+Z undoes it: the document as it stood before, and the nodes whose bodies differ from the
/// edited state marked for the rebuild (`changed_bodies_vs`, the door of the window's undo).
fn undo(p: &mut Project, before: Project) {
    let edited = std::mem::replace(p, before);
    let changed = p.changed_bodies_vs(&edited);
    for n in &mut p.timeline {
        if n.kind.bodies().iter().any(|b| changed.contains(b)) {
            n.dirty = true;
        }
    }
}

/// THE ERRORS AND WARNINGS A REBUILD LEFT that the document did not open with, each said with its node.
fn new_reds(o: &Opened) -> Vec<String> {
    let p = &o.project;
    let mut out = Vec::new();
    // a node standing on a body that went red is red with it: the root is told, its cascade is not
    let fresh: Vec<_> = p.regen_errors.iter().filter(|(n, e)| o.errors.get(n) != Some(e)).collect();
    let rooted = fresh.iter().any(|(_, e)| !matches!(e, CoreError::SourceBodyNotBuilt));
    for (node, e) in fresh.into_iter().filter(|(_, e)| !(rooted && matches!(e, CoreError::SourceBodyNotBuilt))) {
        let what = if is_lost_reference(e) { "lost a reference" } else { "went red" };
        out.push(format!("node {node} {what}: {e:?}"));
    }
    for (node, e) in p.regen_warnings.iter().filter(|(n, e)| o.warnings.get(n) != Some(e) && is_lost_reference(e)) {
        out.push(format!("node {node} lost a reference: {e:?}"));
    }
    out.sort();
    out
}

/// One rebuild of what the edit marked, measured: the faults it shows, and - given what stood before (`was`) - every
/// element that did not move and was renamed.
fn rebuild(o: &mut Opened, was: Option<&Marks>) -> Vec<String> {
    o.project.snap_rebinds.store(0, Ordering::Relaxed);
    let shapes = std::mem::take(&mut o.shapes);
    let (report, shapes) = crate::regenerate_dirty_with_shapes(&mut o.project, shapes);
    o.shapes = shapes;
    let mut out = new_reds(o);
    let snaps = o.project.snap_rebinds.load(Ordering::Relaxed);
    if snaps > 0 {
        out.push(format!("{snaps} references found by place, not by name"));
    }
    out.extend(report.rebinds.iter().map(|r| format!("node {} rebound in body {}: {}", r.node, r.body, r.what)));
    if let Some(was) = was {
        out.extend(moved_names(Change { was, now: &Marks::of(&o.project) }, &o.project));
    }
    out
}

/// THE EDIT `e` BY `factor` AND BACK: the faults after the edit, then the faults after it is undone, where every name
/// has to be the one the document opened with.
pub fn edit_and_back(o: &mut Opened, e: &Edit, factor: f64) -> Vec<String> {
    let value = match e {
        Edit::Feature { value, .. } | Edit::Param { value, .. } | Edit::Sketch { value, .. } => *value * factor,
    };
    let says = format!("{} -> {}", e.says(&o.project), qymcad_core::expr::fmt_num(value));
    let base = o.marks.clone();
    let mut out = Vec::new();
    let before = o.project.clone();
    apply(&mut o.project, e, value);
    let edited = rebuild(o, Some(&base));
    out.extend(edited.into_iter().map(|f| format!("{says}: {f}")));
    // undone, every name is to be the one the document opened with: what the edit renamed is renamed back, and that is
    // told once, by the edit
    undo(&mut o.project, before);
    let back = rebuild(o, None);
    let unlike = renamed(Change { was: &base, now: &Marks::of(&o.project) }, &o.project);
    if !back.is_empty() || !unlike.is_empty() {
        let doc = Doc { name: String::new(), path: o.path.clone() };
        *o = open(&doc);
    }
    out.extend(back.into_iter().map(|f| format!("{says}, undone: {f}")));
    out.extend(unlike.into_iter().map(|f| format!("{says}, undone, not as opened: {f}")));
    out
}
