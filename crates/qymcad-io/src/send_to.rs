//! WHERE A FILE IS SENT: the programs that take a model or a drawing from here - the slicers of a printer, the
//! programs of a laser or an engraver, the programs that make the toolpaths of a mill - found on this computer, and the
//! format each one is handed.
//!
//! THE LIST IS A RESOURCE. The built-in one is compiled in (`send_programs.ron`); a person's own file of the same shape
//! adds programs to it, or replaces one of it by its `id` - a program installed somewhere unusual is reached by writing
//! its path into `paths`, with no rebuild of the program. A program is looked for in the folders programs are installed
//! into, by the start of the name of its folder: a folder carries a version in its name more often than not.

use std::path::{Path, PathBuf};

/// WHAT KIND OF PROGRAM it is: it decides the group of the menu it is listed in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub enum SendKind {
    /// The slicer of a printer: it takes a model.
    #[default]
    Slicer,
    /// The program of a laser or an engraver: it takes a flat drawing, now and then a model.
    Laser,
    /// The program that makes the toolpaths of a mill: a model or a drawing.
    Cam,
}

/// A FORMAT a program is handed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SendFormat {
    ThreeMf,
    Stl,
    Obj,
    Step,
    Svg,
    Dxf,
}

/// WHAT IS SENT: a solid model, or the flat drawing of a sketch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SendWhat {
    Model,
    Drawing,
}

impl SendFormat {
    /// The extension of the file written in this format.
    pub fn ext(self) -> &'static str {
        match self {
            SendFormat::ThreeMf => "3mf",
            SendFormat::Stl => "stl",
            SendFormat::Obj => "obj",
            SendFormat::Step => "step",
            SendFormat::Svg => "svg",
            SendFormat::Dxf => "dxf",
        }
    }

    /// Whether a file of this format carries a model or a flat drawing.
    pub fn carries(self) -> SendWhat {
        match self {
            SendFormat::Svg | SendFormat::Dxf => SendWhat::Drawing,
            _ => SendWhat::Model,
        }
    }
}

/// ONE PROGRAM OF THE LIST: how it is named, what it takes, and where it is looked for.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct SendProgram {
    /// The stable name of the program in the list: a person's own entry with the same `id` replaces the built-in one.
    pub id: String,
    /// The name shown in the menu.
    pub name: String,
    pub kind: SendKind,
    /// The formats the program opens, the one it is best handed first.
    pub formats: Vec<SendFormat>,
    /// The names of its executable, any of them.
    pub exe: Vec<String>,
    /// How the folder it is installed into begins, any of them, compared without case.
    pub folders: Vec<String>,
    /// Full paths of its executable, taken as they stand; `%NAME%` is replaced by the variable of the environment.
    pub paths: Vec<String>,
}

impl SendProgram {
    /// The format this program is best handed `what` in, or `None` when it does not take it.
    pub fn format_for(&self, what: SendWhat) -> Option<SendFormat> {
        self.formats.iter().copied().find(|f| f.carries() == what)
    }
}

/// THE LIST OF PROGRAMS.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct SendCatalogue {
    pub programs: Vec<SendProgram>,
}

/// The built-in list, as it is compiled in.
const BUILT_IN: &str = include_str!("send_programs.ron");

impl SendCatalogue {
    /// The list the program comes with.
    pub fn built_in() -> Self {
        Self::read(BUILT_IN).unwrap_or_default()
    }

    /// A list read from the text of a file of this shape.
    pub fn read(text: &str) -> Result<Self, String> {
        ron::from_str(text).map_err(|e| e.to_string())
    }

    /// This list with `own` laid over it: an entry of `own` replaces the entry of the same `id`, a new one is added.
    pub fn merged(mut self, own: SendCatalogue) -> Self {
        for p in own.programs {
            match self.programs.iter_mut().find(|q| q.id == p.id) {
                Some(slot) => *slot = p,
                None => self.programs.push(p),
            }
        }
        self
    }
}

/// A PROGRAM FOUND ON THIS COMPUTER, and its executable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SendFound {
    pub program: SendProgram,
    pub exe: PathBuf,
}

/// THE FOLDERS PROGRAMS ARE INSTALLED INTO, from the variables of the environment `var` reads: the two folders of
/// programs, the folder of the programs of one person, and the root of the system drive, where some programs put
/// themselves. A variable that is not set gives nothing; each folder is named once.
pub fn search_roots(var: impl Fn(&str) -> Option<String>) -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = Vec::new();
    let mut add = |p: PathBuf| {
        if !roots.contains(&p) {
            roots.push(p);
        }
    };
    for name in ["ProgramFiles", "ProgramW6432", "ProgramFiles(x86)"] {
        if let Some(v) = var(name) {
            add(PathBuf::from(v));
        }
    }
    if let Some(v) = var("LOCALAPPDATA") {
        add(Path::new(&v).join("Programs"));
        add(PathBuf::from(v));
    }
    if let Some(v) = var("SystemDrive") {
        add(PathBuf::from(format!("{v}\\")));
    }
    roots
}

/// `path` with every `%NAME%` replaced by the variable `var` gives; a name it does not know is left as written.
pub fn expand(path: &str, var: impl Fn(&str) -> Option<String>) -> String {
    let mut out = String::new();
    let mut rest = path;
    while let Some(start) = rest.find('%') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        match after.find('%') {
            Some(end) => {
                let name = &after[..end];
                match var(name) {
                    Some(v) if !name.is_empty() => out.push_str(&v),
                    _ => {
                        out.push('%');
                        out.push_str(name);
                        out.push('%');
                    }
                }
                rest = &after[end + 1..];
            }
            None => {
                out.push_str(&rest[start..]);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}

/// The entries of `dir` whose names `keep` takes, sorted by name - a folder whose name carries a newer version sorts
/// later, and the last one is looked at first.
fn entries(dir: &Path, keep: impl Fn(&str) -> bool) -> Vec<PathBuf> {
    let Ok(read) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut out: Vec<PathBuf> = read.flatten().map(|e| e.path()).filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(&keep)).collect();
    out.sort();
    out
}

/// The executable of one of the names `exe` in `dir` itself, in its `bin`, or one folder deeper; the name is compared
/// without case, as the file system of the system it is installed on compares it.
fn exe_in(dir: &Path, exe: &[String]) -> Option<PathBuf> {
    let wanted: Vec<String> = exe.iter().map(|e| e.to_lowercase()).collect();
    let is_exe = |p: &PathBuf| p.is_file() && p.file_name().and_then(|n| n.to_str()).is_some_and(|n| wanted.contains(&n.to_lowercase()));
    let mut places = vec![dir.to_path_buf(), dir.join("bin")];
    places.extend(entries(dir, |_| true).into_iter().filter(|p| p.is_dir()).rev().flat_map(|sub| [sub.join("bin"), sub]));
    places.iter().find_map(|place| entries(place, |_| true).into_iter().find(is_exe))
}

/// WHERE `program` IS on this computer: a path written out in the list first, then the folders of `roots` whose names
/// begin as the list says.
pub fn locate(program: &SendProgram, roots: &[PathBuf], var: impl Fn(&str) -> Option<String>) -> Option<PathBuf> {
    if let Some(p) = program.paths.iter().map(|p| PathBuf::from(expand(p, &var))).find(|p| p.is_file()) {
        return Some(p);
    }
    let starts: Vec<String> = program.folders.iter().map(|f| f.to_lowercase()).filter(|f| !f.is_empty()).collect();
    roots.iter().find_map(|root| {
        let dirs = entries(root, |n| {
            let n = n.to_lowercase();
            starts.iter().any(|s| n.starts_with(s))
        });
        dirs.into_iter().filter(|d| d.is_dir()).rev().find_map(|d| exe_in(&d, &program.exe))
    })
}

/// EVERY PROGRAM OF `catalogue` FOUND under `roots`, grouped by kind and sorted by name within a group.
pub fn find(catalogue: &SendCatalogue, roots: &[PathBuf], var: impl Fn(&str) -> Option<String>) -> Vec<SendFound> {
    let mut found: Vec<SendFound> = catalogue.programs.iter().filter_map(|p| locate(p, roots, &var).map(|exe| SendFound { program: p.clone(), exe })).collect();
    found.sort_by(|a, b| a.program.kind.cmp(&b.program.kind).then_with(|| a.program.name.to_lowercase().cmp(&b.program.name.to_lowercase())));
    found
}

/// The file `base` is sent as in `folder`, in `format`: the characters a file name may not hold on any system are
/// replaced, and an empty name becomes `model`.
pub fn send_path(folder: &Path, base: &str, format: SendFormat) -> PathBuf {
    let clean: String = base.chars().map(|c| if matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') || c.is_control() { '_' } else { c }).collect();
    let clean = clean.trim().trim_end_matches('.');
    let name = if clean.is_empty() { "model" } else { clean };
    folder.join(format!("{name}.{}", format.ext()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// THE BUILT-IN LIST READS, every entry of it has a name, a format and a way to be found, and no `id` is taken twice.
    #[test]
    fn the_built_in_list_reads_whole() {
        let cat = SendCatalogue::read(BUILT_IN).expect("the built-in list of programs reads");
        assert!(cat.programs.len() >= 20, "the built-in list holds {} programs", cat.programs.len());
        for kind in [SendKind::Slicer, SendKind::Laser, SendKind::Cam] {
            assert!(cat.programs.iter().any(|p| p.kind == kind), "the built-in list has no program of the kind {kind:?}");
        }
        let mut ids: Vec<&str> = cat.programs.iter().map(|p| p.id.as_str()).collect();
        ids.sort();
        let before = ids.len();
        ids.dedup();
        assert_eq!(before, ids.len(), "an id is taken twice in the built-in list");
        for p in &cat.programs {
            assert!(!p.id.is_empty() && !p.name.is_empty(), "an entry of the built-in list has no id or no name: {p:?}");
            assert!(!p.formats.is_empty(), "{} takes no format", p.id);
            assert!(!p.exe.is_empty() && !p.folders.is_empty(), "{} cannot be looked for: no executable or no folder", p.id);
        }
    }

    /// A FORMAT IS PICKED BY WHAT IS SENT: the first the program takes of that kind, none when it takes none.
    #[test]
    fn the_format_follows_what_is_sent() {
        let p = SendProgram { formats: vec![SendFormat::Dxf, SendFormat::ThreeMf, SendFormat::Svg, SendFormat::Stl], ..Default::default() };
        assert_eq!(p.format_for(SendWhat::Model), Some(SendFormat::ThreeMf));
        assert_eq!(p.format_for(SendWhat::Drawing), Some(SendFormat::Dxf));
        let flat = SendProgram { formats: vec![SendFormat::Svg], ..Default::default() };
        assert_eq!(flat.format_for(SendWhat::Model), None, "a program of drawings alone is not offered a model");
    }

    /// A PERSON'S ENTRY REPLACES THE BUILT-IN ONE OF ITS `id`, and a new one is added after them.
    #[test]
    fn an_own_entry_replaces_its_namesake() {
        let base = SendCatalogue { programs: vec![SendProgram { id: "a".into(), name: "A".into(), ..Default::default() }, SendProgram { id: "b".into(), name: "B".into(), ..Default::default() }] };
        let own = SendCatalogue { programs: vec![SendProgram { id: "b".into(), name: "Mine".into(), ..Default::default() }, SendProgram { id: "c".into(), name: "C".into(), ..Default::default() }] };
        let names: Vec<String> = base.merged(own).programs.into_iter().map(|p| p.name).collect();
        assert_eq!(names, ["A", "Mine", "C"]);
    }

    /// `%NAME%` IS REPLACED by the variable, an unknown one and a lone `%` are left as written.
    #[test]
    fn a_variable_in_a_path_is_replaced() {
        let var = |n: &str| (n == "ProgramFiles").then(|| "C:\\Program Files".to_string());
        assert_eq!(expand("%ProgramFiles%\\Tool\\tool.exe", var), "C:\\Program Files\\Tool\\tool.exe");
        assert_eq!(expand("%Nope%\\x.exe", var), "%Nope%\\x.exe");
        assert_eq!(expand("50% off", var), "50% off");
    }

    /// THE ROOTS COME FROM THE VARIABLES THAT ARE SET, each once.
    #[test]
    fn the_roots_are_the_folders_of_programs() {
        let var = |n: &str| match n {
            "ProgramFiles" | "ProgramW6432" => Some("C:\\Program Files".to_string()),
            "LOCALAPPDATA" => Some("D:\\Data\\Local".to_string()),
            _ => None,
        };
        let roots = search_roots(var);
        assert_eq!(roots.iter().filter(|r| r.as_os_str() == "C:\\Program Files").count(), 1, "a folder named by two variables is searched once: {roots:?}");
        assert!(roots.contains(&Path::new("D:\\Data\\Local").join("Programs")), "the programs of one person are searched: {roots:?}");
        assert_eq!(roots.len(), 3, "{roots:?}");
    }

    /// A NAME A FILE MAY NOT HOLD is made one that it may.
    #[test]
    fn the_name_sent_is_a_name_a_file_may_hold() {
        let p = send_path(Path::new("out"), "Gehäuse: v2/fix?", SendFormat::Stl);
        assert_eq!(p, Path::new("out").join("Gehäuse_ v2_fix_.stl"));
        assert_eq!(send_path(Path::new("out"), "  ", SendFormat::ThreeMf), Path::new("out").join("model.3mf"));
    }
}
