//! THE ROLLBACK BAR DRAGGED ONE ROW UP REBUILDS NOTHING ABOVE IT.
//!
//! Reported behaviour: the bar dragged in the tree one row up, above a rounding at the end of a part with two threads,
//! and the card "Rebuilding 27 nodes, a thread among them" counted every node of the timeline for minutes.
//!
//! The bar is dragged here by the mouse in real frames, as a person drags it, and what follows is watched: every
//! rebuild in flight after the release is a quiet one (a loud one is what puts up the card counting the threads), and
//! the model left on screen is the threaded sleeve without its rounding.
#[cfg(test)]
mod tests {
    use crate::gui::hand::Hand;
    use crate::gui::App;
    use qymcad_core::model::Id;
    use qymcad_core::thread::{ThreadSpec, ThreadStandard};

    /// A sleeve of 30 x 20 with a hole of 8.5, M10 inside it, its outer rims rounded last.
    struct Sleeve {
        app: App,
        threaded: Id,
        rounded: Id,
    }

    fn round_edges(app: &mut App, body: Id, r: f64) -> Vec<u32> {
        crate::gui::io_jobs::regenerate_now(&mut app.rebuild_ctx());
        app.project.regen_edges.get(&body).map(|es| es.iter().filter(|e| (e.radius - r).abs() < 0.05).map(|e| e.id).collect()).unwrap_or_default()
    }

    fn sleeve() -> Sleeve {
        let mut app = App { project: qymcad_core::model::Project::default(), ..Default::default() };
        app.project.new_document();
        let outer = app.project.add_cylinder(15.0, 20.0);
        let hole = app.project.add_cylinder(4.25, 30.0);
        let sleeve = app.project.add_body_boolean(outer, hole, 0);
        let rim = *round_edges(&mut app, sleeve, 4.25).first().expect("setup: the rim of the hole");
        let spec = ThreadSpec { standard: ThreadStandard::MetricIso, nominal_d: 10.0, pitch: 1.5, internal: true, fit: 0.0, ..Default::default() };
        let threaded = app.project.add_thread(sleeve, rim, spec, 8.0, 0.0, 0.0);
        let rims = round_edges(&mut app, threaded, 15.0);
        assert!(!rims.is_empty(), "setup: the outer rims of the threaded sleeve");
        let rounded = app.project.add_fillet(threaded, 1.0, rims);
        crate::gui::io_jobs::regenerate_now(&mut app.rebuild_ctx());
        assert!(app.project.mesh_index(rounded).is_some(), "setup: the rounding does not build: {:?}", app.project.regen_errors);
        Sleeve { app, threaded, rounded }
    }

    #[test]
    fn the_bar_dragged_one_row_up_rebuilds_nothing_above_it() {
        let Sleeve { mut app, threaded, rounded } = sleeve();
        let fillet = app.project.timeline.iter().position(|n| n.kind.body() == Some(rounded)).expect("the node of the rounding");
        // the words of the row, as written beside its icon
        let label: String = crate::gui::panels_tree::feature_row_label(&app.project, fillet).chars().filter(|c| !('\u{e000}'..='\u{f8ff}').contains(c)).collect();
        let label = label.trim().to_string();
        let part = app.project.body_owner(rounded).and_then(|c| app.project.components.iter().find(|x| x.id == c)).map(|c| crate::i18n::name(&c.name)).expect("the part of the sleeve");
        let mut h = Hand::new(&mut app);
        assert!(h.double_click_word(&part), "the tree shows the part as {part:?}");
        let row = h.written_at(&label).unwrap_or_else(|| panic!("the tree shows no row {label:?}; it shows {:?}", h.words_drawn().iter().map(|w| w.0.clone()).collect::<Vec<_>>()));
        // the bar stands right under the last row, the whole width of the tree; one row up is 20 points
        let from = egui::pos2(row.left() + 40.0, row.bottom() + 10.0);
        h.drag_screen(from, from - egui::vec2(0.0, 24.0));
        assert_eq!(h.app.project.rollback, Some(fillet), "setup: the bar dragged from under the rounding one row up does not stand above it");

        let mut loud = Vec::new();
        let began = std::time::Instant::now();
        let mut rest = 0;
        while rest < 10 && began.elapsed() < std::time::Duration::from_secs(120) {
            h.frame(Vec::new());
            match h.app.regen.busy.as_ref().filter(|b| b.kind == crate::gui::BgKind::Regen) {
                Some(b) if !b.quiet => loud.push(b.label.clone()),
                Some(_) => {}
                None if !h.app.regen.wanted => rest += 1,
                None => {}
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        loud.dedup();
        assert!(loud.is_empty(), "the bar went one row up, above the rounding, and the rebuild after it is the long one: {loud:?}");
        assert!(h.app.project.mesh_index(threaded).is_some(), "the bar went above the rounding and the threaded sleeve is gone from the screen");
        assert!(h.app.project.mesh_index(rounded).is_none(), "the bar went above the rounding and the rounding is still on screen");
    }
}
