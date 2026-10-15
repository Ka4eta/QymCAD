//! THE BAR OF THE TOOLS OF A WORKBENCH, drawn from the catalogue: a button a row of the bench open, in the order and
//! with the "More" list the person set, laid by the shell for the edge it stands at, lit while its command is in hand,
//! pressed through the same door as the search.

use super::*;
use crate::command_catalog::Named;

/// The side of an icon against the side of its button: 22 points on the button of 34 the bar had when it was fixed.
const ICON_SHARE: f32 = 22.0 / 34.0;

/// THE BAR OF THE WORKBENCH OPEN at `edge` (none: as at a side).
pub(crate) fn wb_toolbar(bc: &mut qymcad_ui_state::BarCtx, ui: &mut egui::Ui, edge: Option<qymcad_shell::Edge>) {
    use qymcad_ui_state::catalogue::{arrange, hint, pressed, Command};
    let edge = edge.unwrap_or(qymcad_shell::Edge::Left);
    let bench = bc.workbench.code();
    let rows = crate::command_catalog::of_bench(bench);
    let arranged = arrange(&rows, bc.set.bar_sets.get(bench));
    let look = bc.set.bar.look();
    // a group is numbered by its changes, so a group the person split by moving a button stands apart twice
    let mut group = 0;
    let items: Vec<qymcad_shell::BarItem> = arranged
        .on_bar
        .iter()
        .enumerate()
        .map(|(i, c)| {
            if i > 0 && arranged.on_bar[i - 1].group != c.group {
                group += 1;
            }
            qymcad_shell::BarItem { group }
        })
        .collect();
    let slot = if arranged.in_more.is_empty() { qymcad_shell::MoreSlot::WhenFull } else { qymcad_shell::MoreSlot::Always };
    let plan = qymcad_shell::plan_bar(edge, &look, ui.available_rect_before_wrap(), &items, slot);
    ui.allocate_rect(plan.used, egui::Sense::hover());
    let mut chosen: Option<&'static Command> = None;
    for p in &plan.on_bar {
        let c = arranged.on_bar[p.item];
        let img = qymcad_ui_state::icon_image(ui, c.icon, look.size * ICON_SHARE);
        let r = ui.put(p.rect, egui::Button::image(img).selected(pressed(c.launch, bc))).on_hover_text(hint(c, bc));
        qymcad_shell::note_button(ui.ctx(), p.rect, qymcad_shell::Button::Plain);
        if r.clicked() {
            chosen = Some(c);
        }
    }
    let stroke = ui.visuals().widgets.noninteractive.bg_stroke;
    for line in &plan.lines {
        ui.painter().line_segment([line.min, line.max], stroke);
    }
    if let Some(at) = plan.more {
        let rest: Vec<&'static Command> = plan.in_more.iter().map(|i| arranged.on_bar[*i]).chain(arranged.in_more.iter().copied()).collect();
        let lit = rest.iter().any(|c| pressed(c.launch, bc));
        let more = egui::Button::new(egui::RichText::new(ph::DOTS_THREE).size(look.size * ICON_SHARE)).selected(lit);
        let r = ui.put(at, more).on_hover_text(crate::i18n::tr("bar-more-hint"));
        qymcad_shell::note_button(ui.ctx(), at, qymcad_shell::Button::More);
        egui::Popup::menu(&r).show(|ui| {
            for c in rest {
                let img = qymcad_ui_state::icon_image(ui, c.icon, 18.0);
                let row = ui.add(egui::Button::image_and_text(img, c.name()).selected(pressed(c.launch, bc))).on_hover_text(hint(c, bc));
                qymcad_shell::note_button(ui.ctx(), row.rect, qymcad_shell::Button::Plain);
                if row.clicked() {
                    chosen = Some(c);
                }
            }
        });
    }
    if let Some(c) = chosen {
        crate::command_catalog::press(c, bc);
    }
}
