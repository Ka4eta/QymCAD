# The names of faces, edges and vertices

A feature refers to a face or an edge of the body before it: a fillet to its edges, a sketch to the face it stands
on, a shell to the faces it opens. A reference has to hold through an edit above it - a height changed, a sketch
redrawn, a node suppressed - and through a save and an open. That is what the names are for.

## The contract: the bridge names, an operation only says what a face is

**Every name of a face, an edge or a vertex is given by the bridge**, by its own rules, in one place:
`naming.rs` of this crate. No operation, no tool of a workbench and no code of the document invents a name.

* An operation hands the bridge **roles**: "this face is the wall swept from sketch entity E", "this face is the
  surface of the rounding of edge R" (`door::Roles`). That is what only the operation knows. It hands no names and
  no numbers that stand for names.
* Its result lands through **one door**, `qym_result` (`occt_bridge.cpp`) on the C++ side and `naming::silent` on the
  Rust side: the bridge carries the names of the operands through the history of the algorithm, names the roles,
  and names what the history was silent about, by the rules below.
* The document holds the names in its table (`qymcad_core::names::NameTable`) and resolves references by them. It
  makes none of its own: it only takes into its table names the bridge gave - a vertex when it is first referenced, the
  names of a part copied in from another document.

The guard is `names_are_minted_in_known_places` (`qymcad-core/tests`): it reads every source of every crate, and a call
that creates a name anywhere but the functions it lists turns the build red. A new operation goes through the door; it
does not get a place on the list.

## The principles the bridge follows

1. **A name comes from the recipe and from the history of the operation**, never from the order of a traversal. The
   place of an element is the last resort, and it is never silent.
2. **The same document gives the same names**: two rebuilds in a row rename nothing.
3. **An edit renames only what it touched**: nothing above the node edited, nothing beside what it changed.
4. **The history of an operation is part of what it gives back**: what became of each element. Guessing it after the
   operation is not allowed, except in a named place with its reason.
5. **When two elements become one, the operand the operation works on keeps its name** (the base, not the tool);
   between names of one rank the smaller descriptor wins; the loser is recorded as absorbed.
6. **Every fallback is visible**: a reference found by place marks its step in yellow until the person accepts it.

## What a name is

A structure in the name table, written to the file; outwards it travels as a descriptor, an index into the table with
a tag in the high bits.

* **A face** (`GeoName`): the node that made it, its role (a cap, a wall, a blend, a seam, ...), its source and the
  piece number when one source gave several faces. The source is what the person gave the node: the sketch entity a
  wall was swept from, the contour picked for a region (its caps), the place in the pick of the edge a blend rounds -
  never the name of an element above, which changes when its own neighbours do (`ops::profile::name_sides`,
  `ops::blend::blend_roles`, `a_blend_is_named_after_the_edge_it_was_picked_on`). An edge a description gave (every
  edge of a face) has no place in a pick, and its blend is named after the edge.
* **An edge** (`EdgeName`): the pair of faces it runs between and its number within the pair. An edge has no recipe of
  its own; it is where two surfaces meet.
* **A vertex** (`VertexName`): the three smallest descriptors of the edges meeting at it.

## How the bridge names a result

In `qym_result`, in this order:

1. **The names of the operands are carried through the history** (`carry_ids_with`): `Modified`, `Generated`, removed.
   The body worked on gives every name; a tool gives only the names its roles made.
2. **The roles are named** (`naming::land_roles`).
3. **Two elements became one** - coplanar faces glued into the monolith, the two sides of a strip taken out
   (`merge_names`): a name of the document beats a role of the operation, a role beats a positional number, and of
   one rank the smaller descriptor wins (`a_merged_wall_keeps_the_name_of_the_base`).
4. **A face split in pieces** keeps the name of the whole on one piece; a piece is told first by the faces the node
   made beside it - what cut it - then by the other named faces beside it, in the order of the recipe
   (`naming::pieces`, `a_split_face_is_told_by_what_cut_it`).
5. **What the history was silent about** (`naming::silent`): the seams, in waves, after a named neighbour
   (`naming::seams`); a face with no history and no named neighbour - a body brought in from a file or made of a mesh -
   by where it lies (`naming::placed`); the edges, after their pair of faces (`naming::edges`); the vertices after
   their edges (`naming::name_edge_ends`).

The caps of a profile operation of several regions are named after the profile they are drawn from - the contour the
person picked; a face merged from several profiles takes the caps of the lowest of them (`ops::profile::name_sides`,
`caps_of_merged_regions_are_named_by_their_profiles`).

**Where an algorithm gives no history**, the bridge reads it from something else, named with its reason: the patch over
a boundary (`note_beside_boundary`). The turn of the seam of a round face is carried by its own history: the record of
the cutting asked of what it rebuilt (`QymRepairAsk::Apply`), then the modifier of the turn. The simple offset of a
sheet is run by a modifier the bridge keeps (`SimpleOffset`), whose image of each face and edge is its history; the
algorithm that wraps the same modifier answers none.

## How a reference finds its element

The document, not the bridge, resolves a reference, in this order (`qymcad_core::refs::resolve`,
`Project::resolve_face_id`, `Project::resolve_edge_ids_in`):

1. **The name**, as it is.
2. **A recorded merge**: the name it yielded to; a piece returns to its whole.
3. **The canonical pair** of an edge: its faces reduced to their wholes, when one edge has that pair.
4. **The structure of the name** (`NameTable::same_recipe`): the one live face of the same node, role and source, or
   the one edge between two such faces; two candidates give none.
5. **The place**, last: only the one element standing exactly where the reference saw it, to 1e-6 (`PLACE_EXACT`). The
   step is then marked (`Project::placed_refs`): yellow, in words, in the file, until the person accepts it in its
   properties or picks the element again (`a_reference_found_by_place_waits_for_the_person`).

A reference not found is said: the node goes red, or - a blend that took the rest - yellow with how many edges were
left out. A node deleted hands its consumers to the body below it, and their references are translated to the one
element standing exactly where theirs stood (`Project::geom_name_map`).

## The measure

`names_hold_on_real_documents` opens every example and private sample as the window opens it, rebuilds it twice with
the same names, edits its first number and undoes it: no reference lost by name, none found by place, no face that did
not move renamed, every name back as it opened.
