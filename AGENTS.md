# Agents

The rules for any coding agent working in this repository are in [CLAUDE.md](CLAUDE.md): how comments,
interface strings and tests are written, how the code is laid out, how tools behave and how work is committed.
Read it before changing code. It applies to every agent, whatever its name says.

## Cursor Cloud specific instructions

OCCT 7.9.3 is installed at `/opt/occt` and linked into `/usr/include/opencascade` and `/usr/lib`. `cargo` finds the headers and the libraries with no `OCCT_*` variables. `c++` is `g++`.

A headless geometry check (a solid, a Boolean, a STEP round trip):

```
cargo test -p qymcad-kernel --lib inside::step::
```

The window:

```
cargo run --bin qymcad
```

With no GPU the viewport draws through llvmpipe. `libxkbcommon-x11-0` has to be installed or the window panics before it opens. The developer gate is `python3 tools/gate.py fast` and it is a long run. `CARGO_PROFILE_DEV_DEBUG=0` and `CARGO_PROFILE_TEST_DEBUG=0` keep a large test build smaller.
