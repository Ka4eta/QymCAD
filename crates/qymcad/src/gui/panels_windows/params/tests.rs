//! THE TABLE OF PARAMETERS UNDER A PERSON'S HAND: what is typed into the row of a named dimension, read off the screen
//! of whole frames.
use crate::gui::App;
use crate::gui::hand::Hand;
use qymcad_core::geom::Point2;
use qymcad_core::model::{Constraint, Id};

/// A rectangle 40 x 20 in a sketch of its own, its width named `a_len` and its height `b_len`. Answers the sketch's
/// index.
fn rectangle_with_two_drivers(app: &mut App) -> usize {
    app.project.new_document();
    let corners = vec![Point2::new(0.0, 0.0), Point2::new(40.0, 0.0), Point2::new(40.0, 20.0), Point2::new(0.0, 20.0)];
    let sid = app.project.add_line_sketch("Profile", corners, true);
    app.project.add_sketch_node(sid, "Profile");
    let si = app.project.sketch_index(sid).expect("the sketch just added");
    let pts: Vec<Id> = app.project.sketches[si].points.iter().map(|q| q.id).collect();
    let width = Constraint::Distance { a: pts[0], b: pts[1], d: 40.0, off: 0.0, expr: String::new(), driven: false, axis: 0, at: None };
    let height = Constraint::Distance { a: pts[1], b: pts[2], d: 20.0, off: 0.0, expr: String::new(), driven: false, axis: 0, at: None };
    app.project.sketches[si].constraints.extend([width, height]);
    assert!(app.project.add_named_dim("a_len".into(), sid, vec![pts[0], pts[1]]), "setup: the width is named");
    assert!(app.project.add_named_dim("b_len".into(), sid, vec![pts[1], pts[2]]), "setup: the height is named");
    si
}

/// A NUMBER AT THE END OF A FORMULA IS NO NAME, and Enter after it applies the formula. A name does not begin with a digit,
/// so the list of names has nothing to offer for `2`.
///
/// Measured: `a_len*2` typed and Enter pressed applied nothing, and the field held `a_len*a_len` when it was left. The `2`
/// opened the list - the list looks in the paths too, and the path of both names of this sketch holds a 2 - and Enter put
/// the first name of the list in its place.
#[test]
fn a_number_ending_a_formula_is_not_taken_for_a_name() {
    let mut app = App::default();
    rectangle_with_two_drivers(&mut app);
    let mut hand = Hand::new(&mut app);
    assert!(hand.press_word(&crate::i18n::tr("wb-params"), egui::pos2(0.0, 0.0)), "the button of the parameters is not on screen");
    assert!(hand.type_after_word("b_len", "a_len*2"), "the value of b_len is not on screen");
    hand.key(egui::Key::Enter).settle();
    assert_eq!(hand.app.project.param_map().get("b_len"), Some(&80.0), "Enter after a_len*2 did not make b_len 80");
}

/// THE LIST OF NAMES UNDER A FORMULA SAYS WHERE EACH NAME STANDS IN WORDS, as the table beside it does: the part by its
/// name, not by the key of the catalogue the document stores it under.
///
/// Measured: `a_` typed into a formula opened the list with `name-part-n#2.Profile` beside `a_len`. The table writes the
/// path of both rows of this sketch, so the list is the third place it stands.
#[test]
fn the_list_of_names_says_the_path_in_words() {
    let mut app = App::default();
    rectangle_with_two_drivers(&mut app);
    let target = app.project.named_dims[0].target.clone();
    let path = qymcad_ui_state::path_in_words(&app.project.driver_path(&target));
    let mut hand = Hand::new(&mut app);
    assert!(hand.press_word(&crate::i18n::tr("wb-params"), egui::pos2(0.0, 0.0)), "the button of the parameters is not on screen");
    assert!(hand.type_after_word("b_len", "a_"), "the value of b_len is not on screen");
    hand.settle();
    let in_list = hand.words_drawn().iter().filter(|(t, _)| *t == path).count();
    assert!(in_list >= 3, "the list under the formula does not say where a_len stands in words ({path:?}): {:?}", hand.words_drawn().iter().map(|(t, _)| t).collect::<Vec<_>>());
}
