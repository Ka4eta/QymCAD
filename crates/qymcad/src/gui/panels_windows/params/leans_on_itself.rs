//! A FORMULA THAT COMES BACK TO THE NAME OF ITS OWN NUMBER IS REFUSED in every field that takes a formula for a named
//! number: the popup of a sketch dimension and the field of a feature's number, as the table of parameters refuses it.
//! Taken, `dia + 1` written for `dia` would add one on every evaluation; `depth*2` for `depth` would have no value at all.
#[cfg(test)]
mod tests {
    use crate::gui::hand::{Hand, Typed};
    use crate::gui::App;
    use qymcad_core::errors::ExprError;
    use qymcad_core::feature::FeatureKind;
    use qymcad_core::model::Constraint;

    /// The diameter the first circle of sketch `si` stands at.
    fn diameter(app: &App, si: usize) -> Option<f64> {
        app.project.sketches[si].constraints.iter().find_map(|c| match c {
            Constraint::Diameter { d, diam, .. } => Some(if *diam { *d } else { 2.0 * *d }),
            _ => None,
        })
    }

    /// The words of the refusal of a formula reading `name` itself, as the program says them: the chain `name -> name`.
    fn leans_on(name: &str) -> String {
        crate::gui::error_words::expr_error_text(&ExprError::LeansOnItself(format!("{name} -> {name}")))
    }

    /// Where the extrusion of the plate stands: its index in the timeline and its node.
    struct Plate {
        ti: usize,
        node: qymcad_core::model::Id,
    }

    /// A plate 60 x 40 drawn and extruded 12 by hand, the height of the extrusion named `depth`.
    fn plate_named_depth(hand: &mut Hand) -> Plate {
        assert!(hand.press_word(&qymcad_i18n::tr("plane-xy-table"), egui::pos2(0.0, 0.0)), "setup: a sketch is started on XY");
        hand.sk_tool(2).click2d(0.0, 0.0).click2d(60.0, 40.0);
        assert!(hand.press_word(&qymcad_i18n::tr("wb-finish"), egui::pos2(0.0, 0.0)), "setup: the sketch is finished");
        hand.close_window();
        hand.look_at([30.0, 20.0, 6.0], 8.0).tool(1).set("height", 12.0).enter();
        let (ti, node) = hand.app.project.timeline.iter().enumerate().find(|(_, n)| matches!(n.kind, FeatureKind::Extrude { .. })).map(|(i, n)| (i, n.id)).expect("setup: the extrusion");
        assert!(hand.app.project.add_named_feat_dim("depth".into(), node, "height"), "setup: the height is named");
        Plate { ti, node }
    }

    /// THE POPUP OF A SKETCH DIMENSION. A circle drawn by hand, its diameter typed as 20 and named `dia` in its popup; the
    /// popup opened again by a double click on the number, `dia + 1` typed, Enter.
    #[test]
    fn the_popup_of_a_dimension_refuses_a_formula_of_its_own_name() {
        let (mut app, _ctx) = crate::gui::import_door::tests::running();
        let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
        app.enter_sketch_edit(si);
        let pal = app.scheme.pal.clone();
        let mut hand = Hand::new(&mut app);
        hand.sk_tool(3).click2d(0.0, 0.0).click2d(10.0, 0.0);
        hand.type_text("20").key(egui::Key::Enter);
        hand.key(egui::Key::Escape).key(egui::Key::Escape);
        hand.sk_select();
        let number = |hand: &Hand| hand.words_drawn().iter().map(|(t, _)| t.clone()).find(|t| t.strip_prefix('\u{d8}').is_some_and(|r| r.starts_with(|c: char| c.is_ascii_digit())));
        let first = number(&hand);
        assert!(first.is_some_and(|d| hand.double_click_word(&d)), "setup: no diameter on screen to open: {:?}", hand.words_drawn());
        assert!(hand.type_after_word(&crate::i18n::tr("sk-driver-label"), "dia"), "setup: the popup of the dimension has no field for its name");
        hand.key(egui::Key::Enter).settle();
        assert_eq!(hand.app.project.param_map().get("dia"), Some(&20.0), "setup: the diameter did not take the name dia");

        hand.sk_select();
        let again = number(&hand);
        assert!(again.is_some_and(|d| hand.double_click_word(&d)), "the diameter named dia is not on screen to open again: {:?}", hand.words_drawn());
        assert!(hand.type_after_word("\u{d8}", "dia + 1"), "the popup of the dimension has no field for its value: {:?}", hand.words_drawn());
        hand.key(egui::Key::Enter).settle();
        // solved twice more: a formula leaning on itself, taken, moves on every solve
        hand.app.project.solve_sketch(si);
        hand.app.project.solve_sketch(si);
        assert_eq!(diameter(hand.app, si), Some(20.0), "dia + 1 written for dia moved the diameter");
        let said = hand.status_painted().into_iter().find(|w| w.text.contains(&leans_on("dia")));
        assert!(said.as_ref().is_some_and(|w| w.colour == pal.error_mild()), "the popup does not refuse dia + 1 in the tone of a refusal: {:?}", said.map(|w| (w.text, w.colour)));
    }

    /// THE FIELD OF A FEATURE'S NUMBER. A plate drawn and extruded 12 by hand, its height named `depth`; the extrusion
    /// opened again by a double click on its row in the tree, `depth*2` typed for the height, Enter.
    #[test]
    fn a_feature_reopened_refuses_a_formula_of_its_own_name() {
        let mut app = App::default();
        let pal = app.scheme.pal.clone();
        let mut hand = Hand::new(&mut app);
        let Plate { ti, node } = plate_named_depth(&mut hand);

        let label = crate::gui::panels_tree::feature_row_label(&hand.app.project, ti);
        assert!(hand.double_click_word(&label), "the tree shows the extrusion as {label:?}");
        assert!(hand.app.tools.cmd.edit.is_some(), "the extrusion did not open to be edited; the program says {:?}", hand.app.status);
        hand.formula(Typed { field: "height", text: "depth*2" }).enter();
        assert_eq!(hand.app.project.feat_dim(node, "height"), None, "depth*2 was written for depth");
        assert_eq!(hand.app.project.param_map().get("depth"), Some(&12.0), "the height named depth lost its value");
        let said = hand.status_painted().into_iter().find(|w| w.text.contains(&leans_on("depth")));
        assert!(said.as_ref().is_some_and(|w| w.colour == pal.error_mild()), "the extrusion does not refuse depth*2 in the tone of a refusal: {:?}", said.map(|w| (w.text, w.colour)));
    }

    /// THE FIELD OF A FEATURE'S NUMBER IN ITS PROPERTIES. The extrusion picked by a click on its row in the tree, `depth*2`
    /// typed into the formula field of its height in the panel on the right, Enter.
    #[test]
    fn the_properties_of_a_feature_refuse_a_formula_of_its_own_name() {
        let mut app = App::default();
        let pal = app.scheme.pal.clone();
        let mut hand = Hand::new(&mut app);
        let Plate { ti, node } = plate_named_depth(&mut hand);
        // the row as it is written on screen: the icon before it is drawn by a font of its own and is no word
        let label = crate::gui::panels_tree::feature_row_label(&hand.app.project, ti);
        let row: String = label.chars().filter(|c| !('\u{e000}'..='\u{f8ff}').contains(c)).collect();
        assert!(hand.press_word(row.trim(), egui::pos2(0.0, 0.0)), "the tree shows the extrusion as {label:?}");
        assert!(hand.type_after_word("\u{192}=", "depth*2"), "the properties of the extrusion have no formula field: {:?}", hand.words_drawn());
        hand.key(egui::Key::Enter).settle();
        assert_eq!(hand.app.project.feat_dim(node, "height"), None, "depth*2 was written for depth");
        assert_eq!(hand.app.project.param_map().get("depth"), Some(&12.0), "the height named depth lost its value");
        let said = hand.status_painted().into_iter().find(|w| w.text.contains(&leans_on("depth")));
        assert!(said.as_ref().is_some_and(|w| w.colour == pal.error_mild()), "the properties do not refuse depth*2 in the tone of a refusal: {:?}", said.map(|w| (w.text, w.colour)));
    }
}
