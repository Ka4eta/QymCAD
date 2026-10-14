//! THE BARS OF THE PART: the tool bar, the options of a command, the bars of a section and of a pattern of components.

use super::*;

/// The top bar of a body-to-body boolean, in the style of the sketcher: the kind of operation, the hint
/// about picking body B, and a cancel. It is shown while body A is picked and the click on B is awaited
/// (`bool_pick`).
pub fn bool_tool_bar(pc: &mut qymcad_ui_state::PartCtx, ui: &mut egui::Ui) {
    use qymcad_core::feature::FeatureKind;
    // CREATING: body A is picked, the click on B and the choice of kind are awaited
    if let Some((a, mut op)) = pc.boolean.pick {
        let ctx = ui.ctx().clone();
        // Enter before body B is taken has nothing to make; with B taken it applies
        let mut apply = pc.boolean.b.is_some() && ((!ctx.egui_wants_keyboard_input() && ctx.input(|i| i.key_pressed(egui::Key::Enter))) || qymcad_ui_state::bar_enter_take(&ctx));
        if pc.boolean.b.is_none() {
            qymcad_ui_state::enter_not_ready(&ctx, pc.status, &qymcad_i18n::tr("tb-bool-pick-b"));
        }
        let a_name = pc.project.mesh_index(a).map(|mi| qymcad_i18n::name(&pc.project.mesh_name(mi))).unwrap_or_else(|| "?".into());
        let b_name = pc.boolean.b.and_then(|b| pc.project.mesh_index(b)).map(|mi| qymcad_i18n::name(&pc.project.mesh_name(mi)));
        let mut cancel = false;
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new(format!("{} {}", ph::INTERSECT, qymcad_i18n::tr("bool-bodies-btn"))).strong());
            ui.separator();
            ui.label(qymcad_i18n::tr("f-kind"));
            ui.selectable_value(&mut op, 0u8, qymcad_i18n::tr("f-cut-ab"));
            ui.selectable_value(&mut op, 1u8, qymcad_i18n::tr("f-union"));
            ui.selectable_value(&mut op, 2u8, qymcad_i18n::tr("f-intersect"));
            ui.separator();
            let said = match &b_name {
                Some(b) => qymcad_i18n::tr2("cmd-bool-a-b", "a", &a_name, "b", b),
                None => qymcad_i18n::tr1("cmd-bool-a-is", "name", &a_name),
            };
            ui.label(egui::RichText::new(said).color(pc.scheme.pal.hint()));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button(qymcad_i18n::tr("f-cancel-esc")).clicked() {
                    cancel = true;
                }
                if ui.add_enabled(b_name.is_some(), egui::Button::new(qymcad_i18n::tr("cmd-apply-enter"))).clicked() {
                    apply = true;
                }
            });
        });
        if cancel {
            (pc.boolean.pick, pc.boolean.b) = (None, None);
        } else {
            pc.boolean.pick = Some((a, op));
            if apply {
                apply_boolean(pc);
            }
        }
        return;
    }
    // EDITING: a double click on the boolean node changes its KIND in the bar (as reopening the command
    // does for an extrude)
    if let Some(ti) = pc.boolean.edit {
        let Some(FeatureKind::BodyBoolean { op, .. }) = pc.project.timeline.get(ti).map(|n| n.kind.clone()) else {
            pc.boolean.edit = None;
            return;
        };
        let mut op = op;
        let (mut done, mut changed) = (false, false);
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new(format!("{} {}", ph::INTERSECT, qymcad_i18n::tr("bool-bodies-btn"))).strong());
            ui.separator();
            ui.label(qymcad_i18n::tr("f-kind"));
            changed |= ui.selectable_value(&mut op, 0u8, qymcad_i18n::tr("f-cut-ab")).changed();
            changed |= ui.selectable_value(&mut op, 1u8, qymcad_i18n::tr("f-union")).changed();
            changed |= ui.selectable_value(&mut op, 2u8, qymcad_i18n::tr("f-intersect")).changed();
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button(qymcad_i18n::tr("f-done")).clicked() {
                    done = true;
                }
            });
        });
        if changed {
            // a change of kind is one step of undo, like any edit of a node
            qymcad_ui_state::begin_edit(&mut *pc.edits, &*pc.project, qymcad_i18n::tr("f-operation"));
            if let Some(FeatureKind::BodyBoolean { op: o, .. }) = pc.project.timeline.get_mut(ti).map(|n| &mut n.kind) {
                *o = op;
            }
            pc.project.timeline[ti].dirty = true;
            qymcad_ui_state::mark_dirty_for_rebuild(&mut pc.rebuild()); // the document is marked; the planner does the counting
            qymcad_ui_state::commit_edit(&mut pc.rebuild());
        }
        if done || pc.project.timeline.get(ti).is_none_or(|n| !matches!(n.kind, FeatureKind::BodyBoolean { .. })) {
            pc.boolean.edit = None;
        }
    }
}

/// THE SECTION CONTROL BAR: offset, tilts, flip, switching off.
///
/// This is PANEL DRAWING rather than a phase of the frame - yet it used to sit in the middle of `update`,
/// mixed in with the prologue, the keyboard and holding the selection. While panels live in the shared body,
/// "what the frame does" cannot be told from "what it draws", and editing one touches the other.
/// WHAT THE BAR OF THE TOOLS WAITING ON A PLANE TOUCHES: the section, the mirrored copy of a part and the pencil.
pub struct PlaneBarCtx<'a> {
    pub regen: &'a mut qymcad_ui_state::Rebuilding,
    pub section: &'a mut qymcad_ui_state::SectionTool,
    pub mirror: &'a mut qymcad_ui_state::MirrorParams,
    pub picking: &'a mut qymcad_ui_state::Picking,
    pub project: &'a mut qymcad_core::model::Project,
    pub edits: &'a mut qymcad_ui_state::Edits,
    pub active_path: &'a [qymcad_core::model::Id],
    pub status: &'a mut String,
    pub m3: &'a mut qymcad_ui_state::Measure3,
}

pub fn section_bar(pb: &mut PlaneBarCtx, ui: &mut egui::Ui) {
    // THE 3D MEASURING TOOL holds the hand as any other: its name, what is taken, the answer, the way out. Its words stood
    // only in the status line, and nothing said which elements were taken.
    if pb.m3.on {
        // the answer comes with the second element clicked: Enter before it has nothing to show
        if pb.m3.picks.len() < 2 {
            qymcad_ui_state::enter_not_ready(&ui.ctx().clone(), pb.status, &qymcad_i18n::tr("m3-hint-short"));
        }
        let mut off = false;
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new(format!("{} {}", ph::RULER, qymcad_i18n::tr("hotkey-part-i"))).strong());
            ui.separator();
            if pb.m3.picks.is_empty() {
                ui.label(egui::RichText::new(qymcad_i18n::tr("m3-hint-short")).weak());
            } else {
                let taken: Vec<String> = pb.m3.picks.iter().map(|p| p.what.clone()).collect();
                ui.label(taken.join(" - "));
                ui.separator();
                ui.label(egui::RichText::new(pb.status.as_str()).strong());
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button(qymcad_i18n::tr("cmd-cancel-btn")).clicked() {
                    off = true;
                }
            });
        });
        if off {
            pb.m3.clear();
            *pb.status = qymcad_i18n::tr("in-measure-off");
        }
        return;
    }
    // A COMMAND WAITING FOR ITS SKETCH is in hand as well: its name, what to point at, the way out. It waited with only
    // the status line saying so, and the bar stood empty over a tool held
    if let Some(kind) = pb.picking.sketch_for() {
        let name = match kind {
            3 => "cmd-revolve",
            8 => "cmd-sweep",
            9 => "cmd-loft",
            _ => "cmd-extrude",
        };
        let mut cancel = false;
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new(qymcad_i18n::tr(name)).strong());
            ui.separator();
            ui.label(egui::RichText::new(qymcad_i18n::tr("g-pick-sketch-for")).weak());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button(qymcad_i18n::tr("cmd-cancel-btn")).clicked() {
                    cancel = true;
                }
            });
        });
        // the sketch is pointed at, not typed: Enter before it has nothing to start
        qymcad_ui_state::enter_not_ready(&ui.ctx().clone(), pb.status, &qymcad_i18n::tr("g-pick-sketch-for"));
        if cancel {
            *pb.status = pb.picking.cancel_key().map(qymcad_i18n::tr).unwrap_or_default();
            pb.picking.clear();
        }
        return;
    }
    let PlaneBarCtx { regen, section, mirror, picking, project, edits, active_path, status, .. } = pb;
    let (regen, section, mirror, picking, project, edits, active_path, status) =
        (&mut **regen, &mut **section, &mut **mirror, &mut **picking, &mut **project, &mut **edits, *active_path, &mut **status);
    // THE PENCIL WAITING FOR A PLANE shares the place too: what it is, what to click, and the way out, as the bar of any
    // tool in hand. Without it the words stood only in the status line and nothing on the bar said a tool was held.
    if picking.is_sketch_plane() {
        // the sketch opens on the click that names its plane: Enter before it has nothing to open
        qymcad_ui_state::enter_not_ready(&ui.ctx().clone(), status, &qymcad_i18n::tr("g-sketch-pick-plane"));
        let mut cancel = false;
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new(format!("{} {}", ph::PENCIL_SIMPLE, qymcad_i18n::tr("status-new-sketch"))).strong());
            ui.separator();
            ui.label(egui::RichText::new(qymcad_i18n::tr("g-sketch-pick-plane")).weak());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button(qymcad_i18n::tr("cmd-cancel-btn")).clicked() {
                    cancel = true;
                }
            });
        });
        if cancel {
            *status = picking.cancel_key().map(qymcad_i18n::tr).unwrap_or_default(); // the words Esc says for the same
            picking.clear();
        }
        return;
    }
    // THE MIRRORED COPY OF A PART shares the place: it too waits for a plane, and a tool that waits with no bar left a
    // person clicking at a view that gave no sign anything was held
    if mirror.waiting && mirror.part.is_none() {
        let ctx = ui.ctx().clone();
        qymcad_ui_state::enter_not_ready(&ctx, status, &qymcad_i18n::tr("tb-mirror-pick-part"));
        let mut cancel = false;
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new(format!("{} {}", ph::FLIP_HORIZONTAL, qymcad_i18n::tr("mp-title"))).strong());
            ui.separator();
            ui.label(egui::RichText::new(qymcad_i18n::tr("tb-mirror-pick-part")).weak());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                cancel = ui.button(qymcad_i18n::tr("cmd-cancel-btn")).clicked();
            });
        });
        if cancel || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            mirror.drop_part();
        }
        return;
    }
    if let Some(src) = mirror.part {
        let name = project.components.iter().find(|c| c.id == src).map(|c| qymcad_i18n::name(&c.name)).unwrap_or_default();
        let ctx = ui.ctx().clone();
        let taken = mirror.at.is_some();
        // Enter makes the copy once a plane is taken, and says what is missing before
        let mut apply = taken && ((!ctx.egui_wants_keyboard_input() && ctx.input(|i| i.key_pressed(egui::Key::Enter))) || qymcad_ui_state::bar_enter_take(&ctx));
        if !taken {
            qymcad_ui_state::enter_not_ready(&ctx, status, &qymcad_i18n::tr1("mp-pick", "name", &name));
        }
        let mut cancel = false;
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new(format!("{} {}", ph::FLIP_HORIZONTAL, qymcad_i18n::tr("mp-title"))).strong());
            ui.separator();
            let said = if taken { qymcad_i18n::tr1("mp-plane-taken-of", "name", &name) } else { qymcad_i18n::tr1("mp-pick", "name", &name) };
            ui.label(egui::RichText::new(said).weak());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button(qymcad_i18n::tr("cmd-cancel-btn")).clicked() {
                    cancel = true;
                }
                if ui.add_enabled(taken, egui::Button::new(qymcad_i18n::tr("cmd-apply-enter"))).clicked() {
                    apply = true;
                }
            });
        });
        // Esc puts it down from here: the window's ladder of Esc has no rung for a copy waiting on its plane
        cancel |= ctx.input(|i| i.key_pressed(egui::Key::Escape));
        if cancel {
            mirror.drop_part();
        } else if apply {
            mirror_part_apply(mirror, project, edits, regen, active_path, status);
        }
        return;
    }
    // WAITING FOR THE PLANE the tool is already in hand, and the bar says so and what it wants, with the way out:
    // a tool taken with no bar left a person clicking at a view that gave no sign anything was held
    // ENTER OF THE COMMAND applies once a plane is taken and says what is missing before. Esc goes down the window's
    // ladder, which puts the tool down through `section_cancel` and takes a section still being placed with it.
    // Enter typed into the offset or a tilt closes that field and is the command's Enter as well: one press, as the bar
    // of every command takes it
    let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
    if section.pick && enter {
        if section.plane.is_some() {
            section.pick = false;
            *status = qymcad_i18n::tr("vp-section-on");
        } else {
            *status = qymcad_i18n::tr1("cmd-not-ready", "ask", &qymcad_i18n::tr("tb-section-pick"));
        }
    }
    if section.plane.is_none() && section.pick {
        let mut off = false;
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new(format!("{} {}", ph::SQUARE_HALF, qymcad_i18n::tr("sec-btn"))).strong());
            ui.separator();
            ui.label(egui::RichText::new(qymcad_i18n::tr("tb-section-pick")).weak());
            if ui.button(format!("{} {}", ph::X, qymcad_i18n::tr("sec-off-btn"))).clicked() {
                off = true;
            }
        });
        if off {
            section.pick = false;
            *status = qymcad_i18n::tr("tb-section-off");
            qymcad_ui_state::invalidate(regen);
        }
        return;
    }
    // THE SECTION: the control bar (offset, tilts, flip, off) for as long as the section is active
    if section.plane.is_some() {
        let (mut changed, mut off, mut apply, mut cancel) = (false, false, false, false);
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new(format!("{} {}", ph::SQUARE_HALF, qymcad_i18n::tr("sec-btn"))).strong());
            ui.separator();
            ui.label(qymcad_i18n::tr("sec-offset"));
            changed |= ui.add(egui::DragValue::new(&mut section.offset).speed(0.5).suffix(qymcad_i18n::tr("unit-mm-suffix"))).changed();
            ui.label(qymcad_i18n::tr("sec-tilt1"));
            changed |= ui.add(egui::DragValue::new(&mut section.rot[0]).speed(0.5).range(-89.0..=89.0).suffix("°")).changed();
            ui.label(qymcad_i18n::tr("sec-tilt2"));
            changed |= ui.add(egui::DragValue::new(&mut section.rot[1]).speed(0.5).range(-89.0..=89.0).suffix("°")).changed();
            if ui.button(format!("{} {}", ph::ARROWS_DOWN_UP, qymcad_i18n::tr("sec-flip-btn"))).on_hover_text(qymcad_i18n::tr("sec-flip-hint")).clicked() {
                if let Some((_, n)) = &mut section.plane {
                    n[0] = -n[0];
                    n[1] = -n[1];
                    n[2] = -n[2];
                }
                section.offset = -section.offset;
                changed = true;
            }
            if section.pick {
                // STILL BEING PLACED: the command's own way on and way back, as every tool has them
                if ui.button(format!("{} {}", ph::CHECK, qymcad_i18n::tr("cmd-apply-enter"))).clicked() {
                    apply = true;
                }
                if ui.button(format!("{} {}", ph::X, qymcad_i18n::tr("cmd-cancel-esc"))).clicked() {
                    cancel = true;
                }
            } else if ui.button(format!("{} {}", ph::X, qymcad_i18n::tr("sec-off-btn"))).clicked() {
                off = true;
            }
        });
        // the status line said "the section is on" after it was switched off here: it says so now
        if off {
            section.plane = None;
            section.pick = false;
            *status = qymcad_i18n::tr("tb-section-off");
            changed = true;
        }
        if apply {
            section.pick = false;
            *status = qymcad_i18n::tr("vp-section-on");
        }
        if cancel {
            section_cancel(section, regen);
            *status = qymcad_i18n::tr("tb-section-off");
        }
        if changed {
            qymcad_ui_state::invalidate(regen);
        }
    }
}

/// ESC WITH THE SECTION TOOL PICKING: the tool is put down, a section still being placed with it; what the status line
/// says then.
pub fn section_escape(section: &mut qymcad_ui_state::SectionTool, regen: &mut qymcad_ui_state::Rebuilding) -> String {
    section_cancel(section, regen);
    qymcad_i18n::tr("tb-section-off")
}

/// PUT THE SECTION TOOL DOWN: a section still being placed goes with it (Esc, Cancel, another tool taken); one
/// already applied stays until "Off".
pub fn section_cancel(section: &mut qymcad_ui_state::SectionTool, regen: &mut qymcad_ui_state::Rebuilding) {
    if section.pick && section.plane.is_some() {
        section.plane = None;
        qymcad_ui_state::invalidate(regen);
    }
    section.pick = false;
    section.drag = false;
    section.drag_anchor = None;
}

/// THE TOP BAR OF A COMPONENT PATTERN (in an assembly) - the same interaction as the body pattern: the
/// count and the direction here, the step or the angle in an expression field at the geometry, Enter applies.
///
/// The bars used to be 645 lines sitting straight inside `update`: two thirds of what was left in the frame
/// after the earlier extractions. While a panel lives in the frame's body, "what the frame does" and "what it
/// draws" cannot be told apart, and editing one command touches the whole life cycle of the frame.
pub fn comp_array_bar(pc: &mut qymcad_ui_state::PartCtx, ui: &mut egui::Ui) {
    if pc.carr.mode == 0 {
        return;
    }
    let (mut apply, mut cancel) = (false, false);
    ui.horizontal_wrapped(|ui| {
        let title = if pc.carr.mode == 2 { qymcad_i18n::tr("cmd-comp-circ-array") } else { qymcad_i18n::tr("cmd-comp-lin-array") };
        ui.label(egui::RichText::new(format!("{} {title}", ph::STACK)).strong());
        ui.separator();
        let name = pc.project.components.iter().find(|c| c.id == pc.carr.src).map(|c| qymcad_i18n::name(&c.name)).unwrap_or_default();
        ui.label(egui::RichText::new(qymcad_i18n::tr1("cmd-source", "name", &name)).weak());
        ui.separator();
        ui.label(qymcad_i18n::tr("cmd-copies"));
        pc.arr.count = qymcad_ui_state::num_or_expr(
            &mut qymcad_ui_state::ExprBarCtx { bar_exprs: pc.bar_exprs, project: pc.project, scheme: pc.scheme },
            ui,
            "carr_count",
            pc.arr.count as f64,
            qymcad_ui_state::NumFormat { lo: 1.0, hi: 512.0, integer: true, suffix: "", nonzero: false },
        ) as u32;
        ui.separator();
        if pc.carr.mode == 2 {
            ui.label(qymcad_i18n::tr("cmd-axis"));
            // AN AXIS OF ITS OWN, picked in the view as a pattern of bodies picks it: a datum axis, a straight edge or
            // a cylindrical face, kept as a reference the pattern follows. Reported behaviour: the axis of a circular
            // pattern of parts could only be X, Y or Z through the origin of the assembly.
            let picked = pc.project.datum_axes.iter().find(|d| d.id == pc.arr.axis).map(|d| qymcad_i18n::name(&d.name));
            if ui
                .selectable_label(pc.arr.axis_pick, format!("{} {}", ph::CROSSHAIR, picked.clone().unwrap_or_else(|| qymcad_i18n::tr("cmd-pick-axis"))))
                .on_hover_text(qymcad_i18n::tr("cmd-revolve-pick-axis3d"))
                .clicked()
            {
                pc.arr.axis_pick = !pc.arr.axis_pick;
                if pc.arr.axis_pick {
                    refresh_axis_edges(pc); // the straight edges of every visible body, as axis candidates
                    *pc.status = qymcad_i18n::tr("cmd-array-axis-hint");
                }
            }
            if picked.is_some() {
                if ui.small_button(ph::X).on_hover_text(qymcad_i18n::tr("cmd-axis-drop")).clicked() {
                    (pc.arr.axis, pc.arr.axis_pick) = (0, false);
                }
            } else {
                ui.selectable_value(&mut pc.carr.axis, 0u8, "X");
                ui.selectable_value(&mut pc.carr.axis, 1u8, "Y");
                ui.selectable_value(&mut pc.carr.axis, 2u8, "Z");
            }
            ui.separator();
            let was = pc.arr.full;
            ui.checkbox(&mut pc.arr.full, qymcad_i18n::tr("cmd-full-circle-short"));
            if was != pc.arr.full {
                // the angle field appears and disappears with the checkbox, as in the body pattern
                pc.cmd.params = if pc.arr.full { vec![] } else { vec![qymcad_ui_state::CmdParam::new("cmd-angle", "cangle", 180.0, -3600.0, 3600.0)] };
            }
        } else {
            ui.label(qymcad_i18n::tr("cmd-direction"));
            ui.selectable_value(&mut pc.carr.dir, 0u8, "X");
            ui.selectable_value(&mut pc.carr.dir, 1u8, "Y");
            ui.selectable_value(&mut pc.carr.dir, 2u8, "Z");
            // THE SECOND AND THIRD DIRECTIONS, as a pattern of bodies has them: a grid of parts, each direction with
            // its count, its axis and its step. Reported behaviour: a linear pattern of parts ran one way only.
            ui.separator();
            // a direction ticked on opens as a row of two along the next axis, so the tick itself makes a grid
            if ui.checkbox(&mut pc.arr.two, qymcad_i18n::tr("cmd-dir2")).changed() && pc.arr.two {
                (pc.arr.count2, pc.arr.dir2) = (pc.arr.count2.max(2), (pc.carr.dir + 1) % 3);
            }
            if pc.arr.two {
                pc.arr.count2 = qymcad_ui_state::num_or_expr(
                    &mut qymcad_ui_state::ExprBarCtx { bar_exprs: pc.bar_exprs, project: pc.project, scheme: pc.scheme },
                    ui,
                    "carr_count2",
                    pc.arr.count2 as f64,
                    qymcad_ui_state::NumFormat { lo: 1.0, hi: 512.0, integer: true, suffix: "", nonzero: false },
                ) as u32;
                ui.selectable_value(&mut pc.arr.dir2, 0u8, "X");
                ui.selectable_value(&mut pc.arr.dir2, 1u8, "Y");
                ui.selectable_value(&mut pc.arr.dir2, 2u8, "Z");
                ui.separator();
                if ui.checkbox(&mut pc.arr.three, qymcad_i18n::tr("cmd-dir3")).changed() && pc.arr.three {
                    (pc.arr.count3, pc.arr.dir3) = (pc.arr.count3.max(2), 3 - pc.carr.dir.min(2) - pc.arr.dir2.min(2));
                }
                if pc.arr.three {
                    pc.arr.count3 = qymcad_ui_state::num_or_expr(
                        &mut qymcad_ui_state::ExprBarCtx { bar_exprs: pc.bar_exprs, project: pc.project, scheme: pc.scheme },
                        ui,
                        "carr_count3",
                        pc.arr.count3 as f64,
                        qymcad_ui_state::NumFormat { lo: 1.0, hi: 512.0, integer: true, suffix: "", nonzero: false },
                    ) as u32;
                    ui.selectable_value(&mut pc.arr.dir3, 0u8, "X");
                    ui.selectable_value(&mut pc.arr.dir3, 1u8, "Y");
                    ui.selectable_value(&mut pc.arr.dir3, 2u8, "Z");
                }
            } else {
                pc.arr.three = false; // the third direction only sits on top of the second
            }
            sync_comp_array_params(*pc.arr, pc.cmd);
        }
        if !pc.cmd.params.is_empty() {
            ui.separator();
            apply |= comp_array_fields(pc, ui, "bar");
        }
        ui.separator();
        // a step or an angle the field refuses closes Apply, as a field of every tool does
        let valid = cmd_exprs_valid(pc.cmd, pc.project) && !qymcad_ui_state::bar_field_bad("carr_count");
        apply |= confirm_button(&pc.scheme.pal, ui, valid, format!("{} {}", ph::CHECK, qymcad_i18n::tr("cmd-apply-enter")));
        cancel = refuse_button(&pc.scheme.pal, ui, format!("{} {}", ph::X, qymcad_i18n::tr("cmd-cancel-btn")));
    });
    if apply {
        apply_comp_array(pc);
    } else if cancel {
        *pc.carr = qymcad_ui_state::CompArrayCmd::default();
        pc.cmd.params.clear();
        *pc.status = qymcad_i18n::tr("msg-comp-array-cancelled");
    }
}

/// REOPEN A PATTERN OF COMPONENTS for editing (a double click on its row): the command as it was applied - the count,
/// the axis, the step or the angle, and the second and third directions of a grid.
pub fn open_comp_array_edit(pc: &mut qymcad_ui_state::PartCtx, pid: Id) {
    use qymcad_core::model::CompPatternKind;
    use qymcad_ui_state::CmdParam;
    let Some(pat) = pc.project.comp_pattern(pid) else { return };
    let (mode, dir, axis) = match pat.kind {
        CompPatternKind::Linear { dir, .. } => (1u8, axis_of(dir), 2u8),
        CompPatternKind::Circular { dir, .. } => (2u8, 0u8, axis_of(dir)),
    };
    *pc.carr = qymcad_ui_state::CompArrayCmd { mode, src: pat.src, dir, axis, edit: pid };
    *pc.mode_3d = true;
    match pat.kind {
        CompPatternKind::Circular { angle, count, axis, .. } => {
            pc.arr.count = count.max(1);
            pc.arr.axis = axis;
            pc.arr.full = (angle - 360.0).abs() < 0.1;
            pc.cmd.params = if pc.arr.full { vec![] } else { vec![CmdParam::new("f-angle", "cangle", angle, -3600.0, 3600.0)] };
        }
        CompPatternKind::Linear { step, count, more, .. } => {
            pc.arr.count = count.max(1);
            pc.cmd.params = vec![CmdParam::new("f-pitch", "cstep", step, 0.01, 100000.0)];
            pc.arr.two = more[0].2 > 1;
            pc.arr.three = pc.arr.two && more[1].2 > 1;
            (pc.arr.count2, pc.arr.dir2, pc.arr.count3, pc.arr.dir3) = (more[0].2.max(1), axis_of(more[0].0), more[1].2.max(1), axis_of(more[1].0));
            sync_comp_array_params(*pc.arr, pc.cmd);
            for (key, s) in [("cstep2", more[0].1), ("cstep3", more[1].1)] {
                if let Some(p) = pc.cmd.params.iter_mut().find(|p| p.key == key) {
                    (p.val, p.txt) = (s, qymcad_core::expr::fmt_num(s));
                }
            }
        }
    }
    *pc.status = qymcad_i18n::tr("msg-edit-comp-array");
}

/// THE STEP FIELDS OF A LINEAR PATTERN OF COMPONENTS follow its directions: a second and a third step appear with the
/// second and third direction ticked, and go with them, as a pattern of bodies has them.
pub fn sync_comp_array_params(arr: qymcad_ui_state::ArrayParams, cmd: &mut qymcad_ui_state::FeatCommand) {
    for (want, key, label) in [(arr.two, "cstep2", "f-pitch2"), (arr.two && arr.three, "cstep3", "f-pitch3")] {
        let has = cmd.params.iter().any(|p| p.key == key);
        if want && !has {
            cmd.params.push(qymcad_ui_state::CmdParam::new(label, key, 30.0, 0.01, 100000.0));
        } else if !want && has {
            cmd.params.retain(|p| p.key != key);
        }
    }
}

/// IS THIS TOOL IN HAND - open, OR waiting for the sketch it needs?
///
/// Reported behaviour: "press the tool and it stays highlighted, with the status asking for a sketch." It
/// did not: the wait lives in `Picking`, and the button lit from `armed` alone, so a tool that WAS waiting
/// looked untouched. A tool that acts and does not look taken is the same trouble as one that looks taken
/// and does not act.
pub(crate) fn tool_is_taken(bc: &qymcad_ui_state::BarCtx, kind: u8) -> bool {
    bc.armed.cmd_kind() == kind || bc.picking.sketch_for() == Some(kind)
}

pub fn feat_command_bar(pc: &mut qymcad_ui_state::PartCtx, ui: &mut egui::Ui) {
    // The panel lives inside a `Ui` now; the context is still wanted for windows,
    // input and viewport commands, and it comes from the same place.
    let ctx = &ui.ctx().clone();
    // the top row of the active Part command: the options + Apply/Cancel.
    // The SIZE itself is set on the canvas (the gizmo arrow or a field at the geometry), not here.
    if pc.armed.commanding() {
        let title = match pc.armed.cmd_kind() {
            1 => &qymcad_i18n::tr("cmd-extrude"),
            3 => &qymcad_i18n::tr("cmd-revolve"),
            8 => &qymcad_i18n::tr("cmd-sweep"),
            9 => &qymcad_i18n::tr("cmd-loft"),
            4 => &qymcad_i18n::tr("cmd-fillet"),
            5 => &qymcad_i18n::tr("cmd-chamfer"),
            6 => &qymcad_i18n::tr("cmd-shell"),
            7 => &qymcad_i18n::tr("cmd-hole"),
            10 => &qymcad_i18n::tr("cmd-box"),
            11 => &qymcad_i18n::tr("cmd-cylinder"),
            12 => &qymcad_i18n::tr("cmd-sphere"),
            13 => &qymcad_i18n::tr("cmd-cone"),
            14 => &qymcad_i18n::tr("cmd-torus"),
            15 => &qymcad_i18n::tr("cmd-prism"),
            16 => &qymcad_i18n::tr("cmd-mirror"),
            17 => &qymcad_i18n::tr("cmd-linear-array"),
            18 => &qymcad_i18n::tr("cmd-circular-array"),
            20 => &qymcad_i18n::tr("cmd-plane"),
            21 => &qymcad_i18n::tr("cmd-point"),
            22 => &qymcad_i18n::tr("cmd-axis"),
            23 => &qymcad_i18n::tr("cmd-draft"),
            24 => &qymcad_i18n::tr("cmd-thread"),
            25 => &qymcad_i18n::tr("f-push-face"),
            26 => &qymcad_i18n::tr("feat-name-remove-face"),
            27 => &qymcad_i18n::tr("feat-name-split-body"),
            28 => &qymcad_i18n::tr("feat-name-thicken"),
            29 => &qymcad_i18n::tr("feat-name-split-face"),
            30 => &qymcad_i18n::tr("feat-name-face-copy"),
            36 => &qymcad_i18n::tr("feat-name-offset-surface"),
            31 => &qymcad_i18n::tr("f-surface-replace"),
            32 => &qymcad_i18n::tr("feat-name-patch"),
            33 => &qymcad_i18n::tr("f-stitch"),
            34 => &qymcad_i18n::tr("feat-name-trim"),
            35 => &qymcad_i18n::tr("f-recognise"),
            _ => &qymcad_i18n::tr("cmd-command"),
        };
        let (mut apply, mut cancel, mut reselect) = (false, false, false);
        // The sentences the bar says are gathered here and laid out under the controls, not between them.
        let mut says = BarSays::default();
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new(format!("{} {title}", ph::CUBE)).strong());
            ui.separator();
            match pc.armed.cmd_kind() {
                1 => {
                    ui.label(qymcad_i18n::tr("cmd-operation"));
                    // a Part is one body, so there is "Add" (material into the single body, seeding it if
                    // it is the first). The former "New" and "Join" merged into "Add" - bodies are no
                    // longer bred.
                    ui.selectable_value(&mut pc.feat.op, 0u8, qymcad_i18n::tr("cmd-add"));
                    ui.selectable_value(&mut pc.feat.op, 2u8, qymcad_i18n::tr("cmd-cut"));
                    ui.selectable_value(&mut pc.feat.op, 3u8, qymcad_i18n::tr("cmd-intersect"));
                    ui.separator();
                    ui.label(qymcad_i18n::tr("cmd-extent"));
                    ui.selectable_value(&mut pc.cmd.extent, qymcad_ui_state::ExtentMode::Length, qymcad_i18n::tr("cmd-to-length"));
                    ui.selectable_value(&mut pc.cmd.extent, qymcad_ui_state::ExtentMode::Symmetric, qymcad_i18n::tr("cmd-symmetric"));
                    ui.selectable_value(&mut pc.cmd.extent, qymcad_ui_state::ExtentMode::TwoSided, qymcad_i18n::tr("cmd-two-sides"));
                    if pc.feat.op != 0 {
                        ui.selectable_value(&mut pc.cmd.extent, qymcad_ui_state::ExtentMode::Through, qymcad_i18n::tr("cmd-through-all"));
                    } else if pc.cmd.extent.through() {
                        pc.cmd.extent = qymcad_ui_state::ExtentMode::Length; // "through all" only applies to operations on a body
                    }
                    if pc.cmd.extent.two_sided() {
                        // the second side's distance is an expression field at the geometry (a popup), not here
                        says.tell(qymcad_i18n::tr("cmd-second-side-note"));
                    }
                    ui.separator();
                    // THE BUTTON SHOWS AND TURNS THE DIRECTION IN FORCE. A cut on a face goes into the body by itself,
                    // worked out only at Enter, so the button stood released over a cut already turned into the body
                    // and its first press turned it... into the body. Reported behaviour: a square on the top cut 5
                    // up, away from the block, still took 500 mm^3 out of it.
                    if pc.feat.flip_auto {
                        if let Some(si) = pc.cmd.sketch.filter(|i| *i < pc.project.sketches.len()) {
                            pc.feat.flip = smart_flip(pc.armed.cmd_kind(), pc.feat.op, &pc.project.sketches[si].plane);
                        }
                    }
                    if ui.selectable_label(pc.feat.flip, format!("{} {}", ph::ARROWS_DOWN_UP, qymcad_i18n::tr("cmd-flip-btn"))).on_hover_text(qymcad_i18n::tr("cmd-reverse-hint")).clicked() {
                        let f = !pc.feat.flip;
                        pc.feat.set_flip(f); // the direction was set by hand
                    }
                }
                35 => {
                    // WHAT COMES OUT OF THE MESH: the exact surfaces found on it, or the mesh as it is - a
                    // polyhedron of its flat faces. The choice belongs in the bar of the tool, where a person is
                    // already looking; as buttons in a node's properties it was never found at all.
                    if pc.cmd.edit.is_none() {
                        ui.label(qymcad_i18n::tr("cmd-operation"));
                        ui.selectable_value(&mut pc.feat.op, 0u8, qymcad_i18n::tr("cmd-recognise-exact"));
                        ui.selectable_value(&mut pc.feat.op, 1u8, qymcad_i18n::tr("cmd-recognise-asis"));
                    }
                    // A HEAVY MESH SAYS SO before Enter: how many triangles, and that the field "Simplify" makes it lighter
                    if let Some(n) = pc.recognise.src.and_then(|src| pc.project.mesh_index(src)).map(|mi| pc.project.bodies[mi].mesh.tris.len()).filter(|&n| n >= HEAVY_MESH) {
                        says.tell(format!("{} {}", ph::WARNING, qymcad_i18n::tr1("recognise-heavy", "n", &n.to_string())));
                    }
                    // WHAT WAS FOUND, before Enter: the regions by kind, and those that fit no surface
                    recognise_preview(pc, ctx);
                    if pc.recognise.src.is_some() {
                        ui.separator();
                        match pc.recognise.ready(|f| f.counts) {
                            Some(counts) => {
                                for (k, key) in
                                    ["recognise-planes", "recognise-cylinders", "recognise-cones", "recognise-spheres", "recognise-tori", "recognise-none", "recognise-free", "recognise-helices"]
                                        .into_iter()
                                        .enumerate()
                                {
                                    if counts[k] > 0 {
                                        ui.label(qymcad_i18n::tr1(key, "n", &counts[k].to_string()));
                                    }
                                }
                            }
                            None => {
                                ui.label(qymcad_i18n::tr("recognise-counting"));
                            }
                        }
                    }
                }
                3 => {
                    // A revolve operation on the part's single body: Add (a boss), Cut or Intersect.
                    ui.label(qymcad_i18n::tr("cmd-operation"));
                    ui.selectable_value(&mut pc.feat.op, 0u8, qymcad_i18n::tr("cmd-add"));
                    ui.selectable_value(&mut pc.feat.op, 2u8, qymcad_i18n::tr("cmd-cut"));
                    ui.selectable_value(&mut pc.feat.op, 3u8, qymcad_i18n::tr("cmd-intersect"));
                    ui.separator();
                    // how the angle is laid out: to one side, or symmetrically about half of it; Flip reverses it
                    ui.label(qymcad_i18n::tr("cmd-angle"));
                    ui.selectable_value(&mut pc.cmd.extent, qymcad_ui_state::ExtentMode::Length, qymcad_i18n::tr("cmd-one-side"));
                    ui.selectable_value(&mut pc.cmd.extent, qymcad_ui_state::ExtentMode::Symmetric, qymcad_i18n::tr("cmd-symmetric"));
                    if ui.selectable_label(pc.feat.flip, format!("{} {}", ph::ARROWS_DOWN_UP, qymcad_i18n::tr("cmd-flip-btn"))).on_hover_text(qymcad_i18n::tr("cmd-flip-angle-hint")).clicked() {
                        pc.feat.flip = !pc.feat.flip;
                    }
                    ui.separator();
                    ui.label(qymcad_i18n::tr("cmd-axis"));
                    // the axis comes from a sketch CENTRELINE, from an edge, a cylindrical face or a datum, or from X-Y.
                    let axis_src = if pc.rev.axis_line != 0 {
                        Some(qymcad_i18n::tr("cmd-sketch-centreline"))
                    } else if pc.rev.axis_datum != 0 {
                        Some(pc.project.datum_axes.iter().find(|d| d.id == pc.rev.axis_datum).map(|d| qymcad_i18n::name(&d.name)).unwrap_or_else(|| qymcad_i18n::tr("cmd-axis-lower")))
                    } else {
                        None
                    };
                    match axis_src {
                        Some(name) => {
                            ui.colored_label(pc.scheme.pal.connector(), format!("{} {name}", ph::CROSSHAIR));
                            let pair = format!("{}/{}", qymcad_ui_state::sketch_axis_name(&*pc.project, pc.cmd.sketch, 0), qymcad_ui_state::sketch_axis_name(&*pc.project, pc.cmd.sketch, 1));
                            if ui.small_button(format!("{} {pair}", ph::X)).on_hover_text(qymcad_i18n::tr("cmd-reset-axis-sketch")).clicked() {
                                pc.rev.axis_datum = 0;
                                pc.rev.axis_line = 0;
                                pc.rev.pick_axis = false;
                            }
                        }
                        None => {
                            // The two axes of the sketch's own plane, named by where they point in the
                            // world: on the front plane the second one IS the world Z, and calling it Y
                            // hid the axis a person came looking for.
                            for a in [0u8, 1] {
                                let name = qymcad_ui_state::sketch_axis_name(&*pc.project, pc.cmd.sketch, a);
                                ui.selectable_value(&mut pc.rev.axis, a, name).on_hover_text(qymcad_i18n::tr1("cmd-revolve-about-axis", "axis", name));
                            }
                        }
                    }
                    // A sketch CENTRELINE is a reliable choice made BY A BUTTON (with no click in 3D).
                    // For a sphere: a construction diameter line through the circle's centre IN the sketch plane.
                    let axis_lines = pc.cmd.sketch.map(|si| qymcad_ui_state::profile_axis_lines(&*pc.project, si)).unwrap_or_default();
                    if !axis_lines.is_empty() {
                        ui.separator();
                        if axis_lines.len() == 1 {
                            let l = axis_lines[0];
                            let on = pc.rev.axis_line == l;
                            let name = qymcad_ui_state::axis_line_label(&*pc.project, pc.cmd.sketch.unwrap_or(0), l, 1);
                            if ui.selectable_label(on, format!("{} {name}", ph::LINE_SEGMENT)).on_hover_text(qymcad_i18n::tr("cmd-revolve-any-line-hint")).clicked() {
                                pc.rev.axis_line = if on { 0 } else { l };
                                if pc.rev.axis_line != 0 {
                                    pc.rev.axis_datum = 0;
                                }
                            }
                        } else {
                            // With SEVERAL lines the choice is made BY CLICKING the line itself in the
                            // sketch rather than from a list of numbered lines: a number does not tell
                            // which of them is the one wanted.
                            let si_cur = pc.cmd.sketch.unwrap_or(0);
                            if ui
                                .selectable_label(pc.rev.pick_line, format!("{} {}", ph::CROSSHAIR, qymcad_i18n::tr("cmd-pick-axis-sketch")))
                                .on_hover_text(qymcad_i18n::tr("cmd-revolve-flat-hint"))
                                .clicked()
                            {
                                pc.rev.pick_line = !pc.rev.pick_line;
                                if pc.rev.pick_line {
                                    pc.rev.pick_axis = false;
                                    *pc.mode_3d = false; // the flat half-sketcher: the lines are visible and clickable
                                    pc.view.initialized = false;
                                    *pc.status = qymcad_i18n::tr("cmd-revolve-pick-line");
                                }
                            }
                            if pc.rev.axis_line != 0 {
                                let name = qymcad_ui_state::axis_line_label(&*pc.project, si_cur, pc.rev.axis_line, axis_lines.iter().position(|l| *l == pc.rev.axis_line).map(|i| i + 1).unwrap_or(1));
                                ui.label(egui::RichText::new(format!("{} {name}", ph::CHECK)).color(pc.scheme.pal.hint()));
                                if ui.small_button(qymcad_i18n::tr("cmd-reset-lower")).clicked() {
                                    pc.rev.axis_line = 0;
                                }
                            }
                            let _cur_unused = if pc.rev.axis_line != 0 {
                                axis_lines
                                    .iter()
                                    .position(|l| *l == pc.rev.axis_line)
                                    .map(|i| qymcad_ui_state::axis_line_label(&*pc.project, si_cur, pc.rev.axis_line, i + 1))
                                    .unwrap_or_else(|| qymcad_i18n::tr("cmd-sketch-axis"))
                            } else {
                                qymcad_i18n::tr("cmd-axis-from-sketch")
                            };
                            let _ = _cur_unused;
                            if pc.rev.axis_line != 0 {
                                pc.rev.axis_datum = 0;
                            }
                        }
                    }
                    ui.separator();
                    // the regular 3D axis pick: a STRAIGHT edge of a body, a CYLINDRICAL face or a datum axis
                    if ui.selectable_label(pc.rev.pick_axis, format!("{} {}", ph::CROSSHAIR, qymcad_i18n::tr("cmd-pick-axis-3d"))).on_hover_text(qymcad_i18n::tr("cmd-revolve-axis-hint")).clicked() {
                        pc.rev.pick_axis = !pc.rev.pick_axis;
                        if pc.rev.pick_axis {
                            pc.rev.pick_line = false;
                            // The axis candidates (datum axes, edges) are drawn and hit-tested ONLY in 3D.
                            // Staying in the flat half-sketcher would make the button do nothing: there
                            // would be nothing to click.
                            *pc.mode_3d = true;
                            pc.view.initialized = false;
                            refresh_axis_edges(pc); // the straight edges of every visible body are axis candidates
                            *pc.status = qymcad_i18n::tr("cmd-pick-axis-hint");
                        }
                    }
                }
                8 => {
                    // a sweep operation on the single body: Add, Cut or Intersect
                    ui.label(qymcad_i18n::tr("cmd-operation"));
                    ui.selectable_value(&mut pc.feat.op, 0u8, qymcad_i18n::tr("cmd-add"));
                    ui.selectable_value(&mut pc.feat.op, 2u8, qymcad_i18n::tr("cmd-cut"));
                    ui.selectable_value(&mut pc.feat.op, 3u8, qymcad_i18n::tr("cmd-intersect"));
                    ui.separator();
                    // Sweep: the profile readout + the contour choice + picking the path (a click on a sketch in the tree)
                    let prof_name = pc.project.sketches.iter().find(|s| s.id == pc.sweep.prof_sid).map(|s| qymcad_i18n::name(&s.name)).unwrap_or_else(|| "—".into());
                    ui.label(egui::RichText::new(qymcad_i18n::tr1("cmd-profile-is", "name", &prof_name)).color(pc.scheme.pal.hint()));
                    // choosing the profile's contour through the HALF-SKETCHER (a click on the contour), when there is more than one
                    let prof_cands = pc.project.sweep_profile_contours(pc.sweep.prof_sid);
                    if prof_cands.len() > 1 {
                        let cur = prof_cands.iter().position(|c| *c == pc.sweep.prof_cid).map(|i| i + 1).unwrap_or(1);
                        let act = pc.picking.contour() == Some(qymcad_ui_state::ContourSlot::SweepProfile);
                        if ui
                            .selectable_label(act, format!("{} {}", ph::CROSSHAIR, qymcad_i18n::tr2("cmd-pick-contour-n", "cur", &cur.to_string(), "total", &prof_cands.len().to_string())))
                            .on_hover_text(qymcad_i18n::tr("cmd-open-sketch-flat"))
                            .clicked()
                        {
                            begin_contour_pick(pc, qymcad_ui_state::ContourSlot::SweepProfile, pc.sweep.prof_sid);
                        }
                    }
                    ui.separator();
                    // the path is taken from the tree selection while the pick is active (as for a hole driven by a sketch)
                    if pc.sweep.pick_path {
                        if let qymcad_ui_state::Sel::Sketch(si) = *pc.sel {
                            if let Some(s) = pc.project.sketches.get(si) {
                                if s.id != pc.sweep.prof_sid {
                                    pc.sweep.path_sid = s.id;
                                    pc.sweep.path_cid = 0; // reset the contour choice on a new sketch
                                    pc.sweep.pick_path = false;
                                }
                            }
                        }
                    }
                    let path_txt = if pc.sweep.path_sid != 0 {
                        pc.project
                            .sketches
                            .iter()
                            .find(|s| s.id == pc.sweep.path_sid)
                            .map(|s| format!("{} {}", qymcad_i18n::tr1("cmd-path-is", "name", &qymcad_i18n::name(&s.name)), ph::CHECK))
                            .unwrap_or_else(|| qymcad_i18n::tr("cmd-path-unset"))
                    } else {
                        qymcad_i18n::tr("cmd-pick-path-sketch")
                    };
                    if ui.selectable_label(pc.sweep.pick_path, format!("{} {path_txt}", ph::LINE_SEGMENT)).on_hover_text(qymcad_i18n::tr("cmd-sweep-pick-path")).clicked() {
                        pc.sweep.pick_path = !pc.sweep.pick_path;
                    }
                    // choosing the path's contour through the HALF-SKETCHER (a click on the contour), when there is more than one
                    let path_cands = pc.project.sweep_path_contours(pc.sweep.path_sid);
                    if path_cands.len() > 1 {
                        let cur = path_cands.iter().position(|c| *c == pc.sweep.path_cid).map(|i| i + 1).unwrap_or(1);
                        let act = pc.picking.contour() == Some(qymcad_ui_state::ContourSlot::SweepPath);
                        if ui
                            .selectable_label(act, format!("{} {}", ph::CROSSHAIR, qymcad_i18n::tr2("cmd-pick-contour-n", "cur", &cur.to_string(), "total", &path_cands.len().to_string())))
                            .on_hover_text(qymcad_i18n::tr("cmd-open-path-flat"))
                            .clicked()
                        {
                            begin_contour_pick(pc, qymcad_ui_state::ContourSlot::SweepPath, pc.sweep.path_sid);
                        }
                    }
                    ui.separator();
                    says.tell(qymcad_i18n::tr("cmd-sweep-auto-hint"));
                }
                9 => {
                    // Loft: an ordered list of sections + adding by a click + ruled/smooth.
                    // A new section is taken on the EDGE of a change in the tree selection (while the pick
                    // is active). Only when the selected sketch changes - otherwise a section removed by
                    // its cross would come straight back while the same sketch stayed selected in the tree.
                    if pc.loft.pick {
                        let cur = if let qymcad_ui_state::Sel::Sketch(si) = *pc.sel { pc.project.sketches.get(si).map(|s| s.id) } else { None };
                        if cur != pc.loft.pick_last {
                            pc.loft.pick_last = cur;
                            if let Some(sid) = cur {
                                if !pc.loft.sids.contains(&sid) && !pc.project.sweep_profile_contours(sid).is_empty() {
                                    pc.loft.sids.push(sid);
                                    pc.loft.cids.push(0);
                                }
                            }
                        }
                    }
                    // the sections in order: the number + the name + a contour switch + delete
                    let mut remove: Option<usize> = None;
                    for i in 0..pc.loft.sids.len() {
                        let sid = pc.loft.sids[i];
                        let name = pc.project.sketches.iter().find(|s| s.id == sid).map(|s| qymcad_i18n::name(&s.name)).unwrap_or_else(|| "—".into());
                        ui.label(egui::RichText::new(format!("{}:{name}", i + 1)).color(pc.scheme.pal.hint()));
                        let cands = pc.project.sweep_profile_contours(sid);
                        if cands.len() > 1 {
                            let cur = cands.iter().position(|c| *c == pc.loft.cids[i]).map(|k| k + 1).unwrap_or(1);
                            let act = pc.picking.contour() == Some(qymcad_ui_state::ContourSlot::LoftSection(i));
                            if ui
                                .selectable_label(act, format!("{} {}", ph::CROSSHAIR, qymcad_i18n::tr2("cmd-pick-contour-short", "cur", &cur.to_string(), "total", &cands.len().to_string())))
                                .on_hover_text(qymcad_i18n::tr("cmd-open-section-flat"))
                                .clicked()
                            {
                                begin_contour_pick(pc, qymcad_ui_state::ContourSlot::LoftSection(i), sid);
                            }
                        }
                        if ui.small_button(ph::X).on_hover_text(qymcad_i18n::tr("cmd-remove-section")).clicked() {
                            remove = Some(i);
                        }
                        ui.separator();
                    }
                    if let Some(i) = remove {
                        pc.loft.sids.remove(i);
                        pc.loft.cids.remove(i);
                    }
                    // the "add a section" button (picking in the tree)
                    if ui.selectable_label(pc.loft.pick, format!("{} {}", ph::PLUS, qymcad_i18n::tr("cmd-add-section"))).on_hover_text(qymcad_i18n::tr("cmd-loft-pick-hint")).clicked() {
                        pc.loft.pick = !pc.loft.pick;
                        pc.loft.pick_last = None; // the pick is on, so the current selection may be taken on the next edge
                    }
                    ui.separator();
                    // the kind of surface between the sections
                    ui.label(qymcad_i18n::tr("cmd-faces"));
                    ui.selectable_value(&mut pc.loft.ruled, false, qymcad_i18n::tr("cmd-smooth"));
                    ui.selectable_value(&mut pc.loft.ruled, true, qymcad_i18n::tr("cmd-ruled"));
                    ui.separator();
                    // the kind of result: a separate body, or a boolean with the active body
                    ui.label(qymcad_i18n::tr("cmd-result"));
                    ui.selectable_value(&mut pc.loft.result, 0u8, qymcad_i18n::tr("cmd-add"));
                    ui.selectable_value(&mut pc.loft.result, 1u8, qymcad_i18n::tr("cmd-cut"));
                    ui.selectable_value(&mut pc.loft.result, 2u8, qymcad_i18n::tr("cmd-union"));
                    ui.selectable_value(&mut pc.loft.result, 3u8, qymcad_i18n::tr("cmd-intersection"));
                    // a surface through the sections: the same loft, not closed into a body
                    ui.selectable_value(&mut pc.loft.result, 4u8, qymcad_i18n::tr("cmd-surface"));
                    if pc.loft.result != 0 && pc.loft.result != 4 {
                        let has =
                            qymcad_ui_state::current_body(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: &*pc.set, scheme: pc.scheme, project: &*pc.project, active_path: pc.active_path }).is_some();
                        if has {
                            says.tell(qymcad_i18n::tr("cmd-bool-with-active"));
                        } else {
                            says.wrong(qymcad_i18n::tr("cmd-no-active-body"));
                        }
                    }
                    ui.separator();
                    ui.label(egui::RichText::new(qymcad_i18n::tr1("cmd-sections-n", "n", &pc.loft.sids.len().to_string())).weak().small());
                }
                5 => {
                    // Chamfer: the mode (symmetric, two distances, leg and angle) + the side of the reference face
                    use qymcad_core::feature::ChamferMode;
                    ui.label(qymcad_i18n::tr("cmd-mode"));
                    let prev = pc.chamfer.mode;
                    ui.selectable_value(&mut pc.chamfer.mode, ChamferMode::Symmetric, qymcad_i18n::tr("cmd-symmetric"));
                    ui.selectable_value(&mut pc.chamfer.mode, ChamferMode::TwoDist, qymcad_i18n::tr("cmd-two-distances"));
                    ui.selectable_value(&mut pc.chamfer.mode, ChamferMode::DistAngle, qymcad_i18n::tr("cmd-leg-angle"));
                    if pc.chamfer.mode != prev {
                        // the meaning of field d2 changed (mm <-> degrees), so both the label AND the value
                        // at the geometry are updated (45 mm as a second leg, or 1.5 deg as an angle, are
                        // both meaningless, hence the mode's default)
                        let def = if pc.chamfer.mode == ChamferMode::DistAngle { 45.0 } else { 1.5 };
                        if let Some(p) = pc.cmd.params.iter_mut().find(|p| p.key == "d2") {
                            p.set_label(qymcad_ui_state::chamfer_d2_label(pc.chamfer.mode));
                            p.val = def;
                            p.txt = format!("{def:.2}");
                        }
                    }
                    if pc.chamfer.mode != ChamferMode::Symmetric {
                        ui.separator();
                        if ui.selectable_label(pc.chamfer.flip, format!("{} {}", ph::ARROWS_DOWN_UP, qymcad_i18n::tr("cmd-side-btn"))).on_hover_text(qymcad_i18n::tr("cmd-swap-ref-face")).clicked() {
                            pc.chamfer.flip = !pc.chamfer.flip;
                        }
                        // picking the reference face by hand: a click on a face in 3D. It overrides "Side"
                        // for the edges adjacent to that face. Clicking the button again, or Reset, clears it.
                        ui.separator();
                        let lbl = if pc.chamfer.ref_face != 0 {
                            format!("{} {} {}", ph::CUBE, qymcad_i18n::tr("cmd-ref-face"), ph::CHECK)
                        } else {
                            format!("{} {}", ph::CUBE, qymcad_i18n::tr("cmd-ref-face"))
                        };
                        if ui.selectable_label(pc.chamfer.pick_ref, lbl).on_hover_text(qymcad_i18n::tr("cmd-chamfer-ref-hint")).clicked() {
                            pc.chamfer.pick_ref = !pc.chamfer.pick_ref;
                        }
                        if pc.chamfer.ref_face != 0 && ui.small_button(qymcad_i18n::tr("cmd-reset")).on_hover_text(qymcad_i18n::tr("cmd-neutral-auto-back")).clicked() {
                            pc.chamfer.ref_face = 0;
                            pc.chamfer.pick_ref = false;
                        }
                        says.tell(qymcad_i18n::tr("cmd-asym-note"));
                    }
                    ui.separator();
                    // A COUNT LIES WHEN THE SELECTION IS DESCRIBED. "Edges: 4" is a snapshot of today,
                    // while the description "every edge of this face" will take a fifth one tomorrow. What
                    // is actually recorded is written out in words.
                    let what = match &pc.gsel.described {
                        Some(_) => qymcad_i18n::tr1("expand-described", "what", &qymcad_i18n::tr("expand-face-edges")),
                        None => qymcad_i18n::tr1("cmd-edges-n", "n", &pc.gsel.edges.len().to_string()),
                    };
                    ui.label(egui::RichText::new(what).weak());
                }
                32 => {
                    // PATCH: smooth, or by position. A switch rather than a checkbox in the corner: these
                    // are two DIFFERENT surfaces on one boundary, and the choice is visible before Enter.
                    if ui.selectable_label(!pc.opts.patch_tangent, qymcad_i18n::tr("cmd-patch-flat")).clicked() {
                        pc.opts.patch_tangent = false;
                    }
                    if ui.selectable_label(pc.opts.patch_tangent, qymcad_i18n::tr("cmd-patch-tangent")).clicked() {
                        pc.opts.patch_tangent = true;
                    }
                    ui.separator();
                    let what = match &pc.gsel.described {
                        Some(_) => qymcad_i18n::tr1("expand-described", "what", &qymcad_i18n::tr("expand-face-edges")),
                        None => qymcad_i18n::tr1("cmd-edges-n", "n", &pc.gsel.edges.len().to_string()),
                    };
                    ui.label(egui::RichText::new(what).weak());
                }
                6 => {
                    // Shell: the direction of the thickness + the count of the multi-selected faces
                    ui.label(qymcad_i18n::tr("cmd-thickness"));
                    // a three-position mode: inwards, outwards or centred
                    use qymcad_core::feature::ShellSide;
                    for (side, word) in [(ShellSide::Inward, "cmd-inwards"), (ShellSide::Outward, "cmd-outwards"), (ShellSide::Centred, "cmd-centred")] {
                        if ui.selectable_label(pc.opts.shell_side == side, qymcad_i18n::tr(word)).clicked() {
                            pc.opts.shell_side = side;
                        }
                    }
                    ui.separator();
                    ui.label(egui::RichText::new(qymcad_i18n::tr1("cmd-open-faces-n", "n", &pc.gsel.faces.len().to_string())).color(pc.scheme.pal.hint()));
                }
                23 => {
                    // Draft: the set of faces to tilt + the neutral face (the fixed section plane) + a flip
                    ui.label(egui::RichText::new(qymcad_i18n::tr1("cmd-draft-faces-n", "n", &pc.gsel.faces.len().to_string())).color(pc.scheme.pal.selected()));
                    ui.separator();
                    let lbl = if pc.draft.neutral != 0 {
                        format!("{} {} {}", ph::CUBE, qymcad_i18n::tr("cmd-neutral-face"), ph::CHECK)
                    } else {
                        format!("{} {}", ph::CUBE, qymcad_i18n::tr("cmd-neutral-face"))
                    };
                    if ui.selectable_label(pc.draft.pick_neutral, lbl).on_hover_text(qymcad_i18n::tr("cmd-draft-neutral-hint")).clicked() {
                        pc.draft.pick_neutral = !pc.draft.pick_neutral;
                    }
                    if pc.draft.neutral != 0 && ui.small_button(qymcad_i18n::tr("cmd-reset")).on_hover_text(qymcad_i18n::tr("cmd-clear-neutral")).clicked() {
                        pc.draft.neutral = 0;
                        pc.draft.pick_neutral = false;
                    }
                    ui.separator();
                    if ui.selectable_label(pc.draft.flip, format!("{} {}", ph::ARROWS_DOWN_UP, qymcad_i18n::tr("cmd-flip-back"))).on_hover_text(qymcad_i18n::tr("cmd-flip-draft-hint")).clicked() {
                        pc.draft.flip = !pc.draft.flip;
                    }
                    ui.separator();
                    says.tell(qymcad_i18n::tr("cmd-angle-field-hint"));
                }
                24 => {
                    // Thread: inner/outer + the number of starts + the hand; the pitch, length, angle and depth live at the geometry
                    ui.label(qymcad_i18n::tr("cmd-kind"));
                    ui.selectable_value(&mut pc.thread.internal, false, qymcad_i18n::tr("cmd-thread-external")).on_hover_text(qymcad_i18n::tr("cmd-on-cylinder"));
                    ui.selectable_value(&mut pc.thread.internal, true, qymcad_i18n::tr("cmd-thread-internal")).on_hover_text(qymcad_i18n::tr("cmd-in-hole"));
                    ui.separator();
                    ui.label(qymcad_i18n::tr("cmd-starts"));
                    // The number of starts is a dimension of the part as well, and it must be parametric:
                    // a two-start thread driven by a global variable is an ordinary thing.
                    let st = qymcad_ui_state::num_or_expr(
                        &mut qymcad_ui_state::ExprBarCtx { bar_exprs: &mut *pc.bar_exprs, project: &*pc.project, scheme: &*pc.scheme },
                        ui,
                        "thread_starts",
                        pc.thread.starts.max(1) as f64,
                        qymcad_ui_state::NumFormat { lo: 1.0, hi: 12.0, integer: true, suffix: "", nonzero: false },
                    );
                    pc.thread.starts = (st as u32).max(1);
                    ui.separator();
                    if ui
                        .selectable_label(pc.thread.left, format!("{} {}", ph::ARROWS_COUNTER_CLOCKWISE, qymcad_i18n::tr("cmd-left-thread")))
                        .on_hover_text(qymcad_i18n::tr("cmd-thread-left"))
                        .clicked()
                    {
                        pc.thread.left = !pc.thread.left;
                    }
                    ui.separator();
                    // THE MODE: a thread (a groove) or an AUGER (a helical ribbon outwards)
                    if ui.selectable_label(!pc.thread.auger, format!("{} {}", ph::SPIRAL, qymcad_i18n::tr("cmd-thread-btn"))).clicked() && pc.thread.auger {
                        pc.thread.auger = false;
                        qymcad_ui_state::set_thread_params(&mut *pc.cmd, *pc.thread);
                        // the modes have different fields
                    }
                    if ui.selectable_label(pc.thread.auger, format!("{} {}", ph::SPIRAL, qymcad_i18n::tr("cmd-auger-btn"))).on_hover_text(qymcad_i18n::tr("cmd-auger-hint")).clicked()
                        && !pc.thread.auger
                    {
                        pc.thread.auger = true;
                        qymcad_ui_state::set_thread_params(&mut *pc.cmd, *pc.thread);
                    }
                    ui.separator();
                    if pc.thread.auger {
                        says.tell(qymcad_i18n::tr("cmd-auger-fields-hint"));
                    } else {
                        // THE STANDARD (the thread's type): the geometry is computed by the model core
                        ui.label(qymcad_i18n::tr("cmd-standard"));
                        use qymcad_core::thread::ThreadStandard as TS;
                        for (idx, std) in [(0u8, TS::MetricIso), (1, TS::TrapezoidalTr), (2, TS::Acme), (3, TS::RoundRd), (4, TS::Buttress), (5, TS::Custom)] {
                            let short = match idx {
                                1 => "Tr",
                                2 => "ACME",
                                3 => "Rd",
                                4 => &qymcad_i18n::tr("cmd-stop"),
                                5 => &qymcad_i18n::tr("cmd-custom"),
                                _ => "M",
                            };
                            if ui.selectable_label(pc.thread.form == idx, short).on_hover_text(qymcad_i18n::tr(std.label())).clicked() {
                                pc.thread.form = idx;
                                sync_custom_params(&mut *pc.cmd, *pc.thread); // "custom" opens the angle and the depth, the others hide them
                            }
                        }
                        ui.separator();
                        // a hint about the actual geometry of the chosen size
                        let spec = thread_spec(&*pc.cmd, *pc.thread);
                        let g = spec.geometry();
                        says.tell_hover(
                            qymcad_i18n::trn(
                                "cmd-thread-geom",
                                &[
                                    ("pitch", &qymcad_i18n::num(g.pitch, 2)),
                                    ("d2", &qymcad_i18n::num(g.pitch_d, 2)),
                                    ("d3", &qymcad_i18n::num(g.minor_d, 2)),
                                    ("depth", &qymcad_i18n::num(g.depth, 2)),
                                ],
                            ),
                            qymcad_i18n::tr("cmd-thread-std-hint"),
                        );
                        // WHAT THE MATING PART NEEDS, said out loud. Asked for plainly: a person must
                        // see what diameter of shaft or hole this thread wants and with what parameters
                        // to make its counterpart - otherwise the numbers get looked up in a table, and
                        // a table does not know about the fit that was typed in here.
                        let (own, mate) = spec.blank_diameters();
                        let key = if pc.thread.internal { "cmd-thread-mate-internal" } else { "cmd-thread-mate-external" };
                        says.tell_hover(qymcad_i18n::tr2(key, "own", &qymcad_i18n::num(own, 2), "mate", &qymcad_i18n::num(mate, 2)), qymcad_i18n::tr("cmd-thread-mate-hint"));
                        // A PROFILE THAT DOES NOT FIT THE PITCH is said out loud, with the numbers to
                        // change. It used to be taken in silence and built as rubbish: the passes
                        // overlap, eat the turn between them and leave flat plates that mate with
                        // nothing.
                        // THE CLEARANCE THAT DOES NOT FIT is named too. It saturates against the
                        // pitch without a word, and two different numbers typed in then give one and
                        // the same body - the pair binds and nothing explains why.
                        if let Some((asked, given)) = spec.fit_overflow() {
                            says.wrong(qymcad_i18n::trn(
                                "cmd-thread-fit-capped",
                                &[
                                    ("asked", &qymcad_i18n::num(asked, 2)),
                                    ("given", &qymcad_i18n::num(given, 3)),
                                    // WHAT IS LEFT OVER, said as a DIAMETER correction. The missing
                                    // clearance is measured along the flank, and a radial move is not
                                    // worth the same: a flank stands at the half-angle, so
                                    // `radial_relief` converts one into the other. Twice it, because a
                                    // diameter has two sides.
                                    ("rest", &qymcad_i18n::num(spec.radial_relief() * 2.0, 2)),
                                ],
                            ));
                        }
                        if let Some((width, max_depth, min_pitch)) = spec.profile_overflow() {
                            says.wrong(qymcad_i18n::trn(
                                "cmd-thread-too-wide",
                                &[
                                    ("width", &qymcad_i18n::num(width, 2)),
                                    ("pitch", &qymcad_i18n::num(spec.geometry().pitch, 2)),
                                    ("depth", &qymcad_i18n::num(max_depth, 2)),
                                    ("minpitch", &qymcad_i18n::num(min_pitch, 2)),
                                ],
                            ));
                        }
                    }
                    ui.separator();
                    let tgt = if pc.thread.edge == 0 {
                        qymcad_i18n::tr("cmd-thread-pick-hint")
                    } else {
                        qymcad_i18n::tr1("cmd-actual-diameter", "d", &qymcad_i18n::num(pc.thread.radius * 2.0, 1))
                    };
                    says.tell(tgt);
                }
                7 => {
                    // Hole: the placement mode (a face or a sketch) + the type; the diameter and depth live at the geometry
                    ui.label(qymcad_i18n::tr("cmd-placement"));
                    ui.selectable_value(&mut pc.hole.mode, 0u8, qymcad_i18n::tr("cmd-by-face"));
                    ui.selectable_value(&mut pc.hole.mode, 1u8, qymcad_i18n::tr("cmd-by-sketch"));
                    ui.separator();
                    ui.label(qymcad_i18n::tr("cmd-kind"));
                    ui.selectable_value(&mut pc.hole.kind, 0u8, qymcad_i18n::tr("cmd-simple"));
                    ui.selectable_value(&mut pc.hole.kind, 1u8, qymcad_i18n::tr("cmd-counterbore"));
                    ui.selectable_value(&mut pc.hole.kind, 2u8, qymcad_i18n::tr("cmd-countersink"));
                    ui.separator();
                    if let (0, Some(taken)) = (pc.hole.mode, taken_line(pc)) {
                        ui.label(taken);
                    }
                    if pc.hole.mode == 1 {
                        // "from a sketch": the selected sketch is taken, the number of marker points is shown, plus a normal flip
                        if let qymcad_ui_state::Sel::Sketch(si) = *pc.sel {
                            if let Some(s) = pc.project.sketches.get(si) {
                                pc.hole.sketch = Some(s.id);
                            }
                        }
                        let n = pc.hole.sketch.map(|sid| pc.project.sketch_isolated_points(sid).len()).unwrap_or(0);
                        let txt = match pc.hole.sketch {
                            Some(sid) => pc
                                .project
                                .sketches
                                .iter()
                                .find(|s| s.id == sid)
                                .map(|s| qymcad_i18n::tr2("cmd-sketch-points", "name", &qymcad_i18n::name(&s.name), "n", &n.to_string()))
                                .unwrap_or_else(|| qymcad_i18n::tr("cmd-sketch-unset")),
                            None => qymcad_i18n::tr("cmd-pick-sketch-points"),
                        };
                        let col = if n > 0 { pc.scheme.pal.hint() } else { pc.scheme.pal.hint_action() };
                        ui.label(egui::RichText::new(txt).color(col));
                        ui.separator();
                        ui.checkbox(&mut pc.hole.flip, qymcad_i18n::tr("cmd-flip"));
                    } else {
                        says.tell(qymcad_i18n::tr("cmd-hole-face-hint"));
                    }
                }
                15 => {
                    // Prism: the number of sides + a hint; the diameter and the height are fields at the geometry
                    ui.label(qymcad_i18n::tr("cmd-sides"));
                    pc.prim.n = qymcad_ui_state::num_or_expr(
                        &mut qymcad_ui_state::ExprBarCtx { bar_exprs: &mut *pc.bar_exprs, project: &*pc.project, scheme: &*pc.scheme },
                        ui,
                        "prim_n",
                        pc.prim.n as f64,
                        qymcad_ui_state::NumFormat { lo: 3.0, hi: 64.0, integer: true, suffix: "", nonzero: false },
                    ) as u32;
                    ui.separator();
                    says.tell(qymcad_i18n::tr("cmd-dia-height-hint"));
                }
                10..=14 => {
                    says.tell(qymcad_i18n::tr("cmd-sizes-hint"));
                }
                16 => {
                    // Mirror: keep the original + CLICK-PICK the plane in the viewport + a readout of what was picked
                    use qymcad_core::feature::SketchPlane;
                    ui.checkbox(&mut pc.opts.mirror_keep, qymcad_i18n::tr("cmd-with-original"));
                    ui.separator();
                    let picked = match &pc.mirror.plane {
                        Some(SketchPlane::World(bp)) => qymcad_i18n::tr1("cmd-world-plane", "plane", ["XY", "XZ", "YZ"][*bp as usize]),
                        Some(SketchPlane::Datum(id)) => pc
                            .project
                            .planes
                            .iter()
                            .find(|p| p.id == *id)
                            .map(|p| qymcad_i18n::tr1("cmd-datum-named", "name", &qymcad_i18n::name(&p.name)))
                            .unwrap_or_else(|| qymcad_i18n::tr("cmd-datum")),
                        Some(SketchPlane::Face(b, _)) => qymcad_i18n::tr1("cmd-body-face-n", "b", &b.to_string()),
                        None => qymcad_i18n::tr("cmd-pick-plane"),
                    };
                    ui.label(egui::RichText::new(qymcad_i18n::tr1("cmd-plane-is", "what", &picked)).color(pc.scheme.pal.hint()));
                }
                17 => {
                    if let Some(taken) = taken_line(pc) {
                        ui.label(taken);
                    }
                    // A linear pattern: the count + the direction (X/Y/Z) + a grid (a second direction);
                    // THE STEP is an expression field at the geometry (a popup). Ghost previews on the canvas.
                    ui.label(qymcad_i18n::tr("cmd-copies"));
                    pc.arr.count = qymcad_ui_state::num_or_expr(
                        &mut qymcad_ui_state::ExprBarCtx { bar_exprs: &mut *pc.bar_exprs, project: &*pc.project, scheme: &*pc.scheme },
                        ui,
                        "arr_count",
                        pc.arr.count as f64,
                        qymcad_ui_state::NumFormat { lo: 2.0, hi: 512.0, integer: true, suffix: "", nonzero: false },
                    ) as u32; // one copy is the body alone
                    ui.separator();
                    ui.label(qymcad_i18n::tr("cmd-direction"));
                    ui.selectable_value(&mut pc.arr.dir, 0u8, "X");
                    ui.selectable_value(&mut pc.arr.dir, 1u8, "Y");
                    ui.selectable_value(&mut pc.arr.dir, 2u8, "Z");
                    ui.separator();
                    ui.checkbox(&mut pc.arr.two, qymcad_i18n::tr("cmd-dir2"));
                    if pc.arr.two {
                        pc.arr.count2 = qymcad_ui_state::num_or_expr(
                            &mut qymcad_ui_state::ExprBarCtx { bar_exprs: &mut *pc.bar_exprs, project: &*pc.project, scheme: &*pc.scheme },
                            ui,
                            "arr_count2",
                            pc.arr.count2 as f64,
                            qymcad_ui_state::NumFormat { lo: 1.0, hi: 512.0, integer: true, suffix: "", nonzero: false },
                        ) as u32;
                        ui.selectable_value(&mut pc.arr.dir2, 0u8, "X");
                        ui.selectable_value(&mut pc.arr.dir2, 1u8, "Y");
                        ui.selectable_value(&mut pc.arr.dir2, 2u8, "Z");
                        ui.separator();
                        ui.checkbox(&mut pc.arr.three, qymcad_i18n::tr("cmd-dir3"));
                        if pc.arr.three {
                            pc.arr.count3 = qymcad_ui_state::num_or_expr(
                                &mut qymcad_ui_state::ExprBarCtx { bar_exprs: &mut *pc.bar_exprs, project: &*pc.project, scheme: &*pc.scheme },
                                ui,
                                "arr_count3",
                                pc.arr.count3 as f64,
                                qymcad_ui_state::NumFormat { lo: 1.0, hi: 512.0, integer: true, suffix: "", nonzero: false },
                            ) as u32;
                            ui.selectable_value(&mut pc.arr.dir3, 0u8, "X");
                            ui.selectable_value(&mut pc.arr.dir3, 1u8, "Y");
                            ui.selectable_value(&mut pc.arr.dir3, 2u8, "Z");
                        }
                    } else {
                        pc.arr.three = false; // the third direction only sits on top of the second (a full 3D grid)
                    }
                    ui.separator();
                    says.tell(qymcad_i18n::tr("cmd-pitch-field-hint"));
                }
                18 => {
                    if let Some(taken) = taken_line(pc) {
                        ui.label(taken);
                    }
                    // A circular pattern: the count + the axis by CLICK-PICK (a datum axis or a straight edge)
                    // + a full circle; THE ANGLE is an expression field at the geometry (when it is not a full
                    // circle). Ghost previews.
                    ui.label(qymcad_i18n::tr("cmd-copies"));
                    pc.arr.count = qymcad_ui_state::num_or_expr(
                        &mut qymcad_ui_state::ExprBarCtx { bar_exprs: &mut *pc.bar_exprs, project: &*pc.project, scheme: &*pc.scheme },
                        ui,
                        "arr_count",
                        pc.arr.count as f64,
                        qymcad_ui_state::NumFormat { lo: 2.0, hi: 512.0, integer: true, suffix: "", nonzero: false },
                    ) as u32; // one copy is the body alone
                    ui.separator();
                    ui.label(qymcad_i18n::tr("cmd-axis"));
                    let axname = if pc.arr.axis == 0 {
                        qymcad_i18n::tr("cmd-axis-world-z")
                    } else {
                        pc.project.datum_axes.iter().find(|d| d.id == pc.arr.axis).map(|d| qymcad_i18n::name(&d.name)).unwrap_or_else(|| qymcad_i18n::tr("cmd-axis-world-z"))
                    };
                    // the axis is CLICK-PICKED, as the mirror plane is, rather than chosen from a combo box
                    if ui.selectable_label(pc.arr.axis_pick, format!("{} {axname}", ph::CROSSHAIR)).on_hover_text(qymcad_i18n::tr("cmd-revolve-pick-axis3d")).clicked() {
                        pc.arr.axis_pick = !pc.arr.axis_pick;
                        if pc.arr.axis_pick {
                            refresh_axis_edges(pc); // the straight edges of EVERY visible body, as axis candidates
                            *pc.status = qymcad_i18n::tr("cmd-array-axis-hint");
                        }
                    }
                    if pc.arr.axis != 0 && ui.small_button("Z").on_hover_text(qymcad_i18n::tr("cmd-reset-axis-z")).clicked() {
                        pc.arr.axis = 0;
                        pc.arr.axis_pick = false;
                    }
                    ui.separator();
                    ui.checkbox(&mut pc.arr.full, qymcad_i18n::tr("cmd-full-circle"));
                    if !pc.arr.full {
                        says.tell(qymcad_i18n::tr("cmd-angle-field-lower"));
                    }
                }
                20 => {
                    // A datum PLANE: click-pick a base plane, a datum or a face + the offset at the geometry
                    use qymcad_core::feature::SketchPlane;
                    let picked = match &pc.datum.plane_pick {
                        Some(SketchPlane::World(bp)) => qymcad_i18n::tr1("cmd-world-plane", "plane", ["XY", "XZ", "YZ"][*bp as usize]),
                        Some(SketchPlane::Datum(id)) => pc
                            .project
                            .planes
                            .iter()
                            .find(|p| p.id == *id)
                            .map(|p| qymcad_i18n::tr1("cmd-datum-named", "name", &qymcad_i18n::name(&p.name)))
                            .unwrap_or_else(|| qymcad_i18n::tr("cmd-datum")),
                        Some(SketchPlane::Face(b, _)) => qymcad_i18n::tr1("cmd-body-face-n", "b", &b.to_string()),
                        None => qymcad_i18n::tr("cmd-pick-plane"),
                    };
                    ui.label(egui::RichText::new(qymcad_i18n::tr1("cmd-from-is", "what", &picked)).color(pc.scheme.pal.hint()));
                    ui.separator();
                    says.tell(qymcad_i18n::tr("cmd-offset-field-hint"));
                }
                21 => {
                    // A datum POINT: X/Y/Z coordinates, or associatively at a vertex (by a click)
                    let prev = pc.datum.pt_mode;
                    ui.label(qymcad_i18n::tr("cmd-method"));
                    ui.selectable_value(&mut pc.datum.pt_mode, 0u8, qymcad_i18n::tr("cmd-coordinates"));
                    ui.selectable_value(&mut pc.datum.pt_mode, 1u8, qymcad_i18n::tr("cmd-to-vertex")).on_hover_text(qymcad_i18n::tr("cmd-vertex-assoc-hint"));
                    if prev != pc.datum.pt_mode {
                        sync_datum_point_params(&*pc.armed, &mut *pc.cmd, &*pc.datum);
                    }
                    ui.separator();
                    if pc.datum.pt_mode == 1 {
                        let r = if pc.datum.pt_vert.is_some() { format!("{} {}", qymcad_i18n::tr("cmd-vertex-picked"), ph::CHECK) } else { qymcad_i18n::tr("cmd-pick-vertex") };
                        ui.label(egui::RichText::new(r).color(pc.scheme.pal.hint()));
                    } else {
                        says.tell(qymcad_i18n::tr("cmd-xyz-hint"));
                    }
                }
                22 => {
                    // A datum AXIS: by a click on an edge or a face, by two points, or by hand (origin and direction at the geometry)
                    ui.label(qymcad_i18n::tr("cmd-method"));
                    ui.selectable_value(&mut pc.datum.axis_mode, 0u8, qymcad_i18n::tr("cmd-by-edge-face"));
                    ui.selectable_value(&mut pc.datum.axis_mode, 2u8, qymcad_i18n::tr("cmd-two-points"));
                    ui.selectable_value(&mut pc.datum.axis_mode, 1u8, qymcad_i18n::tr("cmd-manual"));
                    ui.separator();
                    match pc.datum.axis_mode {
                        0 => {
                            let r = if pc.datum.axis_ref.is_some() { format!("{} {}", qymcad_i18n::tr("cmd-axis-picked"), ph::CHECK) } else { qymcad_i18n::tr("cmd-pick-edge-cyl") };
                            ui.label(egui::RichText::new(r).color(pc.scheme.pal.hint()));
                        }
                        2 => {
                            ui.label(egui::RichText::new(qymcad_i18n::tr1("cmd-points-n", "n", &pc.datum.axis_pts.len().to_string())).color(pc.scheme.pal.hint()));
                        }
                        _ => {
                            says.tell(qymcad_i18n::tr("cmd-origin-dir-hint"));
                        }
                    }
                }
                _ => {
                    says.tell(cmd_hint(&*pc.armed, &*pc.gsel, &*pc.trim));
                    if let Some(taken) = taken_line(pc) {
                        ui.label(taken);
                    }
                }
            }
            // a value the geometry refuses closes this Apply as it closes the popup's (`trial`)
            let refused = trial_refusal(pc, ui.ctx()) != Trial::Clear;
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // the button is disabled while a dimension's expression is invalid (nothing stale is applied)
                if confirm_button(
                    &pc.scheme.pal,
                    ui,
                    cmd_ready(pc) && cmd_exprs_valid(&*pc.cmd, &*pc.project) && qymcad_ui_state::bar_fields_valid(ui.ctx()) && !refused,
                    format!("{} {}", ph::CHECK, qymcad_i18n::tr("cmd-apply-enter")),
                ) {
                    apply = true;
                }
                if refuse_button(&pc.scheme.pal, ui, format!("{} {}", ph::X, qymcad_i18n::tr("cmd-cancel-esc"))) {
                    cancel = true;
                }
                // the "pick contours" button belongs to the 3D step (setting the size) only - inside the
                // half-sketcher (mode_3d=false) contours are ALREADY being picked, so duplicating it there
                // serves nothing.
                if matches!(pc.armed.cmd_kind(), 1 | 3) && pc.cmd.sketch.is_some() && *pc.mode_3d {
                    // how many contours are taken, named in the bar: a click on one in the view adds or lets it go
                    ui.label(qymcad_i18n::tr1("cmd-profiles-n", "n", &pc.gsel.profiles.len().to_string()));
                    // THE KEY IN THE HINT FOLLOWS THE FOCUS STATE: "U" or "Alt+U". Otherwise the rule that
                    // a focused field needs Alt would stay a secret, and someone who pressed `U` in a field
                    // and got nothing would not try a second time.
                    let hint = format!(
                        "{}  ({})",
                        qymcad_i18n::tr("cmd-back-to-contours"),
                        qymcad_ui_state::hotkey_hint(
                            &qymcad_ui_state::DrawCtx { cam: pc.cam, set: &*pc.set, scheme: pc.scheme, project: &*pc.project, active_path: pc.active_path },
                            ctx,
                            "part.contour-reselect"
                        )
                    );
                    if ui.button(format!("{} {}", ph::POLYGON, qymcad_i18n::tr("cmd-pick-contours"))).on_hover_text(&hint).clicked() {
                        reselect = true;
                    }
                }
            });
        });
        says.show(ui, &pc.scheme.pal);
        if apply {
            apply_feat_cmd(pc);
        }
        if cancel {
            qymcad_ui_state::cancel_feat_cmd(pc);
        }
        if reselect {
            enter_contour_reselect(pc);
        }
        // after the bar (the extent mode is chosen) the "second side" field at the geometry is synced
        sync_dir_cmd_params(&*pc.armed, &mut *pc.cmd, &*pc.project);
        sync_torus_params(&*pc.armed, &mut *pc.cmd, &*pc.project); // the tube thinner than the ring
                                                                   // the pattern: a second step when there is a second direction, and an angle when it is not a full circle
        sync_array_params(*pc.arr, &*pc.armed, &mut *pc.cmd, &*pc.project);
        // the datum axis: the origin and direction fields in manual mode
        sync_datum_axis_params(&*pc.armed, &mut *pc.cmd, &*pc.datum);
        sync_hole_params(&*pc.armed, &mut *pc.cmd, *pc.hole, &*pc.project); // the recess fields for a counterbore or a countersink
    }
}

/// The shared Create buttons: THE DATUMS — they exist both in a Part and in an Assembly. A sketch is
/// added separately and ONLY in a Part (see `create_panel_part`).
///
/// SKETCHES ARE GONE FROM ASSEMBLIES. The button stood there marked "skeleton", so the intent of
/// top-down layout was there, but nothing could refer to that skeleton: a joint anchor understands
/// `Origin`, `BasePlane`, `FaceCenter`, `EdgeMid` and `Vertex` — there is no reference to a sketch, and
/// there is no extrude in the Assembly workbench either. A sketch there was inert: it could be drawn
/// but not used. Layout belongs in the part-modelling workbench.
///
/// DATUMS STAY IN AN ASSEMBLY — they are not inert: a mirrored copy of a component and the view's
/// section both consume them.
pub fn create_panel_common(bc: &mut qymcad_ui_state::BarCtx, ui: &mut egui::Ui) {
    use qymcad_core::feature::{BasePlane, SketchPlane};
    let _ = (BasePlane::XY, SketchPlane::default);
    // Datums are COMMANDS: an options bar, fields at the geometry, click-picked references, a preview, Enter and Esc
    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::DatumPlane, &qymcad_i18n::tr("g-datum-plane-hint"), bc.armed.cmd_kind() == 20) {
        bc.ask.push(qymcad_ui_state::take_or_drop(bc.armed.cmd_kind() == 20, qymcad_ui_state::BarAsk::FeatCmd(20)));
    }
    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::DatumPoint, &qymcad_i18n::tr("g-datum-point-hint"), bc.armed.cmd_kind() == 21) {
        bc.ask.push(qymcad_ui_state::take_or_drop(bc.armed.cmd_kind() == 21, qymcad_ui_state::BarAsk::FeatCmd(21)));
    }
    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::DatumAxis, &qymcad_i18n::tr("g-datum-axis-hint"), bc.armed.cmd_kind() == 22) {
        bc.ask.push(qymcad_ui_state::take_or_drop(bc.armed.cmd_kind() == 22, qymcad_ui_state::BarAsk::FeatCmd(22)));
    }
}

/// The one doorway into the properties panel.
/// The Sketch button — ONLY in the Part workbench (in an assembly a sketch is inert, see `create_panel_common`).
pub fn create_panel_sketch_button(bc: &mut qymcad_ui_state::BarCtx, ui: &mut egui::Ui) {
    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::SketchPickPlane, &qymcad_i18n::tr("g-sketch-pick-hint"), bc.picking.is_sketch_plane()) {
        bc.ask.push(qymcad_ui_state::BarAsk::ToggleSketchPick);
    }
}

pub fn toolbar(bc: &mut qymcad_ui_state::BarCtx, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        // CONTEXT BREADCRUMBS instead of tabs (drilling in and out): Assembly > Part > ... (> Sketch)
        qymcad_ui_state::ensure_active_path(&mut *bc.active_path, &mut *bc.project);
        let path = bc.active_path.clone();
        for (i, cid) in path.iter().enumerate() {
            if i > 0 {
                ui.label("›");
            }
            let name = bc.project.components.iter().find(|c| c.id == *cid).map(|c| qymcad_i18n::name(&c.name)).unwrap_or_else(|| qymcad_i18n::tr("wb-assembly"));
            // the root and an assembly wear the same icon: both hold other things rather than being a shape
            let assembly = i == 0 || bc.project.component_kind(*cid) == Some(qymcad_core::feature::ComponentKind::Assembly);
            let icon = if assembly { ph::STACK } else { ph::CUBE };
            let is_here = i + 1 == path.len() && bc.sketch_ses.editing.is_none();
            if ui.selectable_label(is_here, format!("{icon} {name}")).clicked() {
                bc.ask.push(qymcad_ui_state::BarAsk::GotoPath(i));
            }
        }
        if let Some(sid) = bc.sketch_ses.editing {
            ui.label("›");
            let nm = bc.project.sketches.iter().find(|s| s.id == sid).map(|s| qymcad_i18n::name(&s.name)).unwrap_or_else(|| qymcad_i18n::tr("wb-sketch"));
            ui.label(egui::RichText::new(format!("{} {nm}", ph::PENCIL)).color(bc.scheme.pal.hint()));
        }
        if (bc.sketch_ses.editing.is_some() || bc.active_path.len() > 1) && ui.button(qymcad_i18n::tr("wb-finish")).on_hover_text(qymcad_i18n::tr("wb-finish-hint")).clicked() {
            bc.ask.push(qymcad_ui_state::BarAsk::ExitContext);
        }
        ui.separator();
        // the global parameters are reachable from any workbench
        if ui.selectable_label(bc.win.is(WinKind::Params), qymcad_i18n::tr("wb-params")).on_hover_text(qymcad_i18n::tr("wb-params-hint")).clicked() {
            bc.win.toggle(WinKind::Params);
        }
        ui.separator();

        if let Some(pts) = bc.pending_import.draw_pts.as_ref() {
            // a sketch is being drawn: finish or cancel
            let n = pts.len();
            ui.label(qymcad_i18n::tr1("wb-points-n", "n", &n.to_string()));
            if ui.button(qymcad_i18n::tr("wb-close")).clicked() {
                qymcad_ui_state::finish_drawing(qymcad_ui_state::editing_in!(bc), &mut *bc.pending_import, *bc.sketch_ses, true);
            }
            if ui.button(qymcad_i18n::tr("wb-line")).clicked() {
                qymcad_ui_state::finish_drawing(qymcad_ui_state::editing_in!(bc), &mut *bc.pending_import, *bc.sketch_ses, false);
            }
            if ui.button(qymcad_i18n::tr("wb-cancel")).clicked() {
                bc.pending_import.draw_pts = None;
            }
            ui.separator();
        }

        // Snap and the grid: while DRAWING a sketch (the cursor) AND in 3D inside a Part or an Assembly (snapping the gizmo to the grid).
        let snap_ctx = bc.sketch_ses.editing.is_some() || (*bc.mode_3d && matches!(bc.workbench, qymcad_ui_state::Workbench::Part | qymcad_ui_state::Workbench::Assembly));
        if snap_ctx {
            ui.toggle_value(&mut bc.set.snap.on, format!("{} {}", ph::MAGNET, qymcad_i18n::tr("wb-snap"))).on_hover_text(qymcad_i18n::tr("wb-snap-hint"));
            if bc.set.snap.on {
                ui.add(egui::DragValue::new(&mut bc.set.snap.grid).speed(0.5).range(0.1..=100.0).prefix(qymcad_i18n::tr("wb-grid")).suffix(qymcad_i18n::tr("unit-mm-suffix")));
                // the gizmo's ROTATION step belongs to 3D inside a Part or an Assembly (a sketch has no use for it)
                if *bc.mode_3d && matches!(bc.workbench, qymcad_ui_state::Workbench::Part | qymcad_ui_state::Workbench::Assembly) {
                    ui.add(egui::DragValue::new(&mut bc.set.snap.rot_deg).speed(1.0).range(0.5..=90.0).prefix(qymcad_i18n::tr("wb-rotation")).suffix(qymcad_i18n::tr("unit-deg-suffix")));
                }
            }
            // the automatic constraints belong to drawing a sketch only
            if bc.sketch_ses.editing.is_some() {
                ui.toggle_value(&mut bc.set.auto_constrain, format!("{} {}", ph::MAGIC_WAND, qymcad_i18n::tr("wb-auto-constraints"))).on_hover_text(qymcad_i18n::tr("wb-auto-constraints-hint"));
            }
            ui.separator();
        }
        // "in context" applies inside ANY component (a part or a subassembly), not at the root: top-down
        // references to the neighbours + showing the PARENT datums. The root has no ancestors, so it is hidden there.
        if qymcad_ui_state::current_ctx_id(&*bc.active_path, &*bc.project) != bc.project.root {
            ui.toggle_value(&mut bc.win.context, format!("{} {}", ph::STACK, qymcad_i18n::tr("wb-in-context"))).on_hover_text(qymcad_i18n::tr("wb-in-context-hint"));
            ui.separator();
        }
    });
}

pub fn wb_toolbar(bc: &mut qymcad_ui_state::BarCtx, ui: &mut egui::Ui) {
    // Two columns of buttons (wrapping by width) + vertical scrolling - reliable at any window size.
    // The width 94 = 16 (the 8+8 margins) + 6 (the floating scrollbar) + 72 (two 34-wide button columns + a
    // 3-point gap + a margin). Before that, 90 with 38-wide buttons could not fit the second column, giving
    // one column and a wide empty strip on the right.
    // show_separator_line(false): by default egui draws a faint vertical line at the right edge of ANY panel,
    // and it read as a stray gap between the tools and the tree.
    ui.add_space(6.0);
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        ui.spacing_mut().item_spacing = egui::vec2(3.0, 3.0);
        ui.horizontal_wrapped(|ui| {
            match bc.workbench {
                qymcad_ui_state::Workbench::Sketch => {
                    // a full-width category label, which therefore starts a new row (breaking the columns)
                    let cat = |ui: &mut egui::Ui, s: &str| {
                        ui.add_sized([84.0, 12.0], egui::Label::new(egui::RichText::new(s).size(8.5).weak()));
                    };
                    // "Finish" lives in the breadcrumbs (one place for it); here there are only sketch tools
                    // --- Creation ---
                    cat(ui, &qymcad_i18n::tr("tb-group-create"));
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::SketchSelect, &qymcad_i18n::tr("tb-select-hint"), qymcad_ui_state::in_select_mode(bc.armed)) {
                        bc.ask.push(qymcad_ui_state::BarAsk::SketchSelectMode);
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::SketchPoint, &qymcad_i18n::tr("tb-point-hint"), bc.armed.draw_kind() == 5) {
                        bc.ask.push(qymcad_ui_state::take_or_drop(bc.armed.draw_kind() == 5, qymcad_ui_state::BarAsk::SketchTool(5)));
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::SketchLine, &qymcad_i18n::tr("tb-line-hint"), bc.armed.draw_kind() == 1) {
                        bc.ask.push(qymcad_ui_state::take_or_drop(bc.armed.draw_kind() == 1, qymcad_ui_state::BarAsk::SketchTool(1)));
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::SketchRect, &qymcad_i18n::tr("tb-rect-hint"), bc.armed.draw_kind() == 2) {
                        bc.ask.push(qymcad_ui_state::take_or_drop(bc.armed.draw_kind() == 2, qymcad_ui_state::BarAsk::SketchTool(2)));
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::SketchCircle, &qymcad_i18n::tr("tb-circle-hint"), bc.armed.draw_kind() == 3) {
                        bc.ask.push(qymcad_ui_state::take_or_drop(bc.armed.draw_kind() == 3, qymcad_ui_state::BarAsk::SketchTool(3)));
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::SketchCircle3Pt, &qymcad_i18n::tr("tb-circle-3pt"), bc.armed.draw_kind() == 10) {
                        bc.ask.push(qymcad_ui_state::take_or_drop(bc.armed.draw_kind() == 10, qymcad_ui_state::BarAsk::SketchTool(10)));
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::SketchArc, &qymcad_i18n::tr("tb-arc-hint"), bc.armed.draw_kind() == 4) {
                        bc.ask.push(qymcad_ui_state::take_or_drop(bc.armed.draw_kind() == 4, qymcad_ui_state::BarAsk::SketchTool(4)));
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::SketchPolygon, &qymcad_i18n::tr("tb-polygon-hint"), bc.armed.draw_kind() == 6) {
                        bc.ask.push(qymcad_ui_state::take_or_drop(bc.armed.draw_kind() == 6, qymcad_ui_state::BarAsk::SketchTool(6)));
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::SketchSlot, &qymcad_i18n::tr("tb-slot-hint"), bc.armed.draw_kind() == 7) {
                        bc.ask.push(qymcad_ui_state::take_or_drop(bc.armed.draw_kind() == 7, qymcad_ui_state::BarAsk::SketchTool(7)));
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::SketchEllipse, &qymcad_i18n::tr("tb-ellipse-hint"), bc.armed.draw_kind() == 8) {
                        bc.ask.push(qymcad_ui_state::take_or_drop(bc.armed.draw_kind() == 8, qymcad_ui_state::BarAsk::SketchTool(8)));
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::SketchSpline, &qymcad_i18n::tr("tb-spline-hint"), bc.armed.draw_kind() == 9) {
                        bc.ask.push(qymcad_ui_state::take_or_drop(bc.armed.draw_kind() == 9, qymcad_ui_state::BarAsk::SketchTool(9)));
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::SketchText, &qymcad_i18n::tr("tb-text-hint"), bc.armed.draw_kind() == 11) {
                        bc.ask.push(qymcad_ui_state::take_or_drop(bc.armed.draw_kind() == 11, qymcad_ui_state::BarAsk::SketchTool(11)));
                    }
                    // --- The line kind ---
                    cat(ui, &qymcad_i18n::tr("tb-type"));
                    // WHAT IS SELECTED IS TURNED, as the construction toggle of the professional systems does; with nothing selected
                    // the button switches what is drawn next
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::SketchConstruction, &qymcad_i18n::tr("tb-construction-hint"), bc.tool.construction) {
                        qymcad_ui_state::construction_toggle(qymcad_ui_state::editing_in!(bc), &*bc.sel_sk, &*bc.sketch_ses, &mut bc.tool.construction);
                    }
                    // --- Editing and replication (over the selected entities) ---
                    cat(ui, &qymcad_i18n::tr("tb-group-edit"));
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::SketchDelete, &qymcad_i18n::tr("tb-delete-hint"), bc.sel_sk.modify == Some(qymcad_ui_state::EditTool::Delete)) {
                        qymcad_ui_state::modify_button(qymcad_ui_state::editing_in!(bc), &mut qymcad_ui_state::tools_in!(bc), *bc.sk_pat, &*bc.tool_prefs, qymcad_ui_state::EditTool::Delete);
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::SketchMirror, &qymcad_i18n::tr("tb-mirror-sketch-hint"), bc.sel_sk.modify == Some(qymcad_ui_state::EditTool::Mirror)) {
                        qymcad_ui_state::modify_button(qymcad_ui_state::editing_in!(bc), &mut qymcad_ui_state::tools_in!(bc), *bc.sk_pat, &*bc.tool_prefs, qymcad_ui_state::EditTool::Mirror);
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::SketchArrayLinear, &qymcad_i18n::tr("tb-lin-array-hint"), bc.armed.pat_op() == 1) {
                        start_pattern(&mut qymcad_ui_state::tools_in!(bc), &mut *bc.status, 1);
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::SketchArrayCircular, &qymcad_i18n::tr("tb-circ-array-hint"), bc.armed.pat_op() == 2) {
                        start_pattern(&mut qymcad_ui_state::tools_in!(bc), &mut *bc.status, 2);
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::SketchProject, &qymcad_i18n::tr("tb-project-body-hint"), bc.armed.click_op() == 6) {
                        qymcad_ui_state::set_click_op(&mut qymcad_ui_state::tools_in!(bc), &mut *bc.mode_3d, 6);
                        *bc.status = qymcad_i18n::tr("tb-project-hint");
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::SketchFillet, &qymcad_i18n::tr("tb-fillet-sketch-hint"), bc.armed.click_op() == 4) {
                        qymcad_ui_state::start_corner_tool(bc, 4);
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::SketchChamfer, &qymcad_i18n::tr("tb-chamfer-sketch-hint"), bc.armed.click_op() == 5) {
                        qymcad_ui_state::start_corner_tool(bc, 5);
                    }
                    // "FILLET ALL" IS A TOOL IN HAND like its neighbours: pressed, it puts down what is held and takes what is
                    // selected or waits for a shape; pressed again, it is put down
                    let fillet_all_held = bc.picking.fillet_all() || bc.corner.at.is_some_and(|(_, pid, _)| pid == 0);
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::SketchFilletAll, &qymcad_i18n::tr("tb-fillet-all-hint"), fillet_all_held) {
                        qymcad_ui_state::exit_draw_tools(&mut qymcad_ui_state::tools_in!(bc));
                        if !fillet_all_held {
                            qymcad_ui_state::fillet_all_corners(&*bc.project, &mut *bc.corner, &mut *bc.picking, *bc.sel, &*bc.sel_sk, &mut *bc.status, &*bc.tool_prefs);
                        }
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::SketchOffset, &qymcad_i18n::tr("tb-offset-hint"), bc.sel_sk.modify == Some(qymcad_ui_state::EditTool::Offset)) {
                        qymcad_ui_state::modify_button(qymcad_ui_state::editing_in!(bc), &mut qymcad_ui_state::tools_in!(bc), *bc.sk_pat, &*bc.tool_prefs, qymcad_ui_state::EditTool::Offset);
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::SketchMove, &qymcad_i18n::tr("tb-move-hint"), bc.armed.move_op() == 1) {
                        start_move_tool(&mut qymcad_ui_state::tools_in!(bc), &mut *bc.status, 1);
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::SketchCopy, &qymcad_i18n::tr("tb-copy-hint"), bc.armed.move_op() == 2) {
                        start_move_tool(&mut qymcad_ui_state::tools_in!(bc), &mut *bc.status, 2);
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::SketchRotate, &qymcad_i18n::tr("tb-rotate-hint"), bc.armed.move_op() == 3) {
                        start_move_tool(&mut qymcad_ui_state::tools_in!(bc), &mut *bc.status, 3);
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::SketchTrim, &qymcad_i18n::tr("tb-trim-hint"), bc.armed.click_op() == 1) {
                        qymcad_ui_state::set_click_op(&mut qymcad_ui_state::tools_in!(bc), &mut *bc.mode_3d, 1);
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::SketchExtend, &qymcad_i18n::tr("tb-extend-hint"), bc.armed.click_op() == 2) {
                        qymcad_ui_state::set_click_op(&mut qymcad_ui_state::tools_in!(bc), &mut *bc.mode_3d, 2);
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::SketchBreak, &qymcad_i18n::tr("tb-break-hint"), bc.armed.click_op() == 3) {
                        qymcad_ui_state::set_click_op(&mut qymcad_ui_state::tools_in!(bc), &mut *bc.mode_3d, 3);
                    }
                    // --- Dimensions ---
                    cat(ui, &qymcad_i18n::tr("tb-group-dim"));
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::SketchDimLinear, &qymcad_i18n::tr("tb-dim-hint"), bc.armed.dim_kind() == 1) {
                        qymcad_ui_state::set_dim_tool(&mut qymcad_ui_state::tools_in!(bc), &mut *bc.mode_3d, &*bc.project, *bc.sel, *bc.sketch_ses, &mut *bc.status, 1);
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::SketchDimAngle, &qymcad_i18n::tr("tb-dim-angle-hint"), bc.armed.dim_kind() == 2) {
                        qymcad_ui_state::set_dim_tool(&mut qymcad_ui_state::tools_in!(bc), &mut *bc.mode_3d, &*bc.project, *bc.sel, *bc.sketch_ses, &mut *bc.status, 2);
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::SketchDimRadius, &qymcad_i18n::tr("tb-dim-radius-hint"), bc.armed.dim_kind() == 3) {
                        qymcad_ui_state::set_dim_tool(&mut qymcad_ui_state::tools_in!(bc), &mut *bc.mode_3d, &*bc.project, *bc.sel, *bc.sketch_ses, &mut *bc.status, 3);
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::SketchMeasure, &qymcad_i18n::tr("tb-measure-hint"), bc.armed.measuring()) {
                        let on = !bc.armed.measuring();
                        // THROUGH THE DOOR, in the order every other tool uses. Setting the flag here and asking
                        // for "back to selection" afterwards turned the tool on and then straight off again.
                        qymcad_ui_state::set_measure(&mut qymcad_ui_state::tools_in!(bc), on);
                        if on {
                            *bc.mode_3d = false;
                            *bc.status = qymcad_i18n::tr("in-measure-hint");
                        } else {
                            *bc.status = qymcad_i18n::tr("in-measure-off");
                        }
                    }
                    // --- Constraints ---
                    cat(ui, &qymcad_i18n::tr("tb-group-constraints"));
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::ConstraintCoincident, &qymcad_i18n::tr("con-coincident-hint"), bc.sel_sk.constraint == Some(0)) {
                        bc.ask.push(qymcad_ui_state::BarAsk::Constraint(0));
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::ConstraintHorizontal, &qymcad_i18n::tr("con-horizontal-hint"), bc.sel_sk.constraint == Some(1)) {
                        bc.ask.push(qymcad_ui_state::BarAsk::Constraint(1));
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::ConstraintVertical, &qymcad_i18n::tr("con-vertical-hint"), bc.sel_sk.constraint == Some(2)) {
                        bc.ask.push(qymcad_ui_state::BarAsk::Constraint(2));
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::ConstraintParallel, &qymcad_i18n::tr("con-parallel-hint"), bc.sel_sk.constraint == Some(3)) {
                        bc.ask.push(qymcad_ui_state::BarAsk::Constraint(3));
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::ConstraintPerpendicular, &qymcad_i18n::tr("con-perpendicular-hint"), bc.sel_sk.constraint == Some(4)) {
                        bc.ask.push(qymcad_ui_state::BarAsk::Constraint(4));
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::ConstraintEqual, &qymcad_i18n::tr("con-equal"), bc.sel_sk.constraint == Some(5)) {
                        bc.ask.push(qymcad_ui_state::BarAsk::Constraint(5));
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::ConstraintCollinear, &qymcad_i18n::tr("con-collinear-hint"), bc.sel_sk.constraint == Some(7)) {
                        bc.ask.push(qymcad_ui_state::BarAsk::Constraint(7));
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::ConstraintConcentric, &qymcad_i18n::tr("con-concentric-hint"), bc.sel_sk.constraint == Some(8)) {
                        bc.ask.push(qymcad_ui_state::BarAsk::Constraint(8));
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::ConstraintTangent, &qymcad_i18n::tr("con-tangent-hint"), bc.sel_sk.constraint == Some(9)) {
                        bc.ask.push(qymcad_ui_state::BarAsk::Constraint(9));
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::ConstraintSymmetric, &qymcad_i18n::tr("con-symmetric-hint"), bc.sel_sk.constraint == Some(10)) {
                        bc.ask.push(qymcad_ui_state::BarAsk::Constraint(10));
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::ConstraintMidpoint, &qymcad_i18n::tr("con-midpoint-hint"), bc.sel_sk.constraint == Some(11)) {
                        bc.ask.push(qymcad_ui_state::BarAsk::Constraint(11));
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::ConstraintLock, &qymcad_i18n::tr("con-fix"), bc.sel_sk.constraint == Some(6)) {
                        bc.ask.push(qymcad_ui_state::BarAsk::Constraint(6));
                    }
                }
                qymcad_ui_state::Workbench::Part => {
                    // a full-width category label, breaking the columns (as in the Sketch)
                    let cat = |ui: &mut egui::Ui, s: &str| {
                        ui.add_sized([84.0, 12.0], egui::Label::new(egui::RichText::new(s).size(8.5).weak()));
                    };
                    // --- Create (a sketch + the datums) ---
                    cat(ui, &qymcad_i18n::tr("tb-group-create"));
                    create_panel_sketch_button(bc, ui);
                    create_panel_common(bc, ui);
                    // --- From a sketch ---
                    cat(ui, &qymcad_i18n::tr("tb-group-sketch3d"));
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::PartExtrude, &qymcad_i18n::tr("tb-extrude-hint"), tool_is_taken(bc, 1)) {
                        bc.feat.op = 0;
                        bc.ask.push(qymcad_ui_state::take_or_drop(tool_is_taken(bc, 1), qymcad_ui_state::BarAsk::FeatCmd(1)));
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::PartRevolve, &qymcad_i18n::tr("tb-revolve-hint"), tool_is_taken(bc, 3)) {
                        bc.ask.push(qymcad_ui_state::take_or_drop(tool_is_taken(bc, 3), qymcad_ui_state::BarAsk::FeatCmd(3)));
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::PartSweep, &qymcad_i18n::tr("tb-sweep-hint"), tool_is_taken(bc, 8)) {
                        bc.ask.push(qymcad_ui_state::take_or_drop(tool_is_taken(bc, 8), qymcad_ui_state::BarAsk::FeatCmd(8)));
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::PartLoft, &qymcad_i18n::tr("tb-loft-hint"), tool_is_taken(bc, 9)) {
                        bc.ask.push(qymcad_ui_state::take_or_drop(tool_is_taken(bc, 9), qymcad_ui_state::BarAsk::FeatCmd(9)));
                    }
                    // --- The 3D primitives (a command: sizes at the geometry + a preview + Enter/Esc) ---
                    cat(ui, &qymcad_i18n::tr("tb-group-prim"));
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::PartBox, &qymcad_i18n::tr("tb-box-hint"), bc.armed.cmd_kind() == 10) {
                        bc.ask.push(qymcad_ui_state::take_or_drop(bc.armed.cmd_kind() == 10, qymcad_ui_state::BarAsk::PrimCmd(10)));
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::PartCylinder, &qymcad_i18n::tr("tb-cylinder-hint"), bc.armed.cmd_kind() == 11) {
                        bc.ask.push(qymcad_ui_state::take_or_drop(bc.armed.cmd_kind() == 11, qymcad_ui_state::BarAsk::PrimCmd(11)));
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::PartSphere, &qymcad_i18n::tr("tb-sphere-hint"), bc.armed.cmd_kind() == 12) {
                        bc.ask.push(qymcad_ui_state::take_or_drop(bc.armed.cmd_kind() == 12, qymcad_ui_state::BarAsk::PrimCmd(12)));
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::PartCone, &qymcad_i18n::tr("tb-cone-hint"), bc.armed.cmd_kind() == 13) {
                        bc.ask.push(qymcad_ui_state::take_or_drop(bc.armed.cmd_kind() == 13, qymcad_ui_state::BarAsk::PrimCmd(13)));
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::PartTorus, &qymcad_i18n::tr("tb-torus-hint"), bc.armed.cmd_kind() == 14) {
                        bc.ask.push(qymcad_ui_state::take_or_drop(bc.armed.cmd_kind() == 14, qymcad_ui_state::BarAsk::PrimCmd(14)));
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::PartPrism, &qymcad_i18n::tr("tb-prism-hint"), bc.armed.cmd_kind() == 15) {
                        bc.ask.push(qymcad_ui_state::take_or_drop(bc.armed.cmd_kind() == 15, qymcad_ui_state::BarAsk::PrimCmd(15)));
                    }
                    // --- Operations on a body ---
                    cat(ui, &qymcad_i18n::tr("tb-body"));
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::PartFillet, &qymcad_i18n::tr("tb-fillet-body-hint"), bc.armed.cmd_kind() == 4) {
                        bc.ask.push(qymcad_ui_state::take_or_drop(bc.armed.cmd_kind() == 4, qymcad_ui_state::BarAsk::FeatCmd(4)));
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::PartChamfer, &qymcad_i18n::tr("tb-chamfer-body-hint"), bc.armed.cmd_kind() == 5) {
                        bc.ask.push(qymcad_ui_state::take_or_drop(bc.armed.cmd_kind() == 5, qymcad_ui_state::BarAsk::FeatCmd(5)));
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::PartShell, &qymcad_i18n::tr("tb-shell-hint"), bc.armed.cmd_kind() == 6) {
                        bc.ask.push(qymcad_ui_state::take_or_drop(bc.armed.cmd_kind() == 6, qymcad_ui_state::BarAsk::FeatCmd(6)));
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::PartSection, &qymcad_i18n::tr("tb-section-hint-bar"), bc.section.pick || bc.section.plane.is_some()) {
                        bc.ask.push(qymcad_ui_state::BarAsk::ToggleSection);
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::PartHole, &qymcad_i18n::tr("tb-hole-hint"), bc.armed.cmd_kind() == 7) {
                        bc.ask.push(qymcad_ui_state::take_or_drop(bc.armed.cmd_kind() == 7, qymcad_ui_state::BarAsk::FeatCmd(7)));
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::PartDraft, &qymcad_i18n::tr("tb-draft-hint"), bc.armed.cmd_kind() == 23) {
                        bc.ask.push(qymcad_ui_state::take_or_drop(bc.armed.cmd_kind() == 23, qymcad_ui_state::BarAsk::FeatCmd(23)));
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::PartPushFace, &qymcad_i18n::tr("tb-push-face-hint"), bc.armed.cmd_kind() == 25) {
                        bc.ask.push(qymcad_ui_state::take_or_drop(bc.armed.cmd_kind() == 25, qymcad_ui_state::BarAsk::FeatCmd(25)));
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::PartRemoveFace, &qymcad_i18n::tr("tb-remove-face-hint"), bc.armed.cmd_kind() == 26) {
                        bc.ask.push(qymcad_ui_state::take_or_drop(bc.armed.cmd_kind() == 26, qymcad_ui_state::BarAsk::FeatCmd(26)));
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::PartThicken, &qymcad_i18n::tr("tb-thicken-hint"), bc.armed.cmd_kind() == 28) {
                        bc.ask.push(qymcad_ui_state::take_or_drop(bc.armed.cmd_kind() == 28, qymcad_ui_state::BarAsk::FeatCmd(28)));
                    }
                    // the bridge from the parametric side into the design layer: a face becomes a surface
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::PartFaceCopy, &qymcad_i18n::tr("tb-face-copy-hint"), bc.armed.cmd_kind() == 30) {
                        bc.ask.push(qymcad_ui_state::take_or_drop(bc.armed.cmd_kind() == 30, qymcad_ui_state::BarAsk::FeatCmd(30)));
                    }
                    // a face taken out as a sheet moved along its normal - the offset surface of the professional systems
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::PartOffsetSurface, &qymcad_i18n::tr("tb-offset-surface-hint"), bc.armed.cmd_kind() == 36) {
                        bc.ask.push(qymcad_ui_state::take_or_drop(bc.armed.cmd_kind() == 36, qymcad_ui_state::BarAsk::FeatCmd(36)));
                    }
                    // the far end of that bridge: a surface goes back into the body
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::PartSurfaceReplace, &qymcad_i18n::tr("tb-surface-replace-hint"), bc.armed.cmd_kind() == 31) {
                        bc.ask.push(qymcad_ui_state::take_or_drop(bc.armed.cmd_kind() == 31, qymcad_ui_state::BarAsk::FeatCmd(31)));
                    }
                    // the first shape the body did not have: a surface built from the edges
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::PartPatch, &qymcad_i18n::tr("tb-patch-hint"), bc.armed.cmd_kind() == 32) {
                        bc.ask.push(qymcad_ui_state::take_or_drop(bc.armed.cmd_kind() == 32, qymcad_ui_state::BarAsk::FeatCmd(32)));
                    }
                    // pieces of surface become one, and a shell that closes becomes a body
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::PartStitch, &qymcad_i18n::tr("tb-stitch-hint"), bc.armed.cmd_kind() == 33) {
                        bc.ask.push(qymcad_ui_state::take_or_drop(bc.armed.cmd_kind() == 33, qymcad_ui_state::BarAsk::FeatCmd(33)));
                    }
                    // trim a surface with the neighbouring geometry
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::PartTrimSurface, &qymcad_i18n::tr("tb-trim-surface-hint"), bc.armed.cmd_kind() == 34) {
                        bc.ask.push(qymcad_ui_state::take_or_drop(bc.armed.cmd_kind() == 34, qymcad_ui_state::BarAsk::FeatCmd(34)));
                    }
                    // an imported mesh becomes a body a person can work on
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::PartRecognise, &qymcad_i18n::tr("tb-recognise-hint"), bc.armed.cmd_kind() == 35) {
                        bc.ask.push(qymcad_ui_state::take_or_drop(bc.armed.cmd_kind() == 35, qymcad_ui_state::BarAsk::FeatCmd(35)));
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::PartSplitBody, &qymcad_i18n::tr("tb-split-body-hint"), bc.armed.cmd_kind() == 27) {
                        bc.ask.push(qymcad_ui_state::take_or_drop(bc.armed.cmd_kind() == 27, qymcad_ui_state::BarAsk::FeatCmd(27)));
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::PartMeasure, &qymcad_i18n::tr("tb-measure3d-hint"), bc.m3.on) {
                        bc.ask.push(qymcad_ui_state::BarAsk::ToggleMeasure3d);
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::PartSplitFace, &qymcad_i18n::tr("tb-split-face-hint"), bc.armed.cmd_kind() == 29) {
                        bc.ask.push(qymcad_ui_state::take_or_drop(bc.armed.cmd_kind() == 29, qymcad_ui_state::BarAsk::FeatCmd(29)));
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::PartThread, &qymcad_i18n::tr("tb-thread-hint"), bc.armed.cmd_kind() == 24) {
                        bc.ask.push(qymcad_ui_state::take_or_drop(bc.armed.cmd_kind() == 24, qymcad_ui_state::BarAsk::FeatCmd(24)));
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::PartBooleanBodies, &qymcad_i18n::tr("tb-bool-bodies-hint"), bc.boolean.pick.is_some()) {
                        if bc.boolean.pick.is_some() {
                            // pressed again with the tool in hand: put down, as every tool button does
                            bc.ask.push(qymcad_ui_state::BarAsk::CancelAllTools(qymcad_ui_state::Then::Nothing));
                        } else {
                            // taking the boolean puts down the tool in hand first, or both bars stood at once
                            let a = qymcad_ui_state::selected_body(&*bc.project, &*bc.sel);
                            bc.ask.push(qymcad_ui_state::BarAsk::CancelAllTools(qymcad_ui_state::Then::Boolean(a)));
                        }
                    }
                    // THE MIRROR AND THE BODY PATTERNS LIVE HERE, UNDER "BODY". A separate "Patterns" category
                    // used to stand beside it showing the same two icons as "Body" above: those were COMPONENT
                    // patterns (an assembly tool), these are BODY patterns. What that looked like was
                    // duplicates - and it was worse than duplicates, because two buttons that looked alike did
                    // different things. The component ones moved into the Assembly, where they belong.
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::PartMirror, &qymcad_i18n::tr("tb-mirror-body-hint"), bc.armed.cmd_kind() == 16) {
                        bc.ask.push(qymcad_ui_state::take_or_drop(bc.armed.cmd_kind() == 16, qymcad_ui_state::BarAsk::FeatCmd(16)));
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::PartArrayLinear, &qymcad_i18n::tr("tb-lin-array-body-hint"), bc.armed.cmd_kind() == 17) {
                        bc.ask.push(qymcad_ui_state::take_or_drop(bc.armed.cmd_kind() == 17, qymcad_ui_state::BarAsk::FeatCmd(17)));
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::PartArrayCircular, &qymcad_i18n::tr("tb-circ-array-body-hint"), bc.armed.cmd_kind() == 18) {
                        bc.ask.push(qymcad_ui_state::take_or_drop(bc.armed.cmd_kind() == 18, qymcad_ui_state::BarAsk::FeatCmd(18)));
                    }
                }
                qymcad_ui_state::Workbench::Assembly => {
                    let cat = |ui: &mut egui::Ui, s: &str| {
                        ui.add_sized([84.0, 12.0], egui::Label::new(egui::RichText::new(s).size(8.5).weak()));
                    };
                    // --- Create ---
                    cat(ui, &qymcad_i18n::tr("tb-group-create"));
                    create_panel_common(bc, ui); // the datums; a sketch is inert in an assembly, so it was removed
                                                 // A NEW PART OR SUBASSEMBLY IS ONE STEP OF UNDO of its own name
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::AssemblyNewPart, &qymcad_i18n::tr("tb-new-part-hint"), false) {
                        qymcad_ui_state::begin_edit(bc.edits, bc.project, qymcad_i18n::tr("hotkey-assembly-n"));
                        let id = bc.project.add_part(bc.project.free_part_name());
                        qymcad_ui_state::close_edit(bc.edits, bc.project);
                        bc.ask.push(qymcad_ui_state::BarAsk::EnterComponent(id));
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::AssemblyNewSubassembly, &qymcad_i18n::tr("tb-new-subassembly-hint"), false) {
                        qymcad_ui_state::begin_edit(bc.edits, bc.project, qymcad_i18n::tr("hotkey-assembly-u"));
                        let id = bc.project.add_assembly(qymcad_i18n::tr1("node-assembly-n", "n", &bc.project.components.len().to_string()));
                        qymcad_ui_state::close_edit(bc.edits, bc.project);
                        bc.ask.push(qymcad_ui_state::BarAsk::EnterComponent(id));
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::AssemblyInsertComponent, &qymcad_i18n::tr("tb-insert-component-hint"), false) {
                        bc.ask.push(qymcad_ui_state::BarAsk::Import(qymcad_ui_state::Want::Part));
                    }
                    // COMPONENT PATTERNS ARE AN ASSEMBLY TOOL. They used to sit in the PART workbench, where
                    // there are no components, and there they duplicated the look of the body patterns.
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::AssemblyArrayLinear, &qymcad_i18n::tr("tb-comp-lin-array-hint"), bc.carr.mode == 1) {
                        bc.ask.push(qymcad_ui_state::take_or_drop(bc.carr.mode == 1, qymcad_ui_state::BarAsk::CompArray(1)));
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::AssemblyArrayCircular, &qymcad_i18n::tr("tb-comp-circ-array-hint"), bc.carr.mode == 2) {
                        bc.ask.push(qymcad_ui_state::take_or_drop(bc.carr.mode == 2, qymcad_ui_state::BarAsk::CompArray(2)));
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::AssemblyMirror, &qymcad_i18n::tr("tb-mirror-part-hint"), bc.mirror.in_hand()) {
                        // both a PART and a SUBASSEMBLY are accepted (the whole subtree is mirrored); with the tool in
                        // hand the press goes to the door as it is, which puts it down
                        let src = bc.mirror.part.or_else(|| match *bc.sel {
                            qymcad_ui_state::Sel::Component(ci) => bc.project.components.get(ci).map(|c| c.id),
                            _ => qymcad_ui_state::selected_body(&*bc.project, &*bc.sel).and_then(|b| bc.project.body_owner(b)),
                        });
                        // THROUGH A DOOR, in the order that works: release the others, THEN take this one. Pushing the release
                        // and arming straight after put them the other way round - the release runs after the frame - and the
                        // button did nothing. With nothing picked before it the tool waits for a click on a part.
                        bc.ask.push(qymcad_ui_state::BarAsk::MirrorPart(src));
                    }
                    // ONE DOOR, not a second copy of it. This block used to repeat what `toggle_section` does,
                    // and the copy could not release the other tools from here, so it pushed the request and
                    // armed the section straight after. The request runs AFTER the frame and clears exactly
                    // that: switching the section ON through this button did nothing at all.
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::AssemblySection, &qymcad_i18n::tr("tb-section-hint"), bc.section.pick || bc.section.plane.is_some()) {
                        bc.ask.push(qymcad_ui_state::BarAsk::ToggleSection);
                    }
                    // --- The mates: buttons per mate kind, as in the sketcher. Clicking a kind starts the
                    // face pick with that kind (face A, then face B). The list and the editing of existing ones
                    // live in the properties panel on the right, as the sketch dimensions do.
                    cat(ui, &qymcad_i18n::tr("tb-group-joint"));
                    {
                        // ONE BUTTON, with the kind chosen in the top bar. There used to be seven buttons, one
                        // per kind, while the kind was changed by a combo box in that same bar anyway: the
                        // choice was made twice and took up the whole category.
                        let tip = qymcad_i18n::tr1("jt-joint-tip", "kind", &qymcad_i18n::tr(bc.joint.new_kind.label()));
                        if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::AssemblyJoint, &tip, bc.joint.pick_faces) {
                            bc.ask.push(qymcad_ui_state::take_or_drop(bc.joint.pick_faces, qymcad_ui_state::BarAsk::JointPick));
                        }
                        // the Ground tool: a click on a part fixes or releases it.
                        if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::AssemblyGround, &qymcad_i18n::tr("tb-ground-hint"), bc.joint.ground_pick) {
                            bc.ask.push(qymcad_ui_state::BarAsk::GroundPick);
                        }
                        // Group: fasten a set of parts to one another without pairwise joints.
                        if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::AssemblyGroup, &qymcad_i18n::tr("j-group-tip"), bc.joint.group_pick.is_some()) {
                            bc.ask.push(qymcad_ui_state::BarAsk::GroupPick);
                        }
                        // Width: place a part midway between two walls.
                        if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::AssemblyWidth, &qymcad_i18n::tr("j-width-tip"), bc.joint.width_pick.is_some()) {
                            bc.ask.push(qymcad_ui_state::BarAsk::WidthPick);
                        }
                        // Tangent: lay a cylinder onto a plane.
                        if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::AssemblyTangent, &qymcad_i18n::tr("j-tangent-tip"), bc.joint.tangent_pick.is_some()) {
                            bc.ask.push(qymcad_ui_state::BarAsk::TangentPick);
                        }
                        // Relation: tie the degrees of freedom of two mates together.
                        if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::AssemblyRelation, &qymcad_i18n::tr("j-relation-tip"), bc.joint.relation_pick.is_some()) {
                            bc.ask.push(qymcad_ui_state::BarAsk::RelationPick);
                        }
                    }
                }
            }
        });
    });
}

/// HOW A SKETCH CHAMFER IS GIVEN, on the bar of the chamfer: equal legs, two legs, or a leg and an angle - the words of
/// the chamfer of a part - and, for the last two, the second value beside the first leg. Reported (issue #35): the
/// sketch chamfer took one distance only, and a chamfer of 5 x 3 or of 5 at 30 deg had to be built by hand.
/// The ways of a sketch fillet, before its field: by its radius, its chord or the length of its arc.
pub(crate) fn fillet_modes(bc: &mut qymcad_ui_state::BarCtx, ui: &mut egui::Ui) {
    use qymcad_core::model::FilletBy;
    for by in [FilletBy::Radius, FilletBy::Chord, FilletBy::ArcLength] {
        if ui.selectable_label(bc.tool_prefs.fillet_by == by, qymcad_i18n::tr(qymcad_ui_state::fillet_label(by))).clicked() {
            bc.tool_prefs.fillet_by = by;
        }
    }
    ui.separator();
}

/// The modes of a sketch chamfer, before its fields: the fields of one chamfer stand together after them.
pub(crate) fn chamfer_modes(bc: &mut qymcad_ui_state::BarCtx, ui: &mut egui::Ui) {
    use qymcad_core::feature::ChamferMode;
    for (mode, word) in [(ChamferMode::Symmetric, "cmd-symmetric"), (ChamferMode::TwoDist, "cmd-two-distances"), (ChamferMode::DistAngle, "cmd-leg-angle")] {
        if ui.selectable_label(bc.tool_prefs.chamfer_mode == mode, qymcad_i18n::tr(word)).clicked() {
            qymcad_ui_state::set_chamfer_mode(bc.tool_prefs, mode);
        }
    }
    ui.separator();
}

/// The second field of a sketch chamfer of two legs or of a leg and an angle, right after the first.
pub(crate) fn chamfer_second_field(bc: &mut qymcad_ui_state::BarCtx, ui: &mut egui::Ui) {
    use qymcad_core::feature::ChamferMode;
    if bc.tool_prefs.chamfer_mode != ChamferMode::Symmetric {
        let angle = bc.tool_prefs.chamfer_mode == ChamferMode::DistAngle;
        ui.label(qymcad_i18n::tr(qymcad_ui_state::chamfer_d2_label(bc.tool_prefs.chamfer_mode)));
        bc.tool_prefs.chamfer_second = qymcad_ui_state::num_or_expr(
            &mut qymcad_ui_state::ExprBarCtx { bar_exprs: &mut *bc.bar_exprs, project: &*bc.project, scheme: &*bc.scheme },
            ui,
            "sk_chamfer2",
            bc.tool_prefs.chamfer_second,
            qymcad_ui_state::NumFormat {
                lo: 0.01,
                hi: if angle { 179.99 } else { 10000.0 },
                integer: false,
                suffix: &qymcad_i18n::tr(if angle { "unit-deg-suffix" } else { "unit-mm-suffix" }),
                nonzero: false,
            },
        );
    }
}

pub fn tool_options_bar(bc: &mut qymcad_ui_state::BarCtx, ui: &mut egui::Ui) {
    if qymcad_ui_state::edit_si(&*bc.project, &*bc.sketch_ses).is_none() {
        return;
    }
    // EDIT -> COPY, CUT, INSERT hold the hand too: the base point or the place is awaited. Their words stood only in the
    // status line, and the bar said "Select" over a click that would copy.
    // FILLET ALL holds the hand: its name, what to do, the way out; the radius is typed in the field at the shape
    if bc.picking.fillet_all() {
        let mut cancel = false;
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new(qymcad_i18n::tr("tool-fillet")).strong());
            ui.separator();
            let ask = if bc.corner.at.is_some() { qymcad_i18n::tr("sk-enter-apply-expr") } else { qymcad_i18n::tr("g-fillet-all-hint") };
            ui.label(egui::RichText::new(ask).weak());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button(qymcad_i18n::tr("cmd-cancel-btn")).clicked() {
                    cancel = true;
                }
            });
        });
        if bc.corner.at.is_none() {
            // the shape is clicked, not typed: Enter before it has nothing to round
            qymcad_ui_state::enter_not_ready(&ui.ctx().clone(), bc.status, &qymcad_i18n::tr("g-fillet-all-hint"));
        }
        if cancel {
            bc.picking.clear();
            bc.corner.clear();
            *bc.status = qymcad_i18n::tr("in-fillet-all-cancelled");
        }
        return;
    }
    // another tool taken puts the copy down: one thing in hand at a time
    if !matches!(*bc.armed, qymcad_ui_state::Armed::None) {
        bc.clip.drop_geom();
    }
    let waiting = match (bc.clip.geom_pending.as_ref(), bc.clip.geom_place) {
        (Some((_, true)), _) => Some(("menu-cut", "g-cut-base-point")),
        (Some((_, false)), _) => Some(("tool-copy", "g-copy-base-point")),
        (None, true) => Some(("win-insert", "g-insert-click")),
        (None, false) => None,
    };
    if let Some((name, ask)) = waiting {
        let mut cancel = false;
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new(qymcad_i18n::tr(name)).strong());
            ui.separator();
            ui.label(egui::RichText::new(qymcad_i18n::tr(ask)).weak());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button(qymcad_i18n::tr("cmd-cancel-btn")).clicked() {
                    cancel = true;
                }
            });
        });
        // a point is clicked, not typed: Enter before it has nothing to do
        qymcad_ui_state::enter_not_ready(&ui.ctx().clone(), bc.status, &qymcad_i18n::tr(ask));
        if cancel {
            bc.clip.drop_geom();
            *bc.status = qymcad_i18n::tr(if name == "win-insert" { "in-insert-cancelled" } else { "in-copy-cancelled" });
        }
        return;
    }
    ui.horizontal_wrapped(|ui| {
        // the name of the active tool or operation
        let name = if bc.armed.pat_op() != 0 {
            if bc.armed.pat_op() == 1 {
                qymcad_i18n::tr("tool-lin-array")
            } else {
                qymcad_i18n::tr("tool-circ-array")
            }
        } else if bc.armed.move_op() != 0 {
            match bc.armed.move_op() {
                1 => qymcad_i18n::tr("tool-move"),
                2 => qymcad_i18n::tr("tool-copy"),
                _ => qymcad_i18n::tr("tool-rotate"),
            }
        } else if bc.armed.click_op() != 0 {
            match bc.armed.click_op() {
                1 => qymcad_i18n::tr("tool-trim"),
                2 => qymcad_i18n::tr("tool-extend"),
                3 => qymcad_i18n::tr("tool-break"),
                4 => qymcad_i18n::tr("tool-fillet"),
                5 => qymcad_i18n::tr("tool-chamfer"),
                6 => qymcad_i18n::tr("tool-project-body"),
                _ => "—".to_string(),
            }
        } else if bc.armed.dim_kind() != 0 {
            qymcad_i18n::tr("tool-dim")
        } else if bc.armed.measuring() {
            qymcad_i18n::tr("tool-measure")
        } else if bc.armed.draw_kind() != 0 {
            match bc.armed.draw_kind() {
                1 => qymcad_i18n::tr("tool-line"),
                2 => qymcad_i18n::tr("tool-rect"),
                3 => qymcad_i18n::tr("tool-circle"),
                4 => qymcad_i18n::tr("tool-arc"),
                5 => qymcad_i18n::tr("tool-point"),
                6 => qymcad_i18n::tr("tool-polygon"),
                7 => qymcad_i18n::tr("tool-slot"),
                8 => qymcad_i18n::tr("sk-ellipse"),
                9 => qymcad_i18n::tr("sk-spline"),
                10 => qymcad_i18n::tr("tool-circle-3pt"),
                _ => "—".to_string(),
            }
        } else {
            match bc.armed.modify() {
                Some(qymcad_ui_state::EditTool::Fillet) => qymcad_i18n::tr("tool-fillet"),
                Some(qymcad_ui_state::EditTool::Chamfer) => qymcad_i18n::tr("tool-chamfer"),
                Some(qymcad_ui_state::EditTool::Offset) => qymcad_i18n::tr("tool-offset"),
                Some(qymcad_ui_state::EditTool::Mirror) => qymcad_i18n::tr("tool-mirror"),
                Some(qymcad_ui_state::EditTool::LinearPattern) => qymcad_i18n::tr("tool-lin-array-short"),
                Some(qymcad_ui_state::EditTool::CircularPattern) => qymcad_i18n::tr("tool-circ-array-short"),
                _ => qymcad_i18n::tr("tool-select"),
            }
        };
        ui.label(egui::RichText::new(name).strong());
        ui.separator();
        // ONLY the parameters of the active tool or operation
        if bc.armed.draw_kind() != 0 {
            ui.checkbox(&mut bc.tool.construction, qymcad_i18n::tr("opt-construction-short")).on_hover_text(qymcad_i18n::tr("opt-construction-hint"));
            let mut free = bc.tool.ties == qymcad_ui_state::Ties::Free;
            if ui.checkbox(&mut free, qymcad_i18n::tr("opt-no-ties-short")).on_hover_text(qymcad_i18n::tr("opt-no-ties-hint")).changed() {
                bc.tool.ties = if free { qymcad_ui_state::Ties::Free } else { qymcad_ui_state::Ties::Laid };
            }
            if bc.armed.draw_kind() == 11 {
                ui.separator();
                ui.label(qymcad_i18n::tr("tool-text"));
                ui.add(egui::TextEdit::singleline(&mut bc.tool_prefs.text).desired_width(140.0));
                ui.label(qymcad_i18n::tr("opt-height-short"));
                bc.tool_prefs.text_h = qymcad_ui_state::num_or_expr(
                    &mut qymcad_ui_state::ExprBarCtx { bar_exprs: &mut *bc.bar_exprs, project: &*bc.project, scheme: &*bc.scheme },
                    ui,
                    "text_h",
                    bc.tool_prefs.text_h,
                    qymcad_ui_state::NumFormat { lo: 0.1, hi: 1000.0, integer: false, suffix: &qymcad_i18n::tr("unit-mm-suffix"), nonzero: false },
                );
                // WHICH FONT IT WILL BE WRITTEN IN, named on the button itself. "Font..." alone said nothing:
                // a person pressed it to find out what they were about to write with.
                let named = qymcad_ui_state::font_label(&bc.tool_prefs.font, "opt-font");
                if ui.button(named).on_hover_text(qymcad_i18n::tr("opt-pick-font")).clicked() {
                    bc.ask.push(qymcad_ui_state::BarAsk::PickFont);
                }
                ui.checkbox(&mut bc.tool_prefs.text_note, qymcad_i18n::tr("opt-note")).on_hover_text(qymcad_i18n::tr("opt-text-as-note"));
            }
            if bc.armed.draw_kind() == 6 {
                ui.separator();
                ui.label(qymcad_i18n::tr("opt-sides"));
                bc.tool_prefs.poly_n = qymcad_ui_state::num_or_expr(
                    &mut qymcad_ui_state::ExprBarCtx { bar_exprs: &mut *bc.bar_exprs, project: &*bc.project, scheme: &*bc.scheme },
                    ui,
                    "poly_n",
                    bc.tool_prefs.poly_n as f64,
                    qymcad_ui_state::NumFormat { lo: 3.0, hi: 64.0, integer: true, suffix: "", nonzero: false },
                ) as u32;
                ui.selectable_value(&mut bc.tool_prefs.poly_mode, 0u8, qymcad_i18n::tr("opt-polygon-inscribed"));
                ui.selectable_value(&mut bc.tool_prefs.poly_mode, 1u8, qymcad_i18n::tr("opt-polygon-circumscribed"));
                ui.selectable_value(&mut bc.tool_prefs.poly_mode, 2u8, qymcad_i18n::tr("opt-by-edge"));
            }
            if bc.armed.draw_kind() == 2 {
                ui.separator();
                ui.label(qymcad_i18n::tr("opt-rect-mode"));
                if ui.selectable_label(bc.tool_prefs.rect_mode == 0, qymcad_i18n::tr("opt-rect-2corners")).clicked() {
                    bc.tool_prefs.rect_mode = 0;
                    bc.tool.pts.clear();
                }
                if ui.selectable_label(bc.tool_prefs.rect_mode == 1, qymcad_i18n::tr("opt-rect-centre")).clicked() {
                    bc.tool_prefs.rect_mode = 1;
                    bc.tool.pts.clear();
                }
                if ui.selectable_label(bc.tool_prefs.rect_mode == 2, qymcad_i18n::tr("opt-rect-3pt")).on_hover_text(qymcad_i18n::tr("opt-rect-rotated")).clicked() {
                    bc.tool_prefs.rect_mode = 2;
                    bc.tool.pts.clear();
                }
            }
            if bc.armed.draw_kind() == 3 {
                ui.separator();
                ui.label(qymcad_i18n::tr("opt-circle-mode"));
                if ui.selectable_label(bc.tool_prefs.circ_mode == 0, qymcad_i18n::tr("opt-circle-centre-radius")).clicked() {
                    bc.tool_prefs.circ_mode = 0;
                    bc.tool.pts.clear();
                    bc.tool.circ_tan = None;
                }
                if ui.selectable_label(bc.tool_prefs.circ_mode == 1, qymcad_i18n::tr("opt-circle-2pt")).on_hover_text(qymcad_i18n::tr("opt-circle-diameter-ends")).clicked() {
                    bc.tool_prefs.circ_mode = 1;
                    bc.tool.pts.clear();
                    bc.tool.circ_tan = None;
                }
                if ui.selectable_label(bc.tool_prefs.circ_mode == 2, qymcad_i18n::tr("opt-tangent-m")).on_hover_text(qymcad_i18n::tr("opt-tangent-hint")).clicked() {
                    bc.tool_prefs.circ_mode = 2;
                    bc.tool.pts.clear();
                    bc.tool.circ_tan = None;
                }
            }
            if bc.armed.draw_kind() == 4 {
                ui.separator();
                ui.label(qymcad_i18n::tr("opt-arc-mode"));
                if ui.selectable_label(bc.tool_prefs.arc_mode == 0, qymcad_i18n::tr("opt-arc-cse")).clicked() {
                    bc.tool_prefs.arc_mode = 0;
                    bc.tool.pts.clear();
                }
                if ui.selectable_label(bc.tool_prefs.arc_mode == 1, qymcad_i18n::tr("opt-rect-3pt")).on_hover_text(qymcad_i18n::tr("opt-arc-3pt")).clicked() {
                    bc.tool_prefs.arc_mode = 1;
                    bc.tool.pts.clear();
                }
                if ui.selectable_label(bc.tool_prefs.arc_mode == 2, qymcad_i18n::tr("opt-tangent")).on_hover_text(qymcad_i18n::tr("opt-arc-smooth-hint")).clicked() {
                    bc.tool_prefs.arc_mode = 2;
                    bc.tool.pts.clear();
                }
            }
        } else if bc.armed.dim_kind() != 0 {
            ui.label(egui::RichText::new(qymcad_i18n::tr("opt-dim-hint")).weak());
        } else if bc.armed.click_op() != 0 {
            if bc.armed.click_op() == 4 || bc.armed.click_op() == 5 {
                if bc.armed.click_op() == 5 {
                    chamfer_modes(bc, ui);
                } else {
                    fillet_modes(bc, ui);
                }
                ui.label(qymcad_i18n::tr(if bc.armed.click_op() == 4 {
                    qymcad_ui_state::fillet_label(bc.tool_prefs.fillet_by)
                } else {
                    qymcad_ui_state::chamfer_d1_label(bc.tool_prefs.chamfer_mode)
                }));
                // with a corner clicked, the most it takes; short of it, the largest the field ever takes
                let corner_hi = bc
                    .corner
                    .at
                    .and_then(|(si, pid, ch)| {
                        (pid != 0).then(|| qymcad_ui_state::corner_limit_in(&*bc.project, si, pid, qymcad_ui_state::corner_tool(ch, bc.tool_prefs), bc.tool_prefs.fillet_by)).flatten()
                    })
                    .map_or(10000.0, |l| l * (1.0 - 1e-9));
                bc.tool_prefs.fillet = qymcad_ui_state::num_or_expr(
                    &mut qymcad_ui_state::ExprBarCtx { bar_exprs: &mut *bc.bar_exprs, project: &*bc.project, scheme: &*bc.scheme },
                    ui,
                    "sk_fillet",
                    bc.tool_prefs.fillet,
                    qymcad_ui_state::NumFormat { lo: 0.01, hi: corner_hi, integer: false, suffix: &qymcad_i18n::tr("unit-mm-suffix"), nonzero: false },
                );
                if bc.armed.click_op() == 5 {
                    chamfer_second_field(bc, ui);
                }
            }
            if bc.armed.click_op() == 6 {
                // WHAT IS TAKEN: one edge under the cursor, or the whole outline of the sketch's host face
                if ui.selectable_label(!bc.tool.proj_face, qymcad_i18n::tr("opt-edge")).on_hover_text(qymcad_i18n::tr("opt-project-edge-hint")).clicked() {
                    bc.tool.proj_face = false;
                }
                if ui.selectable_label(bc.tool.proj_face, qymcad_i18n::tr("opt-face-outline")).on_hover_text(qymcad_i18n::tr("opt-face-outline-hint")).clicked() {
                    bc.tool.proj_face = true;
                }
            }
            if bc.armed.click_op() == 2 {
                ui.checkbox(&mut bc.tool_prefs.extend_both, qymcad_i18n::tr("opt-extend-both")).on_hover_text(qymcad_i18n::tr("opt-extend-both-hint"));
            }
            let h = match bc.armed.click_op() {
                1 => &qymcad_i18n::tr("opt-trim-hint"),
                // the line held, the tool says what it waits for next
                2 if bc.tool.extend.is_some() => &qymcad_i18n::tr("opt-extend-to-hint"),
                2 => &qymcad_i18n::tr("opt-extend-hint"),
                3 => &qymcad_i18n::tr("opt-break-hint"),
                4 => &qymcad_i18n::tr("opt-fillet-hint"),
                5 => &qymcad_i18n::tr("opt-chamfer-hint"),
                6 => &qymcad_i18n::tr("opt-project-hint"),
                _ => "",
            };
            ui.label(egui::RichText::new(h).weak());
        } else if bc.armed.pat_op() != 0 {
            // the pattern: the parameters + a hint (the preview is live, Enter applies)
            ui.label(qymcad_i18n::tr("opt-count"));
            bc.sk_pat.count = qymcad_ui_state::num_or_expr(
                &mut qymcad_ui_state::ExprBarCtx { bar_exprs: &mut *bc.bar_exprs, project: &*bc.project, scheme: &*bc.scheme },
                ui,
                "skpat_count",
                bc.sk_pat.count as f64,
                qymcad_ui_state::NumFormat { lo: 2.0, hi: 200.0, integer: true, suffix: "", nonzero: false },
            ) as u32;
            if bc.armed.pat_op() == 1 {
                ui.label(qymcad_i18n::tr("opt-step-x"));
                bc.sk_pat.dx = qymcad_ui_state::num_or_expr(
                    &mut qymcad_ui_state::ExprBarCtx { bar_exprs: &mut *bc.bar_exprs, project: &*bc.project, scheme: &*bc.scheme },
                    ui,
                    "skpat_dx",
                    bc.sk_pat.dx,
                    qymcad_ui_state::NumFormat { lo: -100000.0, hi: 100000.0, integer: false, suffix: &qymcad_i18n::tr("unit-mm-suffix"), nonzero: false },
                );
                ui.label(qymcad_i18n::tr("opt-step-y"));
                bc.sk_pat.dy = qymcad_ui_state::num_or_expr(
                    &mut qymcad_ui_state::ExprBarCtx { bar_exprs: &mut *bc.bar_exprs, project: &*bc.project, scheme: &*bc.scheme },
                    ui,
                    "skpat_dy",
                    bc.sk_pat.dy,
                    qymcad_ui_state::NumFormat { lo: -100000.0, hi: 100000.0, integer: false, suffix: &qymcad_i18n::tr("unit-mm-suffix"), nonzero: false },
                );
                ui.separator();
                ui.label(qymcad_i18n::tr("opt-rows"));
                bc.sk_pat.count2 = qymcad_ui_state::num_or_expr(
                    &mut qymcad_ui_state::ExprBarCtx { bar_exprs: &mut *bc.bar_exprs, project: &*bc.project, scheme: &*bc.scheme },
                    ui,
                    "skpat_count2",
                    bc.sk_pat.count2 as f64,
                    qymcad_ui_state::NumFormat { lo: 1.0, hi: 200.0, integer: true, suffix: "", nonzero: false },
                ) as u32;
                if bc.sk_pat.count2 > 1 {
                    ui.label(qymcad_i18n::tr("opt-row-step-x"));
                    bc.sk_pat.dx2 = qymcad_ui_state::num_or_expr(
                        &mut qymcad_ui_state::ExprBarCtx { bar_exprs: &mut *bc.bar_exprs, project: &*bc.project, scheme: &*bc.scheme },
                        ui,
                        "skpat_dx2",
                        bc.sk_pat.dx2,
                        qymcad_ui_state::NumFormat { lo: -100000.0, hi: 100000.0, integer: false, suffix: &qymcad_i18n::tr("unit-mm-suffix"), nonzero: false },
                    );
                    ui.label(qymcad_i18n::tr("opt-row-step-y"));
                    bc.sk_pat.dy2 = qymcad_ui_state::num_or_expr(
                        &mut qymcad_ui_state::ExprBarCtx { bar_exprs: &mut *bc.bar_exprs, project: &*bc.project, scheme: &*bc.scheme },
                        ui,
                        "skpat_dy2",
                        bc.sk_pat.dy2,
                        qymcad_ui_state::NumFormat { lo: -100000.0, hi: 100000.0, integer: false, suffix: &qymcad_i18n::tr("unit-mm-suffix"), nonzero: false },
                    );
                }
            } else {
                ui.label(qymcad_i18n::tr("cmd-angle"));
                bc.sk_pat.angle = qymcad_ui_state::num_or_expr(
                    &mut qymcad_ui_state::ExprBarCtx { bar_exprs: &mut *bc.bar_exprs, project: &*bc.project, scheme: &*bc.scheme },
                    ui,
                    "skpat_angle",
                    bc.sk_pat.angle,
                    qymcad_ui_state::NumFormat { lo: -360.0, hi: 360.0, integer: false, suffix: "°", nonzero: false },
                );
                ui.label(egui::RichText::new(if bc.pat.center.is_some() { qymcad_i18n::tr("opt-centre-set") } else { qymcad_i18n::tr("opt-click-rotation-centre") }).weak());
            }
            ui.label(egui::RichText::new(if bc.pat.edit.is_some() { qymcad_i18n::tr("opt-enter-update") } else { qymcad_i18n::tr("opt-enter-apply") }).weak());
            // APPLY AND CANCEL AS BUTTONS, as every command bar has them: a pattern can be finished and refused with the mouse
            ui.separator();
            let ctx = ui.ctx().clone();
            if ui.add_enabled(qymcad_ui_state::bar_fields_valid(&ctx), egui::Button::new(qymcad_i18n::tr("cmd-apply-enter"))).clicked() {
                qymcad_ui_state::bar_apply_press(&ctx);
            }
            if ui.button(qymcad_i18n::tr("cmd-cancel-btn")).clicked() {
                bc.ask.push(qymcad_ui_state::BarAsk::CancelAllTools(qymcad_ui_state::Then::Nothing));
            }
        } else {
            match bc.armed.modify() {
                Some(qymcad_ui_state::EditTool::Fillet | qymcad_ui_state::EditTool::Chamfer) => {
                    if bc.armed.modify() == Some(qymcad_ui_state::EditTool::Chamfer) {
                        chamfer_modes(bc, ui);
                    } else {
                        fillet_modes(bc, ui);
                    }
                    ui.label(qymcad_i18n::tr(if bc.armed.modify() == Some(qymcad_ui_state::EditTool::Fillet) {
                        qymcad_ui_state::fillet_label(bc.tool_prefs.fillet_by)
                    } else {
                        qymcad_ui_state::chamfer_d1_label(bc.tool_prefs.chamfer_mode)
                    }));
                    bc.tool_prefs.fillet = qymcad_ui_state::num_or_expr(
                        &mut qymcad_ui_state::ExprBarCtx { bar_exprs: &mut *bc.bar_exprs, project: &*bc.project, scheme: &*bc.scheme },
                        ui,
                        "sk_fillet",
                        bc.tool_prefs.fillet,
                        qymcad_ui_state::NumFormat { lo: 0.01, hi: 10000.0, integer: false, suffix: &qymcad_i18n::tr("unit-mm-suffix"), nonzero: false },
                    );
                    if bc.armed.modify() == Some(qymcad_ui_state::EditTool::Chamfer) {
                        chamfer_second_field(bc, ui);
                    }
                }
                Some(qymcad_ui_state::EditTool::Offset) => {
                    // the box at the copy types the distance: the bar shows it rather than its own old text over it
                    if bc.sel_sk.offset.at.is_some() && !ui.memory(|m| m.has_focus(qymcad_ui_state::bar_field_id("sk_offset"))) {
                        bc.bar_exprs.remove("sk_offset");
                    }
                    ui.label(qymcad_i18n::tr("opt-distance"));
                    bc.tool_prefs.offset = qymcad_ui_state::num_or_expr(
                        &mut qymcad_ui_state::ExprBarCtx { bar_exprs: &mut *bc.bar_exprs, project: &*bc.project, scheme: &*bc.scheme },
                        ui,
                        "sk_offset",
                        bc.tool_prefs.offset,
                        qymcad_ui_state::NumFormat { lo: -10000.0, hi: 10000.0, integer: false, suffix: &qymcad_i18n::tr("unit-mm-suffix"), nonzero: true },
                    );
                }
                Some(qymcad_ui_state::EditTool::LinearPattern) => {
                    ui.label(qymcad_i18n::tr("opt-count"));
                    bc.sk_pat.count = qymcad_ui_state::num_or_expr(
                        &mut qymcad_ui_state::ExprBarCtx { bar_exprs: &mut *bc.bar_exprs, project: &*bc.project, scheme: &*bc.scheme },
                        ui,
                        "skpat_count",
                        bc.sk_pat.count as f64,
                        qymcad_ui_state::NumFormat { lo: 2.0, hi: 200.0, integer: true, suffix: "", nonzero: false },
                    ) as u32;
                    ui.label(qymcad_i18n::tr("opt-step-x"));
                    bc.sk_pat.dx = qymcad_ui_state::num_or_expr(
                        &mut qymcad_ui_state::ExprBarCtx { bar_exprs: &mut *bc.bar_exprs, project: &*bc.project, scheme: &*bc.scheme },
                        ui,
                        "skpat_dx",
                        bc.sk_pat.dx,
                        qymcad_ui_state::NumFormat { lo: -100000.0, hi: 100000.0, integer: false, suffix: &qymcad_i18n::tr("unit-mm-suffix"), nonzero: false },
                    );
                    ui.label(qymcad_i18n::tr("opt-step-y"));
                    bc.sk_pat.dy = qymcad_ui_state::num_or_expr(
                        &mut qymcad_ui_state::ExprBarCtx { bar_exprs: &mut *bc.bar_exprs, project: &*bc.project, scheme: &*bc.scheme },
                        ui,
                        "skpat_dy",
                        bc.sk_pat.dy,
                        qymcad_ui_state::NumFormat { lo: -100000.0, hi: 100000.0, integer: false, suffix: &qymcad_i18n::tr("unit-mm-suffix"), nonzero: false },
                    );
                }
                Some(qymcad_ui_state::EditTool::CircularPattern) => {
                    ui.label(qymcad_i18n::tr("opt-count"));
                    bc.sk_pat.count = qymcad_ui_state::num_or_expr(
                        &mut qymcad_ui_state::ExprBarCtx { bar_exprs: &mut *bc.bar_exprs, project: &*bc.project, scheme: &*bc.scheme },
                        ui,
                        "skpat_count",
                        bc.sk_pat.count as f64,
                        qymcad_ui_state::NumFormat { lo: 2.0, hi: 200.0, integer: true, suffix: "", nonzero: false },
                    ) as u32;
                    ui.label(qymcad_i18n::tr("cmd-angle"));
                    bc.sk_pat.angle = qymcad_ui_state::num_or_expr(
                        &mut qymcad_ui_state::ExprBarCtx { bar_exprs: &mut *bc.bar_exprs, project: &*bc.project, scheme: &*bc.scheme },
                        ui,
                        "skpat_angle",
                        bc.sk_pat.angle,
                        qymcad_ui_state::NumFormat { lo: -360.0, hi: 360.0, integer: false, suffix: "°", nonzero: false },
                    );
                }
                Some(qymcad_ui_state::EditTool::Mirror) => {
                    ui.label(egui::RichText::new(qymcad_i18n::tr("opt-mirror-axis-hint")).weak());
                }
                _ => {
                    ui.checkbox(&mut bc.tool.construction, qymcad_i18n::tr("opt-construction-short"));
                    ui.label(egui::RichText::new(qymcad_i18n::tr("opt-select-hint")).weak());
                }
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(egui::RichText::new(qymcad_i18n::tr1("opt-selected-n", "n", &bc.sel_sk.items.len().to_string())).weak());
        });
    });
}
