//! SENDING A MODEL OR A DRAWING STRAIGHT INTO ANOTHER PROGRAM: a slicer, the program of a laser, the program that makes
//! the toolpaths of a mill. The file is written where the system keeps passing files, in the format the program is best
//! handed, and the program is started with it.
//!
//! A MODEL GOES THE WAY OF AN EXPORT, by the same door: the mesh quality is asked as for any mesh, the bodies are
//! written by the same job, and only the name of the file is given here rather than by the chooser of the system. So
//! the send moves through stages - picked, armed for the next chooser, being written - and the program is started once
//! the written file is there and no write is running. A drawing of a sketch is written at once.
//!
//! The armed stage lasts a moment: a chooser that comes up later than that belongs to something else, and is put up as
//! usual.

use std::cell::RefCell;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use egui_phosphor::regular as ph;
use qymcad_io::{SendFormat, SendFound, SendKind, SendWhat};
use qymcad_ui_state::{MeshFormat, Notice};

/// HOW LONG A PICK WAITS FOR ITS CHOOSER: the export asks for the file in the frame it is started in, or the one after.
const ARMED_FOR: Duration = Duration::from_secs(2);

/// HOW LONG A FILE MAY BE AWAITED with no write running: the write starts in the frame after the name is given, and a
/// write that never started (the export refused) leaves nothing to wait for.
const WRITE_STARTS_WITHIN: Duration = Duration::from_secs(3);

/// A PROGRAM PICKED IN THE MENU, and the format it is to be handed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SendPick {
    pub(crate) name: String,
    pub(crate) exe: PathBuf,
    pub(crate) format: SendFormat,
}

/// WHAT THE MENU ANSWERED: a program picked, or something to be said on the status line.
pub(crate) enum SendAnswer {
    Pick(SendPick),
    Say(Notice),
}

/// The program, its executable and the file it is to be started with.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Delivery {
    name: String,
    exe: PathBuf,
    path: PathBuf,
}

/// WHERE A SEND OF A MODEL STANDS.
enum Stage {
    /// The program is picked; the mesh quality is being asked.
    Picked(Delivery),
    /// The next chooser to write a file is answered with the file of the delivery, until `until`.
    Armed { delivery: Delivery, until: Instant },
    /// The name is given; the program starts once the file is there and no write is running.
    Writing { delivery: Delivery, since: Instant },
}

/// The programs found, and what reading a person's own list said, if it failed and has not been said yet.
struct Programs {
    found: Vec<SendFound>,
    untold: Option<String>,
}

thread_local! {
    /// THE PROGRAMS FOUND, looked for once and again on asking: walking the folders of programs on every frame a menu
    /// is open would be felt.
    static PROGRAMS: RefCell<Option<Programs>> = const { RefCell::new(None) };
    static STAGE: RefCell<Option<Stage>> = const { RefCell::new(None) };
}

/// WHAT AN EXPORT STARTS: the door of an exact file, or the mesh quality for a mesh.
pub(crate) enum Begin {
    Exact(qymcad_kernel::ExactFormat),
    Mesh(MeshFormat),
}

/// A PERSON'S OWN LIST OF PROGRAMS: beside the settings, in the shape of the built-in one.
pub(crate) fn own_list() -> Option<PathBuf> {
    qymcad_paths::data_root().map(|d| d.join("send_programs.ron"))
}

/// The folder a file is sent from: the system's folder of passing files, in a folder of the program's own.
fn folder() -> PathBuf {
    let f = std::env::temp_dir().join("FieldCAD").join("send");
    let _ = std::fs::create_dir_all(&f);
    f
}

/// LOOK FOR THE PROGRAMS: the built-in list with a person's own laid over it, searched in the folders of programs.
fn look() -> Programs {
    let mut untold = None;
    let mut catalogue = qymcad_io::SendCatalogue::built_in();
    if let Some(path) = own_list().filter(|p| p.is_file()) {
        match std::fs::read_to_string(&path).map_err(|e| e.to_string()).and_then(|t| qymcad_io::SendCatalogue::read(&t)) {
            Ok(own) => catalogue = catalogue.merged(own),
            Err(e) => untold = Some(e),
        }
    }
    let var = |n: &str| std::env::var(n).ok();
    let roots = qymcad_io::search_roots(var);
    Programs { found: qymcad_io::find(&catalogue, &roots, var), untold }
}

/// The catalogue key naming a group of the menu.
fn group_key(kind: SendKind) -> &'static str {
    match kind {
        SendKind::Slicer => "send-group-slicer",
        SendKind::Laser => "send-group-laser",
        SendKind::Cam => "send-group-cam",
    }
}

/// The name a person knows a format by.
fn format_name(format: SendFormat) -> &'static str {
    match format {
        SendFormat::ThreeMf => "3MF",
        SendFormat::Stl => "STL",
        SendFormat::Obj => "OBJ",
        SendFormat::Step => "STEP",
        SendFormat::Svg => "SVG",
        SendFormat::Dxf => "DXF",
    }
}

/// THE SUBMENU "SEND TO", drawn into a menu: the programs found that take `what`, by group, then the commands of the
/// list itself. Returns what was picked, or what is to be said, this frame.
pub(crate) fn send_submenu(ui: &mut egui::Ui, what: SendWhat) -> Option<SendAnswer> {
    let mut answer = None;
    ui.menu_button(format!("{}  {}", ph::ARROW_SQUARE_OUT, crate::i18n::tr("send-to")), |ui| {
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend); // a menu item is a command, not a paragraph
        PROGRAMS.with_borrow_mut(|slot| {
            let programs = slot.get_or_insert_with(look);
            if let Some(e) = programs.untold.take() {
                answer = Some(SendAnswer::Say(Notice::warning(crate::i18n::tr1("send-own-list-broken", "error", &e))));
            }
            let mut any = false;
            for kind in [SendKind::Slicer, SendKind::Laser, SendKind::Cam] {
                let here: Vec<&SendFound> = programs.found.iter().filter(|f| f.program.kind == kind && f.program.format_for(what).is_some()).collect();
                if here.is_empty() {
                    continue;
                }
                if any {
                    ui.separator();
                }
                any = true;
                ui.label(egui::RichText::new(crate::i18n::tr(group_key(kind))).weak());
                for f in here {
                    let Some(format) = f.program.format_for(what) else { continue };
                    let label = crate::i18n::trn("send-program-as", &[("program", f.program.name.as_str()), ("format", format_name(format))]);
                    if ui.button(label).on_hover_text(f.exe.display().to_string()).clicked() {
                        answer = Some(SendAnswer::Pick(SendPick { name: f.program.name.clone(), exe: f.exe.clone(), format }));
                        ui.close();
                    }
                }
            }
            if !any {
                ui.label(egui::RichText::new(crate::i18n::tr("send-none")).weak());
            }
        });
        ui.separator();
        if ui.button(format!("{}  {}", ph::ARROWS_CLOCKWISE, crate::i18n::tr("send-rescan"))).clicked() {
            PROGRAMS.set(None); // looked for again when the menu is drawn next
        }
        if ui.button(format!("{}  {}", ph::FILE_TEXT, crate::i18n::tr("send-own-list"))).on_hover_text(crate::i18n::tr("send-own-list-hint")).clicked() {
            PROGRAMS.set(None); // what the person writes there is read the next time the menu opens
            if let Some(said) = open_own_list(ui.ctx().os()) {
                answer = Some(SendAnswer::Say(said));
            }
            ui.close();
        }
    });
    answer
}

/// OPEN A PERSON'S OWN LIST in the program the system opens such a file with, writing a list with one example first
/// when there is none. Says something only when it failed.
fn open_own_list(os: egui::os::OperatingSystem) -> Option<Notice> {
    let Some(path) = own_list() else { return Some(Notice::refusal(crate::i18n::tr("send-own-list-nowhere"))) };
    if !path.is_file() {
        let text = format!(
            "// {}\n(\n    programs: [\n        // (id: \"my-program\", name: \"My program\", kind: Slicer, formats: [ThreeMf, Stl], exe: [\"program.exe\"], folders: [\"Program\"], paths: [\"C:\\\\Tools\\\\Program\\\\program.exe\"]),\n    ],\n)\n",
            crate::i18n::tr("send-own-list-note")
        );
        let written = path.parent().map(std::fs::create_dir_all).transpose().and_then(|_| std::fs::write(&path, text));
        if let Err(e) = written {
            return Some(Notice::error(crate::i18n::tr1("send-own-list-failed", "error", &e.to_string())));
        }
    }
    let (bin, args) = crate::gui::open_command(os, path.to_string_lossy().into_owned());
    crate::system::start(bin, &args).err().map(|e| Notice::error(crate::i18n::tr1("send-own-list-failed", "error", &e.to_string())))
}

/// BEGIN SENDING A MODEL named `base` to the program of `pick`: what the export is to start, and the stage it waits in.
/// A flat format starts no export of a model and gives `None`.
pub(crate) fn begin(pick: SendPick, base: &str) -> Option<Begin> {
    let begin = match pick.format {
        SendFormat::Step => Begin::Exact(qymcad_kernel::ExactFormat::Step),
        SendFormat::ThreeMf => Begin::Mesh(MeshFormat::ThreeMf),
        SendFormat::Stl => Begin::Mesh(MeshFormat::Stl),
        SendFormat::Obj => Begin::Mesh(MeshFormat::Obj),
        SendFormat::Svg | SendFormat::Dxf => return None,
    };
    let delivery = Delivery { path: qymcad_io::send_path(&folder(), base, pick.format), name: pick.name, exe: pick.exe };
    let stage = match begin {
        Begin::Exact(_) => Stage::Armed { delivery, until: Instant::now() + ARMED_FOR },
        Begin::Mesh(_) => Stage::Picked(delivery),
    };
    STAGE.set(Some(stage));
    Some(begin)
}

/// THE MESH QUALITY IS CHOSEN: a picked send is armed for the chooser the export puts up next.
pub(crate) fn quality_chosen() {
    STAGE.with_borrow_mut(|s| {
        *s = match s.take() {
            Some(Stage::Picked(delivery)) => Some(Stage::Armed { delivery, until: Instant::now() + ARMED_FOR }),
            other => other,
        }
    });
}

/// NOTHING IS BEING SENT: the quality was cancelled, or an ordinary export was asked for instead.
pub(crate) fn forget() {
    STAGE.set(None);
}

/// THE ANSWER OF A CHOOSER TO WRITE A FILE, when a send is armed for it: the file of the delivery, the one there before
/// removed so that the one found later is the one just written.
pub(crate) fn answer_for_save() -> Option<PathBuf> {
    STAGE.with_borrow_mut(|s| match s.take() {
        Some(Stage::Armed { delivery, until }) if Instant::now() <= until => {
            let _ = std::fs::remove_file(&delivery.path);
            let path = delivery.path.clone();
            *s = Some(Stage::Writing { delivery, since: Instant::now() });
            Some(path)
        }
        Some(Stage::Armed { .. }) => None, // armed too long ago: the chooser is someone else's
        other => {
            *s = other;
            None
        }
    })
}

/// ONCE A FRAME: the file being written is there and no write is running - the program is started with it, and the
/// status says so. A write that never started is given up.
pub(crate) fn watch(status: &mut Notice, regen: &qymcad_ui_state::Rebuilding, ctx: &egui::Context) {
    if regen.busy.is_some() {
        return; // the write is running: its result wakes the frames
    }
    let ready = STAGE.with_borrow_mut(|s| match s.take() {
        Some(Stage::Writing { delivery, .. }) if delivery.path.is_file() => Some(delivery),
        Some(Stage::Writing { since, .. }) if since.elapsed() > WRITE_STARTS_WITHIN => None,
        other => {
            if matches!(other, Some(Stage::Writing { .. })) {
                ctx.request_repaint_after(Duration::from_millis(200)); // the write starts on a later frame: keep looking
            }
            *s = other;
            None
        }
    });
    if let Some(delivery) = ready {
        *status = launch(&delivery);
    }
}

/// START THE PROGRAM of `delivery` with its file, and say what came of it.
fn launch(delivery: &Delivery) -> Notice {
    let path = delivery.path.to_string_lossy().into_owned();
    match crate::system::start(&delivery.exe.to_string_lossy(), std::slice::from_ref(&path)) {
        Ok(()) => Notice::info(crate::i18n::trn("send-sent", &[("program", delivery.name.as_str()), ("path", path.as_str())])),
        Err(e) => Notice::error(crate::i18n::trn("send-failed", &[("program", delivery.name.as_str()), ("error", e.to_string().as_str())])),
    }
}

/// SEND THE DRAWING OF THE SKETCH `si` to the program of `pick`: written at once, as a sketch is exported, and the
/// program started with it.
pub(crate) fn send_sketch(project: &qymcad_core::model::Project, si: usize, pick: &SendPick) -> Notice {
    let edges = crate::gui::sketching::sketch_export_edges(project, si);
    if edges.is_empty() {
        return Notice::refusal(crate::i18n::tr("io-export-empty-sketch"));
    }
    let base = project.sketches.get(si).map(|s| crate::i18n::name(&s.name)).unwrap_or_else(|| crate::i18n::tr("io-sketch-lower"));
    let path = qymcad_io::send_path(&folder(), &base, pick.format);
    let p = path.to_string_lossy().into_owned();
    let written = match pick.format {
        SendFormat::Dxf => qymcad_io::export_dxf(&edges, &p),
        _ => qymcad_io::export_svg(&edges, &p),
    };
    match written {
        Ok(()) => launch(&Delivery { name: pick.name.clone(), exe: pick.exe.clone(), path }),
        Err(e) => Notice::error(crate::i18n::name(&e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pick(format: SendFormat) -> SendPick {
        SendPick { name: "Program".into(), exe: PathBuf::from("program.exe"), format }
    }

    /// AN EXACT FILE IS ARMED AT ONCE: the next chooser to write a file is answered with the file sent, once.
    #[test]
    fn an_exact_send_answers_the_next_chooser_once() {
        forget();
        assert!(matches!(begin(pick(SendFormat::Step), "Part"), Some(Begin::Exact(qymcad_kernel::ExactFormat::Step))));
        let path = answer_for_save().expect("the chooser of the export is answered with the file sent");
        assert_eq!(path.file_name().and_then(|n| n.to_str()), Some("Part.step"));
        assert_eq!(answer_for_save(), None, "a second chooser is someone else's and is put up as usual");
        forget();
    }

    /// A MESH WAITS FOR ITS QUALITY: picked, it answers no chooser; the quality chosen, it answers the next one; the
    /// quality cancelled, it answers none.
    #[test]
    fn a_mesh_send_is_armed_by_its_quality() {
        forget();
        assert!(matches!(begin(pick(SendFormat::ThreeMf), "Part"), Some(Begin::Mesh(MeshFormat::ThreeMf))));
        assert_eq!(answer_for_save(), None, "a chooser before the quality is chosen is not the export's");
        quality_chosen();
        assert!(answer_for_save().is_some_and(|p| p.extension().is_some_and(|x| x == "3mf")), "the chooser after the quality is answered with the 3MF sent");
        assert!(matches!(begin(pick(SendFormat::Stl), "Part"), Some(Begin::Mesh(MeshFormat::Stl))));
        forget();
        quality_chosen();
        assert_eq!(answer_for_save(), None, "a send forgotten arms nothing");
    }

    /// A FLAT FORMAT starts no export of a model.
    #[test]
    fn a_drawing_format_starts_no_model_export() {
        forget();
        assert!(begin(pick(SendFormat::Svg), "Part").is_none());
        quality_chosen();
        assert_eq!(answer_for_save(), None);
    }

    /// THE PROGRAM IS STARTED WITH THE FILE once it is written and no write runs, and the status says so; before the file
    /// is there nothing is started.
    #[test]
    fn the_program_starts_once_the_file_is_there() {
        crate::system::stand_in(true);
        forget();
        let _ = begin(pick(SendFormat::Step), "Started");
        let path = answer_for_save().expect("the chooser is answered");
        let regen = qymcad_ui_state::Rebuilding::default();
        let ctx = egui::Context::default();
        let mut status = Notice::default();
        watch(&mut status, &regen, &ctx);
        let started_early = crate::system::with_stand_in(|s| s.started.len()).unwrap_or_default();
        assert_eq!(started_early, 0, "a program was started before its file was written");
        std::fs::write(&path, b"ISO-10303-21;").expect("the file sent is written");
        watch(&mut status, &regen, &ctx);
        let started = crate::system::with_stand_in(|s| s.started.clone()).unwrap_or_default();
        crate::system::stand_in(false);
        let _ = std::fs::remove_file(&path);
        assert_eq!(started.len(), 1, "the program is started once: {started:?}");
        assert_eq!(started[0].0, "program.exe");
        assert_eq!(started[0].1, [path.to_string_lossy().into_owned()], "the program is started with the file sent");
        assert_eq!(status.tone, qymcad_ui_state::Tone::Info, "the status says the file was sent: {status}");
        assert!(status.words.contains("Program"), "the status names the program: {status}");
    }

    /// THE DRAWING OF A SKETCH is written at once and handed to the program.
    #[test]
    fn a_sketch_is_sent_as_a_drawing() {
        use qymcad_core::model::{EntityKind, Project, SketchEntity, SketchPoint};
        let mut p = Project::default();
        p.new_document();
        let si = p.new_sketch("Plate");
        let next = p.alloc_id() + 1;
        let s = &mut p.sketches[si];
        s.points.push(SketchPoint { id: next, x: 0.0, y: 0.0 });
        s.points.push(SketchPoint { id: next + 1, x: 40.0, y: 0.0 });
        s.entities.push(SketchEntity { id: next + 2, kind: EntityKind::Line { a: next, b: next + 1 }, construction: false });
        crate::system::stand_in(true);
        let status = send_sketch(&p, si, &pick(SendFormat::Dxf));
        let started = crate::system::with_stand_in(|s| s.started.clone()).unwrap_or_default();
        crate::system::stand_in(false);
        assert_eq!(status.tone, qymcad_ui_state::Tone::Info, "the drawing is sent: {status}");
        assert_eq!(started.len(), 1, "the program is started once: {started:?}");
        let file = PathBuf::from(&started[0].1[0]);
        assert_eq!(file.extension().and_then(|x| x.to_str()), Some("dxf"));
        let text = std::fs::read_to_string(&file).expect("the drawing sent is on disk");
        let _ = std::fs::remove_file(&file);
        assert!(!text.trim().is_empty(), "the drawing sent is empty");
    }
}
