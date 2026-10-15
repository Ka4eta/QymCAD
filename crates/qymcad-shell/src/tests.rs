use super::*;

/// THE SHELL DOES NOT KNOW A SINGLE WORKBENCH BY NAME.
///
/// This is the whole rule of the crate, and it can only be held by reading its files: a shell that
/// mentions a workbench is a shell that will grow a second mention, and then the workbench cannot be
/// taken out without editing it. The keys travel through as opaque strings for exactly this reason.
#[test]
fn the_shell_names_no_workbench() {
    /// A file of the crate and its text.
    struct Source {
        file: &'static str,
        text: &'static str,
    }
    for Source { file, text: src } in [Source { file: "lib.rs", text: include_str!("lib.rs") }, Source { file: "draw.rs", text: include_str!("draw.rs") }] {
        // THE CODE, WITHOUT THE PROSE. The doc comments name a tree and a toolbar on purpose - as examples
        // of what MIGHT stand in a place - and forbidding that would forbid explaining the file.
        let code: String =
            src.split("\n#[cfg(test)]").next().expect("the working part is the file above its tests").lines().map(|l| l.trim_start()).filter(|l| !l.starts_with("//")).collect::<Vec<_>>().join("\n");
        // WHOLE WORDS. A plain `contains("part")` found it inside "apart" and reported the shell for a word
        // it had not said - a needle that fires on prose teaches everyone to ignore it.
        let words: Vec<String> = code.split(|c: char| !c.is_alphanumeric() && c != '_').map(|w| w.to_lowercase()).collect();
        for name in ["sketch", "part", "assembly", "cam", "joint", "tree", "viewport", "toolbar", "properties", "sketcher", "status", "props", "wbtools", "menubar"] {
            assert!(!words.iter().any(|w| w == name), "{file} names `{name}` - a place is a KIND of container, and what stands in it is not the shell's business");
        }
    }
}

/// Two places at the left and one at the right, as a workbench would register them, plus the menu and the middle.
fn four() -> Shell {
    let mut s = Shell::default();
    s.put(Place::new("menu", Slot::Menu));
    s.put(Place::new("alpha", Slot::Left).sized(200.0, true));
    s.put(Place::new("gamma", Slot::Left));
    s.put(Place::new("beta", Slot::Right));
    s.put(Place::new("middle", Slot::Centre));
    s
}

/// A place stands where it was registered until somebody moves it; a strip of its own, after the ones registered at
/// its edge before it.
#[test]
fn a_place_stands_where_it_was_registered() {
    let s = four();
    assert_eq!(s.slot_of("alpha"), Some(Slot::Left));
    assert_eq!(s.slot_of("beta"), Some(Slot::Right));
    assert_eq!(s.strips(Edge::Left), vec![vec!["alpha"], vec!["gamma"]], "two places registered at one edge stand side by side, the first nearer the edge");
    assert_eq!(s.slot_of("nobody"), None, "an unregistered key has no place, rather than a default one");
}

/// THE MENU AND THE MIDDLE ARE NOT IN THE LAYOUT: no place for them, no moving, nothing kept for them.
#[test]
fn the_menu_and_the_middle_are_not_in_the_layout() {
    let mut s = four();
    assert_eq!(s.movable(), vec!["alpha", "gamma", "beta"], "the layout lists the places at the edges only");
    assert_eq!(s.place_of("menu"), None);
    assert_eq!(s.slot_of("menu"), Some(Slot::Menu), "the menu still stands above everything");
    s.move_to("menu", Target::NewStrip { edge: Edge::Bottom, before: 0 });
    s.set_seen("menu", Seen::Hidden);
    assert!(s.saved().is_empty(), "the menu cannot be moved or hidden");
    let mut kept = BTreeMap::new();
    kept.insert("menu".to_string(), PanelPlace { edge: Edge::Bottom, ..PanelPlace::default() });
    s.restore(kept);
    assert_eq!(s.slot_of("menu"), Some(Slot::Menu), "a record of a file cannot move the menu either");
}

/// A STACK: a place taken into the strip of another stands after it along the edge, in one strip.
#[test]
fn a_place_is_taken_under_another() {
    let mut s = four();
    s.move_to("beta", Target::Into { edge: Edge::Left, strip: 0, before: 1 });
    assert_eq!(s.strips(Edge::Left), vec![vec!["alpha", "beta"], vec!["gamma"]]);
    assert!(s.strips(Edge::Right).is_empty(), "nothing is left at the right");
    s.move_to("gamma", Target::Into { edge: Edge::Left, strip: 0, before: 0 });
    assert_eq!(s.strips(Edge::Left), vec![vec!["gamma", "alpha", "beta"]], "a place goes before the panel it is dropped before");
}

/// A STRIP OF ITS OWN, before the strip named or innermost.
#[test]
fn a_place_gets_a_strip_of_its_own() {
    let mut s = four();
    s.move_to("beta", Target::NewStrip { edge: Edge::Left, before: 0 });
    assert_eq!(s.strips(Edge::Left), vec![vec!["beta"], vec!["alpha"], vec!["gamma"]]);
    s.move_to("alpha", Target::NewStrip { edge: Edge::Left, before: 9 });
    assert_eq!(s.strips(Edge::Left), vec![vec!["beta"], vec!["gamma"], vec!["alpha"]], "a count past the strips puts it innermost");
}

/// A place taken to another edge forgets its sizes: they were the sizes of where it stood.
#[test]
fn a_moved_place_forgets_its_sizes() {
    let mut s = four();
    s.resize("alpha", Resize::Across(333.0));
    assert_eq!(s.place_of("alpha").and_then(|p| p.across), Some(333.0));
    s.move_to("alpha", Target::NewStrip { edge: Edge::Bottom, before: 0 });
    let p = s.place_of("alpha").expect("alpha has a place");
    assert_eq!(p.edge, Edge::Bottom);
    assert_eq!(p.across, None);
}

/// THE THICKNESS OF A STRIP is every panel's of it: dragged on one, the other keeps it too when it is moved out.
#[test]
fn a_strip_is_resized_whole() {
    let mut s = four();
    s.move_to("gamma", Target::Into { edge: Edge::Left, strip: 0, before: 1 });
    s.resize("gamma", Resize::Across(250.0));
    assert_eq!(s.place_of("alpha").and_then(|p| p.across), Some(250.0));
    assert_eq!(s.place_of("gamma").and_then(|p| p.across), Some(250.0));
    s.resize("alpha", Resize::Along(120.0));
    assert_eq!(s.place_of("alpha").and_then(|p| p.along), Some(120.0));
    assert_eq!(s.place_of("gamma").and_then(|p| p.along), None, "the length along is the panel's own");
}

/// Hiding keeps the place: shown again, it comes back where it stood.
#[test]
fn a_hidden_place_comes_back_where_it_stood() {
    let mut s = four();
    s.move_to("beta", Target::Into { edge: Edge::Left, strip: 1, before: 0 });
    s.set_seen("beta", Seen::Hidden);
    assert_eq!(s.place_of("beta").map(|p| p.seen), Some(Seen::Hidden));
    s.set_seen("beta", Seen::Shown);
    assert_eq!(s.strips(Edge::Left), vec![vec!["alpha"], vec!["beta", "gamma"]]);
}

/// Putting back returns exactly the registration and leaves nothing to keep.
#[test]
fn putting_back() {
    let mut s = four();
    s.move_to("alpha", Target::NewStrip { edge: Edge::Bottom, before: 0 });
    s.set_seen("beta", Seen::Hidden);
    s.reset();
    assert_eq!(s.slot_of("alpha"), Some(Slot::Left), "putting back must give the registered place");
    assert_eq!(s.place_of("beta").map(|p| p.seen), Some(Seen::Shown));
    assert!(s.saved().is_empty(), "nothing was moved, so nothing is worth saving");
}

/// ONLY THE CHANGES ARE SAVED, and they survive a restart. Saving the defaults too would freeze them: a place added
/// by a later version would never reach anybody who had opened the settings once.
#[test]
fn only_the_changes_are_saved() {
    let mut s = four();
    s.move_to("beta", Target::NewStrip { edge: Edge::Top, before: 0 });
    s.set_over("beta", Over::Over);
    assert_eq!(s.saved().keys().cloned().collect::<Vec<_>>(), vec!["beta".to_string()], "only the moved place is written");

    let mut fresh = four();
    fresh.restore(s.saved());
    assert_eq!(fresh.place_of("beta"), s.place_of("beta"), "the layout must survive a restart");
    assert_eq!(fresh.slot_of("alpha"), Some(Slot::Left));
}

/// A place put back by hand where it was registered leaves no record either.
#[test]
fn a_place_put_back_by_hand_leaves_no_record() {
    let mut s = four();
    s.move_to("beta", Target::NewStrip { edge: Edge::Top, before: 0 });
    s.move_to("beta", Target::NewStrip { edge: Edge::Right, before: 0 });
    assert!(s.saved().is_empty(), "beta stands where it was registered: {:?}", s.saved());
}

/// A saved place that nobody registers any more is dropped rather than kept as a ghost.
#[test]
fn a_saved_place_of_a_panel_that_is_gone_is_dropped() {
    let mut s = four();
    let mut kept = BTreeMap::new();
    kept.insert("alpha".to_string(), PanelPlace { edge: Edge::Top, ..PanelPlace::default() });
    kept.insert("gone".to_string(), PanelPlace::default());
    s.restore(kept);
    assert_eq!(s.slot_of("alpha"), Some(Slot::Top));
    assert_eq!(s.slot_of("gone"), None, "a place nobody registered must not come back from the file");
    assert_eq!(s.saved().len(), 1, "and it must not be written out again either");
}

/// Moving an unregistered key does nothing: the file cannot invent a panel.
#[test]
fn an_unknown_key_cannot_be_placed() {
    let mut s = four();
    s.move_to("nobody", Target::NewStrip { edge: Edge::Top, before: 0 });
    s.set_seen("nobody", Seen::Hidden);
    assert!(s.saved().is_empty(), "a key nobody registered must not enter the layout");
}

/// THE MIDDLE IS LAST. egui requires the central panel after every side one, and until now that was
/// held only by the order of statements in the frame.
#[test]
fn the_middle_comes_last() {
    assert_eq!(*Slot::ORDER.last().expect("the order is not empty"), Slot::Centre);
    assert_eq!(Slot::ORDER[0], Slot::Menu, "the menu is above everything");
}

/// A PLACE THAT MAY NOT BE HIDDEN stays shown: asked by the person, and asked by a record of a file.
#[test]
fn a_place_never_hidden_stays_shown() {
    let mut s = four();
    s.put(Place::new("delta", Slot::Top).never_hidden());
    s.set_seen("delta", Seen::Hidden);
    assert_eq!(s.place_of("delta").map(|p| p.seen), Some(Seen::Shown));
    assert!(!s.hideable("delta") && s.hideable("alpha") && !s.hideable("menu"));
    let mut kept = BTreeMap::new();
    kept.insert("delta".to_string(), PanelPlace { edge: Edge::Bottom, seen: Seen::Hidden, ..PanelPlace::default() });
    s.restore(kept);
    let p = s.place_of("delta").expect("delta has a place");
    assert_eq!(p.edge, Edge::Bottom, "the file moves it");
    assert_eq!(p.seen, Seen::Shown, "and cannot hide it");
}
