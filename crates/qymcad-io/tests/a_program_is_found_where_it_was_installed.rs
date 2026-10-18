//! A PROGRAM A FILE IS SENT TO IS FOUND WHERE IT WAS INSTALLED: in a folder whose name begins as the list says and
//! carries a version, in that folder's `bin`, one folder deeper, or at a path the list writes out - and a program that
//! is not there is not offered.
use std::path::{Path, PathBuf};

use qymcad_io::{find, locate, SendCatalogue, SendFormat, SendKind, SendProgram};

/// A folder of its own for one check, empty at the start.
fn fresh(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("qymcad-send-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a folder for the check is made");
    dir
}

/// An empty file at `path`, its folders made.
fn touch(path: &Path) {
    std::fs::create_dir_all(path.parent().expect("a file has a folder")).expect("the folders of the file are made");
    std::fs::write(path, b"").expect("the file is written");
}

/// A program named `id`, its executable `<id>.exe`, installed into a folder whose name begins with `id`.
fn program(id: &str) -> SendProgram {
    SendProgram { id: id.into(), name: id.into(), kind: SendKind::Slicer, formats: vec![SendFormat::Stl], exe: vec![format!("{id}.exe")], folders: vec![id.into()], paths: Vec::new() }
}

fn no_vars(_: &str) -> Option<String> {
    None
}

#[test]
fn a_program_is_found_in_a_folder_named_with_its_version() {
    let root = fresh("version");
    touch(&root.join("Slicer 5.9.0").join("Slicer.exe"));
    touch(&root.join("Slicer 5.10.0").join("Slicer.exe"));
    let found = locate(&program("slicer"), std::slice::from_ref(&root), no_vars).expect("the program in a folder named with its version is found");
    assert!(found.starts_with(&root), "found outside the folder searched: {}", found.display());
}

#[test]
fn a_program_is_found_in_bin_and_one_folder_deeper() {
    let root = fresh("deeper");
    touch(&root.join("Maker 1.0").join("bin").join("maker.exe"));
    touch(&root.join("Vendor").join("Tool 3").join("tool.exe"));
    assert_eq!(locate(&program("maker"), std::slice::from_ref(&root), no_vars), Some(root.join("Maker 1.0").join("bin").join("maker.exe")));
    assert_eq!(locate(&SendProgram { folders: vec!["Vendor".into()], ..program("tool") }, std::slice::from_ref(&root), no_vars), Some(root.join("Vendor").join("Tool 3").join("tool.exe")));
}

#[test]
fn the_names_are_compared_without_case() {
    let root = fresh("case");
    touch(&root.join("LASERPROG").join("LaserProg.EXE"));
    assert!(locate(&program("laserprog"), std::slice::from_ref(&root), no_vars).is_some(), "a name written in another case is not found");
}

#[test]
fn a_path_written_out_is_taken_as_it_stands() {
    let root = fresh("path");
    let exe = root.join("Somewhere").join("odd.exe");
    touch(&exe);
    let dir = root.to_string_lossy().into_owned();
    let var = move |n: &str| (n == "TOOLS").then(|| dir.clone());
    let p = SendProgram { paths: vec![format!("%TOOLS%{}Somewhere{}odd.exe", std::path::MAIN_SEPARATOR, std::path::MAIN_SEPARATOR)], folders: vec!["nothing-like-it".into()], ..program("odd") };
    assert_eq!(locate(&p, &[], var), Some(exe));
}

#[test]
fn a_program_that_is_not_there_is_not_offered() {
    let root = fresh("absent");
    touch(&root.join("Present").join("present.exe"));
    touch(&root.join("Absent").join("other.exe"));
    let cat = SendCatalogue { programs: vec![program("present"), program("absent")] };
    let found = find(&cat, std::slice::from_ref(&root), no_vars);
    let ids: Vec<&str> = found.iter().map(|f| f.program.id.as_str()).collect();
    assert_eq!(ids, ["present"], "only the program whose executable lies there is offered");
}

#[test]
fn the_built_in_list_finds_a_slicer_installed_as_it_installs_itself() {
    let root = fresh("built-in");
    touch(&root.join("Prusa3D").join("PrusaSlicer").join("prusa-slicer.exe"));
    let found = find(&SendCatalogue::built_in(), std::slice::from_ref(&root), no_vars);
    assert!(found.iter().any(|f| f.program.id == "prusaslicer"), "the built-in list does not find the slicer in its own folder: {:?}", found.iter().map(|f| &f.program.id).collect::<Vec<_>>());
}
