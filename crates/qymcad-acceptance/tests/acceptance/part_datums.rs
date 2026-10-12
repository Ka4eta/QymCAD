//! THE DATUMS: a plane, an axis and a point, each made every way the program offers, and each read back from the
//! document - where it stands and which way it looks.
use qymcad::{Key, Session};
use qymcad_acceptance::build;
use qymcad_acceptance::probe;

/// The datums of the document, newest last.
fn datums(s: &mut Session) -> Vec<qymcad::Datum> {
    s.document().datums
}

/// The last datum of `kind`.
fn last(s: &mut Session, kind: &str) -> qymcad::Datum {
    datums(s).into_iter().rfind(|d| d.kind == kind).unwrap_or_else(|| panic!("the document holds no {kind}: {:?}", datums(s)))
}

/// Take the tool whose hint is `hint`.
fn take(s: &mut Session, hint: &str) {
    let hint = s.word(hint);
    s.press_hint(&hint);
}

/// Press the word `key` names in the bar of options at the top.
fn bar_word(s: &mut Session, key: &str) {
    let word = s.word(key);
    s.press_word_near(&word, qymcad::pos2(640.0, 24.0));
}

/// Type `text` into the field under the caption `caption`.
fn field(s: &mut Session, caption: &str, text: &str) {
    s.fill(caption, text);
}

/// Whether the place `at` is `want`, to a thousandth.
fn stands_at(at: [f64; 3], want: [f64; 3]) -> bool {
    at.iter().zip(want).all(|(a, b)| (a - b).abs() < 1e-3)
}

/// Whether the direction `dir` points along `want` (either way).
fn points_along(dir: [f64; 3], want: [f64; 3]) -> bool {
    let dot: f64 = dir.iter().zip(want).map(|(a, b)| a * b).sum();
    let n: f64 = dir.iter().map(|a| a * a).sum::<f64>().sqrt() * want.iter().map(|a| a * a).sum::<f64>().sqrt();
    n > 0.0 && (dot.abs() / n - 1.0).abs() < 1e-3
}

/// WHERE THE ROUND SIDE OF A SHAFT CAN BE CLICKED: only the facets turned towards the eye can be, so the places round
/// it are tried in turn.
fn the_side_of_a_shaft(s: &mut Session, r: f64, z: f64) -> qymcad::Pos2 {
    let middle = s.document().bodies.iter().rfind(|b| !b.consumed && !b.sheet).map(|b| [(b.min[0] + b.max[0]) / 2.0, (b.min[1] + b.max[1]) / 2.0]).unwrap_or([0.0, 0.0]);
    for k in 0..(72 * 3) {
        let a = (k % 72) as f64 * std::f64::consts::TAU / 72.0;
        let r = r - 0.05 * (k / 72) as f64;
        let p = [middle[0] + r * a.cos(), middle[1] + r * a.sin(), z];
        let mut seen = None;
        let message = qymcad_acceptance::refusal(|| seen = Some(s.face_at(p)));
        if message.is_empty() {
            if let Some(at) = seen {
                return at;
            }
        }
    }
    panic!("no place on the round side of the shaft can be seen from here");
}

/// A datum plane `dist` from what stands under the pointer at `p`, made the way a person makes one.
fn plane_from(s: &mut Session, p: [f64; 3], dist: &str) {
    take(s, "g-datum-plane-hint");
    let at = s.in_space(p);
    s.click(at);
    let field = s
        .widgets()
        .into_iter()
        .filter(|w| w.kind == qymcad::Kind::TextField)
        .min_by(|a, b| a.rect.center().distance(at).total_cmp(&b.rect.center().distance(at)))
        .unwrap_or_else(|| panic!("the datum plane asks for no offset; on screen: {:?}", s.words()));
    s.click(field.rect.center()).chord(qymcad::Modifiers::COMMAND, Key::A).type_text(dist).key(Key::Enter);
}

probe! {
    /// A PLANE FROM A PLANE OF THE WORLD stands the offset away from it, facing the same way.
    fn a_plane_is_made_from_a_plane_of_the_world() {
        let mut s = Session::start();
        build::into_the_first_part(&mut s);
        plane_from(&mut s, [0.0, 0.0, 0.0], "20");
        let d = last(&mut s, "plane");
        assert!(stands_at(d.at, [0.0, 0.0, 20.0]), "the plane 20 above the table stands at {:?}", d.at);
        assert!(points_along(d.dir, [0.0, 0.0, 1.0]), "the plane above the table does not lie flat: it looks {:?}", d.dir);
    }
}

probe! {
    /// A PLANE FROM A FACE OF A BODY stands the offset away from that face.
    fn a_plane_is_made_from_a_face_of_a_body() {
        let mut s = Session::start();
        build::block(&mut s);
        take(&mut s, "g-datum-plane-hint");
        let top = s.face_at([20.0, 15.0, 10.0]);
        s.click(top);
        let field = s
            .widgets()
            .into_iter()
            .filter(|w| w.kind == qymcad::Kind::TextField)
            .min_by(|a, b| a.rect.center().distance(top).total_cmp(&b.rect.center().distance(top)))
            .unwrap_or_else(|| panic!("the datum plane asks for no offset; on screen: {:?}", s.words()));
        s.click(field.rect.center()).chord(qymcad::Modifiers::COMMAND, Key::A).type_text("5").key(Key::Enter);
        let d = last(&mut s, "plane");
        assert!((d.at[2] - 15.0).abs() < 1e-3, "the plane 5 above the top of a block 10 thick stands at {:?}", d.at);
        assert!(points_along(d.dir, [0.0, 0.0, 1.0]), "the plane over the top face does not lie flat: it looks {:?}", d.dir);
    }
}

probe! {
    /// A PLANE FROM ANOTHER PLANE adds its offset to the one it was made from.
    fn a_plane_is_made_from_another_plane() {
        let mut s = Session::start();
        build::into_the_first_part(&mut s);
        plane_from(&mut s, [0.0, 0.0, 0.0], "20");
        plane_from(&mut s, [0.0, 0.0, 20.0], "5");
        let all: Vec<qymcad::Datum> = datums(&mut s).into_iter().filter(|d| d.kind == "plane").collect();
        assert!(all.len() == 2, "two planes were made, and the document holds {}", all.len());
        assert!((all[1].at[2] - 25.0).abs() < 1e-3, "the plane 5 above the one at 20 stands at {:?}", all[1].at);
    }
}

probe! {
    /// A POINT BY ITS COORDINATES stands where the three numbers say.
    fn a_point_is_made_by_its_coordinates() {
        let mut s = Session::start();
        build::into_the_first_part(&mut s);
        take(&mut s, "g-datum-point-hint");
        bar_word(&mut s, "cmd-coordinates");
        for (caption, value) in [("X", "5"), ("Y", "6"), ("Z", "7")] {
            field(&mut s, caption, value);
        }
        s.key(Key::Enter);
        let d = last(&mut s, "point");
        assert!(stands_at(d.at, [5.0, 6.0, 7.0]), "the point typed as (5, 6, 7) stands at {:?}", d.at);
    }
}

probe! {
    /// A POINT AT A VERTEX stands on the corner of the body that was clicked.
    fn a_point_is_made_at_a_vertex_of_a_body() {
        let mut s = Session::start();
        build::block(&mut s);
        take(&mut s, "g-datum-point-hint");
        bar_word(&mut s, "cmd-to-vertex");
        let corner = s.vertex_at([40.0, 30.0, 10.0]);
        s.click(corner);
        s.key(Key::Enter);
        let d = last(&mut s, "point");
        assert!(stands_at(d.at, [40.0, 30.0, 10.0]), "the point put on the corner of the block stands at {:?}", d.at);
    }
}

probe! {
    /// AN AXIS ALONG AN EDGE runs the way that edge runs.
    fn an_axis_is_made_along_an_edge() {
        let mut s = Session::start();
        build::block(&mut s);
        take(&mut s, "g-datum-axis-hint");
        let edge = s.edge_at([20.0, 0.0, 10.0]);
        s.click(edge);
        s.key(Key::Enter);
        let d = last(&mut s, "axis");
        assert!(points_along(d.dir, [1.0, 0.0, 0.0]), "the axis along the front top edge of the block runs {:?}", d.dir);
        assert!((d.at[2] - 10.0).abs() < 1e-3 && (d.at[1]).abs() < 1e-3, "the axis does not lie on that edge: it starts at {:?}", d.at);
    }
}

probe! {
    /// AN AXIS ALONG A ROUND FACE runs up the middle of it - the middle of that face, not of the world.
    fn an_axis_is_made_along_a_round_face() {
        let mut s = Session::start();
        build::into_the_first_part(&mut s);
        // the cylinder is put on a point of its own, away from the origin, so that the world's own axis cannot pass
        // for the axis of the face
        take(&mut s, "g-datum-point-hint");
        bar_word(&mut s, "cmd-coordinates");
        field(&mut s, "X", "25");
        s.key(Key::Enter);
        take(&mut s, "tb-cylinder-hint");
        let place = s.in_space([25.0, 0.0, 0.0]);
        s.click(place);
        s.key(Key::Enter);
        let cylinder = s.document().bodies.iter().rfind(|b| !b.consumed && !b.sheet).cloned().unwrap_or_else(|| panic!("no cylinder was made"));
        assert!((cylinder.min[0] - 15.0).abs() < 0.1, "the cylinder was not put on the point at 25: it runs from {:?}", cylinder.min);
        take(&mut s, "g-datum-axis-hint");
        let side = the_side_of_a_shaft(&mut s, 10.0, 10.0);
        s.click(side);
        s.key(Key::Enter);
        let d = last(&mut s, "axis");
        assert!(points_along(d.dir, [0.0, 0.0, 1.0]), "the axis of an upright cylinder runs {:?}", d.dir);
        assert!((d.at[0] - 25.0).abs() < 1e-3 && (d.at[1]).abs() < 1e-3, "the axis does not run up the middle of the cylinder at (25, 0): it starts at {:?}", d.at);
    }
}

probe! {
    /// AN AXIS THROUGH TWO POINTS runs from one to the other - not along the world's own axis it would take by
    /// default.
    fn an_axis_is_made_through_two_points() {
        let mut s = Session::start();
        build::into_the_first_part(&mut s);
        for y in [0.0, 20.0] {
            take(&mut s, "g-datum-point-hint");
            bar_word(&mut s, "cmd-coordinates");
            field(&mut s, "X", "10");
            field(&mut s, "Y", &y.to_string());
            s.key(Key::Enter);
        }
        take(&mut s, "g-datum-axis-hint");
        bar_word(&mut s, "cmd-two-points");
        for y in [0.0, 20.0] {
            let at = s.in_space([10.0, y, 0.0]);
            s.click(at);
        }
        s.key(Key::Enter);
        let d = last(&mut s, "axis");
        assert!(points_along(d.dir, [0.0, 1.0, 0.0]), "the axis through (10, 0, 0) and (10, 20, 0) runs {:?}", d.dir);
        assert!((d.at[0] - 10.0).abs() < 1e-3, "the axis does not pass through the points: it starts at {:?}", d.at);
    }
}

probe! {
    /// AN AXIS BY HAND runs where the numbers say.
    fn an_axis_is_made_by_hand() {
        let mut s = Session::start();
        build::into_the_first_part(&mut s);
        take(&mut s, "g-datum-axis-hint");
        bar_word(&mut s, "cmd-manual");
        s.key(Key::Enter);
        let d = last(&mut s, "axis");
        assert!(d.dir.iter().any(|v| v.abs() > 1e-6), "the axis made by hand has no direction at all: {:?}", d.dir);
    }
}

/// The value of the text field nearest to `at` on the screen.
fn field_near(s: &mut Session, at: qymcad::Pos2) -> f64 {
    let f = s.widgets().into_iter().filter(|w| w.kind == qymcad::Kind::TextField).min_by(|a, b| a.rect.center().distance(at).total_cmp(&b.rect.center().distance(at))).expect("a field");
    f.value.trim().parse().unwrap_or(f64::NAN)
}

probe! {
    /// THE OFFSET OF A DATUM PLANE IS DRAGGED BY ITS ARROW: a plane taken from the table, offset 20, its arrow from the
    /// origin to (0, 0, 20) pulled 40 px up - the offset grows, and Enter puts the plane where the field says. Reported
    /// behaviour: the datum tools had no arrow, their values were typed only.
    fn the_offset_of_a_datum_plane_is_dragged_by_its_arrow() {
        let mut s = Session::start();
        build::into_the_first_part(&mut s);
        take(&mut s, "g-datum-plane-hint");
        let at = s.in_space([0.0, 0.0, 0.0]);
        s.click(at);
        let field = s.widgets().into_iter().filter(|w| w.kind == qymcad::Kind::TextField).min_by(|a, b| a.rect.center().distance(at).total_cmp(&b.rect.center().distance(at))).expect("the field of the offset");
        s.click(field.rect.center()).chord(qymcad::Modifiers::COMMAND, Key::A).type_text("20");
        let tip = s.projected([0.0, 0.0, 20.0]);
        s.drag(tip, tip + qymcad::vec2(0.0, -40.0), qymcad::PointerButton::Primary, qymcad::Modifiers::NONE);
        let pulled = field_near(&mut s, tip);
        assert!(pulled > 20.5, "the arrow pulled up 40 px left the offset at {pulled}");
        s.key(Key::Enter);
        let d = last(&mut s, "plane");
        assert!((d.at[2] - pulled).abs() < 1e-3, "the plane stands at {:?}, the field said {pulled}", d.at);
    }
}

probe! {
    /// A DATUM POINT IS DRAGGED BY ITS ARROWS: a point by its coordinates, X 10, the arrow along X pulled 40 px along
    /// itself - X grows, Y and Z stay; Enter puts the point there.
    fn a_datum_point_is_dragged_by_its_arrows() {
        let mut s = Session::start();
        build::into_the_first_part(&mut s);
        take(&mut s, "g-datum-point-hint");
        bar_word(&mut s, "cmd-coordinates");
        field(&mut s, "X", "10");
        // the arrow along X on the screen: from the point, the way +X runs there
        let (from, along) = (s.projected([10.0, 0.0, 0.0]), s.projected([11.0, 0.0, 0.0]));
        let way = (along - from).normalized();
        // the middle of the arrow: the fields of the point must not stand over it
        let grab = from + way * 30.0;
        s.drag(grab, grab + way * 40.0, qymcad::PointerButton::Primary, qymcad::Modifiers::NONE);
        s.key(Key::Enter);
        let d = last(&mut s, "point");
        assert!(d.at[0] > 10.5 && d.at[1].abs() < 1e-6 && d.at[2].abs() < 1e-6, "the point pulled along X stands at {:?}", d.at);
    }
}

probe! {
    /// THE ORIGIN OF AN AXIS BY HAND IS DRAGGED BY ITS ARROWS: an axis set by hand, its origin X 10, the arrow along X
    /// pulled 40 px along itself - the origin's X grows; the modes that take an edge or a face are as they were.
    fn the_origin_of_an_axis_by_hand_is_dragged_by_its_arrows() {
        let mut s = Session::start();
        build::into_the_first_part(&mut s);
        take(&mut s, "g-datum-axis-hint");
        bar_word(&mut s, "cmd-manual");
        field(&mut s, "O.x", "10");
        let (from, along) = (s.projected([10.0, 0.0, 0.0]), s.projected([11.0, 0.0, 0.0]));
        let way = (along - from).normalized();
        let grab = from + way * 30.0;
        s.drag(grab, grab + way * 40.0, qymcad::PointerButton::Primary, qymcad::Modifiers::NONE);
        s.key(Key::Enter);
        let d = last(&mut s, "axis");
        assert!(d.at[0] > 10.5, "the origin of the axis pulled along X stands at {:?}", d.at);
    }
}

probe! {
    /// A FACE BEHIND A BASE PLANE IS PICKED, NOT THE PLANE: a block from y 20 to 50, 10 high, the plane XZ (y 0) between
    /// it and the eye, the datum plane taken and the front face of the block clicked near the origin - the plane is made
    /// from that face, offset from y 20 outward, not from XZ. Reported behaviour: the square of a base plane at the origin took the
    /// pointer over the faces behind it, a sketch or a datum could not be put on them.
    fn a_face_behind_a_base_plane_is_picked_before_the_plane() {
        let mut s = Session::start();
        build::into_the_first_part(&mut s);
        let xy = s.word("plane-xy-table");
        s.press_word(&xy);
        let rect = s.word("tb-rect-hint");
        s.press_hint(&rect).click_on_sketch(0.0, 20.0).click_on_sketch(40.0, 50.0);
        let finish = s.word("wb-finish");
        s.press_word(&finish);
        let extrude = s.word("tb-extrude-hint");
        s.press_hint(&extrude);
        s.key(Key::Enter);
        s.key(Key::Enter);
        take(&mut s, "g-datum-plane-hint");
        let at = s.projected([8.0, 20.0, 5.0]);
        s.click(at);
        s.key(Key::Enter);
        let d = last(&mut s, "plane");
        assert!(points_along(d.dir, [0.0, 1.0, 0.0]) && d.at[1] < 19.0, "the plane picked on the front face at y 20 stands at {:?} looking {:?} - taken from a base plane, not from the face", d.at, d.dir);
    }
}
