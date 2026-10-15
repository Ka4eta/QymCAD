//! A BAR OF BUTTONS LAID OUT BY ITS PLACE: in one or two columns at a side, in one or two rows at the top and the
//! bottom, its groups apart, set to the start, the middle or the end, and what does not fit gathered in a "More"
//! button at its end rather than cut off.
//!
//! The plan is worked out apart from the drawing, from the room, the look and the groups alone, so that where every
//! button lands is a pure function a check can call; the host draws a button into each place the plan gives.

use super::*;

/// The gap between two buttons of one group.
pub const GAP: f32 = 3.0;

/// The gap between two groups, a line across its middle.
pub const GROUP_GAP: f32 = 9.0;

/// How many lines of buttons a bar has across its edge: columns at a side, rows at the top and the bottom.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, serde::Serialize, serde::Deserialize)]
pub enum BarLines {
    One,
    #[default]
    Two,
}

impl BarLines {
    fn count(self) -> usize {
        match self {
            BarLines::One => 1,
            BarLines::Two => 2,
        }
    }
}

/// Where the buttons stand along the edge when they do not fill it.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, serde::Serialize, serde::Deserialize)]
pub enum BarAlign {
    #[default]
    Start,
    Centre,
    End,
}

/// THE LOOK OF A BAR: the side of a button in points, the lines across the edge, the alignment along it.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct BarLook {
    pub size: f32,
    pub lines: BarLines,
    pub align: BarAlign,
}

impl BarLook {
    /// The thickness of the bar across its edge: its lines of buttons.
    pub fn across(&self) -> f32 {
        let n = self.lines.count() as f32;
        n * self.size + (n - 1.0) * GAP
    }
}

/// A BUTTON TO LAY: only its group counts here - a line stands between two groups, and a group starts a slot of its
/// own along the edge.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct BarItem {
    pub group: u32,
}

/// A button laid on the bar: which of the items, and where.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct BarPlace {
    pub item: usize,
    pub rect: egui::Rect,
}

/// THE PLAN OF A BAR: the buttons on it, the line between groups, the "More" button and what stands in it, and the
/// room the whole takes.
#[derive(Clone, PartialEq, Debug)]
pub struct BarPlan {
    pub on_bar: Vec<BarPlace>,
    /// The lines between groups, each across the bar from one end to the other.
    pub lines: Vec<egui::Rect>,
    /// The "More" button, when anything is left over.
    pub more: Option<egui::Rect>,
    /// The items that stand in "More", in their order.
    pub in_more: Vec<usize>,
    /// The room the bar takes, from the top left of the room given.
    pub used: egui::Rect,
}

/// A slot along the edge and a lane across it, as the items are walked.
struct Walk {
    /// Where along the edge each item stands: the start of its slot.
    at: Vec<f32>,
    /// Its lane across the edge.
    lane: Vec<usize>,
    /// Where along the edge each group line stands, before the item it is named by.
    lines: Vec<f32>,
    /// The end of the last slot.
    end: f32,
}

/// WALK THE ITEMS ALONG THE EDGE: lanes across first, then the next slot; a new group starts a slot, after a gap
/// with a line in it.
fn walk(items: &[BarItem], look: &BarLook) -> Walk {
    let lanes = look.lines.count();
    let mut w = Walk { at: Vec::new(), lane: Vec::new(), lines: Vec::new(), end: 0.0 };
    let mut slot = 0.0;
    let mut lane = 0;
    for (i, it) in items.iter().enumerate() {
        if i > 0 {
            if it.group != items[i - 1].group {
                slot += look.size + GROUP_GAP;
                w.lines.push(slot - GROUP_GAP / 2.0);
                lane = 0;
            } else if lane == lanes {
                slot += look.size + GAP;
                lane = 0;
            }
        }
        w.at.push(slot);
        w.lane.push(lane);
        lane += 1;
        w.end = slot + look.size;
    }
    w
}

/// Has a bar a "More" button only when something does not fit, or always - the person put some of its commands there.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MoreSlot {
    WhenFull,
    Always,
}

/// THE PLAN OF A BAR at `edge` in `room`. Everything fits: every item has its place. It does not: the items that fit
/// before a "More" button at the end are laid, the rest stand in it.
pub fn plan_bar(edge: Edge, look: &BarLook, room: egui::Rect, items: &[BarItem], more: MoreSlot) -> BarPlan {
    let upright = edge.upright();
    let along_room = if upright { room.height() } else { room.width() };
    let mut w = walk(items, look);
    let mut shown = items.len();
    let mut more_at = None;
    let free_after = |w: &Walk, n: usize| {
        if n > 0 && w.lane[n - 1] + 1 < look.lines.count() {
            (w.at[n - 1], w.lane[n - 1] + 1)
        } else if n > 0 {
            (w.end + GAP, 0)
        } else {
            (0.0, 0)
        }
    };
    let fits = |w: &Walk, n: usize| free_after(w, n).0 + look.size <= along_room + 0.5;
    if more == MoreSlot::Always && fits(&w, items.len()) {
        more_at = Some(free_after(&w, items.len()));
    } else if more == MoreSlot::Always || w.end > along_room + 0.5 {
        // the longest start of the items that leaves room for "More" in a free lane of its last slot or a slot after it
        shown = 0;
        for k in (0..items.len()).rev() {
            let prefix = walk(&items[..k], look);
            if fits(&prefix, k) {
                shown = k;
                more_at = Some(free_after(&prefix, k));
                w = prefix;
                break;
            }
        }
        if more_at.is_none() {
            more_at = Some((0.0, 0));
        }
    }
    let end = more_at.map(|(slot, _)| slot + look.size).unwrap_or(w.end).max(if shown > 0 { w.end } else { 0.0 });
    let shift = match look.align {
        BarAlign::Start => 0.0,
        BarAlign::Centre => ((along_room - end) / 2.0).max(0.0),
        BarAlign::End => (along_room - end).max(0.0),
    };
    let place = |slot: f32, lane: usize| {
        let across = lane as f32 * (look.size + GAP);
        let min = if upright { egui::pos2(room.min.x + across, room.min.y + shift + slot) } else { egui::pos2(room.min.x + shift + slot, room.min.y + across) };
        egui::Rect::from_min_size(min, egui::vec2(look.size, look.size))
    };
    let thick = look.across();
    let mut plan = BarPlan { on_bar: Vec::new(), lines: Vec::new(), more: None, in_more: Vec::new(), used: egui::Rect::NOTHING };
    for i in 0..shown {
        plan.on_bar.push(BarPlace { item: i, rect: place(w.at[i], w.lane[i]) });
    }
    for at in &w.lines {
        let mid = shift + at;
        plan.lines.push(if upright {
            egui::Rect::from_min_max(egui::pos2(room.min.x, room.min.y + mid), egui::pos2(room.min.x + thick, room.min.y + mid))
        } else {
            egui::Rect::from_min_max(egui::pos2(room.min.x + mid, room.min.y), egui::pos2(room.min.x + mid, room.min.y + thick))
        });
    }
    if let Some((slot, lane)) = more_at {
        plan.more = Some(place(slot, lane));
        plan.in_more = (shown..items.len()).collect();
    }
    let along = shift + end;
    plan.used = if upright { egui::Rect::from_min_size(room.min, egui::vec2(thick, along)) } else { egui::Rect::from_min_size(room.min, egui::vec2(along, thick)) };
    plan
}

/// THE BUTTONS OF THE BARS, as the last frame drew them, for whoever looks at the window from outside: the buttons a
/// person can rest the pointer on, and the "More" button of each bar.
#[derive(Clone, Default)]
struct Noted {
    buttons: Vec<egui::Rect>,
    more: Vec<egui::Rect>,
    pass: u64,
}

fn noted_id() -> egui::Id {
    egui::Id::new("shell-bar-buttons")
}

/// What a bar is writing down: a button, or its "More".
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Button {
    Plain,
    More,
}

/// WRITE DOWN A BUTTON drawn this pass.
pub fn note_button(ctx: &egui::Context, rect: egui::Rect, kind: Button) {
    let pass = ctx.cumulative_pass_nr();
    ctx.data_mut(|d| {
        let n: &mut Noted = d.get_temp_mut_or_default(noted_id());
        if n.pass != pass {
            *n = Noted { pass, ..Noted::default() };
        }
        match kind {
            Button::Plain => n.buttons.push(rect),
            Button::More => n.more.push(rect),
        }
    });
}

/// What the last pass that laid out the panels wrote down; nothing when that pass drew no bar.
fn noted(ctx: &egui::Context) -> Noted {
    let last = draw::last_pass(ctx);
    ctx.data(|d| d.get_temp::<Noted>(noted_id())).filter(|n| Some(n.pass) == last).unwrap_or_default()
}

/// THE BUTTONS OF THE BARS AND OF AN OPEN "MORE" LIST, as the last frame drew them.
pub fn noted_buttons(ctx: &egui::Context) -> Vec<egui::Rect> {
    noted(ctx).buttons
}

/// THE "MORE" BUTTONS OF THE BARS, as the last frame drew them.
pub fn noted_more(ctx: &egui::Context) -> Vec<egui::Rect> {
    noted(ctx).more
}
