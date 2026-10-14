//! A COMMAND APPLIED: the feature it makes or edits.

use super::*;

/// Apply the SWEEP: the profile (`sweep_prof_sid`) along the path (`sweep_path_sid`). Both sketches are
/// entered into the timeline if they are not there yet, then the Sweep node follows. The contours come
/// from the selection in the bar (`sweep_prof_cid` and `sweep_path_cid`; 0 means the first suitable one
/// is taken automatically).
pub fn apply_sweep_cmd(feat: qymcad_ui_state::FeatTarget, project: &mut Project, status: &mut String, sweep: qymcad_ui_state::SweepParams) -> Option<Id> {
    if sweep.prof_sid == 0 || sweep.path_sid == 0 {
        *status = qymcad_i18n::tr("msg-need-profile-path");
        return None;
    }
    if sweep.prof_sid == sweep.path_sid {
        *status = qymcad_i18n::tr("msg-profile-path-differ");
        return None;
    }
    let prof_name = project.sketches.iter().find(|s| s.id == sweep.prof_sid).map(|s| qymcad_i18n::name(&s.name)).unwrap_or_default();
    let path_name = project.sketches.iter().find(|s| s.id == sweep.path_sid).map(|s| qymcad_i18n::name(&s.name)).unwrap_or_default();
    project.add_sketch_node(sweep.prof_sid, prof_name);
    project.add_sketch_node(sweep.path_sid, path_name);
    // the profile contours and the boolean go IN ONE node (see the revolve: the reason is the same).
    let profiles: Vec<Id> = if sweep.prof_cid != 0 { vec![sweep.prof_cid] } else { Vec::new() };
    // a sweep carries AN OPERATION: add (0) becomes a pad, plus cut (2) and intersect (3)
    let occt = match feat.op {
        2 => 0,
        3 => 2,
        _ => 1,
    };
    let part = project.sketch_owner(sweep.prof_sid).and_then(|o| project.active_body(o)).unwrap_or(0);
    let (src, op) = if part != 0 && matches!(feat.op, 1..=3) { (part, occt) } else { (0, 1) };
    let body = project.add_sweep_multi_op(sweep.prof_sid, profiles, sweep.path_sid, sweep.path_cid, src, op);
    (body != 0).then(|| if src == 0 { project.finish_base_body(body, 1) } else { body })
}

/// Apply the trim: the sheet with the "keep" point plus the tool.
pub fn apply_trim_cmd(project: &mut Project, status: &mut String, trim: &qymcad_ui_state::TrimTool) -> Option<Id> {
    let Some((src, keep)) = trim.keep else {
        *status = qymcad_i18n::tr("msg-trim-pick-sheet");
        return None;
    };
    let Some(tool) = trim.tool else {
        *status = qymcad_i18n::tr("msg-trim-pick-tool");
        return None;
    };
    Some(project.add_trim(src, tool, keep))
}

/// Update an existing DATUM with the parameters of the command (reopened by a double click): it edits
/// `project.planes`, `points` and `axes` plus the feature dims (the offset and the coordinates stay
/// parametric). Resolving happens in `qymcad_ui_state::regenerate_all` after the apply.
pub fn update_datum_feat(
    armed: &qymcad_ui_state::Armed,
    cmd: &qymcad_ui_state::FeatCommand,
    datum: &qymcad_ui_state::DatumCommand,
    edges: &qymcad_ui_state::EdgeCache,
    project: &mut Project,
    fid: Id,
) -> Option<Id> {
    use qymcad_core::feature::SketchPlane;
    use qymcad_core::model::{AxisDef, PlaneDef};
    match armed.cmd_kind() {
        20 => {
            let dist = qymcad_ui_state::cmd_val(cmd, "dist");
            let pi = project.planes.iter().position(|p| p.id == fid)?;
            match datum.plane_pick {
                Some(SketchPlane::World(bp)) => project.planes[pi].def = PlaneDef::OffsetBase { base: bp, dist },
                Some(SketchPlane::Face(body, key)) => project.planes[pi].def = PlaneDef::OffsetFace { body, face: key, dist },
                Some(SketchPlane::Datum(did)) => project.planes[pi].def = PlaneDef::OffsetPlane { plane: did, dist },
                _ => match &mut project.planes[pi].def {
                    PlaneDef::OffsetBase { dist: d, .. } | PlaneDef::OffsetFace { dist: d, .. } | PlaneDef::OffsetPlane { dist: d, .. } => *d = dist,
                    PlaneDef::Manual | PlaneDef::FaceGone | PlaneDef::PlaneGone => {}
                },
            }
            turn_datum_plane(cmd, project, fid);
            store_cmd_exprs(cmd, project, fid); // the offset, the tilts and the turn stay parametric
            Some(fid)
        }
        21 => {
            use qymcad_core::model::PointDef;
            let pi = project.datum_points.iter().position(|p| p.id == fid)?;
            if datum.pt_mode == 1 {
                // "at a vertex" mode: picking another vertex makes a new associative reference; otherwise
                // the previous one IS KEPT
                if let Some((body, edge, end, at)) = datum.pt_vert {
                    project.datum_points[pi].def = PointDef::AtVertex { body, edge, end };
                    project.datum_points[pi].at = at;
                }
                return Some(fid);
            }
            let at = [qymcad_ui_state::cmd_val(cmd, "x"), qymcad_ui_state::cmd_val(cmd, "y"), qymcad_ui_state::cmd_val(cmd, "z")];
            project.datum_points[pi].at = at;
            project.datum_points[pi].def = PointDef::Manual; // back to hand-typed coordinates
            store_cmd_exprs(cmd, project, fid); // x, y and z stay parametric
            Some(fid)
        }
        22 => {
            let ai = project.datum_axes.iter().position(|a| a.id == fid)?;
            match datum.axis_mode {
                1 => {
                    // editing the coordinates by hand IS A CHANGE OF DEFINITION to a manual one rather than
                    // a note written beside the parametric definition (that would create a second truth).
                    let o = [qymcad_ui_state::cmd_val(cmd, "ox"), qymcad_ui_state::cmd_val(cmd, "oy"), qymcad_ui_state::cmd_val(cmd, "oz")];
                    let dv = [qymcad_ui_state::cmd_val(cmd, "dx"), qymcad_ui_state::cmd_val(cmd, "dy"), qymcad_ui_state::cmd_val(cmd, "dz")];
                    project.datum_axes[ai].set_manual(o, dv);
                }
                2 => {
                    if datum.axis_pts.len() == 2 {
                        let (p0, p1) = (datum.axis_pts[0], datum.axis_pts[1]);
                        if p0.0 != 0 && p1.0 != 0 {
                            project.datum_axes[ai].def = AxisDef::TwoPoints { a: p0.0, b: p1.0 };
                        } else {
                            let d = [p1.1[0] - p0.1[0], p1.1[1] - p0.1[1], p1.1[2] - p0.1[2]];
                            project.datum_axes[ai].set_manual(p0.1, d);
                        }
                    }
                }
                // mode 0: a new reference (a click) replaces the definition; without a re-pick the existing
                // definition IS KEPT
                _ => match datum.axis_hit {
                    Some(qymcad_ui_state::AxisHit::Edge(i)) => {
                        if let Some(&(body, edge, _)) = edges.axes.get(i) {
                            project.datum_axes[ai].def = AxisDef::FromEdge { body, edge };
                        }
                    }
                    Some(qymcad_ui_state::AxisHit::Face(body, fid2)) => project.datum_axes[ai].def = AxisDef::FromFace { body, face: fid2 },
                    _ => {} // nothing was re-picked, so the definition stays as it is
                },
            }
            Some(fid)
        }
        _ => None,
    }
}

/// A hole: a cylinder cut at the centre of the picked face, perpendicular to it.
pub fn apply_hole_cmd(cmd: &qymcad_ui_state::FeatCommand, hole: qymcad_ui_state::HoleCommand, project: &mut Project, sel: qymcad_ui_state::Sel, status: &mut String) -> Option<Id> {
    // the kind plus the diameter and depth of the recess (for a counterbore or a countersink)
    let (dia2, depth2) = if hole.kind != 0 { (qymcad_ui_state::cmd_val(cmd, "dia2"), qymcad_ui_state::cmd_val(cmd, "depth2")) } else { (0.0, 0.0) };
    // "by sketch" mode drills many holes into the picked body, at the isolated points of a sketch
    if hole.mode == 1 {
        let Some(sid) = hole.sketch else {
            *status = qymcad_i18n::tr("msg-pick-sketch-points");
            return None;
        };
        let Some(src) = qymcad_ui_state::selected_body(project, &sel) else {
            *status = qymcad_i18n::tr("msg-pick-body-drill");
            return None;
        };
        if project.sketch_isolated_points(sid).is_empty() {
            *status = qymcad_i18n::tr("msg-no-isolated-points");
            return None;
        }
        let body = project.add_hole_from_sketch(
            src,
            sid,
            qymcad_core::model::HoleTool { kind: hole.kind, diameter: qymcad_ui_state::cmd_val(cmd, "diameter"), depth: qymcad_ui_state::cmd_val(cmd, "depth"), dia2, depth2 },
            hole.flip,
        );
        store_cmd_exprs(cmd, project, body);
        return Some(body);
    }
    let qymcad_ui_state::Sel::Face(mi, fi) = sel else {
        *status = qymcad_i18n::tr("msg-pick-face-centre");
        return None;
    };
    let src = project.mesh_id(mi)?;
    let face = project.bodies.get(mi).and_then(|b| b.faces.get(fi))?;
    // the face is referred to by its persistent id, so the hole holds on to it through a rebuild
    let key = qymcad_core::feature::FaceKey { index: fi as u32, centroid: [face.centroid.x, face.centroid.y, face.centroid.z], normal: face.normal, id: face.id };
    let at = hole_centre(key.centroid, key.normal, qymcad_ui_state::cmd_val(cmd, "off_u"), qymcad_ui_state::cmd_val(cmd, "off_v"));
    let body = project.add_hole_at(
        src,
        key,
        at,
        qymcad_core::model::HoleTool { kind: hole.kind, diameter: qymcad_ui_state::cmd_val(cmd, "diameter"), depth: qymcad_ui_state::cmd_val(cmd, "depth"), dia2, depth2 },
    );
    store_cmd_exprs(cmd, project, body);
    Some(body)
}

/// Apply the thread: `add_thread` on the picked rim, plus storing the parameter expressions.
pub fn apply_thread_cmd(cmd: &qymcad_ui_state::FeatCommand, project: &mut Project, status: &mut String, thread: qymcad_ui_state::ThreadParams) -> Option<Id> {
    let src = thread.src?;
    if thread.edge == 0 {
        *status = qymcad_i18n::tr("msg-pick-cyl-first");
        return None;
    }
    // the input is a standard plus a size, as in a professional CAD; the geometry (the diameters, the
    // depth, the profile) is computed by the core from the formulas of the standard rather than
    // estimated by eye.
    let body = if thread.auger {
        let spec = qymcad_core::thread::AugerSpec {
            shaft_d: thread.radius * 2.0, // taken from the actual geometry; the regen refines it
            outer_d: qymcad_ui_state::cmd_val(cmd, "outer"),
            pitch: qymcad_ui_state::cmd_val(cmd, "pitch"),
            thickness: qymcad_ui_state::cmd_val(cmd, "thickness"),
            starts: thread.starts.max(1),
            left: thread.left,
            edge_r: qymcad_ui_state::cmd_val(cmd, "edge_r"),
        };
        project.add_auger(src, thread.edge, spec, qymcad_ui_state::cmd_val(cmd, "length"), qymcad_ui_state::cmd_val(cmd, "lead_in"), qymcad_ui_state::cmd_val(cmd, "lead_out"))
    } else {
        project.add_thread(src, thread.edge, thread_spec(cmd, thread), qymcad_ui_state::cmd_val(cmd, "length"), qymcad_ui_state::cmd_val(cmd, "lead_in"), qymcad_ui_state::cmd_val(cmd, "lead_out"))
    };
    store_cmd_exprs(cmd, project, body);
    Some(body)
}

/// APPLY THE RECOGNITION: the mesh piece the tool is aimed at becomes a body - the exact surfaces found on it, or,
/// where a person asks for it, a polyhedron of its flat faces. The tolerance multiplies the one the mesh itself
/// gives: 1 suits a mesh from a CAD, a coarser mesh needs more.
///
/// Both used to be buttons in the properties of a node that only shows after stepping into the part - that is, a
/// path nobody finds. Reported behaviour: an STL is imported, the part is stepped into, and there is no way at all
/// to turn the mesh into an object.
pub fn apply_recognise_cmd(pc: &mut qymcad_ui_state::PartCtx) -> Option<Id> {
    let Some(src) = pc.recognise.src else {
        *pc.status = qymcad_i18n::tr("msg-recognise");
        return None;
    };
    let simplify = qymcad_ui_state::cmd_val(pc.cmd, "simplify");
    let body = if pc.feat.op == 1 {
        pc.project.add_mesh_solid(src, simplify)
    } else {
        pc.project.add_mesh_recognised(src, qymcad_ui_state::cmd_val(pc.cmd, "tol"), qymcad_ui_state::cmd_val(pc.cmd, "sharp"), simplify)
    };
    Some(body)
}

/// Apply the stitch: two or more sheets.
pub fn apply_stitch_cmd(cmd: &qymcad_ui_state::FeatCommand, project: &mut Project, status: &mut String, stitch_parts: &[Id]) -> Option<Id> {
    if stitch_parts.len() < 2 {
        *status = qymcad_i18n::tr("msg-stitch-needs-two");
        return None;
    }
    let body = project.add_stitch(stitch_parts.to_vec(), qymcad_ui_state::cmd_val(cmd, "tol"));
    store_cmd_exprs(cmd, project, body); // the tolerance stays parametric, like every number of a feature
    Some(body)
}

/// Apply the active datum command (Enter). Parametric: the offset and the coordinates are expressions
/// stored as feature dimensions.
pub fn apply_datum_cmd(
    armed: &qymcad_ui_state::Armed,
    cmd: &qymcad_ui_state::FeatCommand,
    datum: &qymcad_ui_state::DatumCommand,
    edges: &qymcad_ui_state::EdgeCache,
    project: &mut Project,
    status: &mut String,
) -> Option<Id> {
    use qymcad_core::feature::SketchPlane;
    match armed.cmd_kind() {
        20 => {
            let dist = qymcad_ui_state::cmd_val(cmd, "dist");
            let id = match datum.plane_pick {
                Some(SketchPlane::World(bp)) => project.add_offset_plane(bp, dist),
                Some(SketchPlane::Face(body, key)) => project.add_plane_from_face(body, key, dist),
                Some(SketchPlane::Datum(did)) => project.add_offset_from_plane(did, dist), // parametric
                None => {
                    *status = qymcad_i18n::tr("msg-click-base-plane");
                    return None;
                }
            };
            turn_datum_plane(cmd, project, id);
            store_cmd_exprs(cmd, project, id); // the offset, the tilts and the turn stay parametric in all three cases
            Some(id)
        }
        21 => {
            // "at a vertex" gives an associative point that travels with the vertex; otherwise the
            // coordinates, which stay parametric
            if datum.pt_mode == 1 {
                let Some((body, edge, end, at)) = datum.pt_vert else {
                    *status = qymcad_i18n::tr("msg-click-vertex");
                    return None;
                };
                return Some(project.add_point_at_vertex(at, body, edge, end));
            }
            let at = [qymcad_ui_state::cmd_val(cmd, "x"), qymcad_ui_state::cmd_val(cmd, "y"), qymcad_ui_state::cmd_val(cmd, "z")];
            let id = project.add_point_at(at);
            store_cmd_exprs(cmd, project, id); // x, y and z stay parametric
            Some(id)
        }
        22 => {
            // mode 2, "two points": if BOTH are datum points the axis is a parametric TwoPoints;
            // otherwise it is a manual one built from the coordinates
            if datum.axis_mode == 2 {
                if datum.axis_pts.len() < 2 {
                    *status = qymcad_i18n::tr("msg-click-two-points");
                    return None;
                }
                let (p0, p1) = (datum.axis_pts[0], datum.axis_pts[1]);
                if p0.0 != 0 && p1.0 != 0 {
                    return Some(project.add_axis_two_points(p0.0, p1.0)); // associative to the points
                }
                let d = [p1.1[0] - p0.1[0], p1.1[1] - p0.1[1], p1.1[2] - p0.1[2]];
                if d[0].abs() + d[1].abs() + d[2].abs() < 1e-9 {
                    *status = qymcad_i18n::tr("msg-points-coincide");
                    return None;
                }
                return Some(project.add_axis_manual(p0.1, d));
            }
            // mode 0, "by an edge or a face", gives an ASSOCIATIVE axis that travels with its source
            if datum.axis_mode == 0 {
                return match datum.axis_hit {
                    Some(qymcad_ui_state::AxisHit::Edge(i)) => edges.axes.get(i).map(|&(body, edge, _)| project.add_axis_from_edge(body, edge)),
                    Some(qymcad_ui_state::AxisHit::Face(body, fid)) => Some(project.add_axis_from_face(body, fid)),
                    Some(qymcad_ui_state::AxisHit::Datum(_)) | None => match datum.axis_ref {
                        Some((o, d)) => Some(project.add_axis_manual(o, d)),
                        None => {
                            *status = qymcad_i18n::tr("msg-click-edge-axis");
                            None
                        }
                    },
                };
            }
            let (o, d) = (
                [qymcad_ui_state::cmd_val(cmd, "ox"), qymcad_ui_state::cmd_val(cmd, "oy"), qymcad_ui_state::cmd_val(cmd, "oz")],
                [qymcad_ui_state::cmd_val(cmd, "dx"), qymcad_ui_state::cmd_val(cmd, "dy"), qymcad_ui_state::cmd_val(cmd, "dz")],
            );
            if d[0].abs() + d[1].abs() + d[2].abs() < 1e-9 {
                *status = qymcad_i18n::tr("msg-axis-not-set");
                return None;
            }
            Some(project.add_axis_manual(o, d))
        }
        _ => None,
    }
}

/// A fillet (4) or a chamfer (5) on the picked edges of the picked body (an empty pick means every edge).
pub fn apply_edge_cmd(pc: &mut qymcad_ui_state::PartCtx, cmd: u8) -> Option<Id> {
    // the target of a fillet or a chamfer is the body whose PERSISTENT edge ids actually sit in the edge
    // selection (`pc.edges.body`, kept in step by `refresh_edges`) rather than `qymcad_ui_state::selected_body()`:
    // otherwise, if `pc.sel` moved to another body between picking the edges and pressing Enter, the
    // chamfer would go to ids belonging to someone else or to none at all.
    // A part is one body, so when neither edges nor a body are picked, THE SINGLE body of the part is
    // taken (`active_body`), and the tool works straight away - a chamfer on EVERY edge, for instance -
    // without a "pick a body" step.
    let ctx = qymcad_ui_state::current_ctx_id(pc.active_path, pc.project);
    // the fallback through `qymcad_ui_state::current_body()` sees the body of the active part even WITHOUT entering it
    let Some(body) = pc
        .edges
        .body
        .or_else(|| qymcad_ui_state::selected_body(pc.project, pc.sel))
        .or_else(|| qymcad_ui_state::current_body(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: pc.set, scheme: pc.scheme, project: pc.project, active_path: pc.active_path }))
    else {
        *pc.status = qymcad_i18n::tr("msg-no-body");
        return None;
    };
    // a hard gate: the body is alive and not consumed by a chain, or it would be a ghost branch...
    if pc.project.consumed_bodies().contains(&body) {
        *pc.status = qymcad_i18n::tr("msg-body-consumed");
        return None;
    }
    // ...and belongs to the current context (a body of a neighbouring part must not be filleted from
    // inside this one).
    if pc.project.body_owner(body).is_some_and(|o| !pc.project.component_is_within(o, ctx)) {
        *pc.status = qymcad_i18n::tr("msg-edge-other-part");
        return None;
    }
    let edges: Vec<u32> = pc.gsel.edges.iter().copied().collect();
    // IT IS RECORDED THE WAY IT WAS PICKED. If there is a description ("every edge of this face", "every
    // edge parallel to this one"), the description goes in: it survives an edit that adds elements.
    // Otherwise the list of picked ids goes in.
    let described = pc.gsel.described_ref();
    let last = if cmd == 4 {
        let r = qymcad_ui_state::cmd_val(pc.cmd, "radius");
        // THE "VERTEX -> RADIUS" TABLE. It works with a description and with a list alike: the radius is
        // set at a point rather than along an edge, so it needs no direction of an edge.
        let at = fillet_vertex_table(pc.cmd);
        if !at.is_empty() {
            let q = pc.gsel.recorded(&edges);
            pc.project.add_fillet_at_vertices(body, r, q, at)
        } else if let Some(q) = described {
            pc.project.add_fillet_ref(body, r, q)
        } else {
            pc.project.add_fillet(body, r, edges)
        }
    } else {
        use qymcad_core::feature::ChamferMode;
        // the asymmetric modes (two distances, or a leg plus an angle) work only on picked edges;
        // otherwise it is symmetric
        if pc.chamfer.mode != ChamferMode::Symmetric && !edges.is_empty() {
            pc.project.add_chamfer_ex(
                body,
                qymcad_ui_state::cmd_val(pc.cmd, "dist"),
                qymcad_core::model::ChamferShape { mode: pc.chamfer.mode, d2: qymcad_ui_state::cmd_val(pc.cmd, "d2"), flip: pc.chamfer.flip, ref_face: pc.chamfer.ref_face },
                edges,
            )
        } else {
            match described {
                Some(q) => pc.project.add_chamfer_ref(body, qymcad_ui_state::cmd_val(pc.cmd, "dist"), q),
                None => pc.project.add_chamfer(body, qymcad_ui_state::cmd_val(pc.cmd, "dist"), edges),
            }
        }
    };
    store_cmd_exprs(pc.cmd, pc.project, last); // radius, radius2, dist and d2 stay parametric
    Some(last)
}

/// Apply the LOFT: a body through a set of sketch sections (`loft_sids`, two or more). Every section is
/// entered into the timeline if it is not there yet, then the Loft node follows, with the picked contours
/// (`loft_cids`) and the kind of faces (`loft_ruled`).
pub fn apply_loft_cmd(pc: &mut qymcad_ui_state::PartCtx) -> Option<Id> {
    if pc.loft.sids.len() < 2 {
        *pc.status = qymcad_i18n::tr("msg-loft-needs-two");
        return None;
    }
    // A cut, a pad or an intersection through sections: the target is the active body of the context.
    // "Surface" is a fourth kind of result rather than a separate tool. The question is the same one -
    // what should come out - and splitting it across two buttons would mean asking for a tool to be
    // chosen before that question has been answered.
    let surface = pc.loft.result == 4;
    let (mut src, mut op): (Id, u8) = (0, 0);
    if pc.loft.result != 0 && !surface {
        match qymcad_ui_state::current_body(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: pc.set, scheme: pc.scheme, project: pc.project, active_path: pc.active_path }) {
            Some(b) => {
                src = b;
                op = pc.loft.result - 1; // 1 becomes a cut (0), 2 a union (1), 3 an intersection (2)
            }
            None => {
                *pc.status = qymcad_i18n::tr("msg-loft-no-body");
            }
        }
    }
    let (sids, cids) = (pc.loft.sids.clone(), pc.loft.cids.clone());
    for &sid in &sids {
        let name = pc.project.sketches.iter().find(|s| s.id == sid).map(|s| qymcad_i18n::name(&s.name)).unwrap_or_default();
        pc.project.add_sketch_node(sid, name);
    }
    let body = pc.project.add_loft(sids, cids, pc.loft.ruled, src, op, surface);
    if body == 0 {
        return None;
    }
    // a SOLID loft (src == 0) is a material base and goes into the single body of the part; a cut loft
    // (src != 0) is already a boolean
    Some(if src == 0 { pc.project.finish_base_body(body, 1) } else { body })
}

/// Apply the removal of faces.
///
/// APPLY THE FACE COPY: the faces are recorded AS A QUERY - a description if there is one, otherwise a
/// list of picks. A copy must travel with its base exactly as everything else does.
/// APPLY THE OFFSET: the picked faces, as a description if one was given, and the distance of the field.
pub fn apply_offset_surface_cmd(pc: &mut qymcad_ui_state::PartCtx) -> Option<Id> {
    let src = pc
        .gsel
        .faces_body
        .or_else(|| qymcad_ui_state::op_target_body(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: pc.set, scheme: pc.scheme, project: pc.project, active_path: pc.active_path }, *pc.sel))?;
    if pc.gsel.faces.is_empty() {
        *pc.status = qymcad_i18n::tr("msg-offset-surface");
        return None;
    }
    let picks: Vec<u32> = pc.gsel.faces.iter().copied().collect();
    let q = pc.gsel.recorded(&picks);
    let body = pc.project.add_offset_surface(src, q, qymcad_ui_state::cmd_val(pc.cmd, "dist"));
    store_cmd_exprs(pc.cmd, pc.project, body);
    Some(body)
}

pub fn apply_face_copy_cmd(pc: &mut qymcad_ui_state::PartCtx) -> Option<Id> {
    let src = pc
        .gsel
        .faces_body
        .or_else(|| qymcad_ui_state::op_target_body(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: pc.set, scheme: pc.scheme, project: pc.project, active_path: pc.active_path }, *pc.sel))?;
    if pc.gsel.faces.is_empty() {
        *pc.status = qymcad_i18n::tr("msg-faces-not-found");
        return None;
    }
    let picks: Vec<u32> = pc.gsel.faces.iter().copied().collect();
    let q = pc.gsel.recorded(&picks);
    Some(pc.project.add_face_copy(src, q))
}

/// WHETHER THE PICKED EDGES BOUND A PATCH: two or more, or one closed edge - the rim of a hole is a whole circle, and
/// the patch over it a disc, as in the professional systems. Reported behaviour: the rule for straight edges refused
/// the circle too.
pub(crate) fn patch_boundary(project: &Project, body: Id, gsel: &qymcad_ui_state::GeomSelection) -> bool {
    let Some(es) = project.regen_edges.get(&body) else { return false };
    let picked: Vec<&qymcad_core::geom::MeshEdge> = gsel.edges.iter().filter_map(|id| es.iter().find(|e| e.id == *id)).collect();
    if picked.is_empty() || picked.len() != gsel.edges.len() {
        return false;
    }
    // A BOUNDARY CLOSES: a ring (a circle, an ellipse) closes on itself, and every end of an open edge meets exactly one
    // other end. An edge picked past the loop - one more than the boundary needs - leaves ends that meet nothing, and
    // a surface spans no such thing: the node came in red, the kernel saying so after Enter.
    let key = |p: [f64; 3]| p.map(|c| (c * 1e4).round() as i64);
    let mut ends: std::collections::HashMap<[i64; 3], usize> = std::collections::HashMap::new();
    for e in &picked {
        let d = [e.a[0] - e.b[0], e.a[1] - e.b[1], e.a[2] - e.b[2]];
        if (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt() < 1e-6 {
            continue; // a ring
        }
        *ends.entry(key(e.a)).or_default() += 1;
        *ends.entry(key(e.b)).or_default() += 1;
    }
    ends.values().all(|&n| n == 2)
}

/// APPLY THE PATCH: the edges go in as a query (a description if there is one), otherwise as a list of
/// picks.
pub fn apply_patch_cmd(pc: &mut qymcad_ui_state::PartCtx) -> Option<Id> {
    let src = pc
        .edges
        .body
        .or_else(|| qymcad_ui_state::op_target_body(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: pc.set, scheme: pc.scheme, project: pc.project, active_path: pc.active_path }, *pc.sel))?;
    if !patch_boundary(pc.project, src, pc.gsel) {
        *pc.status = qymcad_i18n::tr("msg-patch-needs-edges");
        return None;
    }
    let picks: Vec<u32> = pc.gsel.edges.iter().copied().collect();
    let q = pc.gsel.recorded(&picks);
    Some(pc.project.add_patch(src, q, pc.opts.patch_tangent))
}

/// Apply the cut: the plane (as a reference - a datum or a world one) plus the offset. The number of
/// pieces is counted BEFORE the feature is created: the timeline must create exactly as many bodies as
/// will come out.
pub fn apply_split_cmd(pc: &mut qymcad_ui_state::PartCtx) -> Option<Id> {
    let src = qymcad_ui_state::op_target_body(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: pc.set, scheme: pc.scheme, project: pc.project, active_path: pc.active_path }, *pc.sel)?;
    if pc.split.plane.is_none() {
        *pc.status = qymcad_i18n::tr("msg-click-cut-plane");
        return None;
    }
    let Some(pieces) = split_piece_count(pc, src).filter(|&n| n >= 2) else {
        *pc.status = qymcad_i18n::tr("msg-plane-cuts-nothing");
        return None;
    };
    let sp = pc.split.plane?;
    let (plane, datum, face) = qymcad_ui_state::resolve_mirror_plane(sp); // the same reference the mirror uses
    let offset = qymcad_ui_state::cmd_val(pc.cmd, "offset");
    let parts = pc.project.add_split_body(src, plane, datum, offset, pieces);
    let first = *parts.first()?;
    if let Some((b, k)) = face {
        pc.project.set_op_face(first, b, k);
    }
    store_cmd_exprs(pc.cmd, pc.project, first); // the offset stays parametric
    *pc.status = qymcad_i18n::tr1("cmd-split-done", "n", &pieces.to_string());
    Some(first)
}

/// The cutting plane in THE LOCAL FRAME of the body (where its B-rep lives), with the offset already
/// applied. The pick gives world coordinates, and without the conversion a cut inside an assembly would
/// drift by the transform of the component.
pub fn split_plane_local(pc: &mut qymcad_ui_state::PartCtx, src: Id) -> Option<([f64; 3], [f64; 3])> {
    use qymcad_core::feature::{apply12, apply12_dir, mat_inv12};
    let sp = pc.split.plane?;
    let (o, n) = qymcad_ui_state::mirror_plane_world(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: pc.set, scheme: pc.scheme, project: pc.project, active_path: pc.active_path }, &sp)?;
    let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    if l < 1e-9 {
        return None;
    }
    let u = [n[0] / l, n[1] / l, n[2] / l];
    let d = qymcad_ui_state::cmd_val(pc.cmd, "offset");
    let ow = [o[0] + u[0] * d, o[1] + u[1] * d, o[2] + u[2] * d];
    let inv = mat_inv12(&pc.project.body_display_transform(src, qymcad_ui_state::current_ctx_id(pc.active_path, pc.project)));
    Some((apply12(&inv, ow), apply12_dir(&inv, u)))
}

/// How many pieces a cutting plane yields - counted by the LIVE B-rep. It cannot be derived from the
/// plane itself: one plane cuts a U-shaped part into three, and "always two halves" would lose a piece.
/// `None` means the plane does not cut the body at all.
pub fn split_piece_count(pc: &mut qymcad_ui_state::PartCtx, src: Id) -> Option<usize> {
    let (o, n) = split_plane_local(pc, src)?;
    qymcad_ui_state::ensure_brep(&mut pc.rebuild()); // there is nothing to cut with while there is no live B-rep
    qymcad_kernel::ask::split_pieces(pc.live.shapes.get(&src)?, qymcad_core::feature::PlaneAt { origin: o, normal: n })
}

/// Apply the thickening: one selected face plus a thickness.
pub fn apply_thicken_cmd(pc: &mut qymcad_ui_state::PartCtx) -> Option<Id> {
    // THE BODY IS TAKEN FROM THE FACE THAT WAS CLICKED rather than from the selection in the tree. Now
    // that surfaces exist, a sheet lives in the scene alongside the part, and it is the sheet that gets
    // thickened: with the part selected in the tree, the face of the sheet would be looked for on the
    // part and not found.
    let src = pc
        .gsel
        .faces_body
        .or_else(|| qymcad_ui_state::op_target_body(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: pc.set, scheme: pc.scheme, project: pc.project, active_path: pc.active_path }, *pc.sel))?;
    let Some(fid) = pc.gsel.faces.iter().copied().next() else {
        *pc.status = qymcad_i18n::tr("msg-click-face-thicken");
        return None;
    };
    let t = qymcad_ui_state::cmd_val(pc.cmd, "thickness");
    if t.abs() < 1e-9 {
        *pc.status = qymcad_i18n::tr("msg-zero-thickness");
        return None;
    }
    let body = pc.project.add_thicken(src, fid, t);
    store_cmd_exprs(pc.cmd, pc.project, body); // the thickness stays parametric
    Some(body)
}

/// Apply the division of faces.
pub fn apply_split_face_cmd(pc: &mut qymcad_ui_state::PartCtx) -> Option<Id> {
    let src = qymcad_ui_state::op_target_body(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: pc.set, scheme: pc.scheme, project: pc.project, active_path: pc.active_path }, *pc.sel)?;
    if pc.split.plane.is_none() {
        *pc.status = qymcad_i18n::tr("msg-click-plane");
        return None;
    }
    // WHAT WILL COME OUT IS CHECKED BEFORE THE FEATURE IS CREATED: a plane that misses the body divides
    // nothing, and a node in the timeline that is certain to go red is of no use to anyone.
    let (o, n) = split_plane_local(pc, src)?;
    qymcad_ui_state::ensure_brep(&mut pc.rebuild());
    if !pc.live.shapes.get(&src).is_some_and(|sh| qymcad_kernel::ask::splits_faces(sh, qymcad_core::feature::PlaneAt { origin: o, normal: n })) {
        *pc.status = qymcad_i18n::tr("msg-plane-splits-nothing");
        return None;
    }
    let sp = pc.split.plane?;
    let (plane, datum, face) = qymcad_ui_state::resolve_mirror_plane(sp);
    let offset = qymcad_ui_state::cmd_val(pc.cmd, "offset");
    let body = pc.project.add_split_face(src, plane, datum, offset);
    if let Some((b, k)) = face {
        pc.project.set_op_face(body, b, k);
    }
    store_cmd_exprs(pc.cmd, pc.project, body);
    *pc.status = qymcad_i18n::tr("msg-faces-split");
    Some(body)
}

/// Create the array from the options picked by the command. Associative; the step and the angle stay
/// parametric.
pub fn apply_array_cmd(pc: &mut qymcad_ui_state::PartCtx) -> Option<Id> {
    let src = qymcad_ui_state::op_target_body(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: pc.set, scheme: pc.scheme, project: pc.project, active_path: pc.active_path }, *pc.sel)?;
    if pc.armed.cmd_kind() == 18 {
        let angle = if pc.arr.full { 360.0 } else { qymcad_ui_state::cmd_val(pc.cmd, "angle") };
        let body = pc.project.add_circular_array_axis(src, pc.arr.count.max(1), angle, pc.arr.axis);
        store_bar_counts(pc.bar_exprs, pc.project, body);
        if pc.arr.full {
            pc.project.set_feat_dim(body, "angle", String::new());
        } else {
            store_cmd_exprs(pc.cmd, pc.project, body); // the `angle` key stays parametric (the regen reads the feature dims)
        }
        Some(body)
    } else {
        let (dx, dy, dz) = qymcad_ui_state::arr_vec(pc.arr.dir, qymcad_ui_state::cmd_val(pc.cmd, "step"));
        let (dx2, dy2, dz2, c2) = if pc.arr.two {
            let (a, b, c) = qymcad_ui_state::arr_vec(pc.arr.dir2, qymcad_ui_state::cmd_val(pc.cmd, "step2"));
            (a, b, c, pc.arr.count2.max(1))
        } else {
            (0.0, 0.0, 0.0, 1)
        };
        let (dx3, dy3, dz3, c3) = if pc.arr.two && pc.arr.three {
            let (a, b, c) = qymcad_ui_state::arr_vec(pc.arr.dir3, qymcad_ui_state::cmd_val(pc.cmd, "step3"));
            (a, b, c, pc.arr.count3.max(1))
        } else {
            (0.0, 0.0, 0.0, 1)
        };
        let body = pc
            .project
            .add_linear_array_grid3(src, [ArrayAxis { d: [dx, dy, dz], count: pc.arr.count.max(1) }, ArrayAxis { d: [dx2, dy2, dz2], count: c2 }, ArrayAxis { d: [dx3, dy3, dz3], count: c3 }]);
        store_cmd_exprs(pc.cmd, pc.project, body); // the logical `step`, `step2` and `step3` so that reopening returns the TEXT of the expression
        store_bar_counts(pc.bar_exprs, pc.project, body);
        // THE TEXTS ARE TAKEN FIRST, and the document is written afterwards. Reading the command and
        // writing the document in one expression borrows the application twice at once; taken apart,
        // the order of the two is also said out loud instead of being left to the compiler.
        let (d1, d2, d3) = (pc.arr.dir, pc.arr.dir2, pc.arr.dir3);
        let (t1, t2, t3) = (cmd_txt(pc.cmd, "step"), cmd_txt(pc.cmd, "step2"), cmd_txt(pc.cmd, "step3"));
        let (two, three) = (pc.arr.two, pc.arr.three);
        qymcad_ui_state::store_arr_component(pc.project, body, ["dx", "dy", "dz"], d1, t1);
        if two {
            qymcad_ui_state::store_arr_component(pc.project, body, ["dx2", "dy2", "dz2"], d2, t2);
        }
        if two && three {
            qymcad_ui_state::store_arr_component(pc.project, body, ["dx3", "dy3", "dz3"], d3, t3);
        }
        Some(body)
    }
}

/// Create the mirror from the plane picked by the command. Associative.
pub fn apply_mirror_cmd(pc: &mut qymcad_ui_state::PartCtx) -> Option<Id> {
    let src = qymcad_ui_state::op_target_body(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: pc.set, scheme: pc.scheme, project: pc.project, active_path: pc.active_path }, *pc.sel)?;
    let Some(sp) = pc.mirror.plane else {
        *pc.status = qymcad_i18n::tr("msg-click-mirror-plane");
        return None;
    };
    let (plane, datum, face) = qymcad_ui_state::resolve_mirror_plane(sp);
    let node = pc.project.add_mirror(src, plane, pc.opts.mirror_keep, datum);
    if let Some((b, k)) = face {
        pc.project.set_op_face(node, b, k);
    }
    Some(node)
}

/// Apply the accumulated drag of the component gizmo: `transform = accumulated * start` ->
/// `set_component_transform` (cheap, with no rebuild of bodies). The start is pinned, so the motion is
/// smooth; snapping comes from the panel or from Ctrl.
pub fn apply_comp_giz(pc: &mut qymcad_ui_state::PartCtx) {
    qymcad_ui_state::begin_edit(pc.edits, pc.project, qymcad_i18n::tr("status-move-component")); // THE BOUNDARY OF AN OPERATION
    let Some((comp, _, _, _)) = pc.comp_giz.drag else { return };
    if let Some(t) = qymcad_ui_state::comp_giz_accum(pc.comp_giz, pc.set, pc.comp_giz.snap) {
        // the parts grouped with it go along (a group holds them where they stood before the hand)
        if !pc.project.move_component_by_hand(comp, t) {
            *pc.status = qymcad_i18n::tr("j-group-grounded");
        }
        qymcad_ui_state::invalidate_placement(pc.regen); // THE PLACEMENT moved; the shape of the bodies did not change
    }
    qymcad_ui_state::commit_edit(&mut pc.rebuild());
}

/// APPLY THE FACE REPLACEMENT: the faces go in as a query (a description if there is one), the surface
/// as a body.
pub fn apply_surface_replace_cmd(pc: &mut qymcad_ui_state::PartCtx) -> Option<Id> {
    let src = pc
        .gsel
        .faces_body
        .or_else(|| qymcad_ui_state::op_target_body(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: pc.set, scheme: pc.scheme, project: pc.project, active_path: pc.active_path }, *pc.sel))?;
    let surface = (*pc.repl_surface)?;
    if pc.gsel.faces.is_empty() {
        *pc.status = qymcad_i18n::tr("msg-faces-not-found");
        return None;
    }
    let picks: Vec<u32> = pc.gsel.faces.iter().copied().collect();
    // THE SHEET MUST CLOSE WHAT IS TAKEN AWAY, and that is asked of the live bodies before Enter lays a node: a sheet
    // over the top taken for the top and a side left the side open - the node came in red, "4 edges left unpaired".
    if let (Some(b), Some(sheet)) = (pc.live.shapes.get(&src), pc.live.shapes.get(&surface)) {
        if let Err(free) = qymcad_kernel::ask::sheet_closes(b, &picks, sheet) {
            *pc.status = qymcad_i18n::error_words::error_text(&qymcad_core::errors::CoreError::SurfaceDoesNotClose { free });
            return None;
        }
    }
    let q = pc.gsel.recorded(&picks);
    Some(pc.project.add_surface_replace(src, q, surface))
}

/// A shell: the picked faces become open and the body becomes hollow (a multiple selection by id, plus
/// a direction).
pub fn apply_shell_cmd(pc: &mut qymcad_ui_state::PartCtx) -> Option<Id> {
    // the source is the body the picked faces belong to; the faces are the multiple selection, by
    // persistent id. It falls back to the selection when the body of the faces is not pinned yet.
    let src = pc
        .gsel
        .faces_body
        .or_else(|| match *pc.sel {
            qymcad_ui_state::Sel::Face(mi, _) | qymcad_ui_state::Sel::Mesh(mi) => pc.project.mesh_id(mi),
            _ => qymcad_ui_state::selected_body(pc.project, pc.sel),
        })
        .or_else(|| qymcad_ui_state::current_body(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: pc.set, scheme: pc.scheme, project: pc.project, active_path: pc.active_path }))?; // a part is one body; `qymcad_ui_state::current_body` falls back without entering it
    let faces: Vec<u32> = pc.gsel.faces.iter().copied().collect();
    if faces.is_empty() {
        *pc.status = qymcad_i18n::tr("msg-click-face-open");
        return None;
    }
    let body = pc.project.add_shell_mode(src, qymcad_ui_state::cmd_val(pc.cmd, "thickness"), faces, pc.opts.shell_side);
    store_cmd_exprs(pc.cmd, pc.project, body);
    Some(body)
}

/// Apply the component array (Enter): create a new one or update the one being edited.
pub fn apply_comp_array(pc: &mut qymcad_ui_state::PartCtx) {
    // A COUNT THE BAR REFUSED is not applied with the one it held before: a row of 0 came out as a row of 3
    if qymcad_ui_state::bar_field_bad("carr_count") {
        *pc.status = qymcad_i18n::tr("sk-fix-bar-field");
        return;
    }
    // nor a step or an angle its field refused: the value the field held before would go in silently
    if let Some((label, msg)) = pc.cmd.params.iter().find_map(|p| field_value(p, &pc.project.param_map()).err().map(|m| (qymcad_i18n::tr(p.label_key()), m))) {
        *pc.status = format!("{label}: {msg}");
        return;
    }
    let kind = qymcad_ui_state::comp_array_kind(*pc.arr, pc.carr, pc.cmd);
    // a layout past the limit is refused with the limit in words, before the "one instance" reading of a zero count
    if kind.count() == 0 {
        *pc.status = qymcad_i18n::error_words::error_text(&qymcad_core::feature::pattern_too_large(&kind.counts()));
        return;
    }
    // a row of one instance is the source alone: nothing to make, and said so
    if kind.count() < 2 {
        *pc.status = qymcad_i18n::tr("msg-comp-array-one");
        return;
    }
    qymcad_ui_state::begin_edit(pc.edits, pc.project, if pc.carr.edit != 0 { qymcad_i18n::tr("status-edit-comp-array") } else { qymcad_i18n::tr("f-comp-array") });
    let ok = if pc.carr.edit != 0 { pc.project.set_comp_pattern(pc.carr.edit, kind) } else { pc.project.add_comp_pattern(pc.carr.src, kind) != 0 };
    *pc.status = if ok { qymcad_i18n::tr1("cmd-comp-array-done", "n", &pc.arr.count.max(1).to_string()) } else { qymcad_i18n::tr("msg-array-no-body") };
    *pc.carr = qymcad_ui_state::CompArrayCmd::default();
    pc.cmd.params.clear();
    qymcad_ui_state::mark_dirty_for_rebuild(&mut pc.rebuild());
    qymcad_ui_state::commit_edit(&mut pc.rebuild());
}

/// A DRAFT: tilt the picked faces of the body relative to the neutral face (`draft_neutral`) by an
/// angle.
pub fn apply_draft_cmd(pc: &mut qymcad_ui_state::PartCtx) -> Option<Id> {
    let src = pc
        .gsel
        .faces_body
        .or_else(|| match *pc.sel {
            qymcad_ui_state::Sel::Face(mi, _) | qymcad_ui_state::Sel::Mesh(mi) => pc.project.mesh_id(mi),
            _ => qymcad_ui_state::selected_body(pc.project, pc.sel),
        })
        .or_else(|| qymcad_ui_state::current_body(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: pc.set, scheme: pc.scheme, project: pc.project, active_path: pc.active_path }))?; // a part is one body; `qymcad_ui_state::current_body` falls back without entering it
    let faces: Vec<u32> = pc.gsel.faces.iter().copied().collect();
    if faces.is_empty() {
        *pc.status = qymcad_i18n::tr("msg-click-face-draft");
        return None;
    }
    if pc.draft.neutral == 0 {
        *pc.status = qymcad_i18n::tr("msg-pick-neutral");
        return None;
    }
    let body = pc.project.add_draft(src, faces, pc.draft.neutral, qymcad_ui_state::cmd_val(pc.cmd, "angle"), pc.draft.flip);
    store_cmd_exprs(pc.cmd, pc.project, body); // the angle stays parametric
    Some(body)
}

/// Extrude or revolve along the picked contours. Returns the last body created.
pub fn apply_sketch_cmd(pc: &mut qymcad_ui_state::PartCtx, cmd: u8) -> Option<Id> {
    let si = pc.cmd.sketch.filter(|&si| si < pc.project.sketches.len())?;
    let sid = pc.project.sketches[si].id;
    // THE CONTEXT: a feature built from a sketch belongs to the component that OWNS the sketch (the part)
    // rather than to the active context. Extruding from AN ASSEMBLY (or without having entered the part)
    // would make `body_parent()` create a NEW part and put the feature there while the sketch stayed in
    // the old one - a cross-component reference is forbidden, and the sketch would not extrude. The owner
    // of the sketch is made active, so the feature lands in its part and the body builds.
    if let Some(owner) = pc.project.sketch_owner(sid) {
        pc.project.set_active_component(Some(owner));
    }
    let closed = qymcad_ui_state::sketch_closed_contours(pc.project, si);
    let mut targets: Vec<Id> = closed.iter().copied().filter(|c| pc.gsel.profiles.contains(c)).collect();
    if targets.is_empty() {
        // nothing picked: the one contour of the sketch is the profile; of several, none is chosen for the person -
        // Enter with nothing taken extruded them all at once, two squares apart into a part of two pieces
        if closed.is_empty() {
            *pc.status = qymcad_i18n::tr("msg-no-closed-contour");
            return None;
        }
        if closed.len() > 1 {
            *pc.status = qymcad_i18n::tr("msg-pick-a-contour");
            return None;
        }
        targets = closed.clone();
    }
    // NESTING IS NO LONGER FILTERED. Every picked contour is A REGION OF ITS OWN (itself minus its direct
    // children), and the regions are merged by the profile fuse in the core. qymcad_ui_state::Picking the outer and the
    // middle of three nested contours gives a plate with a hole the size of the small one; picking two
    // concentric circles gives a solid disc. It works at any depth. A nested picked contour used to be
    // thrown into the `fill` of the outer one, and everything deeper was lost - the part came out solid,
    // which was reported as not being able to extrude with a hole in the middle.
    let fill: Vec<Id> = Vec::new();
    // The target body is looked for IN THE CONTEXT OF THE SKETCH OWNER (where the feature will land)
    // rather than in the navigation context of the interface: without having entered the part by a double
    // click, a cut reported the part as empty and every extrude made a NEW body instead of the single
    // body of the part.
    let owner_body = pc.project.sketch_owner(sid).and_then(|o| pc.project.active_body(o));
    // A body is REQUIRED only for A CUT or AN INTERSECTION. A pad (op 1) with no body simply creates a new
    // one, as a join does in an empty part, rather than refusing silently with "build a body first".
    let need_target = (cmd == 1 || cmd == 3) && matches!(pc.feat.op, 2 | 3);
    let target = if need_target { owner_body.unwrap_or(0) } else { 0 };
    if need_target && target == 0 {
        *pc.status = format!("{} {}", ph::WARNING, qymcad_i18n::tr("cmd-cut-needs-body"));
        return None;
    }
    let reach = qymcad_ui_state::cmd_reach(pc.cmd, *pc.feat);
    // the second side is an expression field at the geometry (`qymcad_ui_state::cmd_val("down")`), not a drag value in the bar
    let down = if pc.cmd.extent.two_sided() { qymcad_ui_state::cmd_val(pc.cmd, "down").abs() } else { 0.0 };
    let through = pc.cmd.extent.through();
    let name = qymcad_i18n::name(&pc.project.sketches[si].name);
    pc.project.add_sketch_node(sid, name);
    let mut last = 0;
    // ALL the picked contours go through ONE operation and make ONE node (the core fuses N tools into a
    // single boolean against the body of the part). No chain of Extrude plus BodyBoolean or Combine, and
    // no collapsing.
    if cmd == 1 {
        let part = owner_body.unwrap_or(0); // the body of the part that OWNS the sketch (0 means an empty part, so create one)
        let h = qymcad_ui_state::cmd_val(pc.cmd, "height").abs();
        // console diagnostics ON REQUEST (`QYM_EXTRUDE_DEBUG=1`), as with the contour analysis. It used to
        // print unconditionally, and an ordinary session poured a line into stderr on every extrude.
        if std::env::var("QYM_EXTRUDE_DEBUG").is_ok() {
            eprintln!(
                "[extrude] sid={sid} targets={targets:?} fill={fill:?} part={part} h={h} op={} reach={reach:?} down={down} through={through} active={:?}",
                pc.feat.op, pc.project.active_component
            );
        }
        last = if part == 0 {
            // AN EMPTY part gets A NEW body: an Extrude node plus `finish_base_body` - THE SAME path that
            // revolve takes (in an empty part `finish_base_body` simply returns the seed body). That makes
            // an extrude in an empty part behave like a revolve, which works, and like a sketch on an
            // existing body.
            let e = pc.project.add_extrude_multi(sid, targets.clone(), h, reach, down, fill.clone());
            if e != 0 {
                pc.project.finish_base_body(e, 1)
            } else {
                0
            }
        } else {
            // with a body present: add (0) becomes a pad (1); pad (1) stays 1; cut (2) becomes a cut (0);
            // intersect (3) becomes an intersection (2)
            let occt = match pc.feat.op {
                2 => 0,
                3 => 2,
                _ => 1,
            };
            pc.project.add_combine_multi_op(part, sid, targets.clone(), qymcad_core::model::CombineSpan { height: h, down, extent: qymcad_core::feature::Extent { through, reach }, fill: &fill }, occt)
        };
        if last != 0 {
            store_cmd_exprs(pc.cmd, pc.project, last); // the dimensions (the height) go onto the node of the operation
        } else {
            qymcad_ui_state::cmd_fail(pc.cmd_failed, pc.status, format!("{} {}", ph::WARNING, qymcad_i18n::tr("cmd-op-failed")));
        }
    } else {
        // A REVOLVE puts ALL the contours and the boolean into ONE node, exactly as an extrude does.
        //
        // There used to be a loop over the contours here: a `Revolve` per contour (which is a NEW body,
        // that is, an add) plus a `BodyBoolean` per cut. Two contours produced four timeline nodes, which
        // read as the revolve falling apart into two features that add instead of cutting; editing opened
        // only one contour, and deleting either of them took the whole chain below with it.
        let part = owner_body.unwrap_or(0);
        let occt = match pc.feat.op {
            2 => 0,
            3 => 2,
            _ => 1,
        };
        // A pad in AN EMPTY part is simply a new body: there is nothing to boolean against.
        let (rev_src, rev_op) = if part != 0 && matches!(pc.feat.op, 1..=3) { (part, occt) } else { (0, 1) };
        let body = pc.project.add_revolve_multi_op(
            sid,
            targets.clone(),
            qymcad_core::model::RevolveAxis { axis: pc.rev.axis, datum: pc.rev.axis_datum, line: pc.rev.axis_line },
            qymcad_core::model::RevolveTurn { angle: qymcad_ui_state::cmd_val(pc.cmd, "angle"), reach: qymcad_ui_state::cmd_reach(pc.cmd, *pc.feat) },
            rev_src,
            rev_op,
        );
        if body != 0 {
            store_cmd_exprs(pc.cmd, pc.project, body);
            // a new body (nothing to boolean) is merged into the single body of the part, as with an extrude
            last = if rev_src == 0 { pc.project.finish_base_body(body, 1) } else { body };
        }
    }
    (last != 0).then_some(last)
}

pub fn apply_remove_face_cmd(pc: &mut qymcad_ui_state::PartCtx) -> Option<Id> {
    let src = qymcad_ui_state::op_target_body(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: pc.set, scheme: pc.scheme, project: pc.project, active_path: pc.active_path }, *pc.sel)?;
    if pc.gsel.faces.is_empty() {
        *pc.status = qymcad_i18n::tr("msg-click-faces-remove");
        return None;
    }
    let keys: Vec<qymcad_core::feature::FaceKey> = pc
        .project
        .mesh_index(src)
        .and_then(|mi| pc.project.bodies.get(mi))
        .map(|b| {
            b.faces
                .iter()
                .filter(|f| pc.gsel.faces.contains(&f.id))
                .map(|f| qymcad_core::feature::FaceKey { index: 0, centroid: [f.centroid.x, f.centroid.y, f.centroid.z], normal: f.normal, id: f.id })
                .collect()
        })
        .unwrap_or_default();
    if keys.is_empty() {
        *pc.status = qymcad_i18n::tr("msg-faces-not-found");
        return None;
    }
    Some(pc.project.add_remove_face(src, keys))
}

/// Apply "push a face": one selected face plus an offset.
pub fn apply_push_face_cmd(pc: &mut qymcad_ui_state::PartCtx) -> Option<Id> {
    // THE BODY IS TAKEN FROM THE FACE THAT WAS CLICKED rather than from the selection in the tree. When
    // the two diverged, the search for the face key failed and the tool SILENTLY did nothing: no node, no
    // error, no message - it reads as "pressed it, and nothing happened". Thicken was mended the same
    // way.
    let src = pc
        .gsel
        .faces_body
        .or_else(|| qymcad_ui_state::op_target_body(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: pc.set, scheme: pc.scheme, project: pc.project, active_path: pc.active_path }, *pc.sel))?;
    let fid = pc.gsel.faces.iter().copied().next();
    let Some(fid) = fid else {
        *pc.status = qymcad_i18n::tr("msg-click-face-push");
        return None;
    };
    // the face key is taken from the built body: the offset is later resolved BY NAME, not by index
    let key = pc.project.mesh_index(src).and_then(|mi| pc.project.bodies.get(mi)).and_then(|b| b.faces.iter().find(|f| f.id == fid)).map(|f| qymcad_core::feature::FaceKey {
        index: 0,
        centroid: [f.centroid.x, f.centroid.y, f.centroid.z],
        normal: f.normal,
        id: f.id,
    });
    // AND IF THE FACE WAS NOT FOUND AFTER ALL, SAY SO. A silent return from here was exactly that
    // "pressed it, and nothing": silence is indistinguishable from a broken program.
    let Some(key) = key else {
        *pc.status = qymcad_i18n::tr("msg-faces-not-found");
        return None;
    };
    let dist = qymcad_ui_state::cmd_val(pc.cmd, "dist");
    if dist.abs() < 1e-9 {
        *pc.status = qymcad_i18n::tr("msg-zero-offset");
        return None;
    }
    let body = pc.project.add_push_face(src, key, dist);
    store_cmd_exprs(pc.cmd, pc.project, body); // the offset stays parametric
    Some(body)
}

/// Moved out of the frame because it is a decision about the DOCUMENT - which two bodies are combined -
/// and the frame's business is only where the click landed.
/// APPLY THE BOOLEAN whose two bodies are taken: one node, one step of undo, the tool put down.
pub fn apply_boolean(pc: &mut qymcad_ui_state::PartCtx) {
    let (Some((a, op)), Some(b)) = (pc.boolean.pick, pc.boolean.b) else { return };
    // a body taken and then taken away by Ctrl+Z is not combined: the node would only stand red
    if [a, b].iter().any(|&x| pc.project.mesh_index(x).is_none()) {
        (pc.boolean.pick, pc.boolean.b) = (None, None);
        *pc.status = qymcad_i18n::tr1("cmd-picks-gone", "ask", &qymcad_i18n::tr("tb-bool-bodies-hint"));
        return;
    }
    qymcad_ui_state::begin_edit(&mut *pc.edits, &*pc.project, qymcad_i18n::tr("f-operation"));
    let res = pc.project.add_body_boolean(a, b, op);
    (pc.boolean.pick, pc.boolean.b) = (None, None);
    qymcad_ui_state::mark_dirty_for_rebuild(&mut pc.rebuild()); // the document is marked; the scheduler does the computing
    let (project, sel, view) = (&mut *pc.project, &mut *pc.sel, &mut *pc.view);
    qymcad_ui_state::select_body(project, sel, view, res);
    *pc.status = qymcad_i18n::tr("vp-bool-created");
    qymcad_ui_state::commit_edit(&mut pc.rebuild());
}

/// CREATE A PRIMITIVE from the fields of the running command.
///
/// The closure here used to take `&Self` for one reason: it needed `cmd` while `project` was borrowed
/// mutably in the same expression, and naming the whole application was the way round it. Over a context
/// the two are separate places, so the closure takes the command alone and the document stays free.
pub fn apply_prim_cmd(pc: &mut qymcad_ui_state::PartCtx) -> Option<Id> {
    let cmd = pc.cmd.clone();
    let v = |k: &str| qymcad_ui_state::cmd_val(&cmd, k);
    let body = match pc.armed.cmd_kind() {
        10 => pc.project.add_box(v("dx"), v("dy"), v("dz")),
        11 => pc.project.add_cylinder(v("r"), v("h")),
        12 => pc.project.add_sphere(v("r")),
        13 => pc.project.add_cone(v("r1"), v("r2"), v("h")),
        14 => pc.project.add_torus(v("major"), v("minor")),
        15 => pc.project.add_prism(v("r"), pc.prim.n.max(3), v("h")),
        _ => return None,
    };
    store_cmd_exprs(&cmd, pc.project, body);
    // placement: an oriented move of the primitive into the picked frame (the base sits on the surface)
    let placed = match pc.prim.frame {
        Some(m) if !qymcad_ui_state::is_identity12(&m) => pc.project.add_move(body, m),
        _ => body,
    };
    // A PART IS ONE BODY - the primitive is merged into the single body of the part (the first one seeds it)
    Some(pc.project.finish_base_body(placed, 1))
}

/// Update existing feature `fid` with the parameters of the active command (edit mode).
pub fn update_feat(pc: &mut qymcad_ui_state::PartCtx, fid: Id) -> Option<Id> {
    use qymcad_core::feature::FeatureKind;
    // THE DATUMS edit `project.planes`, `points` and `axes` plus the feature dims rather than
    // `node.kind`, so they take a separate path
    if matches!(pc.armed.cmd_kind(), 20..=22) {
        return update_datum_feat(pc.armed, pc.cmd, pc.datum, pc.edges, pc.project, fid);
    }
    // take everything out of the command state in advance, to avoid borrowing self twice
    let (h, ang, r, dist, th, dia, dep) = (
        qymcad_ui_state::cmd_val(pc.cmd, "height"),
        qymcad_ui_state::cmd_val(pc.cmd, "angle"),
        qymcad_ui_state::cmd_val(pc.cmd, "radius"),
        qymcad_ui_state::cmd_val(pc.cmd, "dist"),
        qymcad_ui_state::cmd_val(pc.cmd, "thickness"),
        qymcad_ui_state::cmd_val(pc.cmd, "diameter"),
        qymcad_ui_state::cmd_val(pc.cmd, "depth"),
    );
    // the "vertex -> radius" table is taken BEFORE the node is edited (otherwise self is borrowed twice)
    let vtable: Vec<(qymcad_core::refs::Ref, f64)> =
        fillet_vertex_table(pc.cmd).into_iter().map(|(desc, val)| (qymcad_core::refs::Ref::one(desc, qymcad_core::refs::Fingerprint::default()), val)).collect();
    let dist_v = qymcad_ui_state::cmd_val(pc.cmd, "dist"); // "push a face": the offset, which may be an expression
    let (rec_tol, rec_sharp) = (qymcad_ui_state::cmd_val(pc.cmd, "tol"), qymcad_ui_state::cmd_val(pc.cmd, "sharp")); // the recognition's tolerance and sharp edge
    let ch_d2 = qymcad_ui_state::cmd_val(pc.cmd, "d2"); // the second leg or the angle of the chamfer, in degrees
    let (ch_mode, ch_flip) = (pc.chamfer.mode, pc.chamfer.flip); // the mode plus the side of the reference face
    let patch_tangent = pc.opts.patch_tangent; // a patch by position or smooth
    let (stitch_now, trim_now) = (pc.stitch_parts.clone(), (pc.trim.keep, pc.trim.tool)); // the sheets of a stitch; what a trim cuts, keeps and cuts with
    let surface_now = *pc.repl_surface; // the sheet a face replacement puts in
    let ch_ref_face = pc.chamfer.ref_face; // the hand-picked reference face (0 means automatic)
    let profile = pc.gsel.profiles.iter().copied().next().unwrap_or(0);
    // ALL the picked contours go into editing a multi-contour node. The edit puts them ALL into the
    // `profiles` of the node rather than a single contour.
    let (edit_contours, edit_fill): (Vec<Id>, Vec<Id>) = match pc.cmd.sketch.filter(|&si| si < pc.project.sketches.len()) {
        Some(si) => {
            // nesting is not filtered - every picked contour goes as a region of its own (see `apply`)
            (qymcad_ui_state::sketch_closed_contours(pc.project, si).into_iter().filter(|c| pc.gsel.profiles.contains(c)).collect(), Vec::new())
        }
        None => (Vec::new(), Vec::new()),
    };
    let edges: Vec<u32> = pc.gsel.edges.iter().copied().collect();
    let picked_edges = pc.gsel.recorded(&edges);
    let faces_set: Vec<u32> = pc.gsel.faces.iter().copied().collect(); // the shell: a multiple selection by id
    let shell_side = pc.opts.shell_side; // the shell: which way the wall goes
    let draft_neutral = pc.draft.neutral; // the draft: the neutral face, 0 means unset
    let draft_flip = pc.draft.flip; // the draft: the direction of the pull
                                    // the mirror: the picked plane resolves to (plane, datum, face)
    let mirror_resolved = pc.mirror.plane.map(qymcad_ui_state::resolve_mirror_plane);
    let mirror_keep = pc.opts.mirror_keep;
    // THE CUT: the new plane plus how many pieces it will yield. It is computed BEFORE the timeline is
    // borrowed; if it disagrees with the number of pieces the feature has, the edit is rejected below -
    // the bodies of the cut have already spread through the timeline (they are referred to and they are
    // visible), and their number cannot be changed in place.
    let (split_resolved, split_pieces) = if pc.armed.cmd_kind() == 27 || pc.armed.cmd_kind() == 29 {
        let n = (pc.armed.cmd_kind() == 27)
            .then(|| {
                qymcad_ui_state::op_target_body(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: pc.set, scheme: pc.scheme, project: pc.project, active_path: pc.active_path }, *pc.sel)
                    .and_then(|src| split_piece_count(pc, src))
            })
            .flatten();
        let r = pc.split.plane.map(qymcad_ui_state::resolve_mirror_plane);
        (r, n)
    } else {
        (None, None)
    };
    let split_offset = qymcad_ui_state::cmd_val(pc.cmd, "offset");
    let thickness_v = qymcad_ui_state::cmd_val(pc.cmd, "thickness"); // the thickening
                                                                     // the array: gather the vector, the angle and the count BEFORE the timeline is borrowed
    let (a_dir, a_dir2, a_dir3, a_two, a_three, a_axis, a_full) = (pc.arr.dir, pc.arr.dir2, pc.arr.dir3, pc.arr.two, pc.arr.three, pc.arr.axis, pc.arr.full);
    let (a_count, a_count2, a_count3) = (pc.arr.count.max(1), pc.arr.count2.max(1), pc.arr.count3.max(1));
    let (a_dx, a_dy, a_dz) = qymcad_ui_state::arr_vec(a_dir, qymcad_ui_state::cmd_val(pc.cmd, "step"));
    let (a_dx2, a_dy2, a_dz2) = if a_two { qymcad_ui_state::arr_vec(a_dir2, qymcad_ui_state::cmd_val(pc.cmd, "step2")) } else { (0.0, 0.0, 0.0) };
    let a_c2 = if a_two { a_count2 } else { 1 };
    let (a_dx3, a_dy3, a_dz3) = if a_two && a_three { qymcad_ui_state::arr_vec(a_dir3, qymcad_ui_state::cmd_val(pc.cmd, "step3")) } else { (0.0, 0.0, 0.0) };
    let a_c3 = if a_two && a_three { a_count3 } else { 1 };
    let a_angle = if a_full { 360.0 } else { qymcad_ui_state::cmd_val(pc.cmd, "angle") };
    let (a_step_txt, a_step2_txt, a_step3_txt) = (cmd_txt(pc.cmd, "step"), cmd_txt(pc.cmd, "step2"), cmd_txt(pc.cmd, "step3"));
    // the primitives: values under the same keys the regen uses (radii)
    let (p_dx, p_dy, p_dz) = (qymcad_ui_state::cmd_val(pc.cmd, "dx"), qymcad_ui_state::cmd_val(pc.cmd, "dy"), qymcad_ui_state::cmd_val(pc.cmd, "dz"));
    let (p_r, p_r1, p_r2, p_ph, p_major, p_minor) = (
        qymcad_ui_state::cmd_val(pc.cmd, "r"),
        qymcad_ui_state::cmd_val(pc.cmd, "r1"),
        qymcad_ui_state::cmd_val(pc.cmd, "r2"),
        qymcad_ui_state::cmd_val(pc.cmd, "h"),
        qymcad_ui_state::cmd_val(pc.cmd, "major"),
        qymcad_ui_state::cmd_val(pc.cmd, "minor"),
    );
    let p_n = pc.prim.n.max(3);
    // if ANOTHER face is picked while editing, it is updated on the feature. Only for the hole: the face
    // goes in by its persistent `FaceKey` (the shell is now a multiple selection by id - `faces_set`
    // above)
    let hole_face: Option<(qymcad_core::feature::FaceKey, [f64; 3], [f64; 3])> = match *pc.sel {
        qymcad_ui_state::Sel::Face(mi, fi) => pc.project.bodies.get(mi).and_then(|b| b.faces.get(fi)).map(|face| {
            let k = qymcad_core::feature::FaceKey { index: fi as u32, centroid: [face.centroid.x, face.centroid.y, face.centroid.z], normal: face.normal, id: face.id };
            (k, hole_centre(k.centroid, k.normal, qymcad_ui_state::cmd_val(pc.cmd, "off_u"), qymcad_ui_state::cmd_val(pc.cmd, "off_v")), k.normal)
        }),
        _ => None,
    };
    let reach = qymcad_ui_state::cmd_reach(pc.cmd, *pc.feat);
    let down = if pc.cmd.extent.two_sided() { qymcad_ui_state::cmd_val(pc.cmd, "down").abs() } else { 0.0 };
    let through = pc.cmd.extent.through();
    let occt = match pc.feat.op {
        1 => 1,
        3 => 2,
        _ => 0,
    };
    let axis = pc.rev.axis;
    let axis_datum_rev = pc.rev.axis_datum; // K4
    let axis_line_rev = pc.rev.axis_line; // 64
    let hole_kind = pc.hole.kind; // F1
    let hole_flip = pc.hole.flip; // the direction in "by sketch" mode
    let (hole_dia2, hole_depth2) = if pc.hole.kind != 0 { (qymcad_ui_state::cmd_val(pc.cmd, "dia2"), qymcad_ui_state::cmd_val(pc.cmd, "depth2")) } else { (0.0, 0.0) };
    // THE THREAD: the values of the parameters and options BEFORE the timeline is borrowed (the angle and
    // the depth have already been taken)
    let (t_pitch, t_length) = (qymcad_ui_state::cmd_val(pc.cmd, "pitch"), qymcad_ui_state::cmd_val(pc.cmd, "length"));
    let (t_starts, t_left) = (pc.thread.starts.max(1), pc.thread.left);
    let t_spec = thread_spec(pc.cmd, *pc.thread); // the whole record, so an edit and an apply cannot differ
    let (t_outer, t_thickness, t_edge_r) = (qymcad_ui_state::cmd_val(pc.cmd, "outer"), qymcad_ui_state::cmd_val(pc.cmd, "thickness"), qymcad_ui_state::cmd_val(pc.cmd, "edge_r")); // the auger
    let (t_lead_in, t_lead_out) = (qymcad_ui_state::cmd_val(pc.cmd, "lead_in"), qymcad_ui_state::cmd_val(pc.cmd, "lead_out"));
    let (sw_prof, sw_path, sw_pcid, sw_hcid) = (pc.sweep.prof_sid, pc.sweep.path_sid, pc.sweep.prof_cid, pc.sweep.path_cid); // captured before the mutable borrow
    let (lf_sids, lf_cids, lf_ruled) = (pc.loft.sids.clone(), pc.loft.cids.clone(), pc.loft.ruled); // the loft, captured before the mutable borrow
                                                                                                    // the kind of loft result maps to (src, op). For a boolean the target is the active body of the
                                                                                                    // context. When EDITING a loft that is already a boolean, the previous target is kept (`active_body`
                                                                                                    // would skip it as consumed); switching from "new body" to "cut" takes the active body.
    let lf_prev_src = pc.project.timeline.iter().find(|n| n.id == fid).and_then(|n| match n.kind {
        FeatureKind::Loft { src, .. } => Some(src),
        _ => None,
    });
    let (lf_src, lf_op): (Id, u8) = if pc.loft.result == 0 {
        (0, 0)
    } else {
        let src = match lf_prev_src {
            Some(s) if s != 0 => s, // already a boolean - do not change the target body
            _ => qymcad_ui_state::current_body(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: pc.set, scheme: pc.scheme, project: pc.project, active_path: pc.active_path }).unwrap_or(0),
        };
        (src, pc.loft.result - 1)
    };
    // THE CUT rejects the edit if the plane now divides the body into a different number of pieces
    let mut bad_pieces: Option<(usize, usize)> = None;
    let found = if let Some(node) = pc.project.timeline.iter_mut().find(|n| n.id == fid) {
        match &mut node.kind {
            FeatureKind::Loft { sketches, contours, ruled, src, op, .. } => {
                // update the set of sections and contours, the kind of faces and the kind of result (two
                // or more sections are required)
                if lf_sids.len() >= 2 {
                    *sketches = lf_sids.clone();
                    *contours = lf_cids.clone();
                }
                *ruled = lf_ruled;
                *src = lf_src; // 0 means a separate body; otherwise a boolean with the active or previous body
                *op = lf_op;
            }
            FeatureKind::Sweep { sketch, profiles, path_sketch, path, .. } => {
                // update the profile, the path and the picked contours; either sketch may be changed
                if sw_prof != 0 {
                    *sketch = sw_prof;
                }
                if sw_path != 0 {
                    *path_sketch = sw_path;
                }
                *profiles = if !edit_contours.is_empty() {
                    edit_contours.clone()
                } else if sw_pcid != 0 {
                    vec![sw_pcid]
                } else {
                    Vec::new()
                };
                *path = sw_hcid;
            }
            FeatureKind::Extrude { profiles: prs, height, reach: rch, down: dn, fill: fl_field, .. } => {
                // ALL the picked contours go into the node (a multi-contour feature is edited whole)
                if !edit_contours.is_empty() {
                    *prs = edit_contours.clone();
                    *fl_field = edit_fill.clone();
                } else if profile != 0 {
                    *prs = vec![profile];
                }
                *height = h;
                *rch = reach;
                *dn = down;
            }
            FeatureKind::Combine { profiles: prs, height, op, extent: ext, down: dnc, fill: fl_field, .. } => {
                if !edit_contours.is_empty() {
                    *prs = edit_contours.clone();
                    *fl_field = edit_fill.clone();
                } else if profile != 0 {
                    *prs = vec![profile];
                }
                *height = h.abs(); // a positive MAGNITUDE; the direction comes from flip, symmetry or the two sides
                *op = occt;
                *ext = qymcad_core::feature::Extent { through, reach };
                *dnc = down;
            }
            FeatureKind::Revolve { profiles: prs, axis: ax, angle, axis_datum, axis_line: al, reach: rch, .. } => {
                // the edit puts ALL the picked contours into the node rather than the first one to hand
                if !edit_contours.is_empty() {
                    *prs = edit_contours.clone();
                } else if profile != 0 {
                    *prs = vec![profile];
                }
                *ax = axis;
                *angle = ang;
                *axis_datum = axis_datum_rev; // K4
                *al = axis_line_rev; // the centre line of the sketch (0 means a datum or X/Y)
                *rch = reach;
            }
            FeatureKind::Fillet { radius, edges: e, at_vertices, .. } => {
                *radius = r;
                *e = picked_edges.clone();
                *at_vertices = vtable; // the "vertex -> radius" table
            }
            FeatureKind::Chamfer { dist: d, edges: e, mode, d2, flip, ref_face, .. } => {
                *d = dist;
                *e = if ch_mode != qymcad_core::feature::ChamferMode::Symmetric && !edges.is_empty() { qymcad_core::refs::Ref::picks(&edges) } else { picked_edges.clone() };
                *mode = ch_mode; // the mode: symmetric, two distances, or a leg plus an angle
                *d2 = ch_d2;
                *flip = ch_flip;
                *ref_face = ch_ref_face; // the hand-picked reference face
            }
            FeatureKind::Shell { thickness, faces, side, .. } => {
                *thickness = th;
                if !faces_set.is_empty() {
                    *faces = qymcad_core::refs::Ref::picks(&faces_set); // a multiple face selection; a hand pick is a query of ids
                }
                *side = shell_side; // which way the wall goes
            }
            FeatureKind::MeshRecognised { tol, sharp, .. } => {
                *tol = rec_tol;
                *sharp = rec_sharp;
            }
            FeatureKind::Thicken { face, thickness, .. } => {
                *thickness = thickness_v;
                if let Some(&id) = faces_set.first() {
                    *face = id; // the face can be reassigned by a click, without recreating the feature
                }
            }
            FeatureKind::SplitFace { plane, datum, offset, face, .. } => {
                if let Some((pl, dt, fc)) = split_resolved {
                    *plane = pl;
                    *datum = dt;
                    *face = fc;
                }
                *offset = split_offset;
            }
            FeatureKind::SplitBody { plane, datum, offset, bodies, face, .. } => {
                // THE NUMBER OF PIECES MUST NOT CHANGE: later features may already refer to the existing
                // bodies, and creating or removing a body through an edit would break those references
                // silently. An honest refusal leaves the cut to be recreated by hand.
                if let Some(n) = split_pieces {
                    if n != bodies.len() {
                        bad_pieces = Some((n, bodies.len()));
                    }
                }
                if bad_pieces.is_none() {
                    if let Some((pl, dt, fc)) = split_resolved {
                        *plane = pl;
                        *datum = dt;
                        *face = fc;
                    }
                    *offset = split_offset;
                }
            }
            FeatureKind::Patch { edges: e, tangent, .. } => {
                if !edges.is_empty() {
                    *e = picked_edges.clone();
                }
                *tangent = patch_tangent;
            }
            FeatureKind::FaceCopy { faces, .. } => {
                if !faces_set.is_empty() {
                    *faces = qymcad_core::refs::Ref::picks(&faces_set.to_vec());
                }
            }
            FeatureKind::OffsetSurface { faces, dist, .. } => {
                *dist = dist_v;
                if !faces_set.is_empty() {
                    *faces = qymcad_core::refs::Ref::picks(&faces_set.to_vec());
                }
            }
            FeatureKind::Stitch { parts, tol, .. } => {
                *tol = rec_tol;
                if stitch_now.len() >= 2 {
                    *parts = stitch_now.clone();
                }
            }
            FeatureKind::SurfaceReplace { faces, surface, .. } => {
                if !faces_set.is_empty() {
                    *faces = qymcad_core::refs::Ref::picks(&faces_set.to_vec());
                }
                if let Some(s) = surface_now {
                    *surface = s;
                }
            }
            FeatureKind::Trim { src, tool, keep, .. } => {
                if let (Some((s, k)), Some(t)) = trim_now {
                    (*src, *keep, *tool) = (s, k, t);
                }
            }
            FeatureKind::RemoveFace { faces, .. } => {
                if !faces_set.is_empty() {
                    // the set of faces is reassigned by clicks without recreating the feature; a hand-picked
                    // set is a query of `Id`s and is replaced whole rather than edited one by one
                    *faces = qymcad_core::refs::Ref::picks(&faces_set.to_vec());
                }
            }
            FeatureKind::PushFace { face, dist, .. } => {
                *dist = dist_v;
                if let Some(&id) = faces_set.first() {
                    *face = qymcad_core::refs::Ref::one(id, face.hint); // the face is reassigned by a click
                }
            }
            FeatureKind::Draft { faces, neutral, angle, flip, .. } => {
                // update the set of tilted faces (by id), the neutral face, the angle and the direction
                if !faces_set.is_empty() {
                    *faces = qymcad_core::refs::Ref::picks(&faces_set);
                }
                if draft_neutral != 0 {
                    *neutral = qymcad_core::refs::Ref::one(draft_neutral, neutral.hint);
                }
                *angle = ang;
                *flip = draft_flip;
            }
            FeatureKind::Hole { face, point, normal, diameter, depth, kind, dia2, depth2, flip, .. } => {
                *diameter = dia;
                *depth = dep;
                *kind = hole_kind; // F1
                *dia2 = hole_dia2;
                *depth2 = hole_depth2;
                *flip = hole_flip; // the drilling direction in "by sketch" mode
                if let Some((k, p, nrm)) = hole_face {
                    // editing the face recomputes the reference to it along with the centre and the normal
                    // of the hole. A hand pick is a query of `Id`: one particular face was pointed at.
                    *face = qymcad_core::refs::Ref::one(k.id, qymcad_core::refs::Fingerprint { centroid: k.centroid, normal: k.normal });
                    *point = p;
                    *normal = nrm;
                }
            }
            FeatureKind::Thread { spec, length, lead_in, lead_out, .. } => {
                // editing a thread means the standard and the size (the core computes the geometry)
                *spec = t_spec;
                *length = t_length;
                *lead_in = t_lead_in;
                *lead_out = t_lead_out;
            }
            FeatureKind::Auger { spec, length, lead_in, lead_out, .. } => {
                // editing an auger (the shaft is taken from the geometry during the regen)
                spec.outer_d = t_outer;
                spec.pitch = t_pitch;
                spec.thickness = t_thickness;
                spec.edge_r = t_edge_r;
                spec.starts = t_starts;
                spec.left = t_left;
                *length = t_length;
                *lead_in = t_lead_in;
                *lead_out = t_lead_out;
            }
            FeatureKind::Mirror { plane, keep, datum, face, .. } => {
                *keep = mirror_keep;
                if let Some((pl, dt, fc)) = mirror_resolved {
                    *plane = pl;
                    *datum = dt;
                    *face = fc;
                }
            }
            FeatureKind::LinearArray { dx, dy, dz, count, dx2, dy2, dz2, count2, dx3, dy3, dz3, count3, .. } => {
                *dx = a_dx;
                *dy = a_dy;
                *dz = a_dz;
                *count = a_count;
                *dx2 = a_dx2;
                *dy2 = a_dy2;
                *dz2 = a_dz2;
                *count2 = a_c2;
                *dx3 = a_dx3;
                *dy3 = a_dy3;
                *dz3 = a_dz3;
                *count3 = a_c3;
            }
            FeatureKind::CircularArray { count, angle, axis, .. } => {
                *count = a_count;
                *angle = a_angle;
                *axis = a_axis;
            }
            // the primitives: the sizes are written back on reopening (the keys are the regen's keys)
            FeatureKind::Box3 { dx, dy, dz, .. } => {
                *dx = p_dx;
                *dy = p_dy;
                *dz = p_dz;
            }
            FeatureKind::Cylinder { r, h, .. } => {
                *r = p_r;
                *h = p_ph;
            }
            FeatureKind::Sphere { r, .. } => {
                *r = p_r;
            }
            FeatureKind::Cone { r1, r2, h, .. } => {
                *r1 = p_r1;
                *r2 = p_r2;
                *h = p_ph;
            }
            FeatureKind::Torus { major, minor, .. } => {
                *major = p_major;
                *minor = p_minor;
            }
            FeatureKind::Prism { r, n, h, .. } => {
                *r = p_r;
                *n = p_n;
                *h = p_ph;
            }
            _ => return None,
        }
        node.dirty = true;
        true
    } else {
        false
    };
    if !found {
        return None;
    }
    if let Some((got, had)) = bad_pieces {
        // the feature was left untouched - the edit was not applied AT ALL rather than half applied
        *pc.status = qymcad_i18n::tr2("cmd-split-count-changed", "got", &got.to_string(), "had", &had.to_string());
        return None;
    }
    store_cmd_exprs(pc.cmd, pc.project, fid); // store or clear the dimension expressions (the logical step, step2 and angle included)
    if matches!(pc.armed.cmd_kind(), 17 | 18) {
        store_bar_counts(pc.bar_exprs, pc.project, fid); // the counts typed in the bar, as the steps are
    }
    // the array: the step expressions go onto the components of the vector (the regen reads those), and the
    // angle is cleared for a full circle
    match pc.armed.cmd_kind() {
        17 => {
            qymcad_ui_state::store_arr_component(pc.project, fid, ["dx", "dy", "dz"], a_dir, a_step_txt.clone());
            if a_two {
                qymcad_ui_state::store_arr_component(pc.project, fid, ["dx2", "dy2", "dz2"], a_dir2, a_step2_txt.clone());
            } else {
                for k in ["dx2", "dy2", "dz2"] {
                    pc.project.set_feat_dim(fid, k, String::new());
                }
            }
            if a_two && a_three {
                qymcad_ui_state::store_arr_component(pc.project, fid, ["dx3", "dy3", "dz3"], a_dir3, a_step3_txt.clone());
            } else {
                for k in ["dx3", "dy3", "dz3"] {
                    pc.project.set_feat_dim(fid, k, String::new());
                }
            }
        }
        18 if a_full => {
            pc.project.set_feat_dim(fid, "angle", String::new());
        }
        _ => {}
    }
    Some(fid)
}

pub fn apply_feat_cmd_inner(pc: &mut qymcad_ui_state::PartCtx) {
    // THE DERIVED VALUE IS COMPUTED HERE rather than when the command opens: the operation may have
    // been switched in the bar after opening (see `smart_flip`).
    if pc.feat.flip_auto {
        if let Some(si) = pc.cmd.sketch.filter(|i| *i < pc.project.sketches.len()) {
            pc.feat.flip = smart_flip(pc.armed.cmd_kind(), pc.feat.op, &pc.project.sketches[si].plane);
        }
    }
    qymcad_ui_state::ensure_brep(&mut pc.rebuild()); // after opening from a bundle there are no live shapes yet - bring the cache up once
                                                     // A sketch command (extrude or revolve) takes two steps when SEVERAL contours are picked: while we
                                                     // are in the flat half-sketcher (`mode_3d = false`) and the profiles ARE picked, Apply or Enter first
                                                     // CONFIRMS the selection and moves to 3D (the gizmo plus the size popup) rather than creating the
                                                     // feature silently at the default height. Otherwise the Apply button (which is enabled as soon as the
                                                     // profile selection is non-empty) would apply 10 mm straight away during a Ctrl multi-pick, with no
                                                     // gizmo and no popup, and would breed a chain of nodes. A single contour already moves to 3D at the
                                                     // start.
    if matches!(pc.armed.cmd_kind(), 1 | 3) && pc.cmd.edit.is_none() && !*pc.mode_3d && !pc.gsel.profiles.is_empty() {
        qymcad_ui_state::borrow_view(*pc.cam, *pc.mode_3d, *pc.view, pc.view_restore); // the view is borrowed for the command and returned on exit
        *pc.mode_3d = true;
        *pc.status = qymcad_i18n::tr("msg-profile-picked");
        return;
    }
    // a hard backstop: DO NOT apply while any dimension carries an invalid expression (a stale value
    // would go through otherwise). The gate is duplicated on the buttons, but Enter can arrive past
    // them.
    if !cmd_exprs_valid(pc.cmd, pc.project) {
        *pc.status = qymcad_i18n::tr1("cmd-expr-error", "icon", ph::X);
        return;
    }
    let before: std::collections::HashSet<Id> = pc.project.timeline.iter().map(|n| n.id).collect();
    // editing an existing feature (a double click) updates it rather than creating a new one
    let last = if let Some(fid) = pc.cmd.edit {
        update_feat(pc, fid)
    } else {
        match pc.armed.cmd_kind() {
            1 | 3 => apply_sketch_cmd(pc, pc.armed.cmd_kind()),
            8 => apply_sweep_cmd(*pc.feat, pc.project, pc.status, *pc.sweep),
            9 => apply_loft_cmd(pc),
            4 | 5 => apply_edge_cmd(pc, pc.armed.cmd_kind()),
            6 => apply_shell_cmd(pc),
            23 => apply_draft_cmd(pc),
            25 => apply_push_face_cmd(pc),
            26 => apply_remove_face_cmd(pc),
            30 => apply_face_copy_cmd(pc),
            36 => apply_offset_surface_cmd(pc),
            31 => apply_surface_replace_cmd(pc),
            32 => apply_patch_cmd(pc),
            33 => apply_stitch_cmd(pc.cmd, pc.project, pc.status, pc.stitch_parts),
            35 => apply_recognise_cmd(pc),
            34 => apply_trim_cmd(pc.project, pc.status, pc.trim),
            27 => apply_split_cmd(pc),
            28 => apply_thicken_cmd(pc),
            29 => apply_split_face_cmd(pc),
            24 => apply_thread_cmd(pc.cmd, pc.project, pc.status, *pc.thread),
            7 => apply_hole_cmd(pc.cmd, *pc.hole, pc.project, *pc.sel, pc.status),
            10..=15 => apply_prim_cmd(pc),                                                           // the primitives
            16 => apply_mirror_cmd(pc),                                                              // the mirror
            17 | 18 => apply_array_cmd(pc),                                                          // the array
            20..=22 => apply_datum_cmd(pc.armed, pc.cmd, pc.datum, pc.edges, pc.project, pc.status), // a datum plane, point or axis
            _ => None,
        }
    };
    let Some(body) = last else { return }; // on failure we stay in the command (the status is already set)
                                           // WHAT WAS PICKED IS GONE: Ctrl+Z with the picks held took away the step that made them, and the node laid on them
                                           // would only stand red. It is not laid - the operation rolls back - the picks go, and the command asks for new ones.
                                           // Every node the operation laid is asked - a datum lays one with no body of its own.
    let fresh: Vec<Id> = pc.project.timeline.iter().map(|n| n.id).filter(|id| !before.contains(id)).collect();
    if fresh.iter().any(|&node| !pc.project.gone_inputs(node).is_empty()) {
        *pc.cmd_failed = true;
        qymcad_ui_state::clear_feat_picks(qymcad_ui_state::feat_picks_in!(pc));
        *pc.status = qymcad_i18n::tr1("cmd-picks-gone", "ask", &cmd_hint(&*pc.armed, &*pc.gsel, &*pc.trim));
        return;
    }
    // WHAT WAS MADE IS NAMED: the node the command left, not a bare "Done" (reported on the block after Enter)
    let made = pc.project.timeline.iter().find(|n| n.kind.body() == Some(body)).map(|n| qymcad_i18n::name(&n.name));
    // A CLEAN finish: the command is dropped and the selection is cleared, so neither the gizmo arrows
    // nor the right-hand panel remain
    pc.cmd.open(pc.armed, 0, *pc.mode_3d); // a clean slate, then open

    qymcad_ui_state::clear_feat_picks(qymcad_ui_state::feat_picks_in!(pc));
    // back from the flat half-sketcher into 3D: RETURN the view that was borrowed rather than refitting
    // it. Refitting threw away everything that had been set up by hand, and finishing a command looked
    // as if the viewport had flown off.
    qymcad_ui_state::return_view(pc);
    *pc.status = made.map_or_else(|| qymcad_i18n::tr("f-done"), |what| qymcad_i18n::tr1("f-done-what", "what", &what));
    // AFTER the status: if the regen fails, `qymcad_ui_state::regenerate_all` writes its own rebuild message - do NOT
    // overwrite it with "done" (otherwise the feature silently fails to build while the screen says
    // "done", which reads as nothing happening at all).
    qymcad_ui_state::mark_dirty_for_rebuild(&mut pc.rebuild()); // the document is marked; the planner does the counting
    *pc.sel = qymcad_ui_state::Sel::None;
}

pub fn apply_feat_cmd(pc: &mut qymcad_ui_state::PartCtx) {
    // NOTHING TO APPLY YET: Enter is refused in words, the command's own ask repeated, and the document untouched -
    // unless the command names what is missing itself (the axis, the faces, the patch's edges, the trimmed sheet)
    // A FIELD THAT CANNOT BE TAKEN refuses Enter as it closes the Apply button: Enter comes from the keyboard, past it
    if let Some((label, msg)) = pc.cmd.params.iter().find_map(|p| field_value(p, &pc.project.param_map()).err().map(|m| (qymcad_i18n::tr(p.label_key()), m))) {
        *pc.status = format!("{label}: {msg}");
        return;
    }
    if !cmd_ready(pc) && !matches!(pc.armed.cmd_kind(), 22 | 25 | 26 | 28 | 30 | 32 | 34) {
        *pc.status = qymcad_i18n::tr1("cmd-not-ready", "ask", &cmd_hint(&*pc.armed, &*pc.gsel, &*pc.trim));
        return;
    }
    // THE BOUNDARY OF THE OPERATION: everything the command does to the document is one undo step with
    // a name of its own. It opens HERE rather than in `start_feat_cmd`: opening a dialog does not change
    // the document and there is nothing to undo there. It closes at the end: success commits, a refusal
    // rolls back and leaves no trace.
    let name = feat_cmd_name(&*pc.armed, *pc.feat);
    // OPENED AND CLOSED BY NAME, without the guard. `qymcad_ui_state::commit_edit` counts the nesting itself
    // (`edits.depth`), so the boundary holds the same as before; what goes is the guard's hold on the
    // WHOLE application, which existed for this one call and no other.
    qymcad_ui_state::begin_edit(&mut *pc.edits, &*pc.project, name);
    *pc.cmd_failed = false;
    apply_feat_cmd_inner(pc);
    // A FACT, NOT A GUESS FROM TEXT. This used to search the status line for a substring meaning "did
    // not succeed" - checking what was written for a person instead of what actually happened. Once the
    // interface was translated the status would no longer match, and a failed operation would land in
    // the undo history as a successful one: Ctrl+Z then rolls back the wrong thing.
    if *pc.cmd_failed {
        qymcad_ui_state::abort_edit(&mut pc.rebuild());
    } else {
        qymcad_ui_state::commit_edit(&mut pc.rebuild());
    }
}

/// Apply the world transform `accum` to the body of mesh `mi`: if the body is already a Move feature,
/// accumulate into its matrix (no chain of Move nodes); otherwise create a new Move, for a B-rep and a raw mesh alike.
pub fn apply_body_move(pc: &mut qymcad_ui_state::PartCtx, mi: usize, accum: [f64; 12]) {
    qymcad_ui_state::begin_edit(&mut *pc.edits, &*pc.project, qymcad_i18n::tr("status-move-body")); // THE BOUNDARY OF AN OPERATION
    use qymcad_core::feature::FeatureKind;
    let Some(body) = pc.project.mesh_id(mi) else { return };
    let is_move = pc.project.timeline.iter().any(|n| n.id == body && matches!(n.kind, FeatureKind::Move { .. }));
    if is_move {
        pc.project.accumulate_move(body, &accum);
        qymcad_ui_state::mark_dirty_for_rebuild(&mut pc.rebuild()); // the document is marked; the planner does the counting
        qymcad_ui_state::select_body(&mut *pc.project, &mut *pc.sel, &mut *pc.view, body);
    } else {
        qymcad_ui_state::move_body_at(&mut pc.rebuild(), mi, accum); // a new Move feature (and selection), a raw mesh's moved by the document
    }
    qymcad_ui_state::after_placement_change(&mut pc.rebuild()); // the source moved -> rebuild whoever consumes it
    qymcad_ui_state::commit_edit(&mut pc.rebuild());
}

/// Ctrl+V OF A TIMELINE NODE: its tool opens again with its values - the diameter, the depth, the kind - as a NEW
/// operation on the part's body as it stands now, and waits for where it goes: a click places it, Enter applies.
/// A copy dropped where the original stands would change nothing, so it is not made by itself.
pub fn paste_feature(pc: &mut qymcad_ui_state::PartCtx, nid: Id) {
    if !pc.project.timeline.iter().any(|n| n.id == nid) {
        *pc.status = qymcad_i18n::tr("tree-clipboard-empty");
        return;
    }
    start_feat_cmd_edit(pc, nid);
    pc.cmd.edit = None; // a new node, not the copied one edited
                        // the face it was made on, found on the body the part ends in now
    let dc = qymcad_ui_state::DrawCtx { cam: pc.cam, set: pc.set, scheme: pc.scheme, project: pc.project, active_path: pc.active_path };
    let now = qymcad_ui_state::current_body(&dc);
    let face = match *pc.sel {
        qymcad_ui_state::Sel::Face(mi, fi) => pc.project.bodies.get(mi).and_then(|b| b.faces.get(fi)).map(|f| qymcad_core::feature::FaceKey {
            index: 0,
            centroid: [f.centroid.x, f.centroid.y, f.centroid.z],
            normal: f.normal,
            id: f.id,
        }),
        _ => None,
    };
    if let (Some(body), Some(key)) = (now, face) {
        if let Some((mi, fi)) = qymcad_pick::resolve_face_sel(&*pc.project, body, &key) {
            *pc.sel = qymcad_ui_state::Sel::Face(mi, fi);
            pc.gsel.faces.clear();
            pc.gsel.faces.insert(key.id);
        }
    }
    *pc.status = qymcad_i18n::tr("g-paste-feature");
}
