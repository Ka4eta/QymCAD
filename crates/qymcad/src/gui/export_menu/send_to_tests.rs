//! THE CHECKS OF SENDING A MODEL OR A DRAWING TO ANOTHER PROGRAM: the stages, the chooser answered once, the program
//! started with its file, a sketch sent as a drawing. A file of their own, so the source of the panels holds no module
//! of checks in front of the panels after it.
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
    use qymcad_core::geom::Point2;
    use qymcad_core::model::Project;
    let mut p = Project::default();
    p.new_document();
    let corners = vec![Point2 { x: 0.0, y: 0.0 }, Point2 { x: 40.0, y: 0.0 }, Point2 { x: 40.0, y: 30.0 }, Point2 { x: 0.0, y: 30.0 }];
    let sid = p.add_line_sketch("Plate", corners, true);
    let si = p.sketch_index(sid).expect("the sketch just made is in the project");
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
