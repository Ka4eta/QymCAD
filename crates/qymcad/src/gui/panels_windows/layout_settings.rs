//! THE SETTINGS OF THE LAYOUT AND OF THE BARS: the diagram of the window with its panels, the look of the bars of
//! tools, the buttons of the bar of a workbench, and the panels of the menu "View".
//!
//! Everything here is read from the shell and the catalogue: the places from what the workbenches registered, the
//! buttons from the rows a workbench gives. This file names no panel and no workbench, so a workbench added later is
//! set up here without a line of it changed - checked by a test over this file.

use super::*;
use crate::command_catalog::Named;

/// A PANEL CARRIED ON THE DIAGRAM, by its key.
struct Carried {
    key: &'static str,
}

/// A BUTTON CARRIED IN THE LIST OF A BAR, by its place in the list.
struct CarriedButton {
    index: usize,
}

/// The thickness of a strip on the diagram: at a side, and at the top or the bottom.
const SIDE_STRIP: f32 = 84.0;
const FLAT_STRIP: f32 = 30.0;

/// The band of the menu at the top of the diagram.
const MENU_BAND: f32 = 18.0;

/// The seam a panel is dropped on to stand in a strip of its own.
const SEAM: f32 = 8.0;

/// An edge's strips on the diagram, from the edge inward: the rectangles of the strips.
fn strip_rects(edge: qymcad_shell::Edge, count: usize, room: egui::Rect) -> Vec<egui::Rect> {
    (0..count)
        .map(|i| {
            let k = i as f32;
            match edge {
                qymcad_shell::Edge::Top => egui::Rect::from_min_size(egui::pos2(room.min.x, room.min.y + k * FLAT_STRIP), egui::vec2(room.width(), FLAT_STRIP)),
                qymcad_shell::Edge::Bottom => egui::Rect::from_min_size(egui::pos2(room.min.x, room.max.y - (k + 1.0) * FLAT_STRIP), egui::vec2(room.width(), FLAT_STRIP)),
                qymcad_shell::Edge::Left => egui::Rect::from_min_size(egui::pos2(room.min.x + k * SIDE_STRIP, room.min.y), egui::vec2(SIDE_STRIP, room.height())),
                qymcad_shell::Edge::Right => egui::Rect::from_min_size(egui::pos2(room.max.x - (k + 1.0) * SIDE_STRIP, room.min.y), egui::vec2(SIDE_STRIP, room.height())),
            }
        })
        .collect()
}

/// What is left inside an edge's strips.
fn inside_of(edge: qymcad_shell::Edge, count: usize, room: egui::Rect) -> egui::Rect {
    let k = count as f32;
    match edge {
        qymcad_shell::Edge::Top => egui::Rect::from_min_max(egui::pos2(room.min.x, room.min.y + k * FLAT_STRIP), room.max),
        qymcad_shell::Edge::Bottom => egui::Rect::from_min_max(room.min, egui::pos2(room.max.x, room.max.y - k * FLAT_STRIP)),
        qymcad_shell::Edge::Left => egui::Rect::from_min_max(egui::pos2(room.min.x + k * SIDE_STRIP, room.min.y), room.max),
        qymcad_shell::Edge::Right => egui::Rect::from_min_max(room.min, egui::pos2(room.max.x - k * SIDE_STRIP, room.max.y)),
    }
}

/// THE SEAM BEFORE THE STRIP `before` of an edge: a panel dropped there takes a strip of its own in that place.
fn seam(edge: qymcad_shell::Edge, strips: &[egui::Rect], before: usize, room: egui::Rect) -> egui::Rect {
    let half = SEAM / 2.0;
    // the line the seam lies on: the outer side of the strip, or the inner side of the last one
    let line = match (strips.get(before), strips.last()) {
        (Some(s), _) => match edge {
            qymcad_shell::Edge::Top => s.min.y,
            qymcad_shell::Edge::Bottom => s.max.y,
            qymcad_shell::Edge::Left => s.min.x,
            qymcad_shell::Edge::Right => s.max.x,
        },
        (None, Some(s)) => match edge {
            qymcad_shell::Edge::Top => s.max.y,
            qymcad_shell::Edge::Bottom => s.min.y,
            qymcad_shell::Edge::Left => s.max.x,
            qymcad_shell::Edge::Right => s.min.x,
        },
        (None, None) => match edge {
            qymcad_shell::Edge::Top => room.min.y + half,
            qymcad_shell::Edge::Bottom => room.max.y - half,
            qymcad_shell::Edge::Left => room.min.x + half,
            qymcad_shell::Edge::Right => room.max.x - half,
        },
    };
    if edge.upright() {
        egui::Rect::from_min_max(egui::pos2(line - half, room.min.y), egui::pos2(line + half, room.max.y))
    } else {
        egui::Rect::from_min_max(egui::pos2(room.min.x, line - half), egui::pos2(room.max.x, line + half))
    }
}

/// THE DIAGRAM OF THE WINDOW: the menu above, the view in the middle, and at each edge its strips with the panels in
/// them. A panel is dragged onto another to stand beside it in its strip, onto a seam to take a strip of its own; its
/// eye hides it; a panel at the top or the bottom may stand over the view. What changes here is seen in the window
/// at once.
pub(crate) fn layout_diagram(set: &mut qymcad_ui_state::Settings, ui: &mut egui::Ui) {
    let mut shell = crate::gui::shell(set);
    ui.label(egui::RichText::new(crate::i18n::tr("settings-layout-drag-hint")).weak().small());
    let width = ui.available_width().clamp(320.0, 560.0);
    let (whole, _) = ui.allocate_exact_size(egui::vec2(width, 250.0), egui::Sense::hover());
    let painter = ui.painter_at(whole);
    let vis = ui.visuals().clone();
    painter.rect_filled(whole, 4.0, vis.extreme_bg_color);
    let menu = egui::Rect::from_min_size(whole.min, egui::vec2(whole.width(), MENU_BAND));
    painter.rect_filled(menu, 0.0, vis.faint_bg_color);
    painter.text(
        menu.left_center() + egui::vec2(6.0, 0.0),
        egui::Align2::LEFT_CENTER,
        format!("{}  {}", ph::LOCK_SIMPLE, crate::i18n::tr("settings-layout-menu")),
        egui::FontId::proportional(11.0),
        vis.weak_text_color(),
    );
    // the strips: the top and the bottom across the whole width, the sides between them, as the window lays them
    let mut room = egui::Rect::from_min_max(egui::pos2(whole.min.x, menu.max.y), whole.max);
    let mut target: Option<qymcad_shell::Target> = None;
    let mut seen: Option<qymcad_shell::Seen> = None;
    let mut over: Option<qymcad_shell::Over> = None;
    let mut touched: Option<&'static str> = None;
    for edge in [qymcad_shell::Edge::Top, qymcad_shell::Edge::Bottom, qymcad_shell::Edge::Left, qymcad_shell::Edge::Right] {
        let strips = shell.strips(edge);
        let rects = strip_rects(edge, strips.len(), room);
        for (si, (members, band)) in strips.iter().zip(&rects).enumerate() {
            let n = members.len().max(1) as f32;
            for (j, key) in members.iter().enumerate() {
                let k = j as f32;
                let tile = if edge.upright() {
                    egui::Rect::from_min_size(egui::pos2(band.min.x, band.min.y + k * band.height() / n), egui::vec2(band.width(), band.height() / n))
                } else {
                    egui::Rect::from_min_size(egui::pos2(band.min.x + k * band.width() / n, band.min.y), egui::vec2(band.width() / n, band.height()))
                }
                .shrink(2.0);
                let pick = tile_ui(&shell, key, tile, ui);
                if pick.seen.is_some() || pick.over.is_some() {
                    seen = pick.seen;
                    over = pick.over;
                    touched = Some(key);
                }
                // dropped on a panel: into its strip, before it or after it by the half the pointer is in
                let zone = ui.interact(tile, egui::Id::new(("layout-tile-drop", *key)), egui::Sense::hover());
                if let Some(c) = zone.dnd_release_payload::<Carried>() {
                    let at = ui.ctx().pointer_interact_pos().unwrap_or(tile.center());
                    let after = if edge.upright() { at.y > tile.center().y } else { at.x > tile.center().x };
                    target = Some(qymcad_shell::Target::Into { edge, strip: si, before: j + usize::from(after) });
                    touched = Some(c.key);
                }
                if zone.dnd_hover_payload::<Carried>().is_some() {
                    painter.rect_stroke(tile, 3.0, vis.selection.stroke, egui::StrokeKind::Inside);
                }
            }
        }
        // the seams: before each strip, and inside the last - a strip of its own there
        for before in 0..=strips.len() {
            let s = seam(edge, &rects, before, room).intersect(room);
            let zone = ui.interact(s, egui::Id::new(("layout-seam", edge, before)), egui::Sense::hover());
            if zone.dnd_hover_payload::<Carried>().is_some() {
                painter.rect_filled(s, 0.0, vis.selection.bg_fill);
            }
            if let Some(c) = zone.dnd_release_payload::<Carried>() {
                target = Some(qymcad_shell::Target::NewStrip { edge, before });
                touched = Some(c.key);
            }
        }
        room = inside_of(edge, strips.len(), room);
    }
    painter.text(room.center(), egui::Align2::CENTER_CENTER, format!("{}  {}", ph::LOCK_SIMPLE, crate::i18n::tr("settings-layout-view")), egui::FontId::proportional(12.0), vis.weak_text_color());
    if let Some(key) = touched {
        if let Some(t) = target {
            shell.move_to(key, t);
        }
        if let Some(s) = seen {
            shell.set_seen(key, s);
        }
        if let Some(o) = over {
            shell.set_over(key, o);
        }
        set.layout = shell.saved();
    }
}

/// What a tile of the diagram asked for: to be shown or hidden, to stand over the view or beside it.
struct TilePick {
    seen: Option<qymcad_shell::Seen>,
    over: Option<qymcad_shell::Over>,
}

/// ONE PANEL ON THE DIAGRAM: its name to carry it by, its eye, and at the top or the bottom its place over the view.
fn tile_ui(shell: &qymcad_shell::Shell, key: &'static str, tile: egui::Rect, ui: &mut egui::Ui) -> TilePick {
    let mut pick = TilePick { seen: None, over: None };
    let Some(place) = shell.place_of(key) else { return pick };
    let name = crate::i18n::tr(&format!("panel-{key}"));
    let hidden = place.seen == qymcad_shell::Seen::Hidden;
    let vis = ui.visuals().clone();
    let icon = 16.0;
    // the switches stand at the end of the tile, out of the part it is carried by
    let mut switches = Vec::new();
    if shell.hideable(key) {
        switches.push(Switch::Eye);
    }
    if !place.edge.upright() {
        switches.push(Switch::Over);
    }
    let body = egui::Rect::from_min_max(tile.min, egui::pos2(tile.max.x - icon * switches.len() as f32, tile.max.y));
    let fill = if hidden { vis.faint_bg_color } else { vis.widgets.inactive.weak_bg_fill };
    ui.painter().rect_filled(tile, 3.0, fill);
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(body).layout(egui::Layout::left_to_right(egui::Align::Center)));
    child.dnd_drag_source(egui::Id::new(("layout-tile", key)), Carried { key }, |ui| {
        ui.set_min_size(body.size());
        let text = egui::RichText::new(&name).small();
        ui.add(egui::Label::new(if hidden { text.weak() } else { text }).truncate());
    });
    for (i, sw) in switches.iter().enumerate() {
        let at = egui::Rect::from_min_size(egui::pos2(body.max.x + i as f32 * icon, tile.center().y - icon / 2.0), egui::vec2(icon, icon));
        match sw {
            Switch::Eye => {
                let glyph = if hidden { ph::EYE_SLASH } else { ph::EYE };
                let hint = if hidden { "settings-layout-show" } else { "settings-layout-hide" };
                if ui.put(at, egui::Button::new(glyph).small().frame(false)).on_hover_text(crate::i18n::tr1(hint, "name", &name)).clicked() {
                    pick.seen = Some(if hidden { qymcad_shell::Seen::Shown } else { qymcad_shell::Seen::Hidden });
                }
            }
            Switch::Over => {
                let on = place.over == qymcad_shell::Over::Over;
                let hint = if on { "settings-layout-beside" } else { "settings-layout-over" };
                if ui.put(at, egui::Button::new(ph::STACK_SIMPLE).small().frame(false).selected(on)).on_hover_text(crate::i18n::tr1(hint, "name", &name)).clicked() {
                    pick.over = Some(if on { qymcad_shell::Over::Beside } else { qymcad_shell::Over::Over });
                }
            }
        }
    }
    pick
}

/// A switch on a tile of the diagram.
enum Switch {
    Eye,
    Over,
}

/// THE LOOK OF THE BARS OF TOOLS: the size of a button, the lines of buttons, where they stand, how opaque the bar is.
pub(crate) fn bar_look(set: &mut qymcad_ui_state::Settings, ui: &mut egui::Ui, show: &dyn Fn(&str) -> bool) {
    egui::Grid::new("settings_bar_look").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
        if show("settings-bar-size") {
            ui.label(crate::i18n::tr("settings-bar-size"));
            let range = qymcad_ui_state::BAR_SIZES;
            ui.add(egui::Slider::new(&mut set.bar.size, range).step_by(2.0).suffix(crate::i18n::tr("unit-pt-suffix")));
            ui.end_row();
        }
        if show("settings-bar-lines") {
            ui.label(crate::i18n::tr("settings-bar-lines"));
            ui.horizontal(|ui| {
                for lines in [qymcad_shell::BarLines::One, qymcad_shell::BarLines::Two] {
                    if ui.selectable_label(set.bar.lines == lines, crate::i18n::tr(lines_word(lines))).clicked() {
                        set.bar.lines = lines;
                    }
                }
            });
            ui.end_row();
        }
        if show("settings-bar-align") {
            ui.label(crate::i18n::tr("settings-bar-align"));
            ui.horizontal(|ui| {
                for align in [qymcad_shell::BarAlign::Start, qymcad_shell::BarAlign::Centre, qymcad_shell::BarAlign::End] {
                    if ui.selectable_label(set.bar.align == align, crate::i18n::tr(align_word(align))).clicked() {
                        set.bar.align = align;
                    }
                }
            });
            ui.end_row();
        }
        if show("settings-bar-opacity") {
            ui.label(crate::i18n::tr("settings-bar-opacity"));
            ui.add(egui::Slider::new(&mut set.bar.opacity, qymcad_ui_state::BAR_OPACITY).suffix(" %"));
            ui.end_row();
        }
    });
}

/// The word of a count of lines of buttons.
fn lines_word(lines: qymcad_shell::BarLines) -> &'static str {
    match lines {
        qymcad_shell::BarLines::One => "bar-lines-one",
        qymcad_shell::BarLines::Two => "bar-lines-two",
    }
}

/// The word of where the buttons stand along a bar.
fn align_word(align: qymcad_shell::BarAlign) -> &'static str {
    match align {
        qymcad_shell::BarAlign::Start => "bar-align-start",
        qymcad_shell::BarAlign::Centre => "bar-align-centre",
        qymcad_shell::BarAlign::End => "bar-align-end",
    }
}

/// THE BUTTONS OF THE BAR OF `bench`: in their order, each on the bar or in "More". A row is dragged to change the
/// order; the standard set comes back with one button.
pub(crate) fn bar_buttons(set: &mut qymcad_ui_state::Settings, ui: &mut egui::Ui, bench: &str) {
    let rows = crate::command_catalog::of_bench(bench);
    let kept = set.bar_sets.get(bench).cloned();
    let order = qymcad_ui_state::catalogue::ordered(&rows, kept.as_ref());
    let mut in_more: Vec<String> = kept.map(|k| k.in_more).unwrap_or_default();
    ui.label(crate::i18n::tr("settings-bar-buttons"));
    ui.label(egui::RichText::new(crate::i18n::tr("settings-bar-drag-hint")).weak().small());
    let mut moved: Option<Move> = None;
    let mut changed = false;
    for (i, c) in order.iter().enumerate() {
        let row = ui.horizontal(|ui| {
            ui.dnd_drag_source(egui::Id::new(("bar button", c.code)), CarriedButton { index: i }, |ui| {
                ui.label(ph::DOTS_SIX_VERTICAL);
                ui.add(qymcad_ui_state::icon_image(ui, c.icon, 18.0));
                ui.label(c.name());
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let more = in_more.iter().any(|m| m == c.code);
                if ui.selectable_label(more, crate::i18n::tr("settings-bar-in-more")).clicked() && !more {
                    in_more.push(c.code.to_string());
                    changed = true;
                }
                if ui.selectable_label(!more, crate::i18n::tr("settings-bar-on-bar")).clicked() && more {
                    in_more.retain(|m| m != c.code);
                    changed = true;
                }
            });
        });
        let r = row.response;
        if r.dnd_hover_payload::<CarriedButton>().is_some() {
            let below = ui.ctx().pointer_interact_pos().is_some_and(|p| p.y > r.rect.center().y);
            let y = if below { r.rect.max.y } else { r.rect.min.y };
            ui.painter().hline(r.rect.x_range(), y, ui.visuals().selection.stroke);
        }
        if let Some(c) = r.dnd_release_payload::<CarriedButton>() {
            let below = ui.ctx().pointer_interact_pos().is_some_and(|p| p.y > r.rect.center().y);
            moved = Some(Move { from: c.index, to: i + usize::from(below) });
        }
    }
    let mut codes: Vec<String> = order.iter().map(|c| c.code.to_string()).collect();
    if let Some(Move { from, to }) = moved {
        let code = codes.remove(from);
        codes.insert(if to > from { to - 1 } else { to }, code);
        changed = true;
    }
    if changed {
        set.bar_sets.insert(bench.to_string(), qymcad_ui_state::BarSet { order: codes, in_more });
    }
    if ui.button(format!("{}  {}", ph::ARROW_COUNTER_CLOCKWISE, crate::i18n::tr("settings-bar-reset"))).clicked() {
        set.bar_sets.remove(bench);
    }
}

/// A button of a bar carried from one place of the list to another.
struct Move {
    from: usize,
    to: usize,
}

/// THE PANELS OF THE MENU "VIEW": a tick a panel that may be hidden. The menu and the bar of the command in hand are
/// not among them.
pub(crate) fn panels_menu(set: &mut qymcad_ui_state::Settings, ui: &mut egui::Ui) {
    let mut shell = crate::gui::shell(set);
    let mut changed = false;
    for key in shell.movable() {
        if !shell.hideable(key) {
            continue;
        }
        let Some(place) = shell.place_of(key) else { continue };
        let mut shown = place.seen == qymcad_shell::Seen::Shown;
        if ui.checkbox(&mut shown, crate::i18n::tr(&format!("panel-{key}"))).changed() {
            shell.set_seen(key, if shown { qymcad_shell::Seen::Shown } else { qymcad_shell::Seen::Hidden });
            changed = true;
        }
    }
    if changed {
        set.layout = shell.saved();
    }
}

#[cfg(test)]
mod tests {
    /// THE SETTINGS OF THE LAYOUT AND THE BAR OF TOOLS NAME NO PANEL AND NO WORKBENCH: they walk what the shell and the
    /// catalogue hold. A key or a code written here is a workbench that cannot be taken out, or a panel a new one would
    /// not be set up beside.
    #[test]
    fn the_layout_settings_and_the_bar_name_no_panel_and_no_workbench() {
        /// A file read and its text.
        struct Source {
            file: &'static str,
            text: &'static str,
        }
        for Source { file, text } in [
            Source { file: "layout_settings.rs", text: include_str!("layout_settings.rs") },
            Source { file: "tool_bar.rs", text: include_str!("../panels_bars/tool_bar.rs") },
        ] {
            let code = text.split("\n#[cfg(test)]").next().expect("the working part is above the tests");
            // the words written in quotes, a literal at a time
            let literals: Vec<&str> = code.split('"').skip(1).step_by(2).collect();
            for name in ["tree", "props", "wbtools", "status", "toolbar", "command_bar", "menubar", "viewport", "sketch", "part", "assembly"] {
                let named = literals.iter().any(|l| *l == name || l.starts_with(&format!("{name}.")) || l.ends_with(&format!("-{name}")));
                assert!(!named, "{file} names `{name}`: what stands in the layout and on the bars comes from the shell and the catalogue");
            }
        }
    }
}
