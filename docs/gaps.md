# Gaps

> **Last reviewed:** 2026-10-10 · **Last updated:** 2026-10-11 · **Change:** minor (G11: the Transform effect's Transform Objects, Transform Patterns and Scale Strokes & Effects options added and its Options checkboxes drawn, #885; G11: bezier drag previews follow the pointer back into the window and to the release, #834; SVG files keep the document's units, #864; G9: a 2026 `.ai` opens from its editing data (unknown operators skipped, gradient meshes left out with a loss note, every artboard compared), layer structure audited and tested, per-page PDF layers merged, unreadable private data named; G1: the shortcut reference's selection and shape-editing modifier rows checked and fixed; G11: Shape Builder cuts art of thousands of segments again, the reproduced case of #937; G4: proportional vertical metrics done, #966; G11: Shape Builder merge seams on grids of lines fixed, #893; scaled text controls corrected, #1034; group font-size preservation deferred) · **Target:** Adobe Illustrator 2026 (30.x)

Every known shortfall against Illustrator, one entry each, **ranked by how much it stops a professional from
switching**. This is the work list: unless you were given a task, pick the highest-ranked gap you can make
progress on, write a plan, and update the entry (and [target-app-parity.md](target-app-parity.md)) in the same PR.

Gap ids are stable (PRs say "Part of gap 2"); the rank can change. Hours are Opus 5.5 agent wall-clock hours for
one agent, estimated unless marked; calibration in
[target-app-parity.md](target-app-parity.md#calibration). **Beta** marks the gaps that must close before the beta
stage ([ROADMAP.md](../ROADMAP.md)).

## Ranked

| Rank | Gap | Hours | Beta | Doc |
|---:|---|---:|:---:|---|
| 1 | [G1 Interaction fidelity](#g1-interaction-fidelity) | 60–90 | yes | [ui-parity.md](ui-parity.md) |
| 2 | [G9 Illustrator `.ai` and EPS files from real users](#g9-illustrator-ai-and-eps-files-from-real-users) | 10–20 | yes | [file-format-parity.md](file-format-parity.md) |
| 3 | [G10 Launch and stability on real machines](#g10-launch-and-stability-on-real-machines) | 20–35 | yes | — |
| 4 | [G8 Hardening at scale](#g8-hardening-at-scale) | 30–50 | yes (part) | — |
| 5 | [G11 Correctness bugs in core tools](#g11-correctness-bugs-in-core-tools) | 8–15 | yes | [target-app-parity.md](target-app-parity.md) |
| 6 | [G4 Advanced type](#g4-advanced-type) | 34–47 | core part | [type-parity.md](type-parity.md) |
| 7 | [G12 Preferences and panels that don't act yet](#g12-preferences-and-panels-that-dont-act-yet) | 13–20 | yes | [ui-parity.md](ui-parity.md) |
| 8 | [G2 Photoshop-style raster effects and the Effect Gallery](#g2-photoshop-style-raster-effects-and-the-effect-gallery) | 24–40 | no | [effects-parity.md](effects-parity.md) |
| 9 | [G13 Illustrator 2026 (30.x) additions](#g13-illustrator-2026-30x-additions) | 15–25 | no | [target-app-parity.md](target-app-parity.md) |
| 10 | [G14 Pen hardware beyond Windows pressure](#g14-pen-hardware-beyond-windows-pressure) | 8–16 | macOS pressure | [hardware-parity.md](hardware-parity.md) |
| 11 | [G5 Brushes, symbols and libraries](#g5-brushes-symbols-and-libraries) | 17–28 | no | [target-app-parity.md](target-app-parity.md) |
| 12 | [G3 3D and Materials](#g3-3d-and-materials) | 45–75 | no | [effects-parity.md](effects-parity.md) |
| 13 | [G7 Views and windows](#g7-views-and-windows) | 15–25 | no | [ui-parity.md](ui-parity.md) |
| 14 | [G6 Automation and scripting](#g6-automation-and-scripting) | 8–14 | no | [target-app-parity.md](target-app-parity.md) |
| 15 | [G15 Performance budgets and a GPU renderer](#g15-performance-budgets-and-a-gpu-renderer) | 15–25 | budgets only | [hardware-parity.md](hardware-parity.md) |
| 16 | [G16 Localization: the five missing key languages](#g16-localization-the-five-missing-key-languages) | 70–110 | no | [localization-parity.md](localization-parity.md) |
| 17 | [G17 Ecosystem: third-party plug-ins](#g17-ecosystem-third-party-plug-ins) | 20–45 | no | — |
| 18 | [G18 Generative AI features](#g18-generative-ai-features) | 30–60 | no | — |
| 19 | [G19 Accessibility](#g19-accessibility) | 15–25 | no | [ui-parity.md](ui-parity.md) |

**Alpha blockers:** none. Every core workflow passes the [alpha gate](roadmap.md#alpha-gate); G9 is the only
partial one, and it blocks beta, not alpha.

**To beta:** G1, G9, G10, G11, G12, the corpus and QA part of G8, the type-core part of G4, macOS pen pressure
from G14 and the budget run of G15: **~170–270 h** one agent, ~50–80 h wall clock with 4–6 agents.

## G1 Interaction fidelity

- **Missing:** a tool-by-tool, panel-by-panel pass over modifiers, cursors, the Properties panel per context,
  isolation mode and small behaviours, against the public tool and shortcut documentation. Itemized in
  [ui-parity.md](ui-parity.md). The Shaper's construction-mode widget and gesture refinements remain. Done so
  far (2026-10-11): the shortcut reference's "Work with selections" and "Edit shapes" rows for the selection
  tools, Scissors, Pencil and Knife (Alt swaps Direct and Group Selection, Shift-click subtracts segments and
  grouped objects, Scissors/Pencil/Knife Alt); still open from those tables: E during a Selection marquee,
  Space moving the zoom marquee.
- **Evidence:** no side-by-side session has ever happened (the clean-room rule forbids running Illustrator, so
  the pass works from the documentation); users keep reporting small differences: #991, #975, #973, #955, #908.
- **Impact:** a power user notices this in the first minutes and stops trusting the tool.
- **Estimate:** 60–90 h; splits well by tool across agents.

## G9 Illustrator `.ai` and EPS files from real users

- **Missing:** the reason the editing data of some real files isn't used (#951: a 100-page book opened as
  its PDF part with 109, later 1400, layers for 14; #868: an Illustrator 2018 file opened as its PDF part,
  flat), which only those files can tell: as of 2026-10-11 the open notes name what stopped it (an unreadable
  private data stream, data without layers, or the reader's own reason), the reader's handling of any
  nesting of layers, sublayers, groups, compound paths, clipping groups, hidden and locked objects and guides
  is under a property test (`crates/eps/src/import/ai/tests_structure.rs`, end to end in
  `crates/engine/src/cmd/fileio/tests_aiimport.rs`), and a PDF part that repeats its layers on every page
  opens with one layer per name. On a user's Illustrator 30.8 file (used locally) the editing data was
  stopped by two operators the reader didn't know, which write a gradient mesh (`/Mesh X!` … `/End X!`): now
  unknown operators between objects are skipped with a note, objects of kinds the reader doesn't read are
  left out with a loss note, every artboard is compared with its page, and the file opens from its editing
  data (groups, compound paths, named artboards) with its 3 meshes left out. **Next:** read gradient meshes
  from the editing data (the structure is known: `/Size`, `/Data`, per node `/P`, `/R`, `/CS`, `/S`, `/N`,
  `/E` entries). Still open: guides when a file falls back to its PDF part (#779: the PDF part
  has none), the page comparison's cost on large files (#758: the reader itself reads 50 000 objects in
  0.6 s in the optimized test profile), legacy type in `.ai` files in PostScript form (those without their prolog open since
  #1027), live effects, brushes and symbols as live objects (#637; pattern fills and global colours are read
  since #1025). Writing Illustrator's own editing data is out of scope (undocumented); Save As `.ai` stays
  PDF-compatible, so Illustrator opens our files as plain PDF art (#868's other direction).
- **Evidence:** the issues above; 28 of the 350 issues are about `.ai`/EPS (about 22 people, the most common single
  format theme), 20 fixed, and the 8 open ones are about structure (layers, guides, speed, live objects), not missing
  art. No real-file `.ai` corpus (users' files may be used locally, never committed).
- **Impact:** blocks anyone who hands files back to Illustrator users. Opening and reworking their own archive
  works, with layer cleanup in some files. This is the main-format gap that keeps the stage at alpha.
- **Estimate:** 10–20 h with users' files to test against.

## G10 Launch and stability on real machines

- **Missing:** the app not starting or staying in the background on some Windows 11 machines (#858, #964 fixed
  2026-10-10, #620, #818), slow pointer on a Linux AppImage (#575), macOS paste from other apps (#954), a
  web-free crash/hang reporter, a field record.
- **Evidence:** open issues; the never-crash lints and `guard` stop panics, not driver, windowing or packaging
  failures.
- **Impact:** an app that doesn't start on someone's machine is 0% for them.
- **Estimate:** 20–35 h, needs reporters' help on hardware we don't have.

## G8 Hardening at scale

- **Missing:** a corpus of real-world SVG/PDF/EPS files; more Affinity versions and platforms (51 pinned files
  now, including 28 original Affinity 3.2.3 feature probes; large interacting documents and complex typography
  missing); Windows, Linux and browser QA by hand; packaging polish (Homebrew cask #588, Flathub #554, update
  checks #615).
- **Impact:** unknown failures on users' files.
- **Estimate:** 30–50 h (the corpus and QA part blocks beta).

## G11 Correctness bugs in core tools

- **Done:** scaled text's size and horizontal-scale controls account for object/group transforms (#1034),
  including exact sizes across grouped labels and resetting horizontal stretch while keeping editable text.
  Shape Builder regions on art of more than 4096 segments (traced or type outlines): the planar map no longer
  gives up there, so lines cut such art instead of the tool offering whole shapes (the reproduced case of #937;
  the map's cost was quadratic in the segments, now linear, and its cap is 65 536).
  SVG files keep the document's units (#864): the root `width`/`height` carry them (`width="210mm"`), so a
  document in millimetres, inches, picas or points saved as SVG, or converted with `vectorcraft-cli convert`,
  opens again in those units at the same artboard size (pixels stay unitless).
  Bezier drag previews no longer freeze (#834): a Pen, Anchor Point or Direct Selection drag gets every
  position the pointer is seen at, including the one after it left the window and came back and the one in
  the frame it lets go, so what the release commits is where the pointer let go.
  The Transform effect (#885): Transform Objects, Transform Patterns and Scale Strokes & Effects as effect parameters
  (`objects`, `patterns`, `strokes`; old files unchanged), drawn live (stroke weights and dashes scale by √|scaleH·scaleV|,
  pattern paints follow the effect's transformation, with copies the original keeping its own and copy k taking them k
  times; the geometry stays with Transform Objects off; Outline Stroke and the other geometry effects after it, in the
  object's list and in its fills' and strokes', take the scaled weight and distances), on paths, groups, layers and type
  alike, baked the same way by Expand Appearance and export, in the dialog as checkboxes beside Reflect X/Y, the
  reference locator, Random and Copies. The "invisible"
  checkboxes measured: an unchecked box was drawn, but its `button_border` outline sat at 1.6:1 against the Medium Dark
  panel (1.6–2.5:1 across the four brightnesses, under the 3:1 a control's outline needs); every checkbox and radio at
  rest now outlines in `text_dim` (3.5–7.6:1), with headless frame tests for the contrast in each brightness and for
  each Options box being painted beside its label inside the dialog's clip.
- **Missing:** Shape Builder on the reporter's own file (#937, until confirmed on it); the Transform effect's Scale Strokes
  & Effects doesn't scale the distances of raster effects (shadows, glows, blur, feather) after it, which keep theirs;
  PDF text boxes moving on open (#722).
- **Impact:** wrong output in everyday work.
- **Estimate:** 8–15 h.

## G4 Advanced type

- **Missing:** hyphenation and justification options, Optical Margin Alignment (type core); CJK vertical
  composition (ruby, mojikumi sets and dialog, kinsoku settings #633, manual tate-chu-yoko), Middle Eastern
  features beyond bidi, variable font axes, spell check with an open dictionary,
  Touch Type, Retype, Snap to Glyph; optional preservation of font sizes during group scaling (#1034 enhancement).
  Itemized in [type-parity.md](type-parity.md).
- **Impact:** professional typography and Japanese/Chinese publishing.
- **Estimate:** 34–47 h (type core 14–19 h blocks beta).

## G12 Preferences and panels that don't act yet

- **Missing:** ~17 Preferences options stored but not acted on (#394: Highlight Alternate Glyphs, Open
  Documents As Tabs, Real-time Drawing, hyphenation language, Appearance of Black, devices…), Properties panel per
  context, context menus beyond the canvas, Layers panel row menus and Collect for Export, SVG Interactivity.
- **Impact:** settings that silently do nothing erode trust.
- **Estimate:** 13–20 h.

## G2 Photoshop-style raster effects and the Effect Gallery

- **Missing:** 29 of 57 filters (all of Artistic and Sketch), the Effect Gallery dialog, Load
  Texture for Glass and Texturizer, Smart Blur's Edge Only and Overlay Edge. Measured in
  [effects-parity.md](effects-parity.md).
- **Done:** the pixel-filter pipeline and 28 filters (Blur, Brush Strokes, Distort, Pixelate, Sharpen, Stylize,
  Texture, Video), Brush Strokes on 2026-10-11.
- **Estimate:** 24–40 h (measured 0.7–1.2 h per filter); parallelizes well.

## G13 Illustrator 2026 (30.x) additions

- **Missing:** gradient dither and perceptual blending, gradient presets (30.0); tangent and perpendicular
  snapping and the consolidated snapping options (30.0); artboard locking, artboard colours and managing
  artboards from the canvas (30.0); the Blend panel with ease in/out and separate colour acceleration (30.7);
  TIFF in Export for Screens (30.2); the Dimension tool (28.x).
- **Evidence:** Adobe's 30.0 and 30.7 release announcements, see
  [target-app-parity.md](target-app-parity.md#how-this-was-measured-2026-10-10).
- **Estimate:** 15–25 h.

## G14 Pen hardware beyond Windows pressure

- **Missing:** pen pressure on macOS (#852) and Wayland (#491, #764) — winit 0.30 reports none there; tilt,
  bearing and barrel rotation for brushes (6D Art Pen, #372); the eraser end.
- **Estimate:** 8–16 h, partly waiting on winit 0.31; needs pen hardware to verify.

## G5 Brushes, symbols and libraries

- **Missing:** the Art brush's Overlap, auto-generated Pattern brush corners, new tile art in Pattern Brush
  Options, the Key Color eyedropper; original brush, symbol and graphic-style libraries generated in code (never
  Adobe's); dynamic symbols; Start Global Edit.
- **Estimate:** 17–28 h.

## G3 3D and Materials

- **Done:** an initial live Revolve in the `three-d` crate (#846, from the contributor who offered in #605).
- **Missing:** Extrude & Bevel, Inflate and Rotate, caps, materials and better lighting, exact visibility for
  intersecting surfaces, 3D Classic, Turntable, 3D export (OBJ, USDA, glTF).
- **Impact:** specialist, but a visible menu of stubs.
- **Estimate:** 45–75 h, the largest single gap; plan in `plan/` before coding.

## G7 Views and windows

- **Missing:** multiple windows and Window › Arrange (Cascade, Tile, Float in Window, Consolidate All Windows),
  panels on another monitor, New View/Edit Views, global and video rulers.
- **Estimate:** 15–25 h.

## G6 Automation and scripting

- **Missing:** a scripting surface over the command registry (Illustrator: JavaScript), Batch, image and graph
  variable kinds, dataset import/export. Actions, Variables (text and visibility) and full agent control over
  MCP are done.
- **Estimate:** 8–14 h.

## G15 Performance budgets and a GPU renderer

- **Missing:** an idle-machine run of `vectorcraft-cli perf` (last: 2026-10-01 on a loaded machine, 290 ms for
  a 50k-path fit against a 16 ms budget), fixes for any budget missed, dirty-region rendering, the GPU backend
  spike (M5).
- **Estimate:** 15–25 h (the budget run and fixes block beta).

## G16 Localization: the five missing key languages

- **Missing:** Hindi, Arabic, Indonesian, Korean, Vietnamese catalogs; a right-to-left interface and interface
  shaping for Arabic and Devanagari; messages for Simplified Chinese and Brazilian Portuguese; native review of
  every catalog. Measured in [localization-parity.md](localization-parity.md).
- **Estimate:** 70–110 h plus human review.

## G17 Ecosystem: third-party plug-ins

- **Missing:** Illustrator's ecosystem is a C++ plug-in SDK and a large commercial plug-in market. VectorCraft
  has sandboxed WebAssembly plug-ins for object filters and live effects ([plugins.md](plugins.md)) and MCP;
  missing are tool plug-ins, panel plug-ins, a plug-in directory and any third-party plug-ins (#710).
- **Estimate:** 20–45 h for the APIs; the ecosystem itself isn't agent work.

## G18 Generative AI features

- **Missing:** Text to Vector Graphic, Generative Recolor, Generative Shape Fill (#766), Generative Expand,
  Concept to Vector, Prompt to Edit, Rewrite Text. Adobe's run on its cloud models; ours would be a pluggable
  provider.
- **Needs:** an owner decision on models and providers.
- **Estimate:** 30–60 h after that decision.

## G19 Accessibility

- **Missing:** AccessKit is on by default, so egui widgets reach screen readers, but custom panels, the canvas and its objects aren't described and nothing has been tested with VoiceOver, Narrator or Orca; keyboard-only canvas operation; high
  contrast. Illustrator's own accessibility is limited, so this is mostly beyond parity.
- **Estimate:** 15–25 h.

## Revision history

| Date | Change | Summary |
|---|---|---|
| 2026-10-11 | minor | G11: #834 fixed (a tool drag's preview skipped the pointer position in the frame it came back into the window, and the move in the release's frame, so a handle or anchor stayed behind until the next move and the release committed the stale shape); estimate unchanged |
| 2026-10-11 | minor | G11: #864 fixed (SVG saves wrote unitless sizes, so a document in mm/in/pt reopened in pixels); estimate unchanged |
| 2026-10-11 | minor | G9: a real Illustrator 30.8 file opens from its editing data (unknown operators skipped, gradient meshes left out with a loss note, every artboard compared with its page); next: read gradient meshes; estimate unchanged |
| 2026-10-11 | minor | G9: layer structure of the editing data audited and property-tested; PDF layers repeated per page merged; unreadable private data named in the notes (#951, #868, #779, #758); estimate unchanged, needs the users' files |
| 2026-10-11 | minor | G11: the Transform effect's Transform Objects, Transform Patterns and Scale Strokes & Effects options (effect parameters, rendering, Expand Appearance, dialog, command docs) and its dialog's Options checkboxes drawn (#885); estimate unchanged, #937 open |
| 2026-10-11 | minor | G11: Shape Builder on art of thousands of segments (traced outlines, type outlines): lines cut it again past 4096 segments, where the planar map used to give up and the tool offered whole shapes (#937's reproduced case; the map's edge-source lookup was quadratic, now indexed, and the cap is 65 536); estimate unchanged until confirmed on the reporter's file |
| 2026-10-11 | minor | G4: proportional vertical metrics (#966) done; estimate unchanged |
| 2026-10-11 | minor | G11: #893 fixed (merged regions of crossing lines left seams); #937 still open, estimate unchanged |
| 2026-10-11 | minor | #1034: corrected scaled type controls; optional group font-size preservation remains separate. Estimates unchanged at this rounding. |
| 2026-10-11 | minor | G9: legacy `.ai` without the prolog opens with its layers (#1027) |
| 2026-10-11 | minor | G9: `.ai` pattern fills and global colours read from the editing data (#1025) |
| 2026-10-10 | minor | G3: initial Revolve landed (#846), 45–75 h left |
| 2026-10-10 | minor | G9 evidence counted from the tracker (28 `.ai`/EPS issues, 20 fixed); impact narrowed to handing files back |
| 2026-10-10 | minor | Alpha blockers checked against the core-workflow gate: none |
| 2026-10-10 | major | Moved from the ROADMAP's "Where we're lacking"; kept ids G1–G8, added G9–G19 (file formats, stability, bugs, preferences, 30.x additions, pen hardware, performance, localization, ecosystem, AI, accessibility) and re-ranked |
