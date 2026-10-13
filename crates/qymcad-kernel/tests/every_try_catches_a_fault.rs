//! EVERY `try` OF THE BRIDGE CATCHES A FAULT OF OCCT. A fault inside an algorithm of OCCT - memory read that is not its
//! own, a division by zero - is a signal, and a `catch` sees it only when the `try` around the call opened with
//! `QYM_SIGNALS`: the handlers of `OSD` set and a point for the signal to jump to. A `try` without it lets the fault end
//! the program, as a rounding of R9.9999998 on the arc of an R10 rounding did.

/// Every C++ file of the bridge, as text.
fn sources() -> Vec<(String, String)> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut out: Vec<(String, String)> = Vec::new();
    for e in std::fs::read_dir(&dir).expect("the sources read").flatten() {
        let p = e.path();
        if p.extension().is_some_and(|x| x == "cpp") {
            out.push((p.file_name().unwrap_or_default().to_string_lossy().into_owned(), std::fs::read_to_string(&p).unwrap_or_default()));
        }
    }
    assert!(out.iter().any(|(n, _)| n == "occt_bridge.cpp"), "the bridge itself was not found");
    out
}

#[test]
fn every_try_opens_with_the_signals() {
    let mut bare: Vec<String> = Vec::new();
    let mut seen = 0;
    for (name, text) in sources() {
        let lines: Vec<&str> = text.lines().collect();
        for (n, line) in lines.iter().enumerate() {
            let t = line.trim_start();
            if t.starts_with("//") {
                continue;
            }
            for (at, _) in line.match_indices("try {") {
                seen += 1;
                let rest = line[at + "try {".len()..].trim();
                let next = if rest.is_empty() { lines.get(n + 1).map(|l| l.trim()).unwrap_or_default() } else { rest };
                if !next.starts_with("QYM_SIGNALS;") {
                    bare.push(format!("{name}:{}", n + 1));
                }
            }
        }
    }
    assert!(seen > 100, "only {seen} try blocks found: the walk does not see the bridge");
    assert!(bare.is_empty(), "{} try blocks of the bridge do not open with QYM_SIGNALS, and a fault of OCCT inside them ends the program:\n{}", bare.len(), bare.join("\n"));
}
