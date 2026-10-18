//! THE LOOK OF THE COMMON RIBBON OF MECHANICAL CAD, in one press of the View menu: the bar of tools along the top right
//! under the menu, its buttons in one row under the names of their groups, the canvas light, the panel of properties
//! put away, and the mouse moving the view as the program of that ribbon does - Shift and the middle button turn the
//! model, the middle button alone moves it.
//!
//! A PRESET, NOT A FACTORY DEFAULT: what it sets is the person's own layout from then on, changed in the settings like
//! any other, and "Reset the layout" in the settings brings the factory one back.

use qymcad_shell::{BarAlign, BarLines, Captions, Edge, Seen, Target};
use qymcad_ui_state::{BarSetting, MouseNav, Settings};

/// THE SIDE OF A BUTTON on the ribbon: large enough to be read at a glance, as the icons of that ribbon are.
const BUTTON: f32 = 32.0;

/// SET THE LOOK into `set`. The theme of the window follows `set.scheme`; whoever calls this applies it.
pub(crate) fn apply(set: &mut Settings) {
    let mut shell = crate::gui::shell(set);
    shell.reset();
    shell.move_to("wbtools", Target::NewStrip { edge: Edge::Top, before: 0 });
    shell.set_seen("props", Seen::Hidden);
    set.layout = shell.saved();
    set.bar = BarSetting { size: BUTTON, lines: BarLines::One, align: BarAlign::Start, opacity: 100, captions: Captions::Shown };
    set.mouse_nav = MouseNav::Fusion;
    set.scheme = "light".into();
}

#[cfg(test)]
mod tests {
    use super::*;

    /// THE BAR STANDS ALONG THE TOP in one row under the names of its groups, the properties are put away, the mouse and
    /// the canvas are those of the ribbon.
    #[test]
    fn the_look_puts_the_bar_on_top_and_the_properties_away() {
        let mut set = Settings::default();
        apply(&mut set);
        let shell = crate::gui::shell(&set);
        let bar = shell.place_of("wbtools").expect("the bar of tools has a place");
        assert_eq!(bar.edge, Edge::Top, "the bar of tools stands along the top: {bar:?}");
        let props = shell.place_of("props").expect("the properties have a place");
        assert_eq!(props.seen, Seen::Hidden, "the properties are put away: {props:?}");
        assert_eq!(shell.place_of("tree").map(|p| p.edge), Some(Edge::Left), "the tree stays on the left");
        assert_eq!(set.bar.lines, BarLines::One, "one row of buttons");
        assert_eq!(set.bar.captions, Captions::Shown, "the names of the groups stand on the bar");
        assert_eq!(set.mouse_nav, MouseNav::Fusion, "the mouse moves the view as the ribbon's program does");
        assert_eq!(set.scheme, "light", "the canvas is light");
    }

    /// PRESSED TWICE, the look is the same: it is laid over the factory layout, not over what the last press left.
    #[test]
    fn the_look_pressed_twice_is_the_same() {
        let mut once = Settings::default();
        apply(&mut once);
        let mut twice = once.clone();
        apply(&mut twice);
        assert_eq!(once.layout, twice.layout, "a second press moved the panels again");
    }
}
