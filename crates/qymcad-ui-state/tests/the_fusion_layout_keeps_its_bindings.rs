//! THE LAYOUT OF A PERSON COMING FROM THAT PROGRAM'S DEFAULT BINDINGS keeps them: Shift and the middle button turn the
//! model, the middle button alone moves the view, the wheel zooms, a bare left drag draws the frame from anywhere, and a
//! short middle click looks at nothing. A layout that only resembles the habit is worse than none.
use egui::PointerButton::{Middle, Primary};
use qymcad_ui_state::MouseNav;

#[test]
fn shift_and_the_middle_button_turn_the_model_and_the_middle_alone_moves_it() {
    let nav = MouseNav::Fusion;
    let turn = nav.rotate();
    assert_eq!(turn.buttons, &[Middle], "the model is turned by the middle button, and the layout names {:?}", turn.buttons);
    assert!(turn.shift && !turn.ctrl && !turn.alt && !turn.any_button, "the model is turned with Shift alone held: {turn:?}");
    let pan = nav.pan();
    assert_eq!(pan.buttons, &[Middle], "the view is moved by the middle button, and the layout names {:?}", pan.buttons);
    assert!(!pan.shift && !pan.ctrl && !pan.alt && !pan.any_button, "the view is moved by the bare middle button: {pan:?}");
    assert_eq!(nav.rotates().len(), 1, "one way of turning the model, and the layout declares {}", nav.rotates().len());
    assert_eq!(nav.pans().len(), 1, "one way of moving the view, and the layout declares {}", nav.pans().len());
}

#[test]
fn the_wheel_zooms_and_no_drag_does() {
    let nav = MouseNav::Fusion;
    assert!(nav.wheel_zooms(), "the wheel zooms under this layout");
    assert!(nav.zooms().is_empty(), "no drag zooms under this layout, and it declares {:?}", nav.zooms());
    assert!(!nav.zooms_sideways(), "a sideways movement zooms under the pen's layout alone");
    assert!(nav.tilts().is_empty(), "this layout does not tilt the view");
}

#[test]
fn a_bare_left_drag_draws_the_frame_from_anywhere_and_the_middle_click_looks_at_nothing() {
    let nav = MouseNav::Fusion;
    let (frame, from_the_model) = nav.frames().expect("this layout draws a frame of selection");
    assert_eq!(frame.buttons, &[Primary], "the frame is drawn with the left button, and the layout names {:?}", frame.buttons);
    assert!(!frame.shift && !frame.ctrl && !frame.alt, "the frame is drawn with the bare left button: {frame:?}");
    assert!(from_the_model, "the frame may begin on the model too");
    assert!(!nav.middle_click_looks(), "a short middle click looks at no point under this layout");
}

#[test]
fn the_layout_is_on_offer_under_its_own_name() {
    assert!(MouseNav::ALL.contains(&MouseNav::Fusion), "the layout is missing from the list the settings window walks over");
    assert_eq!(MouseNav::Fusion.key(), "settings-mouse-fusion", "the catalogue key of the layout's name");
    assert_eq!(MouseNav::Fusion.hint_key(), "settings-mouse-fusion-hint", "the catalogue key of the line describing it");
}
