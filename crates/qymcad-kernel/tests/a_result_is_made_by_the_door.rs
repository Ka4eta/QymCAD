//! A RESULT OF C++ IS MADE BY THE DOOR. `qym_result` carries the names of every operand through the history, records
//! the pieces of a split face, names what the operation generated and numbers what the history is silent about - once
//! for every operation. A `QymShape` put together in place does all of that by hand, or forgets a part of it.
//!
//! The places that still put one together are counted, and the count is held by equality, as the size of `impl App`
//! is: a new place is a regression and is not paid for by raising the mark; a place moved onto `qym_result` lowers it
//! in the same change.

/// The places that put a `QymShape` together in place, on 13.10.2026: reading a file (a new body has no operand),
/// the door itself, and the operations not moved onto the door yet.
const MARK: usize = 53;

/// What puts a result together.
const MAKES: [&str; 2] = ["new QymShape", "make_unique<QymShape>"];

/// Every C++ file of the crates, as text.
fn sources() -> Vec<(String, String)> {
    let crates = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().expect("the crates directory");
    let mut stack: Vec<std::path::PathBuf> = vec![crates.to_path_buf()];
    let mut out: Vec<(String, String)> = Vec::new();
    while let Some(dir) = stack.pop() {
        for e in std::fs::read_dir(&dir).expect("the tree reads").flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
                continue;
            }
            if p.extension().is_none_or(|x| x != "cpp" && x != "hpp") {
                continue;
            }
            let name = p.strip_prefix(crates).unwrap_or(&p).to_string_lossy().replace('\\', "/");
            out.push((name, std::fs::read_to_string(&p).unwrap_or_default()));
        }
    }
    assert!(out.iter().any(|(n, _)| n.ends_with("occt_bridge.cpp")), "the bridge itself was not found by the walk");
    out
}

#[test]
fn no_new_place_puts_a_result_together() {
    let mut places: Vec<String> = Vec::new();
    for (name, text) in sources() {
        for (n, line) in text.lines().enumerate() {
            let t = line.trim_start();
            if t.starts_with("//") || t.starts_with('*') {
                continue;
            }
            places.extend(MAKES.iter().filter(|m| line.contains(*m)).map(|_| format!("{name}:{}", n + 1)));
        }
    }
    assert!(places.len() <= MARK, "{} places put a result of C++ together, the mark is {MARK}: build the result through `qym_result`\n{}", places.len(), places.join("\n"));
    assert!(places.len() >= MARK, "{} places put a result together, fewer than the mark {MARK}: lower the mark to the count", places.len());
}
