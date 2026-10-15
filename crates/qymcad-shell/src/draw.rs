//! THE SHELL DRAWN: the strips of each edge as egui panels, the panels of a strip one after another with a border
//! that is dragged, and the panels standing over the view.

use super::*;

/// The gap between two panels of one strip; the border is dragged in it.
const GAP: f32 = 7.0;

/// THE THICKNESS OF A STRIP AT A SIDE when none of its panels says one: a bar made for the top, put at a side, is as
/// wide as its row of fields otherwise, and the view is squeezed to nothing.
const SIDE: f32 = 240.0;

/// The least length a panel of a strip is squeezed to, so that a border dragged too far still leaves it a handle.
const LEAST: f32 = 40.0;

/// WHERE A PANEL WAS LAID OUT in the last frame that laid out panels, kept for whoever looks at the window from
/// outside.
#[derive(Clone)]
struct Laid {
    key: &'static str,
    rect: egui::Rect,
    pass: u64,
}

/// WHERE THE PANEL `key` STANDS, as the last frame laid it out; `None` when that frame did not draw it.
pub fn laid_out(ctx: &egui::Context, key: &str) -> Option<egui::Rect> {
    let laid: Vec<Laid> = ctx.data(|d| d.get_temp(laid_id())).unwrap_or_default();
    let last = laid.iter().map(|l| l.pass).max()?;
    laid.iter().find(|l| l.key == key && l.pass == last).map(|l| l.rect)
}

fn laid_id() -> egui::Id {
    egui::Id::new("shell-laid-out")
}

/// Write down where a panel was laid out in this pass, dropping what older passes wrote.
fn note(ctx: &egui::Context, key: &'static str, rect: egui::Rect) {
    let pass = ctx.cumulative_pass_nr();
    ctx.data_mut(|d| {
        let laid: &mut Vec<Laid> = d.get_temp_mut_or_default(laid_id());
        laid.retain(|l| l.pass == pass && l.key != key);
        laid.push(Laid { key, rect, pass });
    });
}

pub(crate) fn run_slot(shell: &Shell, slot: Slot, ui: &mut egui::Ui, host: &mut dyn Fills) {
    match slot {
        Slot::Menu => {
            for p in shell.places.iter().filter(|p| p.slot == Slot::Menu) {
                if host.live(p.key) {
                    let r = egui::Panel::top(p.key).show(ui, |ui| host.fill(p.key, ui, None));
                    note(ui.ctx(), p.key, r.response.rect);
                }
            }
        }
        Slot::Centre => {
            for p in shell.places.iter().filter(|p| p.slot == Slot::Centre) {
                if host.live(p.key) {
                    let r = egui::CentralPanel::default().show(ui, |ui| host.fill(p.key, ui, None));
                    note(ui.ctx(), p.key, r.response.rect);
                    over_the_view(shell, r.response.rect, ui, host);
                }
            }
        }
        _ => {
            let Some(edge) = slot.edge() else { return };
            for strip in shell.strips(edge) {
                let members: Vec<&'static str> = strip.into_iter().filter(|k| drawn_beside(shell, k, host)).collect();
                if !members.is_empty() {
                    one_strip(shell, edge, &members, ui, host);
                }
            }
        }
    }
}

/// Is the panel drawn in a strip this frame: wanted, shown and not over the view?
fn drawn_beside(shell: &Shell, key: &'static str, host: &dyn Fills) -> bool {
    shell.place_of(key).is_some_and(|p| p.seen == Seen::Shown && (p.over == Over::Beside || p.edge.upright())) && host.live(key)
}

/// THE GROUND OF A PLACE with the opacity the host asks for.
fn ground(frame: egui::Frame, opacity: f32) -> egui::Frame {
    if opacity >= 1.0 {
        return frame;
    }
    frame.fill(frame.fill.gamma_multiply(opacity.clamp(0.0, 1.0)))
}

/// ONE STRIP: an egui panel at the edge, and in it the panels of the strip.
fn one_strip(shell: &Shell, edge: Edge, members: &[&'static str], ui: &mut egui::Ui, host: &mut dyn Fills) {
    let regs: Vec<&Place> = members.iter().filter_map(|k| shell.registered(k)).collect();
    let first = members[0];
    let id = egui::Id::new(("shell-strip", edge, first));
    let kept = members.iter().find_map(|k| shell.place_of(k).and_then(|p| p.across));
    let registered = regs.iter().filter_map(|p| p.size).reduce(f32::max);
    let resizable = regs.iter().any(|p| p.resizable);
    let mut panel = match edge {
        Edge::Left => egui::Panel::left(id),
        Edge::Right => egui::Panel::right(id),
        Edge::Top => egui::Panel::top(id),
        Edge::Bottom => egui::Panel::bottom(id),
    };
    let side_default = registered.is_none() && edge.upright();
    let resizable = resizable || side_default;
    let size = kept.or(registered).or(side_default.then_some(SIDE));
    if let Some(w) = size {
        panel = if resizable { panel.default_size(w).resizable(true) } else { panel.exact_size(w).resizable(false) };
    }
    let framed = regs.iter().all(|p| p.framed);
    let frame = if framed { host.bar_frame() } else { egui::Frame::side_top_panel(ui.style()) };
    panel = panel.frame(ground(frame, host.opacity(first)));
    if regs.iter().all(|p| !p.separator) {
        panel = panel.show_separator_line(false);
    }
    let r = panel.show(ui, |ui| {
        if let [only] = members {
            host.fill(only, ui, Some(edge));
            Vec::new()
        } else {
            stack(shell, edge, members, ui, host)
        }
    });
    let outer = r.response.rect;
    if let [only] = members {
        note(ui.ctx(), only, outer);
    } else {
        // a panel of a stack reaches across to the edges of the strip, and along to the middle of the border on
        // either side of it: that is where a person sees it end and where the border is taken
        let spans: Vec<egui::Rect> = r.inner;
        for (i, (k, span)) in members.iter().zip(&spans).enumerate() {
            let (lo, hi) = if edge.upright() { (outer.min.y, outer.max.y) } else { (outer.min.x, outer.max.x) };
            let start = if i == 0 { lo } else { (if edge.upright() { span.min.y } else { span.min.x }) - GAP / 2.0 };
            let end = if i + 1 == members.len() { hi } else { (if edge.upright() { span.max.y } else { span.max.x }) + GAP / 2.0 };
            let rect = if edge.upright() { egui::Rect::from_x_y_ranges(outer.x_range(), start..=end) } else { egui::Rect::from_x_y_ranges(start..=end, outer.y_range()) };
            note(ui.ctx(), k, rect);
        }
    }
    // A BORDER DRAGGED is kept in the settings; the size egui settles on by itself is not, or every start would write
    // a record of a layout nobody changed
    if resizable && ui.ctx().input(|i| i.pointer.primary_down()) {
        let now = if edge.upright() { outer.width() } else { outer.height() };
        if size.is_none_or(|s| (s - now).abs() > 0.5) {
            host.resized(first, Resize::Across(now));
        }
    }
}

/// The length a panel takes along its strip.
#[derive(Clone, Copy)]
enum Length {
    /// What its content needs, as the last frame measured it; not yet measured on the first frame.
    Content(Option<f32>),
    /// The length the person dragged it to.
    Kept(f32),
    /// A share of what is left.
    Share,
}

/// THE PANELS OF A STRIP one after another along the edge, with a dragged border between them; where each was laid.
fn stack(shell: &Shell, edge: Edge, members: &[&'static str], ui: &mut egui::Ui, host: &mut dyn Fills) -> Vec<egui::Rect> {
    let full = ui.available_rect_before_wrap();
    let upright = edge.upright();
    let total = if upright { full.height() } else { full.width() };
    let room = (total - GAP * (members.len() - 1) as f32).max(0.0);
    let natural_id = |k: &str| egui::Id::new(("shell-natural", k));
    let wants: Vec<Length> = members
        .iter()
        .map(|k| {
            let reg = shell.registered(k).is_some_and(|p| p.resizable);
            match shell.place_of(k).and_then(|p| p.along) {
                Some(v) => Length::Kept(v),
                None if reg => Length::Share,
                None => Length::Content(ui.ctx().data(|d| d.get_temp::<f32>(natural_id(k)))),
            }
        })
        .collect();
    let lengths = share_out(&wants, room);
    let mut at = if upright { full.min.y } else { full.min.x };
    let mut rects = Vec::new();
    for (k, len) in members.iter().zip(&lengths) {
        let rect = if upright {
            egui::Rect::from_min_size(egui::pos2(full.min.x, at), egui::vec2(full.width(), *len))
        } else {
            egui::Rect::from_min_size(egui::pos2(at, full.min.y), egui::vec2(*len, full.height()))
        };
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect).layout(*ui.layout()).id_salt(("shell-member", *k)));
        child.set_clip_rect(rect.intersect(ui.clip_rect()));
        host.fill(k, &mut child, Some(edge));
        let used = child.min_rect();
        ui.ctx().data_mut(|d| d.insert_temp(natural_id(k), if upright { used.height() } else { used.width() }));
        rects.push(rect);
        at += len + GAP;
    }
    for (i, before) in rects.iter().enumerate().take(members.len().saturating_sub(1)) {
        border(edge, members, &lengths, i, *before, ui, host);
    }
    // the content of a strip of bars is as tall as its tallest bar; a strip of resizable panels fills what it was given
    let thick = rects.iter().map(|r| if upright { r.width() } else { r.height() }).fold(0.0, f32::max);
    let claim = if upright { egui::Rect::from_min_size(full.min, egui::vec2(thick, total)) } else { egui::Rect::from_min_size(full.min, egui::vec2(total, thick)) };
    ui.allocate_rect(claim, egui::Sense::hover());
    rects
}

/// SHARE THE ROOM OF A STRIP: what was kept and what the content measured first, the rest equally among the panels
/// that share, and the last panel takes whatever is left so the strip is filled to its end.
fn share_out(wants: &[Length], room: f32) -> Vec<f32> {
    let n = wants.len();
    let sharing = wants.iter().filter(|w| matches!(w, Length::Share | Length::Content(None))).count();
    let fixed: f32 = wants
        .iter()
        .map(|w| match w {
            Length::Kept(v) | Length::Content(Some(v)) => *v,
            _ => 0.0,
        })
        .sum();
    let each = if sharing > 0 { ((room - fixed) / sharing as f32).max(LEAST) } else { 0.0 };
    let mut out: Vec<f32> = wants
        .iter()
        .map(|w| match w {
            Length::Kept(v) | Length::Content(Some(v)) => v.max(LEAST),
            _ => each,
        })
        .collect();
    // no panel is pushed out of the strip: what is over is taken from the longest, never below the least
    let mut over = out.iter().sum::<f32>() - room;
    while over > 0.5 {
        let Some(i) = (0..n).max_by(|a, b| out[*a].total_cmp(&out[*b])) else { break };
        let cut = over.min(out[i] - LEAST);
        if cut <= 0.0 {
            break;
        }
        out[i] -= cut;
        over -= cut;
    }
    // the last panel reaches the end of the strip
    let short = room - out.iter().sum::<f32>();
    if short > 0.0 {
        if let Some(last) = out.last_mut() {
            *last += short;
        }
    }
    out
}

/// THE BORDER after the panel `i` of a strip: a hairline, and a handle the person drags to change the lengths.
fn border(edge: Edge, members: &[&'static str], lengths: &[f32], i: usize, before: egui::Rect, ui: &mut egui::Ui, host: &mut dyn Fills) {
    let upright = edge.upright();
    let gap = if upright {
        egui::Rect::from_min_max(egui::pos2(before.min.x, before.max.y), egui::pos2(before.max.x, before.max.y + GAP))
    } else {
        egui::Rect::from_min_max(egui::pos2(before.max.x, before.min.y), egui::pos2(before.max.x + GAP, before.max.y))
    };
    let r = ui.interact(gap, egui::Id::new(("shell-border", members[i])), egui::Sense::drag());
    let stroke = if r.hovered() || r.dragged() { ui.visuals().widgets.active.bg_stroke } else { ui.visuals().widgets.noninteractive.bg_stroke };
    if upright {
        ui.painter().hline(gap.x_range(), gap.center().y, stroke);
    } else {
        ui.painter().vline(gap.center().x, gap.y_range(), stroke);
    }
    if r.hovered() || r.dragged() {
        ui.ctx().set_cursor_icon(if upright { egui::CursorIcon::ResizeVertical } else { egui::CursorIcon::ResizeHorizontal });
    }
    if r.dragged() {
        let d = if upright { r.drag_delta().y } else { r.drag_delta().x };
        if d != 0.0 {
            host.resized(members[i], Resize::Along((lengths[i] + d).max(LEAST)));
        }
    }
}

/// THE PANELS STANDING OVER THE VIEW: at its top edge going down, at its bottom edge going up; the view under them
/// is not narrowed, and their ground lets it show through as much as the host says.
fn over_the_view(shell: &Shell, view: egui::Rect, ui: &mut egui::Ui, host: &mut dyn Fills) {
    for edge in [Edge::Top, Edge::Bottom] {
        let mut at = if edge == Edge::Top { view.min.y } else { view.max.y };
        for k in shell.strips(edge).into_iter().flatten() {
            let wanted = shell.place_of(k).is_some_and(|p| p.seen == Seen::Shown && p.over == Over::Over);
            if !wanted || !host.live(k) {
                continue;
            }
            let frame = ground(host.bar_frame(), host.opacity(k));
            let (pivot, pos) = if edge == Edge::Top { (egui::Align2::LEFT_TOP, egui::pos2(view.min.x, at)) } else { (egui::Align2::LEFT_BOTTOM, egui::pos2(view.min.x, at)) };
            let r = egui::Area::new(egui::Id::new(("shell-over", k))).order(egui::Order::Middle).pivot(pivot).fixed_pos(pos).constrain(false).show(ui.ctx(), |ui| {
                ui.set_width(view.width() - frame.total_margin().sum().x);
                frame.show(ui, |ui| host.fill(k, ui, Some(edge)));
            });
            let rect = r.response.rect;
            note(ui.ctx(), k, rect);
            at = if edge == Edge::Top { rect.max.y } else { rect.min.y };
        }
    }
}
