//! THE LAYOUT AS A PERSON SAW IT: the section of the settings laid out without one word over another, the bar of tools
//! seen through to the model, its buttons with an even margin, the captions of its groups when asked for, and the
//! status line standing at the bottom whatever is dragged. Pictures of what was looked at are left in
//! `target/shots`.
use qymcad::{pos2, vec2, Key, Machine, Picture, Rect, Session};
use qymcad_acceptance::{build, probe};

/// Leave a picture of the window in `target/shots/<name>.png`, to be looked at.
fn keep(s: &mut Session, name: &str) {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/shots");
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(dir.join(format!("{name}.png")), s.snapshot().png());
}

/// What the record of the settings of a start says, written as the record writes it.
struct Record<'a> {
    bar: &'a str,
    place: &'a str,
}

/// THE RECORD OF THE SETTINGS with the bar of tools looking and standing as `kept` says.
fn machine_with(kept: Record) -> Machine {
    let Record { bar, place } = kept;
    let mut m = Machine::default();
    let record = m.kept.record("settings").expect("the factory machine keeps its settings").to_string();
    let from = record.find("bar:(").expect("the record holds the look of the bar");
    let to = from + record[from..].find(')').expect("the look of the bar ends") + 1;
    let record = format!("{}bar:({bar}){}", &record[..from], &record[to..]).replacen("layout:{}", &format!("layout:{{{place}}}"), 1);
    m.kept.set_record("settings", record);
    m
}

/// Open the settings at the section `key`, found in the list of sections.
fn settings_at(s: &mut Session, key: &str) {
    let windows = s.word("menu-windows");
    let settings = s.word("win-settings");
    s.menu(&[&windows, &settings]);
    let list = s.find(&s.word("settings-sec-general"), pos2(700.0, 200.0)).map(|r| r.center()).unwrap_or(pos2(700.0, 200.0));
    let section = s.word(key);
    s.press_word_near(&section, list);
}

/// THE WORDS OF THE SECTION OF THE LAYOUT that lie over one another, by twos: the words drawn in the settings window, the
/// section scrolled down as far as it goes so that every row of it is looked at.
fn words_over_words(s: &mut Session) -> Vec<String> {
    let heading = s.word("win-settings");
    let window = s.widgets().into_iter().find(|w| w.kind == qymcad::Kind::Window && w.label.ends_with(&heading)).map(|w| w.rect).expect("the settings window is open");
    let mut out = Vec::new();
    for _ in 0..4 {
        let words = s.words_within(window);
        for (i, a) in words.iter().enumerate() {
            for b in &words[i + 1..] {
                if a.rect.shrink(1.0).intersects(b.rect.shrink(1.0)) {
                    out.push(format!("{:?} {:?} over {:?} {:?}", a.text, a.rect, b.text, b.rect));
                }
            }
        }
        s.wheel(window.center(), vec2(0.0, -200.0), qymcad::Modifiers::NONE);
    }
    out.sort();
    out.dedup();
    out
}

/// WHERE `text` STANDS IN THE SETTINGS, the section scrolled down by the wheel until it comes into sight.
fn in_settings(s: &mut Session, text: &str) -> Rect {
    let heading = s.word("win-settings");
    let window = s.widgets().into_iter().find(|w| w.kind == qymcad::Kind::Window && w.label.ends_with(&heading)).map(|w| w.rect).expect("the settings window is open");
    for _ in 0..12 {
        if let Some(r) = s.find(text, window.center()).filter(|r| window.contains_rect(*r)) {
            return r;
        }
        s.wheel(window.center(), vec2(0.0, -120.0), qymcad::Modifiers::NONE);
    }
    panic!("{text:?} is not in the settings; on screen: {:?}", s.words())
}

probe! {
    /// THE SECTION OF THE LAYOUT, IN A WIDE AND A NARROW WINDOW: no word lies over another - the diagram is a block of its
    /// own and the settings of the bar a grid under it - and every panel is named whole on the diagram.
    fn the_section_of_the_layout_lies_apart() {
        for size in [vec2(1280.0, 800.0), vec2(900.0, 680.0)] {
            let mut s = Session::start_on(Machine { screen: size.into(), ..Machine::default() });
            s.key(Key::Escape);
            settings_at(&mut s, "settings-sec-layout");
            keep(&mut s, &format!("layout-settings-{}", size.x));
            for key in ["panel-toolbar", "panel-command_bar", "panel-wbtools", "panel-tree", "panel-props", "panel-status"] {
                let name = s.word(key);
                // a pinned panel is named after its lock
                let whole = s.shows(&name) || s.words().iter().any(|w| w.ends_with(&format!(" {name}")));
                assert!(whole, "in a window of {size:?} the panel {name:?} is not named whole on the diagram");
            }
            let over = words_over_words(&mut s);
            assert!(over.is_empty(), "in a window of {size:?} words of the section lie over one another:\n{}", over.join("\n"));
        }
    }
}

/// The colour of `p` at the point `at` of the screen.
fn colour(p: &Picture, at: qymcad::Pos2) -> [i32; 3] {
    let c = p.colour_at(at);
    [i32::from(c[0]), i32::from(c[1]), i32::from(c[2])]
}

/// THE BLOCK FILLS THE VIEW: a block built, and the wheel turned at the middle of the view until the block reaches past
/// its top edge, so that what stands under a bar at the top is the block, not the ground of an empty window.
fn block_filling_the_view(s: &mut Session) {
    build::block(s);
    let c = s.canvas();
    for _ in 0..14 {
        s.wheel(c.center(), vec2(0.0, 120.0), qymcad::Modifiers::NONE);
    }
}

probe! {
    /// THE MODEL SEEN THROUGH THE BAR: the bar of tools at the top at 30 % shows the block under it, not the black of an
    /// empty window - a point of the bar where no button stands is the ground of the bar laid thin over the block.
    fn a_translucent_bar_shows_the_view_under_it() {
        let place = "\"wbtools\":(edge:Top,strip:100)";
        let mut solid = Session::start_on(machine_with(Record { bar: "size:34.0,lines:One,align:Start,opacity:100", place }));
        block_filling_the_view(&mut solid);
        let bar = solid.panel("wbtools").expect("the bar is drawn");
        let opaque = solid.snapshot();
        // the ground of the bar where no button stands: in its margin under the buttons, near the right end of the view,
        // where the block is under it
        let at = pos2(solid.canvas().max.x - 30.0, bar.max.y - 1.0);
        let ground = colour(&opaque, at);
        let mut thin = Session::start_on(machine_with(Record { bar: "size:34.0,lines:One,align:Start,opacity:30", place }));
        block_filling_the_view(&mut thin);
        keep(&mut thin, "translucent-bar");
        let bar = thin.panel("wbtools").expect("the bar is drawn");
        let at = pos2(at.x, bar.max.y - 1.0);
        let picture = thin.snapshot();
        let seen = colour(&picture, at);
        let block = colour(&picture, pos2(at.x, bar.max.y + 8.0));
        let mix = |under: [i32; 3]| [0, 1, 2].map(|k| (3 * ground[k] + 7 * under[k]) / 10);
        let far = |a: [i32; 3], b: [i32; 3]| (0..3).map(|k| (a[k] - b[k]).abs()).max().unwrap_or(0);
        assert!(far(block, [0, 0, 0]) > 40, "setup: the block does not reach the top of the view - the colour under the bar is {block:?}");
        assert!(far(seen, mix(block)) < far(seen, mix([0, 0, 0])), "under the bar at 30 % the colour {seen:?} is its ground {ground:?} over black, not over the block {block:?}");
    }
}

/// The margins of the buttons inside the bar: before the first line of them and after the last, across the edge.
struct Margins {
    before: f32,
    after: f32,
}

/// Which way across a bar is measured: across a bar at a side, or across one at the top or the bottom.
#[derive(Clone, Copy, PartialEq)]
enum Across {
    Side,
    Flat,
}

/// Where the buttons stand inside `bar`, across its edge.
fn margins(buttons: &[Rect], bar: Rect, across: Across) -> Margins {
    if across == Across::Side {
        Margins { before: buttons.iter().map(|b| b.min.x).fold(f32::MAX, f32::min) - bar.min.x, after: bar.max.x - buttons.iter().map(|b| b.max.x).fold(f32::MIN, f32::max) }
    } else {
        Margins { before: buttons.iter().map(|b| b.min.y).fold(f32::MAX, f32::min) - bar.min.y, after: bar.max.y - buttons.iter().map(|b| b.max.y).fold(f32::MIN, f32::max) }
    }
}

probe! {
    /// AN EVEN MARGIN ROUND THE BUTTONS: at the top as much above them as below, at a side as much left as right, and a
    /// small one - 4 points at most.
    fn the_buttons_of_the_bar_have_an_even_margin() {
        for place in ["\"wbtools\":(edge:Top,strip:100)", "\"wbtools\":(edge:Left,strip:0)"] {
            let mut s = Session::start_on(machine_with(Record { bar: "size:34.0,lines:Two,align:Start,opacity:100", place }));
            s.key(Key::Escape);
            build::into_the_first_part(&mut s);
            let bar = s.panel("wbtools").expect("the bar is drawn");
            let m = margins(&s.bar_buttons(), bar, if place.contains("Left") { Across::Side } else { Across::Flat });
            assert!((m.before - m.after).abs() <= 0.5 && m.before <= 4.0, "{place}: the buttons stand {} from one side of the bar and {} from the other", m.before, m.after);
        }
    }
}

probe! {
    /// THE CAPTIONS OF THE GROUPS, ASKED FOR in the settings, stand on the bar as a line across it before each group; not
    /// asked for, none.
    fn the_captions_of_the_groups_come_by_the_setting() {
        let mut s = Session::start();
        s.key(Key::Escape);
        build::into_the_first_part(&mut s);
        let group = s.word("tb-group-sketch3d");
        assert!(!s.shows(&group), "a caption of a group stands on the bar with the setting off");
        settings_at(&mut s, "settings-sec-layout");
        let tick = s.word("settings-bar-group-names");
        let caption = in_settings(&mut s, &tick);
        let field = s.widgets().into_iter().filter(|w| w.kind == qymcad::Kind::CheckBox && (w.rect.center().y - caption.center().y).abs() < 8.0).min_by(|a, b| a.rect.min.x.total_cmp(&b.rect.min.x)).expect("a tick stands beside its caption");
        s.click(field.rect.center());
        let title = s.word("win-settings");
        s.close_window(&title);
        keep(&mut s, "group-captions");
        let bar = s.panel("wbtools").expect("the bar is drawn");
        let at = s.find(&group, bar.center()).unwrap_or_else(|| panic!("the caption {group:?} is not on the bar with the setting on"));
        assert!(bar.expand(0.5).contains_rect(at), "the caption {group:?} stands out of the bar: {at:?} and {bar:?}");
    }
}

probe! {
    /// THE STATUS LINE STAYS AT THE BOTTOM: on the diagram it stands locked as the menu does, dragged to the top it does not
    /// go, and the eye still hides it.
    fn the_status_line_stays_at_the_bottom() {
        let mut s = Session::start();
        s.key(Key::Escape);
        settings_at(&mut s, "settings-sec-layout");
        let corner = s.find(&s.word("settings-layout-menu"), pos2(640.0, 300.0)).expect("the diagram is drawn").center();
        let status = s.word("panel-status");
        let from = s.find(&status, corner).expect("the status line stands on the diagram").center();
        // onto the bar of the path at the top: into its strip
        let toolbar = s.find(&s.word("panel-toolbar"), corner).expect("the bar of the path stands on the diagram").center();
        s.drag(from, toolbar, qymcad::PointerButton::Primary, qymcad::Modifiers::NONE);
        let at = s.panel("status").expect("the status line is drawn");
        assert!(at.max.y >= s.screen().y - 1.0, "the status line was dragged away from the bottom: {at:?}");
    }
}
