//! THE RECORDS OF THE STATE OF THE WINDOW: the tools, the commands and their parameters, the caches, the edits.

use super::*;

#[derive(Clone, Copy)]
pub struct View2d {
    pub center: Vec2,
    pub scale: f32,
    pub initialized: bool,
    /// The scale the sheet was fitted at: the wheel zooms on from it (`zoom_limits`), as the 3D view does.
    pub fit: f32,
}

impl Default for View2d {
    fn default() -> Self {
        Self { center: Vec2::ZERO, scale: 4.0, initialized: false, fit: 4.0 }
    }
}

/// A workbench. It switches the left tool bar and the set of panels.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Workbench {
    Sketch,
    Part,
    Assembly,
}

impl Workbench {
    /// The workbench's code for linking to the help - not its label: the label is translated, and a link must not
    /// depend on the language.
    pub fn code(self) -> &'static str {
        match self {
            Workbench::Sketch => "sketch",
            Workbench::Part => "part",
            Workbench::Assembly => "assembly",
        }
    }
}

/// What to export in 3D (STEP or STL): the selected component (with every body nested in its subtree) or the whole
/// project. In both cases ONLY the visible bodies are exported.
#[derive(Clone, Copy)]
pub enum ExportTarget {
    Component(Id),
    Project,
}

/// DERIVED STATE, NOT TRUTH: everything here can be thrown away, and the next frame rebuilds it.
///
/// Fourteen caches used to sit among the fields of `App` looking exactly like the document itself, and the
/// difference matters: losing one of these costs a frame, losing a field of the document costs the part.
///
/// Each is keyed by the revision it was computed at, so it answers "still good?" by comparing numbers rather
/// than by somebody remembering to clear it. THE EMPTY KEY IS `u64::MAX`, NOT ZERO: zero is a real revision,
/// and a cache that starts at it claims to hold the first build of the document before anything is built.
pub struct Caches {
    /// THE TEXTURE CACHE OF THE ViewCube'S LABELS (keyed by text and size). Rasterising the font every frame means
    /// thousands of glyphs a second for nothing; a texture is prepared once and lives until the language or the
    /// cube's size changes, and both of those change the key.
    pub label_tex: std::cell::RefCell<std::collections::HashMap<String, egui::TextureHandle>>,
    /// THE SCENE BUFFER'S BLOCKS BY BODY - one per body, see `SceneBlock`.
    ///
    /// Dragging a part moves ONE body, while the buffer was assembled whole: 463,878 vertices per frame, 30 to 48
    /// ms on a real assembly. A block depends on the body's FORM, its highlight and the shared display settings;
    /// the position lives separately and a move does not invalidate the block.
    pub scene_blocks: std::cell::RefCell<std::collections::HashMap<usize, SceneBlock>>,
    /// WHAT BECAME OF THE BLOCKS ON THE LAST SCENE PASS: [rebuilt, shifted, taken ready-made].
    ///
    /// Not decoration but the only honest answer to "why is this frame expensive": without it an optimisation is
    /// verified with a stopwatch, and a stopwatch is off by whole multiples on a debug build. The
    /// `a_moved_part_does_not_rebuild_its_block` check asks for exactly these numbers.
    pub scene_stats: std::cell::Cell<[u32; 3]>,
    /// The 3D render's cache: (the view key, the texture). It is redrawn only when the view changes.
    pub view: std::cell::RefCell<Option<(u64, egui::TextureHandle)>>,
    /// The GPU viewport is on (a switch; the CPU raster is the fallback).
    /// A perspective projection instead of an orthographic one. It is applied by one formula in `Screen::at`, so the
    /// overlays and both render paths agree. Orthographic by default, as is customary in CAD.
    /// Smooth (Gouraud) shading instead of flat. When true, the colour is computed at EVERY vertex from the
    /// smoothed normal (`Mesh::vertex_normals`) and interpolated across the triangle; sharp edges stay sharp (the
    /// mesh topology is split by face). False gives flat shading (the face normal).
    /// The cache of smoothed vertex normals, parallel to `project.meshes`, plus the `geom_rev` it was built at. It
    /// is recomputed only when the geometry changes, not every frame. The normals are local (before the world transform).
    pub norm: std::cell::RefCell<Cached<Vec<Normals>>>,
    /// The key of the scene uploaded into the GPU vertex buffer (re-uploaded only on a change). It does NOT depend on the camera.
    pub gpu_scene_key: std::cell::Cell<u64>,
    /// The pass of egui (`cumulative_pass_nr`) the scene was handed to the card in. A pass that is discarded takes
    /// its paint callback with it, so a later pass of the same frame hands the scene over again.
    pub gpu_scene_pass: std::cell::Cell<u64>,
    /// The cache of the visible scene's world bounding sphere (a centre and a radius) keyed by the scene - for
    /// tight near and far planes in perspective (the z buffer's precision). Recomputed only when the scene changes,
    /// not every frame.
    pub bounds: std::cell::Cell<Cached<Option<Sphere>>>,
    /// THE CACHE OF CONSUMED BODIES: `(the revision, the bodies)`. The set is computed by a pass over the whole
    /// timeline while being asked for on every body on every frame - recomputing it is out of the question.
    pub consumed: std::cell::RefCell<Cached<std::collections::HashSet<Id>>>,
    /// THE LIST OF VISIBLE BODIES: `(the revision, the context, [(the mesh index, the body id)])`.
    ///
    /// For each body `body_shown` finds its owner by a LINEAR pass over the whole timeline and walks the visibility
    /// tree. In the pick loop that happens for EVERY body on EVERY frame: at 120 bodies the measurement gave 67 ms
    /// per frame, which is an application that does not respond. The list is computed once per rebuild and per
    /// change of context.
    pub shown_bodies: std::cell::RefCell<Cached<ShownBodies>>,
    /// THE CACHE OF THE BODIES' WORLD EXTENTS: `(the revision, a body mapped to its 8 world corners)`.
    ///
    /// Rejecting by "the cursor is outside the extent" did not help by itself, because what was expensive was
    /// REACHING the body: finding the mesh by id and computing its display transform was done for every body on
    /// every frame. The measurement showed 0.56 ms per body - on an assembly that is seconds. The corners are
    /// computed once per rebuild.
    pub bbox_world: std::cell::RefCell<Cached<std::collections::HashMap<Id, [[f64; 3]; 8]>>>,
    /// THE CACHE OF THE BODIES' EDGES FOR PICKING: `(the geometry revision, a body mapped to (polylines, ids))`.
    ///
    /// Without it, picking an edge or a vertex anchor pulled the edges of EVERY body out of the kernel on EVERY
    /// mouse movement. On a couple of cubes that goes unnoticed; on an assembly of a thousand components the
    /// application stops responding. The key is the geometry revision: once the model is rebuilt, the cache throws
    /// itself away.
    pub pick_edges: std::cell::RefCell<Cached<std::collections::HashMap<Id, std::rc::Rc<EdgePolys>>>>,
    /// The cache of the section CAPS: (a key of plane plus geom_rev plus visibility, the caps in WORLD coordinates).
    pub section_caps: std::cell::RefCell<Cached<std::rc::Rc<Vec<qymcad_core::geom::Mesh>>>>,
    /// The meshes' world extents by index plus the `geom_rev` they were computed at. A cheap way of rejecting
    /// bodies for the section caps (a Common boolean is expensive, and the plane cuts a handful of bodies out of a
    /// thousand).
    pub mesh_bounds: std::cell::RefCell<Cached<std::collections::HashMap<usize, WorldBox>>>,
    /// The sketch diagnostics cache. The rank analysis builds a FULL Jacobian and runs Gaussian elimination
    /// (O(m * nv^2)) - without a cache that was computed EVERY FRAME several times over (the panel, the list, the
    /// overlay, the glyphs, the tree), and on large sketches the interface hung. It is recomputed only on an edit, and
    /// while points are dragged only on an edit of the sketch's shape (`sk_dragged`).
    /// A `RefCell`, because drawing goes through `&self` and needs that very cache.
    pub sk_status: std::cell::RefCell<Option<SketchStatus>>,
    /// THE SKETCH WHOSE POINTS ARE BEING DRAGGED, set by the sketch's drag handling after every frame of it. While it
    /// is set, the diagnostics of that sketch are those of its shape (`SketchStatus::shape`) and are not recomputed for
    /// the points moving: 3 000 lines took 3.7 s for their degrees of freedom on every frame. The release clears it,
    /// and the diagnostics are recomputed once where the points came to rest - a degenerate place shows there.
    pub sk_dragged: std::cell::Cell<Option<usize>>,
    /// THE BORDERS OF A BODY MADE OF A MESH THAT DID NOT CLOSE, by body index, each a loop of corners in the body's own
    /// coordinates; keyed by the meshes they were found on.
    pub open_borders: std::cell::RefCell<Cached<std::collections::HashMap<usize, Vec<OpenBorder>>>>,
}

impl Default for Caches {
    fn default() -> Self {
        Self {
            label_tex: std::cell::RefCell::new(std::collections::HashMap::new()),
            scene_blocks: Default::default(),
            scene_stats: Default::default(),
            view: std::cell::RefCell::new(None),
            norm: std::cell::RefCell::new(Cached { rev: u64::MAX, value: Vec::new() }),
            gpu_scene_key: std::cell::Cell::new(u64::MAX),
            gpu_scene_pass: std::cell::Cell::new(0),
            bounds: std::cell::Cell::new(Cached { rev: u64::MAX, value: None }),
            consumed: std::cell::RefCell::new(Cached { rev: u64::MAX, value: std::collections::HashSet::new() }),
            shown_bodies: std::cell::RefCell::new(Cached { rev: u64::MAX, value: ShownBodies::default() }),
            bbox_world: std::cell::RefCell::new(Cached { rev: u64::MAX, value: std::collections::HashMap::new() }),
            pick_edges: std::cell::RefCell::new(Cached { rev: u64::MAX, value: std::collections::HashMap::new() }),
            sk_status: std::cell::RefCell::new(None),
            sk_dragged: std::cell::Cell::new(None),
            section_caps: std::cell::RefCell::new(Cached { rev: 0, value: std::rc::Rc::new(Vec::new()) }),
            mesh_bounds: std::cell::RefCell::new(Cached { rev: 0, value: std::collections::HashMap::new() }),
            open_borders: std::cell::RefCell::new(Cached { rev: u64::MAX, value: std::collections::HashMap::new() }),
        }
    }
}

/// WHAT IS OPEN ON SCREEN: the windows, the dialogues and the little state each of them keeps while open.
///
/// Two dozen `show_*` flags used to lie among the fields of the document, and a reader could not tell at a
/// glance which of them was part of the part and which was merely a window somebody had opened. They are all
/// the same kind of thing - "this is on screen right now" - and none of them belongs in a saved file.
/// ONE WINDOW THAT CAN BE OPEN. The set of these, not ten separate flags, is what "what is on screen" means.
///
/// Ten booleans could not be walked over, could not be closed at once, and could not be checked against the menu:
/// every one of those actions had to be written ten times, and a window added later was silently left out of all
/// three. As a set, "close everything" is one line and "is anything open" is one line.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum WinKind {
    /// "Save as a template": asks for a name.
    SaveTemplate,
    /// The start screen. It comes up at launch and from a menu item; any action dismisses it.
    Start,
    /// The document properties.
    DocProps,
    /// About (Help -> About).
    About,
    /// "Report a problem" (Help -> Report a problem).
    Report,
    /// The command search.
    CmdSearch,
    /// The hotkeys reference (under Help), built from ONE source - see hotkeys.rs.
    Hotkeys,
    /// The project parameters (the named dimensions and formulas).
    Params,
    /// The settings.
    Settings,
    /// The parts library catalogue.
    PartsLibrary,
    /// What the check for a newer version came to (Help -> Check for updates).
    Updates,
    /// The icon theme manager window.
    IconManager,
    /// The developer icon theme packager modal.
    IconPackager,
}

impl WinKind {
    /// EVERY KIND, so that a walk over the windows cannot silently miss one added later. A guard checks that the
    /// count here matches the number of variants declared above.
    pub const ALL: [WinKind; 13] = [
        WinKind::SaveTemplate,
        WinKind::Start,
        WinKind::DocProps,
        WinKind::About,
        WinKind::Report,
        WinKind::CmdSearch,
        WinKind::Hotkeys,
        WinKind::Params,
        WinKind::Settings,
        WinKind::PartsLibrary,
        WinKind::Updates,
        WinKind::IconManager,
        WinKind::IconPackager,
    ];
}

#[derive(Default)]
pub struct Windows {
    /// WHICH WINDOWS ARE OPEN. Private on purpose: the only ways in are `is`, `set`, `open`, `close`, `toggle` and
    /// `close_all`, so no caller can invent an eleventh way of asking the same question.
    pub(crate) open: std::collections::BTreeSet<WinKind>,
    /// The help window, gathered into a record of its own: the four fields only ever move together.
    pub help: HelpWin,
    /// The name typed into the "save as a template" dialogue.
    pub tpl_name: String,
    /// THE START SCREEN WAS OPENED ON REQUEST - from the Windows menu rather than by itself.
    ///
    /// NOT A WINDOW, hence not in the set: it says HOW `WinKind::Start` came up. The rule "do not show it over
    /// someone's work" is written for what comes up BY ITSELF: an uninvited modal over somebody's part is what
    /// gets closed without a look. It has no right to cancel an explicit request: pressing the menu item gave
    /// NOTHING, not even a message.
    pub start_asked: bool,
    /// THE COMMAND SEARCH's own state: the query and the selected row.
    pub cmd_search_query: String,
    pub cmd_search_sel: usize,
    /// NOT A WINDOW: a one-shot request to put the caret in the search field on the next frame, taken by `mem::take`.
    pub cmd_search_focus: bool,
    /// NOT A WINDOW, a VIEW TOGGLE. "In context" (top-down): show the bodies of NEIGHBOURING parts as ghosts while
    /// working inside a part, so that their geometry can be referred to (a sketch on a neighbour's face becomes an
    /// automatic ExternalRef). Nothing opens or closes; the viewport draws differently.
    pub context: bool,
    /// NOT A WINDOW, a VIEW TOGGLE: whether to show the constraint glyphs in the viewport. The dimensions are
    /// always shown.
    pub constraints: bool,
    /// A file just read, waiting for its units and scale (see `ImportScale`).
    pub import_scale: Option<ImportScale>,
    /// Persistent state of the icon theme manager window.
    pub icon_manager: crate::icons::IconManagerState,
    /// Persistent state of the developer icon theme packager modal.
    pub icon_packager: crate::icons::PackagerDialogState,
}

impl Windows {
    /// Is this window open?
    pub fn is(&self, k: WinKind) -> bool {
        self.open.contains(&k)
    }
    /// Open it or close it, whichever `on` says. The form the `egui` windows need: they hand back a bool.
    pub fn set(&mut self, k: WinKind, on: bool) {
        if on {
            self.open.insert(k);
        } else {
            self.open.remove(&k);
        }
    }
    /// Open it.
    pub fn open(&mut self, k: WinKind) {
        self.open.insert(k);
    }
    /// Close it.
    pub fn close(&mut self, k: WinKind) {
        self.open.remove(&k);
    }
    /// Flip it, and say what it became - the menu items need both.
    pub fn toggle(&mut self, k: WinKind) -> bool {
        let on = !self.is(k);
        self.set(k, on);
        on
    }
    /// CLOSE EVERYTHING. One line, and a window added later is closed by it without anybody remembering to.
    pub fn close_all(&mut self) {
        self.open.clear();
    }
    /// Raise or put out the start screen, telling ITS TWO STATES APART as the program does.
    ///
    /// `show_start` means "it comes up by itself on an empty document"; a request from the menu is a
    /// separate thing. Closing puts out both: closed is closed, whatever raised it.
    pub fn show_start(&mut self, on: bool) {
        self.set(WinKind::Start, on);
        if !on {
            self.start_asked = false;
        }
    }

    /// Is anything at all open? Escape and the modal checks ask this.
    pub fn any_open(&self) -> bool {
        !self.open.is_empty()
    }
    /// What is open, in a stable order - for a check, a report or a walk.
    pub fn open_kinds(&self) -> impl Iterator<Item = WinKind> + '_ {
        self.open.iter().copied()
    }
}

/// THE EDGES OF THE SELECTED BODY, ready for picking and for drawing.
///
/// Recomputed when the selection or the geometry changes, and keyed by the revision it was taken at, so a
/// stale set is never offered to a click.
#[derive(Default)]
pub struct EdgeCache {
    /// Whether the extrude gizmo's arrow is being dragged (the feature node's index).
    /// The body whose edges are currently shown for picking (chamfer or fillet), plus their polylines in world space.
    pub body: Option<Id>,
    pub polys: Vec<Vec<[f32; 3]>>,
    /// The edge's PERSISTENT id, parallel to `edge_polys` - for picking by an id that survives a rebuild.
    pub ids: Vec<u32>,
    /// The `geom_rev` at the moment the edge cache was built: when a body is rebuilt (the same Id, a new topology)
    /// the cache goes stale, so it is refreshed and any dangling picked edge ids are cleared.
    pub rev: u64,
    /// The straight edges of EVERY visible body (the body, the edge's persistent id, the polyline in its LOCAL
    /// frame) - for the axis click-pick (a pattern or a datum axis): an edge of ANY body may be picked, and the id
    /// is what makes the axis associative. Loaded on entry.
    pub axes: Vec<(Id, u32, Vec<[f32; 3]>)>,
}

/// TRIMMING A SURFACE: the stroke drawn by hand, what it has already cut, and which piece is being kept.
#[derive(Default)]
pub struct TrimTool {
    /// POWER TRIM: the on-screen trail of the drag and the spans already trimmed (an entity plus a span number).
    /// They live only within one drag - otherwise a second pass over the same line would cut nothing.
    pub path: Vec<Pos2>,
    pub done: std::collections::HashSet<(Id, u32)>,
    /// TRIM: what is being cut and which side is kept: (the sheet body, the click point) plus the tool body.
    ///
    /// The point arrives with the first click: a person points at THE part they are keeping, and that same movement
    /// chooses the sheet. Asking for the side as a separate step would split one gesture into two for the sake of
    /// data that has already been given.
    pub keep: Option<(Id, [f64; 3])>,
    pub tool: Option<Id>,
}

/// WHAT THE OPEN EDIT IS. Work in hand - a sketch, a command, a drag - takes Ctrl+Z as a step of the history under it. A
/// question to the person - an import waiting for its scale - is one step not yet taken: Ctrl+Z takes it back as its
/// Cancel does, and the history under it is not touched.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum OpenEdit {
    #[default]
    Working,
    /// The open edit waits for an answer.
    Asking,
    /// Ctrl+Z came while it waited: its window cancels it on its next frame.
    AskingDropped,
}

/// UNDO, REDO AND WHAT IS SAVED: the history of the document and how far it has been written to disk.
pub struct Edits {
    /// Undo and redo: snapshots of the editor's state.
    pub undo: Vec<Step>,
    pub redo: Vec<Step>,
    /// The currently committed state (the baseline) and its key.
    pub baseline: Snapshot,
    /// the open edit: (its name, the state BEFORE it). Empty means there is no edit and a change is going past the boundary.
    pub open: Option<(String, Snapshot)>,
    /// the nesting depth of the edits: an inner `commit_edit` must not close the OUTER edit
    pub depth: usize,
    /// What the open edit is: work in hand or a question waiting for its answer.
    pub open_kind: OpenEdit,
    pub committed_key: u64,
    /// The places that change the document past `App::edit` - they are gathered during real work and converted one
    /// at a time. A field rather than a global flag: the debt belongs to the session, not to the process.
    ///
    /// Only the debug build keeps it: the record is written by the debug-only watch below, and a field
    /// nobody reads in release is a `dead_code` refusal, which is denied in the manifest.
    #[cfg(debug_assertions)]
    pub debt: std::collections::HashSet<String>,
    pub ready: bool,
    /// The state key as of the last save, load or new project. Being dirty (having unsaved changes) means
    /// `edit_key() != saved_key`.
    pub saved_key: u64,
    /// Allow the window to close (the dialogue has been answered), so that intercepting `close_requested` does not loop.
    pub allow_close: bool,
    /// AUTOSAVE: the timer and the key of the last autosave (so the same thing is not written twice).
    pub last_autosave: std::time::Instant,
    pub autosave_key: u64,
}

impl Edits {
    /// A NEW DOCUMENT STARTS A CLEAN HISTORY: no step to take back, none to put again, and the baseline and the saved
    /// mark taken afresh on the next frame. Reported behaviour: File -> New project came with a step "Edit" nobody made
    /// and unsaved at once, so the next New project asked to save the one just made.
    pub fn fresh(&mut self) {
        self.undo.clear();
        self.redo.clear();
        self.open = None;
        self.depth = 0;
        self.open_kind = OpenEdit::Working;
        self.ready = false;
    }
}

impl Default for Edits {
    fn default() -> Self {
        Self {
            undo: Vec::new(),
            redo: Vec::new(),
            baseline: Snapshot { project: Project::default() },
            open: None,
            depth: 0,
            open_kind: OpenEdit::Working,
            committed_key: 0,
            #[cfg(debug_assertions)]
            debt: std::collections::HashSet::new(),
            ready: false,
            saved_key: 0,
            allow_close: false,
            last_autosave: std::time::Instant::now(),
            autosave_key: 0,
        }
    }
}

/// THE STATE OF THE REBUILD: what is pending, what is running in the background, and how far the geometry
/// has moved.
///
/// The revisions are the whole point: nearly every cache and every check compares against them rather than
/// waiting for somebody to remember to clear things.
#[derive(Default)]
pub struct Rebuilding {
    /// The active background operation (a STEP or STL import or export) - a modal overlay with a spinner while it runs.
    pub busy: Option<Busy>,
    /// A rebuild was requested: it runs in the background on the next frame, with an indicator shown.
    pub wanted: bool,
    /// THE REBUILD WAS STOPPED BY A PERSON - it no longer starts by itself, for the document it was stopped on (its
    /// `rebuild_key`).
    ///
    /// The dirty marks on the nodes remain after a cancellation (the document really has not been rebuilt), and the
    /// scheduler looks at exactly those. Without this mark the next frame would start precisely the work that was
    /// just stopped. Any change of the document and an explicit "rebuild everything" clear it. Reported behaviour:
    /// held as a plain flag it outlived every later edit - a recognition applied after a cancelled one made its node
    /// at once and never computed it, nor anything after it.
    pub paused: Option<u64>,
    /// How many undo steps the document had when the rebuild now asked for was asked for, and when the last one that
    /// ran to its end was: a cancelled rebuild takes back the one edit it was computing, and only that.
    pub computing_depth: usize,
    pub computed_depth: usize,
    /// the application really is running frames (not a headless test) - only then does a rebuild go into the background.
    pub ui_running: bool,
    /// BACKGROUND (non-modal) work - it runs while the model is already being turned: topping up the imports'
    /// B-rep, saving the project. No overlay is shown, only the status line; the result comes over the same channel
    /// as `busy`.
    pub bg: Vec<Busy>,
    /// WHETHER THE IMPORTS' RESTORATION WAS CALLED - for the tests: "rebuild everything" must call it.
    pub import_asked: bool,
    /// The reference rebindings of the last rebuild - they are shown in the status line and mark the nodes in the
    /// tree. They live until the next rebuild and never reach the file.
    pub rebinds: Vec<qymcad_core::feature::Rebind>,
    /// The last line the rebuild itself wrote on the status line: its end replaces only that, not what an operation
    /// wrote since.
    pub line: String,
    /// What stood on the status line before a quiet rebuild wrote its progress over it - put back when it ends well.
    pub over: String,
    /// THE STATE AS OF THE PREVIOUS REBUILD: the document's key, the list of dirty nodes and the number of live
    /// B-reps. The scheduler recomputes only if AT LEAST ONE of those has changed.
    ///
    /// A node that failed to build deliberately stays dirty - the attempt must happen again once its input appears.
    /// But the scheduler read "dirty" as "compute now" and took it up on every frame: with a single red feature the
    /// rebuild window flickered without stopping.
    pub last: (u64, Vec<Id>, usize),
    /// A counter of geometry changes (for invalidating the 3D render cache).
    pub geom_rev: u64,
    /// THE LAYOUT'S REVISION - where the parts stand. It moves on a drag and on any component move.
    ///
    /// It is kept separate from `geom_rev` for exactly the reason every other such pair was separated: A LAYOUT IS
    /// NOT GEOMETRY. Bodies do not change when they move, and everything that depends only on their form (the
    /// edges, the smoothed normals, the scene buffer's blocks) has no reason to be recomputed.
    pub place_rev: u64,
    /// a rebuild was requested inside an edit - it runs ONCE when that edit closes
    pub pending: bool,
}

impl Rebuilding {
    /// Is a REBUILD running in the background, as opposed to some other slow job?
    ///
    /// The `busy` slot holds one job of any kind - an import, an export, a rebuild - so "is it busy"
    /// and "is it rebuilding" are different questions. The second was spelled out three times in three
    /// places, each repeating which kind counts.
    pub fn regen_running(&self) -> bool {
        matches!(&self.busy, Some(b) if b.kind == BgKind::Regen)
    }
}

/// THE LIVE GEOMETRY behind the meshes: the kernel's own solids and the faces they were tessellated into.
///
/// A bundle holds meshes and faces, not solids, so the live B-rep is fetched on demand rather than rebuilt
/// when a file is opened.
#[derive(Default)]
pub struct LiveGeom {
    /// THE BLOBS OF THE LIVE BODIES FOR WRITING, one per body. Serialising every body of a real file costs about
    /// 0.6 s, and without a cache EVERY autosave (once every three minutes) would pay it straight on the UI thread.
    /// An entry here is dropped for exactly those bodies that were rebuilt.
    pub blobs: std::collections::HashMap<Id, Vec<u8>>,
    /// AT WHICH STATE OF THE DOCUMENT the B-rep cache was last brought up. If nothing has changed since, repeating
    /// it is pointless (and the "ready" flag must not be raised - it would be a lie).
    ///
    /// THIS USED TO HOLD THE PICTURE'S REVISION (`geom_rev`), AND THAT WAS THE MAIN DOOR TO THE FLICKER. `geom_rev`
    /// is moved by `invalidate`, which is called on EVERY frame of a part drag - even though the live B-rep does
    /// not depend on where the part stands at all. In a document with joints on faces, a frame always brings the
    /// B-rep up (`needs_live_brep`), and while the live geometry is not fully raised, every frame of the drag
    /// started the preparation afresh: it marks the nodes dirty and DEMANDS a rebuild by an explicit request, and
    /// an explicit request bypasses every check the scheduler makes. Reported behaviour: after pressing "rebuild
    /// everything" and starting to drag, a modal window flickered unbearably (after a full rebuild nothing has a
    /// live B-rep).
    ///
    /// The key now describes the preparation's INPUTS: the timeline's state (`rebuild_key`) and how many bodies
    /// already have a live form. Dragging a part changes neither; new forms appearing does change it, and a
    /// legitimate second attempt still happens.
    pub tried_rev: Option<u64>,
    pub wait: Option<bool>,
    /// Does the cache of live `Shape`s match the timeline? After opening from a bundle the geometry is shown from
    /// the file while the B-rep is not yet built - the first operation that needs it brings it up through
    /// `ensure_brep`.
    pub ready: bool,
    /// The faces keyed by BODY Id (unlike the index-parallel `faces`, this survives deletions and reorderings of
    /// the meshes). The source for `rebuild_faces_from_cache` after a change of topology.
    pub faces: std::collections::HashMap<Id, Vec<MeshFace>>,
    /// A runtime cache of the live B-rep shapes keyed by body Id (for the shared booleans). It is not serialised
    /// and is filled in by extrude, revolve, STEP and the booleans.
    pub shapes: std::collections::HashMap<Id, qymcad_kernel::Shape>,
    /// THE LIVE SHAPES OF IMPORTS AN UNDO TOOK OUT of the document, kept for the redo that brings them back. An import
    /// has no recipe: its shape comes only from parsing the file again, and without it the body came back from redo
    /// as the snapshot's mesh, faces unnamed and no edges to pick.
    pub shelved: std::collections::HashMap<Id, qymcad_kernel::Shape>,
    /// THE BYTES OF THE SOURCES AN UNDO TOOK OUT, for the same redo: a snapshot carries a source without its bytes,
    /// and a drawing brought back without them had no geometry to bring in again.
    pub shelved_sources: std::collections::HashMap<Id, Vec<u8>>,
}

/// THE COLOUR SCHEME AND THE SETTINGS WINDOW: the palette in force, the ones to choose from, and what is
/// being searched for or edited.
pub struct SchemeUi {
    /// THE CURRENT PALETTE, derived from the `scheme` setting. It is kept resolved rather than looked up by name
    /// for every colour: a colour is asked for thousands of times per frame.
    pub pal: qymcad_scheme::Palette,
    /// EVERY KNOWN SCHEME: the built-in ones plus the user's from disk. Kept as a list rather than reading the
    /// directory every frame - the files are re-read only when the editor has changed them.
    pub all: Vec<qymcad_scheme::Palette>,
    /// the state of the custom-scheme screen: which one is being edited and what to say about saving
    pub edit: SchemeEdit,
    /// THE OPEN SECTION of the settings window. INTERFACE state rather than a setting: `Settings` holds only the
    /// values that are saved whole, and "where I am in the window right now" is not one of them.
    pub section: settings_sections::SettingsSection,
    /// The settings search's query.
    pub search: String,
    /// What to say after a section is reset or after an attempt to open the config folder.
    pub note: String,
}

impl Default for SchemeUi {
    fn default() -> Self {
        Self {
            pal: qymcad_scheme::dark(),
            all: qymcad_scheme::builtin(),
            edit: SchemeEdit::default(),
            section: settings_sections::SettingsSection::General,
            search: String::new(),
            note: String::new(),
        }
    }
}

/// WHAT EXACTLY THE COMMAND BUILDS: the operation on the body and the direction. These used to be three
/// independent fields of `App`, and the ORDER in which they were set mattered more than their values: the direction
/// was computed when the command opened while the operation was chosen later - so a cut went outwards and removed
/// nothing. Gathered into one record they cannot drift apart: what is derived (`flip`) is deduced from `op` when
/// it is needed.
#[derive(Clone, Copy, Default, PartialEq, Debug)]
pub struct FeatTarget {
    /// 0 add (a new body), 1 a boss, 2 a cut, 3 an intersection
    pub op: u8,
    /// the direction: true means against the sketch's normal
    pub flip: bool,
    /// the direction is still automatic (nobody has touched it), so it is recomputed on apply
    pub flip_auto: bool,
}

impl FeatTarget {
    /// Open the command: the direction is automatic for now.
    pub fn opened(op: u8) -> Self {
        Self { op, flip: false, flip_auto: true }
    }

    /// The direction was set by hand - it is no longer recomputed.
    pub fn set_flip(&mut self, flip: bool) {
        self.flip = flip;
        self.flip_auto = false;
    }
}

/// THE SWEEP'S PARAMETERS: the profile and the path, each with its own sketch and chosen contour.
/// Five independent fields of `App` turned "which sketch is being picked right now" into implicit state:
/// `pick_path` decided where the next click would land and drifted apart from what had already been chosen.
#[derive(Clone, Copy, Default, PartialEq)]
pub struct SweepParams {
    /// the profile's sketch and the contour chosen in it (0 means the first suitable one)
    pub prof_sid: Id,
    pub prof_cid: Id,
    /// the path's sketch and the contour chosen in it
    pub path_sid: Id,
    pub path_cid: Id,
    /// the next click picks THE PATH (otherwise the profile)
    pub pick_path: bool,
}

/// THE LOFT'S PARAMETERS: an ordered set of sections (a sketch plus a contour in it) and how to join them.
#[derive(Clone, Default, PartialEq)]
pub struct LoftParams {
    /// the sections in order: the sketches and the contours chosen in them (parallel lists)
    pub sids: Vec<Id>,
    pub cids: Vec<Id>,
    /// ruled faces between the sections (otherwise a smooth surface)
    pub ruled: bool,
    /// what to do with the result: 0 a new body, 1 a boss, 2 a cut
    pub result: u8,
    /// the sections are being picked by clicks
    pub pick: bool,
    /// the section added last, so that a second click on it does not duplicate it
    pub pick_last: Option<Id>,
}

/// THE REVOLVE'S PARAMETERS: the axis and the angle. The axis is given in THREE mutually exclusive ways (a base
/// X or Y, a datum axis, a sketch centreline) plus two flags for "an axis or a line is being picked right now" - as
/// separate fields that state is easily driven into an impossible combination.
#[derive(Clone, Copy, Default, PartialEq)]
pub struct RevolveParams {
    /// the sketch's base axis: 0 for X, 1 for Y (used when neither `axis_datum` nor `axis_line` is set)
    pub axis: u8,
    /// a datum axis (it outranks the base one)
    pub axis_datum: Id,
    /// a sketch centreline (it outranks everything)
    pub axis_line: Id,
    /// a datum axis or a centreline is being picked by a click
    pub pick_axis: bool,
    pub pick_line: bool,
    pub angle: f64,
}

/// THE THREAD'S PARAMETERS: what it holds on to (a body and a circular edge) and what is being cut.
#[derive(Clone, Copy, Default, PartialEq, Debug)]
pub struct ThreadParams {
    /// the source body and the circular edge that gave the axis and the radius
    pub src: Option<Id>,
    pub edge: u32,
    pub axis: ([f64; 3], [f64; 3]),
    pub radius: f64,
    pub internal: bool,
    pub starts: u32,
    pub left: bool,
    /// the profile: 0 is metric, the rest follow the standards
    pub form: u8,
    /// an auger (a ribbon) rather than a thread
    pub auger: bool,
}

/// The unfinished drawing as a whole: the shape plus the input of its dimensions.
#[derive(Clone, Default, PartialEq)]
pub struct Placing {
    pub shape: PlacingShape,
    /// the dimension is following the cursor and has not been placed yet (the constraint's index)
    pub dim: Option<usize>,
    /// the dimension input fields and the focus within them
    pub buf: [String; 2],
    pub focus: bool,
    /// a value was typed into the fields: closing them with Enter then lays the dimensions typed
    pub typed: bool,
}

impl Placing {
    /// Are the unfinished shape's dimensions being typed in?
    pub fn active(&self) -> bool {
        self.shape != PlacingShape::None
    }

    /// Abandon the unfinished drawing.
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn rect(&self) -> Option<(Point2, Point2, Vec<Id>)> {
        match &self.shape {
            PlacingShape::Rect { a, b, ids } => Some((*a, *b, ids.clone())),
            _ => None,
        }
    }

    /// A rectangle reopened for its width and height: the indices of those dimensions.
    pub fn rect_dims(&self) -> Option<qymcad_core::model::RectDims> {
        match &self.shape {
            PlacingShape::RectDims(d) => Some(*d),
            _ => None,
        }
    }

    /// A rectangle drawn from its centre: the centre plus the corner under the pointer.
    pub fn rect_center(&self) -> Option<(Point2, Point2, Vec<Id>)> {
        match &self.shape {
            PlacingShape::RectCenter { center, corner, ids } => Some((*center, *corner, ids.clone())),
            _ => None,
        }
    }

    pub fn poly(&self) -> Option<Id> {
        match self.shape {
            PlacingShape::Poly(id) => Some(id),
            _ => None,
        }
    }

    pub fn ellipse(&self) -> Option<(Id, Point2)> {
        match self.shape {
            PlacingShape::Ellipse(id, c) => Some((id, c)),
            _ => None,
        }
    }

    /// Start or update the shape. The previous one disappears - it is no longer being drawn.
    pub fn set(&mut self, shape: PlacingShape) {
        self.shape = shape;
    }
}

/// THE DRAWING TOOLS' PREFERENCES: each has a mode and numbers of its own. This is NOT the state of the current
/// action (that lives in `Placing`) but preferences proper: they survive a change of tool, as in any grown-up CAD -
/// come back to the polygon and the number of sides is the one left there.
#[derive(Clone, Default, PartialEq)]
pub struct SketchToolPrefs {
    /// the polygon: the number of sides and how its two clicks are read (see `polygon_from_clicks`)
    pub poly_n: u32,
    pub poly_mode: u8,
    /// the construction modes: an arc (by three points or by a centre), a rectangle (corners or centre), a circle
    pub arc_mode: u8,
    pub rect_mode: u8,
    pub circ_mode: u8,
    /// EXTEND TAKES BOTH ENDS of its line, each to the nearest geometry on its side ("Both sides" in its bar)
    pub extend_both: bool,
    /// the corner fillet's radius - the first leg of a chamfer - and the offset's distance
    pub fillet: f64,
    pub offset: f64,
    /// HOW A CHAMFER IS GIVEN (as the chamfer of a part): equal legs, two legs, or a leg and an angle; and its second
    /// value - the second leg, or the angle from the first line in degrees
    pub chamfer_mode: qymcad_core::feature::ChamferMode,
    pub chamfer_second: f64,
    /// HOW A FILLET IS GIVEN: by its radius, its chord or the length of its arc; `fillet` holds the value in that way
    pub fillet_by: qymcad_core::model::FilletBy,
    /// a text in a sketch: its contents, its height, and whether it is an annotation rather than geometry
    pub text: String,
    pub text_h: f64,
    pub text_note: bool,
    /// WHAT THE TEXT TOOL WRITES WITH, beside what it writes and how tall.
    ///
    /// A setting of the tool, not a thing the application owns: the top bar shows the family here beside the
    /// height, and the label made by the next click keeps it. Empty until a font is chosen or found.
    pub font: qymcad_core::model::FontRef,
}

/// THE SKETCH PATTERN'S PARAMETERS: two directions with a step and a count each, plus the circular variant.
#[derive(Clone, Copy, Default, PartialEq)]
pub struct SketchPattern {
    pub dx: f64,
    pub dy: f64,
    pub count: u32,
    pub dx2: f64,
    pub dy2: f64,
    pub count2: u32,
    pub angle: f64,
}

/// WHETHER WHAT A DRAWING TOOL PUTS DOWN IS TIED TO WHAT IT LANDS ON, construction or not. Reported behaviour:
/// "construction geometry is built without ties" - drawing construction switched the ties off, and nothing switched
/// them off for ordinary geometry.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Ties {
    /// the pointer snaps to points and lines, and what is drawn is tied there and laid level or upright where it
    /// stands so, as the auto constraints of the sketch say
    #[default]
    Laid,
    /// "No ties": put down exactly where it is clicked - nothing snaps, nothing is tied, the preview snaps to nothing
    Free,
}

/// WHETHER A DRAWING TOOL TIES WHAT IT PUTS DOWN: "No ties" not ticked on its bar and the auto constraints of the
/// sketch on - construction geometry as any other. One rule for what is laid and for what the preview shows: the
/// preview of a line showed the badge of a level tie with "No ties" ticked, a tie the click was not going to lay.
pub fn ties_laid(tool: &SketchTool, set: &Settings) -> bool {
    tool.ties == Ties::Laid && set.auto_constrain
}

/// THE ACTIVE SKETCH TOOL: what is chosen on the panel and what has already been clicked with it.
/// "Which tool" and "how many points it has gathered" used to be different fields of `App`, and changing the tool
/// was not obliged to clear what had been gathered - yet it must: a line's points mean nothing to an arc.
#[derive(Clone, Default, PartialEq)]
pub struct SketchTool {
    /// the points clicked with THE CURRENT tool (an arc has three, a line two, and so on)
    pub pts: Vec<Point2>,
    /// construction geometry is being drawn
    pub construction: bool,
    /// whether what is drawn is tied to what it lands on - "No ties" on the bar of a drawing tool
    pub ties: Ties,
    /// PROJECTION: take not one edge but THE WHOLE outline of the face the sketch stands on. By a switch of its
    /// own rather than a guess from the click: "clicked inside the face" is indistinguishable from a miss past an
    /// edge on outlines with complicated cut-outs.
    pub proj_face: bool,
    pub move_base: Option<Point2>,
    /// THE LINE EXTEND HOLDS: taken with the tool from the selection made before it, or picked by the first click; its
    /// extension is previewed up to what the pointer is over, and Enter or a click applies it
    pub extend: Option<Id>,
    /// the tangent edge given for a circle (picked with THIS tool rather than globally)
    pub circ_tan: Option<EdgeRef>,
    /// THE STRING OF THE TEXT TOOL AS OUTLINES at the origin, for the preview at the pointer: baked once when the
    /// string, its height or its font change (the fonts are not at hand where the canvas is drawn) - (the string,
    /// the height, the font, the outlines)
    pub text_ghost: Option<(String, f64, qymcad_core::model::FontRef, Vec<Vec<Point2>>)>,
    /// letters of the string were taken out because the font cannot write them: a click on the empty string names
    /// the font rather than blaming the string
    pub text_refused: bool,
}

impl SketchTool {
    /// Choose a tool: whatever was clicked with the previous one MUST disappear - it has nothing to do
    /// with the new one - and WHAT IS IN HAND changes with it. Clicking the tool already in hand puts it
    /// down, which is why the old value is read before the new one is written.
    pub fn select(&mut self, armed: &mut Armed, kind: u8) {
        *armed = if armed.draw_kind() == kind { Armed::None } else { Armed::Draw(kind) };
        self.pts.clear();
    }
}

/// THE CORE OF THE ACTIVE PART COMMAND: what is open, what it works on, with which parameters.
///
/// This is the very place where the order of assignments mattered more than the values. Gathered into one record
/// it now has A LIFE CYCLE: `open` (the command is opened) and `close` (it is closed). Opening clears the past by
/// itself - that used to be a scattering of assignments in `start_feat_cmd`, and nothing stopped one being
/// forgotten.
#[derive(Clone, Default)]
pub struct FeatCommand {
    /// the sketch the command works from
    pub sketch: Option<usize>,
    /// the parameters on the canvas (a height, a radius, a thickness) - their values and the expressions typed in
    pub params: Vec<CmdParam>,
    /// the extent (one side, symmetric, two sides, through all) and the second side
    pub extent: ExtentMode,
    pub down: f64,
    /// an EXISTING feature is being edited (its Id) rather than a new one created
    pub edit: Option<Id>,
    /// the arrow being dragged: the key of the field it drives ("height", or "down" for the second side)
    pub drag: Option<&'static str>,
    /// the view was 3D before the command opened - restore it on Esc or on apply
    pub prev_3d: bool,
    /// the parameter field takes the focus on the first frame (so Enter works straight away)
    pub focus: bool,
    /// the body on whose face the command's sketch sits (an associative placement)
    pub ref_body: Option<Id>,
}

impl FeatCommand {
    /// Open command `kind`: the previous state MUST disappear entirely, and WHAT IS IN HAND changes
    /// with it. The two travel together because they are one act - taking this command means letting
    /// whatever was in hand go, and `Armed` is a single field, so it cannot say both.
    pub fn open(&mut self, armed: &mut Armed, kind: u8, prev_3d: bool) {
        *self = Self { prev_3d, focus: true, ..Default::default() };
        // KIND ZERO IS NOT A COMMAND. While the kind lived in a field, "open command 0" was how the
        // apply path SAID "close", and the two readings of one number went unnoticed. The enum has a
        // name for "nothing in hand", and a command numbered zero is not it.
        *armed = if kind == 0 { Armed::None } else { Armed::Command(kind) };
    }

    /// Close the command (applied or cancelled).
    pub fn close(&mut self, armed: &mut Armed) {
        *self = Self::default();
        *armed = Armed::None;
    }
}

/// TYPING A FILLET RADIUS OR A CHAMFER LEG IN A SKETCH: the value is being written, and the corners it will be
/// taken off are the set beside it.
///
/// **THE VALUE BELONGS TO THE SET, NOT TO ONE CORNER.** The lines chosen in the sketch make the corners, the
/// corners are remembered as they were made, and one answer cuts all of them: a person naming four corners of a
/// shape meant one act, and undo taking back one corner of it would leave the drawing in a state nobody asked for.
#[derive(Clone, Default, PartialEq)]
pub struct CornerInput {
    /// WHICH CORNER TOOL IS IN HAND: the sketch, the point the field belongs to, and whether this is a chamfer
    /// rather than a fillet.
    pub at: Option<(usize, Id, bool)>,
    /// THE CORNERS THE CHOSEN LINES HAVE MADE, the ones put away at their point, and the ones named at a point.
    pub set: CornerSet,
    /// where on the canvas the input field stands
    pub pos: Option<Pos2>,
    pub buf: String,
    /// the second value of a chamfer of two legs or of a leg and an angle, typed beside the first
    pub buf2: String,
    pub focus: bool,
    /// where the corner was clicked, on the sheet: the line it stands nearer to is the first of a chamfer
    pub near: Option<qymcad_core::geom::Point2>,
    /// restrict the corners to this set (rounding THE SELECTED corners rather than all of them)
    pub only: Option<std::collections::HashSet<Id>>,
    /// why the value in the field was refused, said beside it
    pub why: Option<String>,
    /// HOW NEAR THE POINT THE CURSOR COUNTS, in pixels, remembered when the popup was opened: `Grab::Corner` read
    /// with the pick precision of the person. Farther than that the cursor is over the rest of the sheet and says
    /// nothing about which of the corners at the point is meant, so the corner does not follow the pointer across
    /// the drawing while the radius is being typed.
    pub track_px: f32,
    /// WHETHER THE SET IN HAND IS BEING BUILT WITH SHIFT. Shift is the only way more than one line is chosen, so
    /// every left click without it leaves the set and the tool goes back to choosing one line at a time.
    pub shifted: bool,
    /// WHETHER A CLICK CUT TWO LINES WHERE THEY CROSS to name a corner there - a step of undo of its own, which the
    /// corner cut joins and Esc takes back
    pub crossing: CrossingCut,
    /// whether the corner follows the pointer or is fixed where it was clicked
    pub aim: Aim,
}

/// WHETHER THE CORNER OF THE SET FOLLOWS THE POINTER OR STANDS WHERE IT WAS CLICKED. A point with more than one corner
/// to give (two crossing lines chosen with Shift, a click at a crossing, a point the corners of several shapes meet at)
/// is named, not a side of it: the corner follows the pointer into the sector it is in, drawn as a corner not named
/// yet, and a click fixes it there. Its preview turns the colour of the set, and the box of its size opens beside it
/// and stays, so the pointer can go to it. Reported behaviour: "the box runs away from the cursor; the corner must be
/// fixed by a click"; "a click at the crossing puts a corner at random - the same choice by a click as for two lines,
/// the standard for the fillet and the chamfer wherever more than one corner can be chosen".
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Aim {
    /// fixed by a click on its side, or the only corner of its point: it stays
    #[default]
    Fixed,
    /// named at a point of more corners than one, not yet by a side: it follows the pointer, no box yet
    Following,
}

/// HOW MANY CORNERS THE POINT OF THE SET HAS TO GIVE (`CornerSet::choice`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Choice {
    One,
    Many,
}

/// WHETHER THE POINTER STEERS THE CORNER OF A SET (`CornerSet::steer`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Steered {
    Yes,
    No,
}

/// WHETHER THE CORNER TOOL CUT LINES AT A CROSSING to name its corner.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum CrossingCut {
    #[default]
    None,
    /// the lines were cut, and the cut is the last step of undo: the corner joins it, Esc takes it back
    Cut,
    /// the tool was put down - Esc, another tool - with the lines cut and no corner cut: the cut is taken back on the
    /// next frame (`qymcad_sketch::sketch_tool_keys`), where the document is at hand
    Left,
}

impl CornerInput {
    /// THE TOOL PUT DOWN: all of the input goes, and lines it cut at a crossing for a corner never cut are left to be
    /// made whole again.
    pub fn put_down(&mut self) {
        let left = self.crossing == CrossingCut::Cut;
        self.clear();
        if left {
            self.crossing = CrossingCut::Left;
        }
    }
}

/// THE POINT THE FIELD OF A WHOLE SET NAMES, and it is not a point of the drawing.
///
/// **ZERO BELONGS TO "ROUND EVERY CORNER"**, whose field has stood for a whole shape since before a set of corners
/// existed, and whose button is lit by that zero. A set of corners is a whole set too, and its field named zero as
/// well: taking the fillet or the chamfer therefore pressed the button of the round-every-corner beside them. Its
/// field names this instead, so the two are told apart by the number itself rather than by a rule each door reads
/// for itself.
pub const CORNER_SET: Id = 1;

impl CornerInput {
    pub fn clear(&mut self) {
        *self = Self::default();
    }
}

/// THE BODY-TO-BODY BOOLEAN: what is chosen as an operand, and whether an existing node is being edited.
#[derive(Clone, Copy, Default, PartialEq)]
pub struct BoolCommand {
    /// the second operand of a 2D boolean (a contour index)
    pub other2d: usize,
    /// the chosen body and its role (0 the base, 1 the tool)
    pub pick: Option<(Id, u8)>,
    /// body B, clicked and waiting for Enter; a second click on it lets it go
    pub b: Option<Id>,
    /// an existing boolean node is being edited (its index in the timeline)
    pub edit: Option<usize>,
}

/// What a bar takes up once every other tool is put down.
#[derive(Clone, Copy, Debug)]
pub enum Then {
    Nothing,
    /// The body boolean, with body A when one was selected before the button.
    Boolean(Option<Id>),
}

impl Then {
    pub fn take(self, boolean: &mut BoolCommand, status: &mut String) {
        if let Then::Boolean(a) = self {
            boolean.pick = a.map(|a| (a, 0));
            boolean.b = None;
            *status = qymcad_i18n::tr(if a.is_some() { "tb-bool-pick-b" } else { "tb-pick-body-a-first" });
        }
    }
}

/// THE MIRROR'S PARAMETERS: a given plane OR a given part - the mirroring goes either by a plane or over a whole
/// part, and as separate fields both could end up given at once.
#[derive(Clone, Default)]
pub struct MirrorParams {
    pub plane: Option<qymcad_core::feature::SketchPlane>,
    pub part: Option<Id>,
    /// the plane the mirrored copy of `part` is made about, clicked and waiting for Enter: (origin, normal) in the
    /// frame of the context it was clicked in
    pub at: Option<([f64; 3], [f64; 3])>,
    /// the tool is in hand with no part yet: a click on a body takes its part
    pub waiting: bool,
}

impl MirrorParams {
    /// Put the mirrored copy of a part down: the part and the plane picked for it go together.
    pub fn drop_part(&mut self) {
        (self.part, self.at, self.waiting) = (None, None, false);
    }

    /// The mirrored copy of a part is in hand: with its part, or waiting for one to be clicked.
    pub fn in_hand(&self) -> bool {
        self.part.is_some() || self.waiting
    }
}

/// WHAT THE PROGRAM OPENS WITH, decided in one place out of the settings.
///
/// A named answer rather than two ifs inside the launch: the launch cannot be run by a check, so a decision
/// living there is a decision nobody measures. Four combinations of two flags give three outcomes, and each
/// one is somebody's first minute.
#[derive(Debug, PartialEq, Clone)]
pub enum Opening {
    /// Load this file - the project of the previous session.
    LastProject(String),
    /// An empty document. `start_screen` says whether the start screen comes up by itself.
    Empty { start_screen: bool },
}

/// The decision itself. `last` is the path remembered from the previous session, if there is one.
///
/// A remembered path that no longer names a file is NOT a reason to show an empty window in silence: the
/// answer falls back to an empty document, and the start screen comes up if it is allowed to.
pub fn opening(set: &Settings, last: Option<&str>) -> Opening {
    match last.filter(|p| set.open_last && std::path::Path::new(p).is_file()) {
        Some(p) => Opening::LastProject(p.to_string()),
        None => Opening::Empty { start_screen: set.show_start_screen },
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            layout: Vec::new(),
            language: String::new(),
            scheme: "dark".into(),
            viewcube_size: 1,
            msaa: 4,
            autosave_secs: 180,
            undo_cap: 40,
            ghost_alpha: 115,
            kernel_threads: 0,   // all the cores but one
            persp_fov_deg: 35.5, // equals the former PERSP_FOV_HALF_TAN of 0.32: 2*atan(0.32)
            hotkeys: Default::default(),
            help_lang: String::new(),
            help_external: false,
            gpu_viewport: true,
            projection: Projection::Ortho,
            shading: Shading::Smooth,
            show_contours: true,
            show_joints: true,
            show_interference: false,
            snap: Snapping::default(),
            auto_constrain: true,
            show_point_numbers: false,
            dim_show_name: false,
            dim_show_formula: false,
            dim_font: DIM_FONT_DEFAULT,
            dim_text: DimTextTurn::default(),
            defaults: Defaults::default(),
            ui_scale: default_ui_scale(),
            recent: Vec::new(),
            recent_limit: default_recent_limit(),
            pick_precision: default_pick_precision(),
            open_last: true,
            show_start_screen: true,
            mouse_nav: default_mouse_nav(),
            zoom_at: default_zoom_at(),
            update_check: default_update_check(),
            update_last_checked: 0,
            zoom_editing: default_zoom_editing(),
            orbit_about: default_orbit_about(),
            import_ask_always: false,
            import_units: Default::default(),
            active_icon_packs: Vec::new(),
            inactive_icon_packs: Vec::new(),
            icon_dev_watch: false,
            watched_icon_packs: Vec::new(),
        }
    }
}

/// THE COMPONENT PATTERN COMMAND (in an assembly): what is being replicated and how.
///
/// Separate from the BODY pattern (`ArrayParams`): they have different sources (a component against a body),
/// different results (instance components against one merged body) and different edits. Shared state would mean
/// that editing one silently changes the other.
#[derive(Clone, Debug)]
pub struct CompArrayCmd {
    /// which command is open: 0 none, 1 linear, 2 circular
    pub mode: u8,
    /// the source component (chosen before the start or by a click)
    pub src: Id,
    /// linear: the direction, 0 for X, 1 for Y, 2 for Z
    pub dir: u8,
    /// circular: the axis, 0 for X, 1 for Y, 2 for Z
    pub axis: u8,
    /// an EXISTING pattern is being edited (its Id) rather than a new one created
    pub edit: Id,
}

impl Default for CompArrayCmd {
    fn default() -> Self {
        Self { mode: 0, src: 0, dir: 0, axis: 2, edit: 0 }
    }
}

/// THE SPLIT'S PARAMETERS: the plane a body is divided into independent pieces by. Separate from the mirror - the
/// tools live at the same time, and a shared field would mean that choosing for one erases the other's choice.
#[derive(Clone, Default)]
pub struct SplitParams {
    pub plane: Option<qymcad_core::feature::SketchPlane>,
}

/// THE OPTIONS OF THE INDIVIDUAL Part tools that have a switch or two each.
#[derive(Clone, Copy, Default, PartialEq)]
pub struct FeatOptions {
    /// the shell: which side of the opened face the wall goes to
    pub shell_side: qymcad_core::feature::ShellSide,
    /// the patch: meet the edges smoothly (tangent to the neighbouring faces) rather than by position alone
    pub patch_tangent: bool,
    /// the mirror: keep the original
    pub mirror_keep: bool,
    /// the rotation angle used when inserting
    pub rot_deg: f64,
}

/// AN EDITING TOOL OF THE SKETCH: a button that works on the selected entities, or waits in hand for them.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EditTool {
    /// Delete the selected entities.
    Delete,
    /// Mirror them about an axis or a line, pointed at after they are picked.
    Mirror,
    /// Copy them in a row.
    LinearPattern,
    /// Copy them round a centre.
    CircularPattern,
    /// Round the corner of two picked lines.
    Fillet,
    /// Cut the corner of two picked lines.
    Chamfer,
    /// Lay a copy at a distance beside them.
    Offset,
}

impl EditTool {
    /// Every editing tool, in the order of the button numbers.
    pub const ALL: [EditTool; 7] = [EditTool::Delete, EditTool::Mirror, EditTool::LinearPattern, EditTool::CircularPattern, EditTool::Fillet, EditTool::Chamfer, EditTool::Offset];

    /// THE BUTTON'S NUMBER: the help table (`qymcad_help::map::SKETCH`, under "mod") knows the tools by it, and
    /// that crate knows nothing of this one.
    pub fn code(self) -> u8 {
        match self {
            EditTool::Delete => 0,
            EditTool::Mirror => 1,
            EditTool::LinearPattern => 2,
            EditTool::CircularPattern => 3,
            EditTool::Fillet => 4,
            EditTool::Chamfer => 5,
            EditTool::Offset => 6,
        }
    }
}

/// THE OFFSET, AIMED: the copy follows the pointer to one side until a click fixes it there, and the box of its
/// distance opens beside it and stays, so the pointer can go to it - the way the corner of a fillet is chosen.
/// Reported behaviour: "a popup with a field, the same choice as the fillet: the side picked, a click fixes it, then
/// the mouse can go to the popup".
#[derive(Clone, Default, PartialEq)]
pub struct OffsetAim {
    /// where the click fixed the side, on the sheet; none while the copy follows the pointer
    pub at: Option<qymcad_core::geom::Point2>,
    /// the distance as typed in the box
    pub buf: String,
    /// the box takes the keyboard on the frame it opens
    pub focus: bool,
}

impl SketchSelection {
    /// Clear the selection, together with whatever was waiting for it. A deferred action without a selection means nothing.
    pub fn clear(&mut self) {
        *self = Self::default();
    }
}

/// A POINT AN AXIS IS PICKED THROUGH: a datum point by its id, or a vertex of a body by its edge and end - both followed
/// by the axis as they move - with where it stood when picked, in the frame of the context.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AxisPoint {
    Datum(Id, [f64; 3]),
    Vertex { body: Id, edge: u32, end: bool, at: [f64; 3] },
}

impl AxisPoint {
    pub fn at(&self) -> [f64; 3] {
        match *self {
            AxisPoint::Datum(_, at) | AxisPoint::Vertex { at, .. } => at,
        }
    }
}

/// THE PART PATTERN'S PARAMETERS: up to three linear directions, or a circular one about an axis.
/// Eleven fields described ONE intention, and "two directions" could end up switched on with a count of 1, or an
/// axis chosen for a linear pattern.
#[derive(Clone, Copy, Default, PartialEq)]
pub struct ArrayParams {
    pub count: u32,
    pub dir: u8,
    /// the second direction
    pub two: bool,
    pub count2: u32,
    pub dir2: u8,
    /// the third direction
    pub three: bool,
    pub count3: u32,
    pub dir3: u8,
    /// a circular pattern about an axis: the axis itself, whether it is a full turn, and whether the axis is being picked by a click
    pub axis: Id,
    pub full: bool,
    pub axis_pick: bool,
    /// the first of two points an axis is being picked through, until the second comes
    pub axis_first: Option<AxisPoint>,
}

/// BUILDING A DATUM: which mode it is in and what references have been gathered.
#[derive(Clone, Default)]
pub struct DatumCommand {
    /// the chosen base plane (for an offset plane)
    pub plane_pick: Option<qymcad_core::feature::SketchPlane>,
    /// the axis mode and the reference geometry gathered
    pub axis_mode: u8,
    pub axis_ref: Option<([f64; 3], [f64; 3])>,
    pub axis_pts: Vec<(Id, [f64; 3])>,
    pub axis_hit: Option<AxisHit>,
    /// the point mode and the chosen vertex
    pub pt_mode: u8,
    pub pt_vert: Option<(Id, u32, bool, [f64; 3])>,
    /// the datums hidden in the view
    pub hidden: std::collections::HashSet<Id>,
    /// the datum's consumers need rebuilding
    pub regen_pending: bool,
}

/// THE DIMENSION TOOL IN A SKETCH: what has been pointed at and what is being typed.
#[derive(Clone, Default)]
pub struct DimTool {
    /// the first reference pointed at, and what has been clicked
    pub first: Option<DimRef>,
    pub pick: Vec<Id>,
    /// typing the value: buffer, focus, editing an existing dimension
    pub buf: String,
    pub focus: bool,
    /// THE FIELD OF THE SIZE OF A CIRCLE OR AN ARC WAS TYPED IN since it opened: only a value typed is laid as a
    /// dimension, as the width and the height of a rectangle are (`Placing::typed`)
    pub typed: bool,
    pub edit: Option<(Id, String, String)>,
    /// THE FIELD OF A DIMENSION JUST MADE: (its constraint, the length of the undo list when it was made). Its value
    /// joins the step that made it - making a dimension and typing its value is one act, undone by one Ctrl+Z - as
    /// long as no other step came between.
    pub fresh: Option<(usize, usize)>,
    /// THE SKETCH AS IT WAS before a provisional length was put on a picked line: Esc puts it back whole, the
    /// points the length brought with it (an axis of the sketch made on demand) as well.
    pub before: Option<Box<qymcad_core::model::Sketch>>,
    /// a hint drawn over the canvas
    pub overlay: Option<String>,
    /// A SILENT REBUILD IS RUNNING: a spinner in the middle of the canvas, no text and no dimming.
    ///
    /// While a background rebuild runs silently, the previous body is still on screen and people draw
    /// wrong conclusions from it. The status line is not enough — nobody looks at it while looking at
    /// the model. A spinner right on the canvas says one thing, but says it in time: what you see is
    /// STALE, a recompute is under way.
    pub spinner: bool,
    /// NODE COUNT of a long rebuild `(done, total)` — next to the caption and nowhere else.
    /// A count means there is also a "Cancel" button: exactly what reports its progress can be stopped.
    pub overlay_progress: Option<(usize, usize)>,
}

/// THE JOINT COMMAND: what has been pointed at, with which anchor, and whether an existing one is being edited.
#[derive(Clone)]
pub struct JointCommand {
    /// parameters of the joint being created
    pub new_kind: qymcad_core::feature::JointKind,
    pub new_offset: f64,
    pub new_angle: f64,
    /// KEEP IT WHERE IT STANDS RIGHT AT CREATION (an "as-built" joint).
    ///
    /// A "keep it where it stands" button in the joint's properties comes TOO LATE: the joint has
    /// already brought the anchors together and dragged the part away, and there is nothing left to
    /// declare "as it stands" — the spoiled placement would be baked in. Measured on a machine
    /// assembly: a slider joint carried an axis together with its spindle 1176.339 mm away. For an
    /// already assembled machine exactly the opposite is needed: the joint is born moving nothing.
    pub new_as_built: bool,
    /// picking mode: by faces, grounding, kind of anchor, the first anchor pointed at
    pub pick_faces: bool,
    pub ground_pick: bool,
    pub anchor_mode: u8,
    pub pick_first: Option<(Id, qymcad_core::feature::AnchorRef)>,
    /// editing an existing joint and re-picking its anchor
    pub edit: Option<Id>,
    /// the joint in `edit` is the one just made by the second pick, still inside its open operation: Apply or Enter
    /// keeps it as one step, Esc, Cancel or another tool takes it away as if it had never been
    pub creating: bool,
    pub edit_repick: Option<(Id, bool)>,
    /// THE SURFACES BEING POINTED AT FOR A TANGENT CONDITION.
    ///
    /// `Some` means the "Tangent" tool is in hand. Two surfaces are enough: a tangent condition has no
    /// connectors at all, and the second pick places it right away.
    pub tangent_pick: Option<Vec<(Id, qymcad_core::feature::AnchorRef)>>,
    /// THE ANCHORS BEING POINTED AT FOR A WIDTH CONDITION: two walls and the part between them.
    ///
    /// `Some` means the "Width" tool is in hand. Order matters: the first two anchors are the walls,
    /// the third is what sits between them.
    pub width_pick: Option<Vec<(Id, qymcad_core::feature::AnchorRef)>>,
    /// THE SET OF PARTS BEING GATHERED INTO A GROUP.
    ///
    /// `Some` means the "Group" tool is in hand; inside are the parts already clicked. Clicking a part
    /// again takes it back out of the set: the set is EDITED, not only appended to.
    pub group_pick: Option<Vec<Id>>,
    /// WHETHER A STANDALONE CONNECTOR IS BEING CREATED.
    ///
    /// A connector is a feature in its own right, like a sketch: it is made UP FRONT, and joints are
    /// attached to it afterwards. Without such a command a connector could only appear inside a joint,
    /// and there was no way to reuse it.
    pub conn_pick: bool,
    /// THE RELATION BETWEEN JOINTS BEING ASSEMBLED RIGHT NOW.
    ///
    /// `Some` means the "Relation" tool is in hand. What gets picked is not faces but THE JOINTS
    /// THEMSELVES: a relation ties their degrees of freedom together, and there is nothing else in it
    /// to point at.
    pub relation_pick: Option<RelationPick>,
    /// THE CONNECTOR WHOSE SECONDARY AXIS WE ARE WAITING TO BE POINTED AT (the "second pick").
    ///
    /// A square face has no long side, and the automatic choice answers arbitrarily; a person then
    /// points at an edge, and the axis will run along it. While this is `Some`, a click on geometry
    /// sets the axis of that connector and nothing else.
    pub axis_pick: Option<Id>,
    /// dragging the joint gizmo
    pub giz_drag: Option<JointGizDrag>,
    pub giz_handle: Option<(u8, bool)>,
}

impl JointCommand {
    /// Take the rigid mate and wait for the two faces.
    pub fn start_rigid_pick(&mut self) {
        self.new_kind = qymcad_core::feature::JointKind::Rigid;
        self.pick_faces = true;
    }
}

impl Default for JointCommand {
    fn default() -> Self {
        Self {
            new_kind: qymcad_core::feature::JointKind::Rigid,
            new_offset: 0.0,
            new_angle: 0.0,
            new_as_built: false,
            pick_faces: false,
            ground_pick: false,
            anchor_mode: 0,
            pick_first: None,
            edit: None,
            creating: false,
            edit_repick: None,
            axis_pick: None,
            conn_pick: false,
            relation_pick: None,
            group_pick: None,
            width_pick: None,
            tangent_pick: None,
            giz_drag: None,
            giz_handle: None,
        }
    }
}

/// THE SECTION TOOL: the cutting plane and its gizmo editing.
#[derive(Clone, Default)]
pub struct SectionTool {
    /// the plane, as (point, normal); None means the section is off
    pub plane: Option<([f64; 3], [f64; 3])>,
    /// the plane is being picked by a click
    pub pick: bool,
    /// shift along the normal and the tilts (deg about U and V)
    pub offset: f64,
    pub rot: [f64; 2],
    /// the gizmo arrow is being dragged; the drag anchor
    pub drag: bool,
    pub drag_anchor: Option<(f64, Pos2)>,
}

impl SectionTool {
    /// Turn the section off: no plane, no pick, no drag.
    pub fn put_away(&mut self) {
        (self.plane, self.pick, self.drag, self.drag_anchor) = (None, false, false, None);
    }
}

/// THE HOLE COMMAND: the kind, how it is placed, and the sketch holding the points.
#[derive(Clone, Copy, Default, Debug)]
pub struct HoleCommand {
    /// kind: simple / counterbore / countersink
    pub kind: u8,
    /// placement: on a face or on sketch points
    pub mode: u8,
    pub sketch: Option<Id>,
    /// drill the other way
    pub flip: bool,
}

/// CHAMFER PARAMETERS: an asymmetric chamfer is measured FROM A REFERENCE FACE, so "which face is the
/// reference" and "are we picking it right now" must live together with the mode itself — otherwise the
/// mode is asymmetric and there is no reference face.
#[derive(Clone, Copy, Default)]
pub struct ChamferParams {
    pub mode: qymcad_core::feature::ChamferMode,
    pub flip: bool,
    pub ref_face: u32,
    pub pick_ref: bool,
}

/// DRAFT PARAMETERS: the neutral face (the angle is measured from it) and the direction.
#[derive(Clone, Copy, Default, Debug)]
pub struct DraftParams {
    pub neutral: u32,
    pub pick_neutral: bool,
    pub flip: bool,
}

/// PRIMITIVE PARAMETERS: how many sides (for a prism) and where it goes.
#[derive(Clone, Copy, Default, Debug)]
pub struct PrimParams {
    pub n: u32,
    pub place: Option<[f64; 3]>,
    pub frame: Option<[f64; 12]>,
}

/// SKETCH PATTERN PARAMETERS (a tool, not a feature): the kind, editing an existing one, the centre.
#[derive(Clone, Copy, Default)]
pub struct PatternTool {
    pub edit: Option<usize>,
    pub center: Option<Point2>,
}

/// THE COMPONENT GIZMO IN AN ASSEMBLY: what is being dragged, along which axis or ring.
#[derive(Clone, Copy, Default)]
pub struct CompGizmo {
    pub drag: Option<(Id, [f64; 12], [f64; 3], f64)>,
    pub snap: bool,
    pub axis: Option<u8>,
    pub ring: Option<u8>,
}

/// TYPING A ROTATION ANGLE: the value, the buffer and the focus — one unfinished input.
#[derive(Clone, Default)]
pub struct RotInput {
    pub angle: f64,
    pub buf: String,
    pub focus: bool,
}

/// WHAT IS BEING DRAGGED IN THE SKETCH RIGHT NOW.
///
/// AN ENUM, NOT SIX `Option`s: exactly ONE THING can be dragged. As separate fields a pair of them
/// could end up occupied at once, and the drag handlers fought over a single pointer — a state that
/// never happens, but which the type allowed.
#[derive(Clone, Default, PartialEq)]
pub enum Dragging {
    #[default]
    None,
    /// a sketch point, as (sketch index, point index)
    Point(usize, usize),
    /// a spline handle, as (sketch, spline, node)
    Handle(usize, usize, usize),
    /// moving the whole selected geometry
    Move(usize, Vec<Id>),
    /// a dimension label / a note / a text
    Dim(usize),
    Note(usize),
    Text(usize),
}

impl Dragging {
    /// Is anything being dragged?
    pub fn active(&self) -> bool {
        *self != Dragging::None
    }

    pub fn clear(&mut self) {
        *self = Dragging::None;
    }

    pub fn pt(&self) -> Option<(usize, usize)> {
        match *self {
            Dragging::Point(a, b) => Some((a, b)),
            _ => None,
        }
    }

    pub fn handle(&self) -> Option<(usize, usize, usize)> {
        match *self {
            Dragging::Handle(a, b, c) => Some((a, b, c)),
            _ => None,
        }
    }

    pub fn mov(&self) -> Option<(usize, Vec<Id>)> {
        match self {
            Dragging::Move(a, ids) => Some((*a, ids.clone())),
            _ => None,
        }
    }

    pub fn dim(&self) -> Option<usize> {
        match *self {
            Dragging::Dim(i) => Some(i),
            _ => None,
        }
    }

    pub fn note(&self) -> Option<usize> {
        match *self {
            Dragging::Note(i) => Some(i),
            _ => None,
        }
    }

    pub fn text(&self) -> Option<usize> {
        match *self {
            Dragging::Text(i) => Some(i),
            _ => None,
        }
    }
}

/// WHAT A GIZMO STANDS ON, for a number typed at it: a body in a part (by its mesh), or a part in an assembly.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum GizmoOf {
    Body(usize),
    Part(Id),
}

/// THE BODY GIZMO: moving or rotating a body with the mouse, plus typing the same value as a number.
/// The same pairing as `CompGizmo` has for a component: "what is being dragged" and "what is being
/// typed" are one intent, and as separate fields the input could outlive the release of the gizmo.
#[derive(Clone, Default)]
pub struct BodyGizmo {
    pub axis: Option<u8>,
    pub ring: Option<u8>,
    pub drag: Option<(usize, [f64; 3], f64)>,
    pub snap: bool,
    /// the gizmo itself is being dragged (an arrow or a ring has been grabbed)
    pub dragging: bool,
    /// typed input: (what the gizmo stands on, axis, is it a rotation?) plus its buffer and focus
    pub num: Option<(GizmoOf, u8, bool)>,
    pub num_buf: String,
    pub num_focus: bool,
}

/// IN-PLACE EDITING OF A SKETCH ELEMENT: what exactly is being edited right on the canvas. Exactly one
/// thing is edited at a time — hence an enum rather than four `Option`s.
#[derive(Clone, Copy, Default, PartialEq)]
pub enum InlineEdit {
    #[default]
    None,
    Note(usize),
    Text(usize),
    Dim(usize),
    Circle(Id),
}

impl InlineEdit {
    pub fn note(&self) -> Option<usize> {
        match *self {
            InlineEdit::Note(i) => Some(i),
            _ => None,
        }
    }
    pub fn text(&self) -> Option<usize> {
        match *self {
            InlineEdit::Text(i) => Some(i),
            _ => None,
        }
    }
    pub fn dim(&self) -> Option<usize> {
        match *self {
            InlineEdit::Dim(i) => Some(i),
            _ => None,
        }
    }
    pub fn circle(&self) -> Option<Id> {
        match *self {
            InlineEdit::Circle(i) => Some(i),
            _ => None,
        }
    }
    pub fn clear(&mut self) {
        *self = InlineEdit::None;
    }
}

/// EDITING AN ANNOTATION (a note or a text) in a sketch: what is being edited and what has been typed.
#[derive(Clone, Default)]
pub struct AnnotEdit {
    pub note: Option<usize>,
    pub note_buf: String,
    pub text: Option<usize>,
    pub text_buf: String,
    pub text_h: f64,
    /// The text field of the popup has just been opened and asks for the caret.
    ///
    /// Without it the popup opened with the focus nowhere: a person had to click into the field before
    /// typing, and Enter - which is what applies the change - did nothing at all until they did.
    pub text_focus: bool,
}

/// THE MEASURING TOOL: whether it is on, together with the points collected — one state, not a flag
/// apart from the points (otherwise the points outlive the switch-off and pop up the next time it is
/// turned on).
#[derive(Clone, Default)]
pub struct Measuring {
    pub pts: Vec<Point2>,
}

impl Measuring {
    pub fn clear(&mut self) {
        *self = Self::default();
    }
}

/// AN IMPORT WAITING TO BE PLACED: the curves have been read, but where they go has not been said yet.
/// Together with the drawing points that came from the same import, this is one unfinished action.
#[derive(Clone, Default)]
pub struct PendingImport {
    /// the curves read, their source and the file name
    pub curves: Option<(Vec<qymcad_core::geom::ProfEdge>, Option<Id>, String)>,
    /// the points to be drawn once it has been placed
    pub draw_pts: Option<Vec<Point2>>,
}

impl PendingImport {
    pub fn clear(&mut self) {
        *self = Self::default();
    }
}

/// A FILE READ AND LAID IN, WAITING FOR ITS UNITS AND SCALE. The bodies are already in the document inside an open
/// edit, so the answer shows on screen as it is given; Enter closes the edit, Esc rolls it back.
pub struct ImportScale {
    /// The file's name, for the window's line.
    pub file: String,
    /// The format's name: the key the unit last chosen for it is remembered under.
    pub format: String,
    /// The file carries no unit (STL, OBJ, PLY): the window offers units, not only a factor.
    pub unitless: bool,
    /// The factor asked for, from the file's own numbers.
    pub factor: f64,
    /// The factor the bodies in the document are at now.
    pub applied: f64,
    /// The sides of the box of everything that came in, as the file has it.
    pub span: [f64; 3],
    /// The body of every mesh that came in: the document scales a piece itself, from the factor it stands at.
    pub meshes: Vec<Id>,
    /// Every solid that came in: its body, and the shape as read.
    pub solids: Vec<(Id, qymcad_kernel::Shape)>,
    /// Every component that came in with the file's tree: its id, and where the file places it.
    pub places: Vec<(Id, [f64; 12])>,
    /// The factor the import stood at when it was asked about again from its node; `None` for a file just read. The
    /// window then applies a new factor rather than bringing a file in, and Esc puts this one back.
    pub again: Option<f64>,
}

/// THE SKETCH WORKING SESSION: which one is open for editing and which one was the last. Kept apart,
/// "open" could be cleared while "last" still held a different sketch — and the next command took hold
/// of the wrong one.
#[derive(Clone, Copy, Default)]
pub struct SketchSession {
    /// the sketch open for editing (None means not in sketch mode)
    pub editing: Option<Id>,
    /// the last one worked on (for commands that act "on the last sketch")
    pub last: Option<Id>,
}

/// THE CLIPBOARD: sketch geometry and tree nodes are different things, but they share one state of "what has been copied".
#[derive(Clone, Default)]
pub struct Clipboard {
    pub geom: Option<qymcad_core::model::GeomClip>,
    pub geom_pending: Option<(Vec<Id>, bool)>,
    /// THE PLACE OF A PASTE IS AWAITED: a ghost follows the pointer, a click places it. A copy or a cut waits for its
    /// base point alone and fills the clipboard (`geom_pending`); nothing follows it until a paste.
    pub geom_place: bool,
    pub tree: Option<TreeClip>,
    pub tree_multi: Option<(Vec<Id>, bool)>,
    pub os_ping: bool,
}

impl Clipboard {
    /// A COPY (or with `cut`, a cut) OF THE GEOMETRY `eids` taken: its base point is awaited; a paste waiting before it
    /// goes.
    pub fn arm_copy(&mut self, eids: Vec<Id>, cut: bool) {
        (self.geom_pending, self.geom_place) = (Some((eids, cut)), false);
    }

    /// An unfinished copy or paste of geometry put down.
    pub fn drop_geom(&mut self) {
        (self.geom_pending, self.geom_place) = (None, false);
    }
}

/// The orbit camera for the software 3D view (the projection comes from `Settings::projection`).
#[derive(Clone, Copy)]
pub struct Cam3 {
    pub yaw: f64,
    pub pitch: f64,
    pub scale: f32,
    pub target: [f64; 3],
    pub init: bool,
    /// The scale the view was last framed at. The wheel's limits are taken from it, so a model of 38 m and one of
    /// 0.1 mm both zoom on from where they were framed.
    pub fit: f32,
    /// THE TILT about the line of sight, in radians: turned by the tilt of the Gesture layout, back to nothing with any
    /// standard view of the cube.
    pub roll: f64,
}

impl Default for Cam3 {
    fn default() -> Self {
        // Front isometric: the camera sits at +X -Y +Z (front = -Y towards the viewer), the usual choice.
        // +Y runs up and away, so the "top" of a sketch on the XY plane matches the "top" in 3D.
        Self { yaw: -0.7, pitch: 0.6, scale: 4.0, target: [0.0; 3], init: false, fit: 4.0, roll: 0.0 }
    }
}

impl Cam3 {
    pub fn basis(&self) -> ([f64; 3], [f64; 3], [f64; 3]) {
        let dir = [self.pitch.cos() * self.yaw.cos(), self.pitch.cos() * self.yaw.sin(), self.pitch.sin()];
        let fwd = [-dir[0], -dir[1], -dir[2]];
        // TOP AND BOTTOM VIEWS (fwd parallel to Z): world Z as the "up" gives a zero cross product, so the
        // basis degenerates and the picture collapses. In that case world Y is taken as the reference
        // "up" (the ViewCube's top no longer breaks the view).
        let ref_up = if fwd[2].abs() > 0.999 { [0.0, 1.0, 0.0] } else { [0.0, 0.0, 1.0] };
        let right = v_norm(v_cross(fwd, ref_up));
        let up = v_norm(v_cross(right, fwd));
        if self.roll == 0.0 {
            return (right, up, fwd);
        }
        let (c, sn) = (self.roll.cos(), self.roll.sin());
        let turn = |a: [f64; 3], b: [f64; 3], ka: f64, kb: f64| [a[0] * ka + b[0] * kb, a[1] * ka + b[1] * kb, a[2] * ka + b[2] * kb];
        (turn(right, up, c, sn), turn(up, right, c, -sn), fwd)
    }
}

/// THE STATE OF THE HELP WINDOW, gathered out of the pile of window flags.
///
/// Four fields that only ever move together - open, which article, where Back leads, what is typed into
/// the search. In the pile they were four unrelated names among fifteen, and nothing said they belonged
/// to one another.
#[derive(Default)]
pub struct HelpWin {
    pub open: bool,
    pub article: String,
    pub back: Vec<String>,
    pub query: String,
}

impl HelpWin {
    /// Step back to the article visited before this one, if there is one.
    pub fn go_back(&mut self) {
        if let Some(prev) = self.back.pop() {
            self.article = prev;
        }
    }

    /// Whether there is somewhere to go back to.
    pub fn can_go_back(&self) -> bool {
        !self.back.is_empty()
    }
}

/// One editable dimension of the active Part command: the label + the key (for `feat_dims`) + the current value +
/// the input buffer (a number OR an expression like `w/2+3`). An on-screen field like the sketch dimensions have.
#[derive(Clone)]
pub struct CmdParam {
    /// A CATALOGUE KEY rather than a finished phrase: the field's label must follow a change of language by itself,
    /// without the command being recreated. The key is stable; the translation is taken at the moment of display.
    pub label: LabelKey,
    /// The dimension's key. A STRING rather than a `&'static str`: a variable fillet has as many fields as there
    /// are vertices in the table, and each has a key of its own of the form `at{vertex descriptor}`. That cannot be
    /// expressed as a list of constants, and a second field mechanism for one tool would fork into copies what
    /// already works: the popup, the expressions, the parametrics, Enter and Esc.
    pub key: String,
    pub val: f64,
    pub txt: String,
    pub lo: f64,
    pub hi: f64,
    /// ZERO IS NO VALUE for this field, though both signs are: a face pushed by 0 is the body unchanged.
    pub nonzero: bool,
    /// WHERE THIS FIELD LIVES IN SPACE. `None` means the command's shared popup; a point means a small window of
    /// its own at that place (a radius at a vertex is shown AT THE VERTEX, otherwise six identical fields in a
    /// column cannot be told apart).
    pub at: Option<[f64; 3]>,
}

impl CmdParam {
    /// The KEY of the field's label. The words are chosen by whoever draws it - a record of the
    /// interface holds keys and never reaches for the dictionary.
    pub fn label_key(&self) -> &'static str {
        self.label.0
    }

    /// Change the label on the fly (on a chamfer the second leg becomes an angle when the mode changes).
    pub fn set_label(&mut self, key: &'static str) {
        self.label = LabelKey(key);
    }

    pub fn new(label: &'static str, key: &str, val: f64, lo: f64, hi: f64) -> Self {
        Self { label: LabelKey(label), key: key.to_string(), val, txt: format!("{val:.2}"), lo, hi, at: None, nonzero: false }
    }

    /// A field AT THE GEOMETRY: the same thing, but with a place of its own in space.
    pub fn at(mut self, p: [f64; 3]) -> Self {
        self.at = Some(p);
        self
    }

    /// A field for which zero is no value.
    pub fn nonzero(mut self) -> Self {
        self.nonzero = true;
        self
    }
}

/// A READY PIECE OF THE SCENE BUFFER FOR ONE BODY.
///
/// THE KEY IS SPLIT IN TWO, AND THAT IS THE WHOLE POINT. `shape` is about the body's FORM and appearance (the
/// mesh, the highlight, the ghost state, the colour, the display settings, the cutting plane); `at` is where the
/// body stands. The position used to be part of one shared key, and MOVING a part declared the block stale
/// entirely: a drag rebuilt 63 blocks out of 138 (the driven side was a gantry subassembly), and rebuilding a
/// block means clipping every triangle by the section plane, world normals and three colours per triangle.
///
/// A move changes NEITHER THE FORM NOR THE COLOUR. If the part is oriented as before and merely shifted, the
/// ready vertices need only have the difference added - one addition per vertex instead of the whole
/// manufacture. A rotation and a section cannot be carried over that way (the colour is computed from the WORLD
/// normal and the section cuts in world space), so there the block is built anew.
pub struct SceneBlock {
    /// The key of the FORM and appearance - everything except the position.
    pub shape: u64,
    /// The position these vertices already stand at.
    pub at: [f64; 12],
    /// The number these vertices name: the body's row in the look table the last time the block was used.
    pub body: u32,
    /// SHARED, NOT OWNED OUTRIGHT: the frame takes the block as it is, without copying a single vertex.
    ///
    /// Every rebuild of the scene used to glue all the blocks into one vector purely to hand it to the
    /// upload - a second full copy of the scene, 739 MB of it on the reference engine, at the worst possible
    /// moment. Now the pieces are uploaded one after another, each straight from its block.
    ///
    /// A move still shifts the vertices in place: after the frame has been drawn the block is the only owner,
    /// so `Arc::make_mut` copies nothing.
    /// ALL the geometry of the body. It used to be split into opaque and translucent here, which tied the
    /// DATA to the look: a body turning into a ghost had to be rebuilt to move its vertices into the other
    /// half. The pass is chosen when drawing, from the look table.
    ///
    /// Several parts only when the body is too big for one buffer of the card (see `BLOCK_VERTEX_CAP`);
    /// ordinarily there is exactly one.
    pub parts: Vec<BlockPart>,
}

/// ONE PIECE OF A BODY as the card takes it: the vertices, and the triangles as indices into them.
///
/// A VERTEX OF THE BLOCK IS A VERTEX OF THE MESH, one for one. Each used to be written out three times over -
/// once per triangle that touches it - because the colour and the normal made neighbours differ; now the
/// vertex carries only a position and the number of its body, so the triangles simply point at it.
/// Measured on a closed mesh: about three times fewer vertices, and 4 bytes of index against 20 of vertex.
///
/// The exception is the vertices BORN OF A SECTION: clipping a triangle by the plane makes points that are in
/// no mesh. They are appended at the end of the block and indexed in order.
#[derive(Clone)]
pub struct BlockPart {
    pub verts: std::sync::Arc<Vec<GpuVert>>,
    pub idx: std::sync::Arc<Vec<u32>>,
}

/// HOW BIG ONE PART OF A BLOCK MAY GROW, in vertices and in indices.
///
/// An indexed draw reads from ONE buffer, so a part must fit inside a single piece of the scene, and a piece
/// may not exceed the device's limit on a buffer. The smallest such limit met in practice is 256 MB: 4.19
/// million vertices at 20 bytes is 84 MB, and 16.7 million indices at 4 bytes is 67 MB, so both stay well
/// inside it on any device. A body bigger than that is cut into parts when the block is built.
pub const BLOCK_VERTEX_CAP: usize = 1 << 22;
/// The companion of `BLOCK_VERTEX_CAP` for the triangles: a mesh has about twice as many triangles as
/// vertices, so the indices are the number that runs out first.
pub const BLOCK_INDEX_CAP: usize = 1 << 24;

/// The extent of an extrude or a cut (in place of the magic numbers 0 to 3). The discriminants are kept as the
/// former u8 in case of outside places, but the comparisons go by variant.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum ExtentMode {
    #[default]
    Length, // to a length (one side)
    Symmetric,
    TwoSided, // two sides (the "down" field is active)
    Through,  // through all (operations on a body only)
}

impl ExtentMode {
    pub fn symmetric(self) -> bool {
        matches!(self, ExtentMode::Symmetric)
    }
    pub fn two_sided(self) -> bool {
        matches!(self, ExtentMode::TwoSided)
    }
    pub fn through(self) -> bool {
        matches!(self, ExtentMode::Through)
    }
    /// Restore the mode from a saved feature's flags (through takes priority, then symmetry, then two-sided).
    pub fn from_extent(reach: qymcad_core::feature::Reach, down: f64, through: bool) -> Self {
        if through {
            ExtentMode::Through
        } else if reach == qymcad_core::feature::Reach::BothWays {
            ExtentMode::Symmetric
        } else if down.abs() > 1e-9 {
            ExtentMode::TwoSided
        } else {
            ExtentMode::Length
        }
    }
}

/// The TREE clipboard: what was copied or cut (by stable Id) and which of the two it was.
#[derive(Clone, Copy, PartialEq)]
pub enum TreeClip {
    /// A sketch node (by the sketch's Id).
    Sketch { sid: Id, cut: bool },
    /// A component - a Part or a subassembly (by the component's Id).
    Component { id: Id, cut: bool },
    /// A node of a part's timeline (by the node's Id): pasting opens its tool with its values, to be placed anew.
    Feature { nid: Id },
}

/// A reference for the dimension tool: a point or a straight line (a line or an axis). A dimension between two
/// references: point to point, point to line (a perpendicular), line to a parallel line, or to an axis.
#[derive(Clone, Copy, PartialEq)]
pub enum DimRef {
    Point(Id),
    Line(Id, Id),
}

/// A base edge for building tangents (a tangent arc or a tangent circle): either a straight line (with ends a
/// and b) or a circle or arc (a centre plus a radius).
#[derive(Clone, Copy, PartialEq)]
pub enum EdgeRef {
    Line { a: Id, b: Id },
    Circle { center: Id, r: f64 },
}

/// The sketch's diagnostics in one piece: the degrees of freedom and the redundancy, the free points, THE ARGUING
/// SET of constraints and the redundant ones. Computed in a single pass in `sketch_diag`.
/// The diagnostics of one sketch and what they were computed for.
pub struct SketchStatus {
    pub si: usize,
    /// `sketch_shape_key` at the time
    pub shape: u64,
    /// `sketch_place_key` at the time
    pub place: u64,
    pub diag: SketchDiag,
}

#[derive(Clone, Default)]
pub struct SketchDiag {
    /// (the degrees of freedom, the redundancy)
    pub dof: (i32, i32),
    /// per sketch point: whether that point can move
    pub free: Vec<bool>,
    /// the indexes of the constraints that contradict one another TOGETHER - any one of the set may be removed
    pub conflicts: std::collections::HashSet<usize>,
    /// the indexes of the constraints whose removal does not raise the degrees of freedom (consistent redundancy)
    pub redundant: std::collections::HashSet<usize>,
}

/// AN UNDO SNAPSHOT IS ONE STATE OF THE DOCUMENT.
///
/// A snapshot used to be assembled from three pieces (the project, the faces and the visibility), while the live
/// B-rep was not part of it at all and was restored by a separate mechanism. While the state is smeared about, an
/// undo regularly under-rolls something: after Ctrl+Z the mesh showed the old geometry while the kernel's cache
/// held the new one. The faces and the visibility now live in the body itself, and a snapshot is the whole project.
#[derive(Clone)]
pub struct Snapshot {
    pub project: Project,
}

/// AN UNFINISHED DRAWING: which shape is being finished right now.
///
/// AN ENUMERATION RATHER THAN THREE `Option`s: exactly ONE THING is ever being drawn, and the type must say so.
/// While these were three independent fields, "a rectangle and an ellipse at once" remained a possible state -
/// held off by discipline in the code rather than by the type system.
#[derive(Clone, Default, PartialEq)]
pub enum PlacingShape {
    #[default]
    None,
    /// a rectangle: two corners and the entities it created
    Rect { a: Point2, b: Point2, ids: Vec<Id> },
    /// a rectangle drawn from the centre: the centre stays put while the width and height are typed,
    /// the corner under the pointer follows them
    RectCenter { center: Point2, corner: Point2, ids: Vec<Id> },
    /// a polygon by its inscribed or circumscribed circle
    Poly(Id),
    /// an ellipse: the entity and its centre
    Ellipse(Id, Point2),
    /// a rectangle reopened by a double click on a side: its width and height dimensions, edited in place
    RectDims(qymcad_core::model::RectDims),
}

/// THE CUSTOM SCHEME SCREEN: what is being edited and what to say about saving.
///
/// The edits go in LIVE - a colour is seen at once, with no Apply. Otherwise choosing a shade would be blind:
/// close the window, look, come back. So only the name of the scheme being edited and a note about the last save
/// live here; the colours themselves live in the scheme, which is already gathered in the list.
#[derive(Clone, Default)]
pub struct SchemeEdit {
    /// whether the colour editor is open
    pub open: bool,
    /// the name in the rename field (empty means no rename is in progress)
    pub rename: String,
    /// what to say about the last save or error
    pub note: String,
}

/// A RELATION BEING PICKED: its kind, the joints already pointed at with their slots, and the number.
///
/// The slot is not picked separately by hand: the kind itself says what is what
/// (`qymcad_core::feature::RelationKind::slots_are_rotations`), and the matching degree of freedom is taken from the joint
/// pointed at. A separate dialogue for choosing the degree is only warranted where a joint has several
/// degrees of the matching sort.
#[derive(Clone)]
pub struct RelationPick {
    pub kind: qymcad_core::feature::RelationKind,
    /// already pointed at, as (joint, slot), in order
    pub picks: Vec<(Id, usize)>,
    /// the relation's number: a gear ratio or a travel per turn, depending on the kind
    pub value: f64,
    pub reversed: bool,
}

impl RelationPick {
    /// Set the kind and the number of the relation being built.
    ///
    /// CHANGING THE KIND CLEARS THE PICKS: the degrees already pointed at were of the right sort for the
    /// PREVIOUS kind, and keeping them would build the relation on the wrong ones. That rule was written
    /// out twice - in the popup and beside it - and two copies of a rule drift.
    pub fn set(&mut self, kind: qymcad_core::feature::RelationKind, value: f64) {
        if self.kind != kind {
            self.picks.clear();
        }
        self.kind = kind;
        self.value = value;
    }
}

impl Default for RelationPick {
    fn default() -> Self {
        Self { kind: qymcad_core::feature::RelationKind::Gear, picks: Vec::new(), value: 1.0, reversed: false }
    }
}

/// AN UNDO STEP IS AN OPERATION. Not "the document changed somehow" but "an Extrude was carried out":
/// a step has a name, and that name is shown. The stack used to hold nameless snapshots, because the
/// boundary of a step was decided by THE FRAME rather than by the command.
pub struct Step {
    pub name: String,
    pub snap: Snapshot,
}

/// The state of the "Save as a standard part" dialogue (right-click on a component in the tree).
/// The manifest metadata, plus the chosen category (a path relative to the user's directory), plus a
/// pre-rendered PNG preview of the body. Writing it out goes through `subproject_of` and `save_part`.
pub struct SavePartDialog {
    /// The source component (a Part or a subassembly) the standard part is taken from.
    pub component: qymcad_core::model::Id,
    pub name: String,
    pub description: String,
    /// Tags separated by commas or spaces (parsed when written out).
    pub tags: String,
    /// The category path relative to `<data>/library/parts` (e.g. "Profiles/Aluminium"). Empty = the root.
    pub category: String,
    /// The user's existing categories (folders) — for picking one quickly from chips.
    pub known_cats: Vec<String>,
    /// The raw preview of the body (256^2, rendered at the moment it opened). Encoded to PNG when written out.
    pub preview: Option<egui::ColorImage>,
    /// The GPU texture of the preview (loaded lazily from `preview` to show it in the window).
    pub tex: Option<egui::TextureHandle>,
}

/// The contour slot of a sweep or a loft for which a contour is being picked in the half-sketcher (as in
/// Extrude: the sketch lies flat, a contour is clicked). The sweep profile, the sweep path, or the i-th
/// section of a loft.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ContourSlot {
    SweepProfile,
    SweepPath,
    LoftSection(usize),
}

/// The active drag of a joint gizmo's handle. The frame (`o` and `dir`) is fixed when the drag starts.
#[derive(Clone, Copy)]
pub struct JointGizDrag {
    pub jid: Id,
    pub slot: u8,   // 0 = angle, 1 = offset, 2 = offset2
    pub ring: bool, // true means a rotation about `dir`, false a translation along it
    pub start: f64, // the parameter's value when the drag started
    pub amt: f64,   // accumulated so far (degrees or millimetres)
    pub o: [f64; 3],
    pub dir: [f64; 3],
}

/// What the click pick of a circular pattern's axis caught: a datum AXIS, a STRAIGHT edge (an index into
/// `edge_polys`), or the axis of a CYLINDRICAL or conical face (the body plus the face's persistent id).
#[derive(Clone, Copy, PartialEq)]
pub enum AxisHit {
    Datum(Id),
    Edge(usize),
    Face(Id, u32),
}

/// A CATALOGUE KEY WRAPPED SO THAT IT CANNOT BE DRAWN.
///
/// Reported behaviour: the thread popup showed `f-nominal-d`, `f-pitch-std` and `f-length` in place of the labels -
/// keys instead of words. The cause was exactly one character: the field was drawn as `ui.label(p.label)` instead
/// of `p.label()`. The key was a plain `&'static str`, and `ui.label` accepts a `&str` - the compiler said nothing
/// and the internal name went to the screen.
///
/// The wrapper does not implement `Into<WidgetText>`, so `ui.label(p.label)` no longer compiles at all. This is not
/// decoration: a translation that rests on attentiveness at every edit ends exactly the way it ended here.
#[derive(Clone, Copy)]
pub struct LabelKey(pub(crate) &'static str);

/// A piece of a mesh file on its way into the document: a piece as the file names it (see `qymcad_io::NamedMesh`) with
/// the faces found on its mesh.
pub struct MeshPiece {
    pub name: String,
    pub mesh: qymcad_core::geom::Mesh,
    pub faces: Vec<qymcad_core::geom::MeshFace>,
    pub color: Option<[u8; 3]>,
    pub place: [f64; 12],
    /// the colour of every triangle, where the file gives one
    pub tri_colors: Vec<[u8; 3]>,
    /// the groups of the file the piece stands in, from the top down
    pub within: Vec<qymcad_core::model::FileGroup>,
}

/// The key of an action WITH REBINDING TAKEN INTO ACCOUNT: the one set by a person if there is
/// one, otherwise the factory key.
/// A BODY'S EDGES, ready for picking: the polylines and the persistent id of each.
///
/// Two parallel lists that are only ever used together, and were a bare pair inside an `Rc`.
#[derive(Default)]
pub struct EdgePolys {
    /// Each edge as a polyline in world space.
    pub polys: Vec<Vec<[f32; 3]>>,
    /// The persistent id of each, parallel to `polys`.
    pub ids: Vec<u32>,
    /// Whether each is smooth - a seam with the same face on both sides, or the tangent edge of a fillet - parallel
    /// to `polys`. Its ends are no corners a person sees.
    pub smooth: Vec<bool>,
}

/// A VALUE KEPT UNTIL THE THING IT WAS COMPUTED FROM CHANGES.
///
/// Nine cache fields were `(u64, T)`, and every one of them carried a doc comment explaining that the first
/// place is the revision - `(the revision, the bodies)`, `(the geometry revision, a body mapped to ...)`.
/// A comment saying what a type should say is a comment nothing checks. Two named fields say it once.
#[derive(Clone, Copy)]
pub struct Cached<T> {
    /// The revision the value was computed at. When it differs from the current one the value is stale.
    pub rev: u64,
    /// What was computed.
    pub value: T,
}

impl<T: Default> Default for Cached<T> {
    fn default() -> Self {
        Cached { rev: 0, value: T::default() }
    }
}

impl<T> Cached<T> {
    /// The value if it was computed at THIS revision; `None` means it has to be recomputed.
    pub fn get(&self, rev: u64) -> Option<&T> {
        (self.rev == rev).then_some(&self.value)
    }
    /// Keep a freshly computed value under the revision it belongs to.
    pub fn put(&mut self, rev: u64, value: T) {
        self.rev = rev;
        self.value = value;
    }
}

/// THE DOCUMENT UNDER THE HAND: the five things every editing gesture reads and writes.
///
/// The document itself; what has gone out of date in it; what is selected; the line the user is told;
/// the flat view. Eight of the thirteen workbench contexts carry exactly these five and nothing said
/// they belonged together, so eight functions took them one by one - `place_input_popup` reached eleven
/// arguments that way.
pub struct Editing<'a> {
    pub project: &'a mut qymcad_core::model::Project,
    /// The undo journal: an edit of the sketch is one step of it, named after its tool.
    pub edits: &'a mut Edits,
    pub regen: &'a mut Rebuilding,
    pub sel: &'a mut Sel,
    pub status: &'a mut String,
    pub view: &'a mut View2d,
}

/// TURN THE SELECTED LINES, ARCS AND CIRCLES INTO CONSTRUCTION GEOMETRY, or back: one named step of undo, the words said.
/// The button and the X key both come here. False when nothing of the sketch being edited is selected - the button then
/// switches what is drawn next instead.
pub fn construction_selected(ed: Editing, sel_sk: &SketchSelection, sketch_ses: &SketchSession) -> bool {
    let Sel::Sketch(si) = *ed.sel else { return false };
    if edit_si(ed.project, sketch_ses) != Some(si) {
        return false;
    }
    let eids: Vec<Id> = sel_sk.items.iter().filter(|(k, _)| *k == 1).map(|(_, id)| *id).collect();
    if eids.is_empty() {
        return false;
    }
    begin_edit(ed.edits, ed.project, qymcad_i18n::tr("tool-construction"));
    let now = ed.project.toggle_construction(si, &eids);
    ed.project.solve_sketch(si);
    invalidate(ed.regen);
    *ed.status = qymcad_i18n::tr(if now { "in-made-construction" } else { "in-made-normal" });
    close_edit(ed.edits, ed.project);
    true
}

/// THE CONSTRUCTION TOGGLE, by the button of the bar or by its key X - one rule for both: what is selected is turned
/// into construction geometry or back, and with nothing selected the kind of what is drawn next is switched. The key
/// switched the drawing mode whatever was selected: a line selected stayed as it was, and the next line came out
/// construction.
pub fn construction_toggle(ed: Editing, sel_sk: &SketchSelection, sketch_ses: &SketchSession, drawing_construction: &mut bool) {
    if !construction_selected(ed, sel_sk, sketch_ses) {
        *drawing_construction = !*drawing_construction;
    }
}

impl Editing<'_> {
    /// A shorter borrow of the same five, so one gesture can hand them on to another.
    pub fn reborrow(&mut self) -> Editing<'_> {
        Editing { project: self.project, edits: self.edits, regen: self.regen, sel: self.sel, status: self.status, view: self.view }
    }
}

/// WHAT IS IN HAND. Exactly one thing, because it is exactly one field.
///
/// Eight tools used to say "I am the one in hand" through eight separate fields in five records - a zero in
/// `tool.kind`, a zero in `dim.kind`, a `false` in `measure.on`, and so on. That two of them could be
/// non-zero at once was a rule held by checks, and the rule was broken three times: an anchor re-pick no
/// door released, a ruler that armed and disarmed itself in one frame, and an array re-opened by double
/// click over whatever was already in hand. Each time the click went to whichever handler stood higher in
/// the code, and the person was certain they were working with the last tool taken.
///
/// One field cannot hold two values. The payload of the tool - which shape is being drawn, which points
/// have been picked - stays in its own record; what moved here is only the answer to "which tool".
///
/// FIVE OF THE THIRTEEN STAYED OUTSIDE, and each for a named reason. Four are sub-modes rather than
/// tools: `DrawTool::has_door` says so on the type, and the corner fillet is reached WITH the click
/// tool in hand, not instead of it. The fifth is `Placing`, whose "in hand" would have had to travel
/// into `PlaceCtx` and push `place_input_popup` past seven arguments - one debt traded for another.
#[derive(Clone, Copy, Default, PartialEq, Debug)]
pub enum Armed {
    /// Nothing: clicks go to selection.
    #[default]
    None,
    /// A sketch drawing tool, by its own code: line, circle, arc, rectangle.
    Draw(u8),
    /// Trim, extend, break, corner fillet - the ones that act on a click.
    ClickOp(u8),
    /// Mirror, offset and the rest of the editing buttons.
    Modify(EditTool),
    /// Move, copy, rotate.
    Move(u8),
    /// The sketch pattern: 1 linear, 2 circular.
    Pattern(u8),
    /// The dimension tool, by kind.
    Dimension(u8),
    /// A feature command of the Part (extrude, hole, fillet...), by kind.
    Command(u8),
    /// The measuring tool.
    Measure,
}

impl Armed {
    /// The code of the drawing tool in hand, or zero. Named as the field it replaces, so that a reading
    /// site says the same thing it said before.
    pub fn draw_kind(&self) -> u8 {
        if let Armed::Draw(k) = self {
            *k
        } else {
            0
        }
    }

    /// The code of the click-acting tool, or zero.
    pub fn click_op(&self) -> u8 {
        if let Armed::ClickOp(k) = self {
            *k
        } else {
            0
        }
    }

    /// The editing tool in hand, if one is.
    pub fn modify(&self) -> Option<EditTool> {
        if let Armed::Modify(k) = self {
            Some(*k)
        } else {
            None
        }
    }

    /// The code of the move tool, or zero.
    pub fn move_op(&self) -> u8 {
        if let Armed::Move(k) = self {
            *k
        } else {
            0
        }
    }

    /// The code of the sketch pattern, or zero.
    pub fn pat_op(&self) -> u8 {
        if let Armed::Pattern(k) = self {
            *k
        } else {
            0
        }
    }

    /// The kind of dimension being placed, or zero.
    pub fn dim_kind(&self) -> u8 {
        if let Armed::Dimension(k) = self {
            *k
        } else {
            0
        }
    }

    /// The kind of feature command being run, or zero.
    pub fn cmd_kind(&self) -> u8 {
        if let Armed::Command(k) = self {
            *k
        } else {
            0
        }
    }

    /// Whether a feature command is open.
    pub fn commanding(&self) -> bool {
        matches!(self, Armed::Command(_))
    }

    /// Whether the measuring tool is in hand.
    pub fn measuring(&self) -> bool {
        matches!(self, Armed::Measure)
    }

    /// Whether anything at all is in hand.
    pub fn any(&self) -> bool {
        !matches!(self, Armed::None)
    }

    /// WHICH TOOL, as the checks name them. `None` means the hand is empty.
    ///
    /// At most one, and not by a rule: there is one field, and a field holds one value.
    pub fn tool(&self) -> Option<DrawTool> {
        match self {
            Armed::None => None,
            Armed::Draw(_) => Some(DrawTool::Draw),
            Armed::ClickOp(_) => Some(DrawTool::ClickOp),
            Armed::Modify(_) => Some(DrawTool::Modify),
            Armed::Move(_) => Some(DrawTool::Move),
            Armed::Pattern(_) => Some(DrawTool::Pattern),
            Armed::Dimension(_) => Some(DrawTool::Dimension),
            Armed::Command(_) => Some(DrawTool::Command),
            Armed::Measure => Some(DrawTool::Measure),
        }
    }
}

/// WHICH BODIES ARE ON SCREEN, and in which context they were counted.
///
/// The context is part of the key, not of the answer: stepping into a component changes what is shown
/// without changing the geometry, so the revision alone would hand back the wrong list.
#[derive(Default)]
pub struct ShownBodies {
    /// The assembly whose contents were walked.
    pub ctx: Id,
    /// The mesh index and body id of everything visible.
    pub list: Vec<(usize, Id)>,
}

/// THE PARAMETERS OF THE FEATURE BEING BUILT, whichever it is.
///
/// Fourteen records - the numbers a revolve, a chamfer, a sweep, a loft, a draft, a hole, a primitive, a
/// mirror, a split, an array, a boolean and a thread are waiting for. At most one feature is being built
/// at a time, so at most one of these is meaningful; the rest hold whatever was typed into them last.
///
/// They lived as fourteen fields of `App`, and a reader had to know which of the hundred were the
/// command's and which the document's. `FeatParams` is that answer written down.
/// THE RECOGNITION TOOL IN HAND: the mesh it is aimed at, and what a count of that mesh found - made in the
/// background, for the mesh, the tolerance and the sharp angle it was asked for, so a person sees what Enter will
/// build before building it.
#[derive(Default)]
pub struct RecogniseTool {
    pub src: Option<Id>,
    /// the mesh, the tolerance and the sharp angle (as bits) last sent to be counted
    pub asked: Option<(Id, u64, u64)>,
    pub found: std::sync::Arc<std::sync::Mutex<Option<RecogniseFound>>>,
}

/// What a count of a mesh found: the kind of surface under each triangle (0 plane, 1 cylinder, 2 cone, 3 sphere,
/// 4 torus, 5 none, 6 a free form, 7 a helix) and how many regions of each kind.
pub struct RecogniseFound {
    pub key: (Id, u64, u64),
    pub kinds: Vec<u8>,
    pub counts: [usize; 8],
}

impl RecogniseTool {
    /// What was found for the mesh in hand as it was last asked, once the count is done.
    pub fn ready<T>(&self, read: impl FnOnce(&RecogniseFound) -> T) -> Option<T> {
        let key = self.asked?;
        let found = self.found.lock().ok()?;
        found.as_ref().filter(|f| f.key == key && Some(key.0) == self.src).map(read)
    }
}

pub struct FeatParams {
    pub stitch_parts: Vec<Id>,
    /// the recognition tool in hand: its mesh and what a count of it found
    pub recognise: RecogniseTool,
    pub repl_surface: Option<Id>,
    pub rev: RevolveParams,
    pub chamfer: ChamferParams,
    pub sweep: SweepParams,
    pub loft: LoftParams,
    pub draft: DraftParams,
    pub hole: HoleCommand,
    pub prim: PrimParams,
    pub mirror: MirrorParams,
    pub split: SplitParams,
    pub arr: ArrayParams,
    pub boolean: BoolCommand,
    pub thread: ThreadParams,
}

impl Default for FeatParams {
    /// THE VALUES A COMMAND OPENS WITH, and four of them are not zero.
    ///
    /// They used to sit in `App::default()`, one line per field among a hundred. Moving the fields here and
    /// leaving the values behind cost a run: the array opened with a count of zero, and the ghosts of the
    /// pattern stopped appearing - `#[derive(Default)]` compiled perfectly and meant something else.
    fn default() -> Self {
        FeatParams {
            stitch_parts: Vec::new(),
            recognise: RecogniseTool::default(),
            repl_surface: None,
            // A full turn, so a revolve without touching anything makes a solid of revolution.
            rev: RevolveParams { angle: 360.0, ..Default::default() },
            chamfer: ChamferParams::default(),
            sweep: SweepParams::default(),
            loft: LoftParams::default(),
            draft: DraftParams::default(),
            hole: HoleCommand::default(),
            // A hexagon: the polygon everyone draws first.
            prim: PrimParams { n: 6, ..Default::default() },
            mirror: MirrorParams::default(),
            split: SplitParams::default(),
            // Three copies along the first direction, two along each of the others.
            arr: ArrayParams { count: 3, count2: 2, count3: 2, dir2: 1, dir3: 2, full: true, ..Default::default() },
            boolean: BoolCommand::default(),
            // A single-start thread.
            thread: ThreadParams { starts: 1, ..Default::default() },
        }
    }
}

/// THE VIEW TURNING TO A NEW ANGLE, while it turns.
///
/// It was `Option<((f64, f64), (f64, f64), Instant)>` - two nameless pairs and a moment. Which pair is
/// where the view came FROM and which where it is going TO could only be told from the body of the tick.
#[derive(Clone, Copy)]
pub struct ViewTurn {
    /// Yaw and pitch it started at.
    pub from: (f64, f64),
    /// The tilt it starts at and goes to: a standard view has none, a look at a point keeps what there is.
    pub roll: (f64, f64),
    /// The centre it moves from and to, when it moves one - a look at a point; the view keeps its scale then, and is
    /// not framed again on arrival.
    pub target: Option<([f64; 3], [f64; 3])>,
    /// Yaw and pitch it is going to.
    pub to: (f64, f64),
    /// When it started; the whole turn lasts a fixed time from here.
    pub since: std::time::Instant,
}

/// THE TOOLS THAT ARE NOT DRAWING TOOLS: what the Part and the Assembly hold while one of them is in hand.
///
/// `DrawState` holds the fourteen a sketch draws with. These ten are the rest - trimming, datums, mates,
/// measuring in 3D, the section, the component array, the clipboard, the sketch array, a rotation being
/// typed and a name being typed. At most one is live, same as there; naming the set is what makes that a
/// thing the code can be asked about rather than a rule people keep.
pub struct SideTools {
    /// Trimming a sketch entity by a click.
    pub trim: TrimTool,
    /// A datum being placed: a plane, an axis or a point.
    pub datum: DatumCommand,
    /// The mate being assembled, and every pick of the Assembly.
    pub joint: JointCommand,
    /// The 3D measuring tool.
    pub m3: Measure3,
    /// The section plane and its handle.
    pub section: SectionTool,
    /// A pattern of components being built.
    pub carr: CompArrayCmd,
    /// What was copied, geometry or a component.
    pub clip: Clipboard,
    /// The sketch array's numbers.
    pub array: ArrayTool,
    /// A rotation being typed into.
    pub rot: RotInput,
    /// A name being typed into (a rename in the tree).
    pub rename: RenameInput,
}

impl Default for SideTools {
    /// THE SKETCH ARRAY OPENS WITH THREE COPIES 20 mm APART, and that is written here rather than derived:
    /// zeroed, the tool would offer an array of nothing at no distance, which is not a starting point.
    fn default() -> Self {
        SideTools {
            trim: TrimTool::default(),
            datum: DatumCommand::default(),
            joint: JointCommand::default(),
            m3: Measure3::default(),
            section: SectionTool::default(),
            carr: CompArrayCmd::default(),
            clip: Clipboard::default(),
            array: ArrayTool { n: 3, dx: 20.0, dy: 0.0 },
            rot: RotInput::default(),
            rename: RenameInput::default(),
        }
    }
}

/// WHAT IS CHOSEN AND WHAT IS UNDER THE CURSOR.
///
/// Four fields of `App` that answer one question, and two of them are easy to confuse: `sel` is what the
/// document considers selected, `tree_sel` is what is highlighted in the tree, and the two are not the
/// same thing - a multiple selection in the tree does not make four bodies "the selection".
pub struct Chosen {
    /// THE SELECTION of the document: one body, face, sketch, plane, component or feature.
    pub sel: Sel,
    /// What is picked in the TREE - several rows may be, and that is not the same as `sel`.
    pub tree_sel: TreeSelection,
    /// THE ANCHOR chosen in the connector list: its handles are the ones being edited.
    pub sel_conn: Option<Id>,
    /// What is under the cursor right now.
    pub hover: Hover,
}

impl Default for Chosen {
    /// `Sel` has no derived default on purpose: "nothing is selected" is a named variant, not a zero.
    fn default() -> Self {
        Chosen { sel: Sel::None, tree_sel: TreeSelection::default(), sel_conn: None, hover: Hover::default() }
    }
}

/// WHAT IS BEING DRAGGED IN THE SCENE, and by which handle.
///
/// The gizmos and the two pulls are one subject: at most one of them is live, and each holds what it needs
/// only while the mouse is down.
#[derive(Default)]
pub struct Dragged {
    /// The component gizmo (moving a part in an assembly).
    pub comp_giz: CompGizmo,
    /// The body gizmo (moving a body inside a part).
    pub body_giz: BodyGizmo,
    /// Dragging the rollback bar along the timeline.
    pub rollback: RollbackDrag,
    /// THE HANDLE OF THE COMMAND BEING DRAGGED, by the value it drives (`handles`).
    pub handle_drag: Option<Handle>,
    /// Pulling a part by a degree of freedom: which one, from where, in which direction.
    pub part_pull: Option<(Id, [f64; 3], [f64; 3])>,
}

/// HOW THE SCENE IS BEING LOOKED AT: the camera, the flat view, and everything a turn or a drag of it
/// needs while it lasts.
///
/// Eight fields of `App` that answer one question. `mode_3d` in particular read as a loose flag among a
/// hundred others; it belongs here, beside the two views it chooses between.
pub struct Viewing {
    /// The orbit camera of the 3D view.
    pub cam: Cam3,
    /// The pan and zoom of the flat view.
    pub view: View2d,
    /// WHICH OF THE TWO is on screen.
    pub mode_3d: bool,
    /// The canvas rectangle of the previous frame - the window in the rebuild barrier.
    pub view_rect: egui::Rect,
    /// A turn to a new angle, while it lasts (the ViewCube).
    pub view_anim: Option<ViewTurn>,
    /// What to come back to after a temporary look from somewhere else: the mode, the camera, the flat view.
    pub view_restore: Option<(bool, Cam3, View2d)>,
    /// The camera is being dragged right now: the raster cache is not worth rebuilding at every step.
    pub view_dragging: bool,
    /// Where the view stood before stepping into a component, one entry per step.
    pub nav_stash: Vec<(Cam3, View2d, bool)>,
}

impl Viewing {
    /// End a running turn of the view at once, putting the camera where it was heading.
    pub fn finish_view_anim(&mut self) {
        if let Some(ViewTurn { to, roll, target, .. }) = self.view_anim.take() {
            self.cam.yaw = to.0;
            self.cam.pitch = to.1;
            self.cam.roll = roll.1;
            if let Some((_, t)) = target {
                self.cam.target = t;
            }
        }
    }
}

impl Default for Viewing {
    /// `Rect::NOTHING` rather than a zero rectangle: an empty rect at the origin would read as a canvas
    /// one pixel wide at the corner, and the barrier that uses it would let a frame through.
    fn default() -> Self {
        Viewing { cam: Cam3::default(), view: View2d::default(), mode_3d: false, view_rect: egui::Rect::NOTHING, view_anim: None, view_restore: None, view_dragging: false, nav_stash: Vec::new() }
    }
}

/// THE DRAWING TOOLS AS THE APPLICATION KEEPS THEM: fourteen records that only ever move together.
///
/// They used to be fourteen fields of `App` among a hundred, so "the tools in hand" was a thing one had to
/// know rather than read. `Tools` already named the SET for the functions that take it; this is the same
/// set OWNED, so the application carries one field where it carried fourteen.
///
/// Which of them is live at a time is still a rule rather than a type - see the debt catalogue, D11. The
/// naming is the step that makes the type possible later.
#[derive(Default)]
pub struct DrawState {
    /// WHAT IS IN HAND. One field, so two tools at once cannot be written down.
    pub armed: Armed,
    pub annot: AnnotEdit,
    pub cmd: FeatCommand,
    pub corner: CornerInput,
    pub dim: DimTool,
    pub drag: Dragging,
    pub gsel: GeomSelection,
    pub inline: InlineEdit,
    pub measure: Measuring,
    pub pat: PatternTool,
    pub pending_import: PendingImport,
    pub picking: Picking,
    pub place: Placing,
    pub sel_sk: SketchSelection,
    pub tool: SketchTool,
}

/// THE TOOLS IN HAND: every record a drawing tool keeps while it is being used.
///
/// FOURTEEN ARGUMENTS THAT ALWAYS TRAVELLED TOGETHER. Six functions took exactly this set and nothing
/// else in common - `exit_draw_tools` took the fourteen and no other argument at all, which is as plain a
/// statement as code makes that they are one thing. The widest signature ran to twenty-one places, and
/// telling `pat` from `place` there meant counting commas.
///
/// WHY A RECORD OF BORROWS rather than an owned one: they are fields of the application, and a tool must
/// change them where they live. This is the same shape as the workbench contexts, and it is built the same
/// way - once, at a doorway.
///
/// Only ONE of these is ever live at a time; that rule is held by a check rather than by the type. Naming
/// the set is the first step towards holding it by the type instead.
pub struct Tools<'a> {
    pub armed: &'a mut Armed,
    /// A note or a dimension being typed into.
    pub annot: &'a mut AnnotEdit,
    /// The feature command in progress (extrude, hole, fillet...).
    pub cmd: &'a mut FeatCommand,
    /// The corner tool: fillet or chamfer on a sketch vertex.
    pub corner: &'a mut CornerInput,
    /// The dimension tool.
    pub dim: &'a mut DimTool,
    /// What is being dragged right now.
    pub drag: &'a mut Dragging,
    /// The picked faces, edges and vertices.
    pub gsel: &'a mut GeomSelection,
    /// The value popup at the geometry.
    pub inline: &'a mut InlineEdit,
    /// The measuring tool.
    pub measure: &'a mut Measuring,
    /// The sketch pattern tool.
    pub pat: &'a mut PatternTool,
    /// A part being brought in from the library, waiting for its place.
    pub pending_import: &'a mut PendingImport,
    /// The click-pick mode: what is being pointed at and why.
    pub picking: &'a mut Picking,
    /// Placing a component.
    pub place: &'a mut Placing,
    /// The sketch selection.
    pub sel_sk: &'a mut SketchSelection,
    /// Which drawing tool is chosen.
    pub tool: &'a mut SketchTool,
}

/// ONE DRAWING TOOL THAT CAN BE IN HAND.
///
/// The rule "taking one tool releases the previous" was guarded for the ASSEMBLY tools only
/// (`AssemblyTool`), and the guard there found a real defect the moment it was made exhaustive. The
/// drawing tools - fourteen records in `Tools`, shared by the Sketch and the Part - had no such guard at
/// all. This names them so they can be walked.
///
/// Two tools at once means ambiguity under the cursor: the click goes to whichever handler stands higher
/// in the code, while the person is certain they are working with the one picked last.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DrawTool {
    /// A sketch drawing tool: line, circle, arc, rectangle.
    Draw,
    /// Trim, extend, break, corner fillet - the ones that act on a click.
    ClickOp,
    /// Mirror, offset and the rest of the editing buttons.
    Modify,
    /// Move, copy, rotate.
    Move,
    /// The sketch pattern: linear or circular.
    Pattern,
    /// The dimension tool.
    Dimension,
    /// A feature command of the Part (extrude, hole, fillet...).
    Command,
    /// The measuring tool.
    Measure,
    /// Pointing at something for a command: a plane for a sketch, every edge for a fillet, a contour.
    Pick,
    /// Placing a primitive shape: a rectangle, a polygon.
    Place,
    /// An import waiting for its place.
    Import,
    /// A fillet or chamfer on a sketch vertex.
    Corner,
    /// Typing into a note or a text.
    Annot,
}

impl DrawTool {
    /// EVERY KIND. Walked by the checks; a guard below holds it against the declaration.
    pub const ALL: [DrawTool; 13] = [
        DrawTool::Draw,
        DrawTool::ClickOp,
        DrawTool::Modify,
        DrawTool::Move,
        DrawTool::Pattern,
        DrawTool::Dimension,
        DrawTool::Command,
        DrawTool::Measure,
        DrawTool::Pick,
        DrawTool::Place,
        DrawTool::Import,
        DrawTool::Corner,
        DrawTool::Annot,
    ];

    /// IS IT TAKEN BY A DOOR OF ITS OWN - a button or a key a person presses.
    ///
    /// The rest are SUB-MODES: they are switched on from inside another tool and cannot be taken on their
    /// own, so "taking this one releases the previous" is not a sentence about them. Named here rather
    /// than left out of a list, so that the reason travels with the type.
    pub fn has_door(self) -> bool {
        !matches!(self, DrawTool::Pick | DrawTool::Import | DrawTool::Corner | DrawTool::Annot)
    }
}

/// WHAT IS IN HAND RIGHT NOW. The same question `armed_assembly_tools` answers for the Assembly.
pub fn armed_draw_tools(t: &Tools) -> Vec<DrawTool> {
    // THE TOOL IN HAND IS ONE FIELD, so this half can return at most one thing and no check has to say so.
    let mut out: Vec<DrawTool> = t.armed.tool().into_iter().collect();
    // THE REST ARE NOT TOOLS ONE TAKES. Four are sub-modes fallen into WITH a tool in hand
    // (`DrawTool::has_door` says which), and placing is the size of a shape being typed while it is
    // drawn. They keep their own records, and more than one of them can be true at once.
    if *t.picking != Picking::None {
        out.push(DrawTool::Pick);
    }
    if t.place.shape != PlacingShape::None {
        out.push(DrawTool::Place);
    }
    if t.pending_import.curves.is_some() || t.pending_import.draw_pts.is_some() {
        out.push(DrawTool::Import);
    }
    if t.corner.at.is_some() {
        out.push(DrawTool::Corner);
    }
    if t.annot.note.is_some() || t.annot.text.is_some() {
        out.push(DrawTool::Annot);
    }
    out
}

/// WHICH BODIES ARE ON SCREEN: the seven things that together decide whether a body is drawn at all.
///
/// Visibility is not a flag on the body. It follows the chain of ticks up the assembly, the active
/// context, the sketch session and the feature being edited, and the caches hold the answer. Every
/// caller needs the same seven, so they travel as one record: `body_shown` alone is asked from
/// twenty-three places, each of which used to spell all seven out.
#[derive(Clone, Copy)]
pub struct BodyView<'a> {
    pub armed: &'a Armed,
    pub active_path: &'a [Id],
    pub cache: &'a Caches,
    pub cmd: &'a FeatCommand,
    pub project: &'a Project,
    pub regen: &'a Rebuilding,
    pub sketch_ses: SketchSession,
    pub win: &'a Windows,
}

/// THE AIMING OF A FEATURE COMMAND: the ten records a command points at geometry with.
///
/// They are cleared together - a command starting or ending lets go of every pick at once, and there is
/// no case where one of them keeps a pick while the rest let go. Held as one record so that a new
/// pickable tool becomes a field here instead of an eleventh argument at four call sites.
pub struct FeatPicks<'a> {
    pub arr: &'a mut ArrayParams,
    pub datum: &'a mut DatumCommand,
    pub draft: &'a mut DraftParams,
    pub loft: &'a mut LoftParams,
    pub mirror: &'a mut MirrorParams,
    pub picking: &'a mut Picking,
    pub recognise: &'a mut RecogniseTool,
    pub stitch_parts: &'a mut Vec<Id>,
    pub sweep: &'a mut SweepParams,
    pub thread: &'a mut ThreadParams,
    pub trim: &'a mut TrimTool,
}
