<div align="center">

# 🦫 Rustybara — Future Plans & Prioritization

**Roadmap triage as of `v0.1.9`** — ordered by priority, weighed against ease of
implementation and necessity, with notes on how each item touches the existing
codebase.

</div>

---

## How to read this

| Axis | Meaning |
| --- | --- |
| **Necessity** | How much the product *needs* it — prepress correctness, distribution/trust, or adoption. `High` / `Med` / `Low`. |
| **Ease** | Implementation effort. 🟢 Easy · 🟡 Moderate · 🔴 Hard / architectural. |
| **Tier** | Recommended scheduling band (T1 = do first → T5 = long-horizon). |

> **Guiding principle:** front-load **quick wins** (T1) to bank momentum and polish
> right after a release, then invest in the **deep prepress features** (T3–T4) that
> define the product. The single highest-*necessity* hard item — lazy parsing — is
> called out as its own milestone because so much else leans on it.

---

## 📊 At a glance

| # | Feature | Tier | Ease | Necessity |
| --- | --- | :---: | :---: | :---: |
| 1 | ~~JPEG quality config~~ ✅ *shipped v0.1.9* | **T1** | 🟢 | Med |
| 2 | ~~Rotate PDF~~ ✅ *shipped v0.1.9* | **T1** | 🟢 | Med |
| 3 | ~~Redefine media box~~ ✅ *shipped v0.1.9* | **T1** | 🟢 | Med |
| 4 | ~~macOS rbv dock-icon bug~~ ✅ *fixed v0.1.9* | **T1** | 🟡 | Med |
| 5 | Code signing (macOS / Windows) | **T2** | 🟡 | **High** |
| 6 | npm library for `rustybara-wasm` | **T2** | 🟡 | Med |
| 7 | Video tutorials (website) | **T2** | 🟢 | Med |
| 8 | Add trim marks (printer's marks) | **T3** | 🟡 | **High** |
| 9 | Compress / optimize file size | **T3** | 🟡 | Med |
| 10 | Undo action | **T3** | 🟡 | Med |
| 11 | Clipping-mask remover / override | **T3** | 🟡🔴 | Med |
| 20 | System / network print | **T3** | 🟡 | Med |
| 12 | **Lazy streaming / large-file parsing** | **T4** | 🔴 | **High** |
| 13 | PDF/X writing | **T4** | 🔴 | **High** |
| 14 | Preflight reporting & validation | **T4** | 🔴 | **High** |
| 15 | Automation (node UI + `.jsx`/`.rbflow`) | **T5** | 🔴 | Med |
| 16 | Print Layout & Imposer (app/add-on) | **T5** | 🔴 | Med |
| 17 | Drawing / copy-paste / curve creation | **T5** | 🔴 | Low |
| 18 | RIP software (app/add-on) | **T5** | 🔴 | Low |
| 19 | 3D fold/model viewer + shareable web proof | **T5** | 🔴 | Med |

---

## 🟢 Tier 1 — Quick wins ✅ **complete (shipped in v0.1.9)**

Low-risk, high-satisfaction, mostly additive. **All four items below shipped in
v0.1.9** — kept here for provenance; the next active band is Tier 2.

### 1. JPEG quality config · 🟢 · Med — ✅ **Shipped in v0.1.9**
*(Originally:)* The encoder quality was **hardcoded to `90`** in [`encode/save.rs`](rustybara/src/encode/save.rs#L124)
(`JpegEncoder::new_with_quality(&mut buf, 90)`). Make it configurable by adding a
`quality` field to [`RenderConfig`](rustybara/src/raster/config.rs), threading it
through `save()`, the `export_images` command in
[`rbara-gui/src/commands.rs`](rbara-gui/src/commands.rs), and a slider in the GUI
**Export** panel. Already on the README roadmap. Lowest-effort item on the list —
a clean warm-up.

### 2. Rotate PDF · 🟢 · Med — ✅ **Shipped in v0.1.9**
*(Originally:)* There was **no `/Rotate` handling anywhere** in the core (verified). A page's
display rotation is just a `/Rotate` dict entry (0/90/180/270). Add a
`PdfPipeline::rotate(degrees)` that sets `/Rotate` on each page object, a
`rotate` command, and a GUI action. Lives naturally beside the other page ops in
[`rustybara/src/pages/`](rustybara/src/pages/). Easy as a *viewing* rotation;
note that physically baking rotation into content/boxes is a separate, harder job
we should explicitly *not* take on here.

### 3. Redefine media box · 🟢 · Med — ✅ **Shipped in v0.1.9**
Box geometry already had a home: [`pages/boxes.rs`](rustybara/src/pages/boxes.rs)
(`PageBoxes`, `set_trim_boxes`). Add a `set_media_box` sibling + command + GUI
field. Pairs conceptually with **#8 (trim marks)** and **#13 (PDF/X)**, both of
which care about box correctness.

### 4. macOS rbv dock-icon bug · 🟡 · Med — ✅ **Fixed in v0.1.9**
*(from the bug list)* On macOS, a terminated `rbv` process exited but its dock icon
lingered (and a zero-mem process could linger). **Re-diagnosed 06/2026** — this was
*not* in `spawn_render`; the two real culprits were (a) Escape hard-exited via
`std::process::exit(0)` instead of the clean `event_loop.exit()` the `CloseRequested`
path uses, and (b) [`open_in_viewer`](rbara-gui/src/commands.rs#L1236) spawned rbv
fire-and-forget and never reaped the `Child`, leaving a defunct process. **Resolution
(v0.1.9):** Escape now calls `event_loop.exit()` at
[`viewer.rs:1383`](rbv/src/viewer.rs#L1383) (no more `process::exit` anywhere in
`rbv/src`), and `open_in_viewer` spawns a detached reaper thread that blocks on
`child.wait()`. Shipped alongside the Windows console-window fix (same launch path).

---

## 🚚 Tier 2 — Distribution & reach

Not features *inside* the app, but they directly gate who can adopt it.

### 5. Code signing (macOS / Windows) · 🟡 · High
Unsigned installers trip Gatekeeper (macOS) and SmartScreen (Windows), which scares
off non-technical print operators — the exact audience. The pipeline already exists
([`installer/macos`](installer/macos/), [`installer/windows`](installer/windows/),
[`.github/workflows/release.yml`](.github/workflows/release.yml)); this adds
Authenticode signing + macOS notarization steps and the cert secrets to CI. Effort
is moderate and partly **non-engineering** (acquiring an Apple Developer cert and a
Windows code-signing cert). High necessity for a credible public release.

### 6. npm library for `rustybara-wasm` · 🟡 · Med
The [`rustybara-wasm`](rustybara-wasm/) crate and
[`wasm-build.yml`](.github/workflows/wasm-build.yml) already exist — this is
*packaging*, not new core work: wasm-bindgen/wasm-pack output, a typed npm package,
and a publish step. Unlocks browser/Node adoption and demos. Reuses the same core
API, so it tracks core releases.

The repository now builds and smoke-tests a typed Node package artifact on every
WASM workflow run. Tags named `rustybara-wasm-v<version>` publish it to npm with
provenance once the `NPM_TOKEN` repository secret is configured; the first registry
release is the remaining distribution step.

### 7. Video tutorials (website) · 🟢 · Med
Non-code, parallelizable, and a real adoption lever for a niche prepress tool. Lives
in [`rustybara-website`](rustybara-website/). Cheap to start; the cost is content
time, not engineering. Can run alongside any tier.

---

## 🎯 Tier 3 — Core prepress depth

Moderate effort, strong day-to-day value for the target user. These build on
primitives that already exist.

### 8. Add trim marks (printer's marks) · 🟡 · High
**Disambiguation (important):** this is *not* the existing **Trim Marks** action
(`trim_marks` → `PdfPipeline::trim`, which *removes* content outside the TrimBox),
nor **Add Trim Box** (`add_trim_box`). This is drawing **crop/registration marks**
into the page. It builds on the box geometry in
[`pages/boxes.rs`](rustybara/src/pages/boxes.rs) (offset marks from the TrimBox into
the bleed) and writes mark strokes into the content stream. High necessity — it's a
staple of print-ready output. Rename carefully in the UI to avoid colliding with the
existing "Trim Marks" label.

### 9. Compress / optimize file size · 🟡 · Med
Two layers: (a) structural — `lopdf`'s `compress()` + object-stream/xref cleanup
(we already lean on `compress()` elsewhere), and (b) image downsampling/recompression
via the [`raster`](rustybara/src/raster/) + [`encode`](rustybara/src/encode/) paths
(shares machinery with **#1 JPEG quality**). Useful, self-contained, measurable.

### 10. Undo action · 🟡 · Med
Today actions write a new file and the GUI swaps the buffer to it
(`replaceProcessedFiles` in [`App.svelte`](rbara-gui/frontend/src/App.svelte),
output naming via `output_path` in `commands.rs`). Undo means snapshotting the prior
output (or backing up before an overwrite) and restoring it. Mostly a GUI
state/file-history concern; interacts with overwrite mode (`hasProcessedInOverwrite`)
and the activity log. Moderate, and a nice UX safety net.

### 11. Clipping-mask remover / override · 🟡🔴 · Med
Touches content-stream interpretation — `W`/`W*` clip operators — in
[`stream/filter.rs`](rustybara/src/stream/filter.rs) and the object model in
[`objects/tree.rs`](rustybara/src/objects/tree.rs). Detecting and neutralizing clip
paths without corrupting the graphics-state stack is fiddly (it's adjacent to the
CTM/`q`/`Q` handling we just fixed). Niche but valuable in prepress cleanup; rated
moderate→hard depending on how robust we want it.

### 20. System / network print · 🟡 · Med
Send the active PDF straight to a network/system printer for **fast proofs** — no
app-switching, no separate print application. The tractable path hands the file to
the **OS print system** rather than speaking raw IPP: `lp`/`lpr` (CUPS) on
macOS/Linux, the Windows spooler (`Start-Process -Verb Print` or the Win32 print
API) on Windows — plus a printer-enumeration call (`lpstat -p` / `EnumPrinters`) to
populate a device picker. In practice a "network device" is just a printer the OS
already has installed, so leaning on the spooler sidesteps protocol work.

No new core PDF work: it's a new `rbara-gui` command + a small frontend (device
list, copies, page range). For image-only devices you could rasterize via the
existing export/pdfium path, but handing the PDF to the spooler preserves vector.
Main effort is the cross-platform divergence (CUPS vs Windows). Explicitly a
proofing *convenience* (speed) — **not** calibrated/halftoned output, which is the
separate RIP item (**#18**).

---

## 🏗️ Tier 4 — Foundations & flagship prepress (dedicated milestones)

High necessity, high effort. Each deserves its own milestone rather than being
squeezed between small tasks.

### 12. Lazy streaming / large-file parsing · 🔴 · High  — *the keystone*
This is the documented **Known Limitation**: `PdfPipeline::open` →
[`lopdf::Document::load`](rustybara/src/pipeline.rs#L88) parses the whole object
graph eagerly, so 200 MB+ files are currently **hard-blocked** on add
(`resource.js` + `App.svelte`; see README "Known Limitations"). Solving it (lazy/
random-access parsing, or a pdfium-backed lazy metadata path) would let us **remove
the size gate** and unblocks comfortable handling of large production files —
which several T5 items (imposition, RIP) implicitly assume. It's the highest-leverage
hard problem on the board; schedule it deliberately, not opportunistically.

### 13. PDF/X writing · 🔴 · High
Print shops require PDF/X-1a/X-4 conformance. Needs an **output intent** (ties into
[`rustybara-icc`](rustybara-icc/) profiles), guaranteed TrimBox/BleedBox (see **#3**,
**#8**, [`pages/boxes.rs`](rustybara/src/pages/boxes.rs)), and conformance metadata
in [`xmp.rs`](rustybara/src/xmp.rs). Complex but squarely on-mission — arguably the
feature that most distinguishes a "prepress toolkit" from a generic PDF tool.

### 14. Preflight reporting & validation · 🔴 · High
The natural companion to **#13** (validate what PDF/X must guarantee). A rules engine
that *reads* rather than writes, and it can reuse a lot we already have:
`build_object_tree` ([`objects/tree.rs`](rustybara/src/objects/tree.rs)),
`detect_color_space`, spot detection
([`rustybara-icc/src/pdf.rs`](rustybara-icc/src/pdf.rs) `find_spot_colorspaces`),
box geometry, and font/outline data ([`outline/`](rustybara/src/outline/)). Large,
but mostly *composition* of existing analysis. Best done after **#13** so the rules
and the writer share a model.

---

## 🚀 Tier 5 — Major new initiatives (long-horizon)

Each is effectively its own project. Sequenced last not because they lack value, but
because their scope dwarfs everything above.

### 15. Automation — node UI + `.jsx`/`.rbflow` scripting · 🔴 · Med
Already designed: see **`rbara-automation-plan.md`** (desktop). Builds on primitives
that exist — the uniform `async` command layer + `ProcessingLock`, the `:` command
bar ([`CmdBar.svelte`](rbara-gui/frontend/src/components/CmdBar.svelte)), the action
log, and per-file XMP provenance. Phased: record/replay → per-step output routing →
node editor → parallel pipelines. Power-user feature; high ceiling, large build.

### 16. Print Layout & Imposer · 🔴 · Med
Builds directly on the existing page primitives —
[`pages/spread.rs`](rustybara/src/pages/spread.rs) (split),
[`pages/stitch.rs`](rustybara/src/pages/stitch.rs), and `extract` — but full
imposition (signatures, n-up, gutters, marks) is a big domain. Likely a separate
app/add-on. Benefits a lot from **#12** (large inputs) and **#8** (marks).

### 17. Drawing / copy-paste gesture / curve creation · 🔴 · Low
This turns `rbv` from a **viewer** into an **editor**. rbv is deliberately a
rasterized-buffer *viewer* (raster-first for speed), with no content-stream *writing*
path today — so this is a foundational shift, not an increment. Lowest necessity:
the product's value is automated prepress correction, not interactive illustration.
Revisit only if a clear user need emerges.

### 18. RIP software · 🔴 · Low
A raster image processor is an entire product domain (halftoning, separations at
output resolution, device profiles). Furthest-horizon; would lean on the color/
separation work in [`rbv/src/separation.rs`](rbv/src/separation.rs) and **#12/#13**,
but is realistically a separate long-term initiative.

### 19. 3D fold/model viewer + shareable web proof · 🔴 · Med — *idea capture (not scheduled)*
Model fold lines / creases / page-flips so a flat dieline can be previewed as a
**folded 3D product** (folder, carton, brochure) — and share that proof online for
client sign-off. This is Esko Studio / ArtiosCAD-3D territory; nothing free/open
fills it, so it's a strong differentiator for the packaging/folder audience. High
*value*, very high *effort* — it spans a new rendering domain **and** hosting infra.

Key architectural calls (so it stays tractable):
- **One shared web 3D viewer, not two.** `rbara-gui` is a Tauri app — its frontend
  is already a webview — so a single three.js/WebGL viewer can live in *both* the
  GUI frontend and [`rustybara-website`](rustybara-website/). **`rbv` stays 2D**
  (it's deliberately a raster-first Skia/GL viewer; a 3D engine would fight that).
- **Embed the fold *spec*, not just a hash.** A hash can't reconstruct a model —
  store the compact spec (panel polygons in trim space, crease lines, fold angles,
  adjacency graph, fold order) in the XMP `rbara:` block ([`xmp.rs`](rustybara/src/xmp.rs)),
  with a content hash for integrity/caching.
- **Sharing = new infrastructure.** The site is static today; shareable proof links
  need file storage + a share route + **privacy/expiry** (prepress files are
  confidential client work). Treat this as the last, infra-heavy phase.

Builds on existing geometry — `PageBoxes` and the panel concept in
[`pages/spread.rs`](rustybara/src/pages/spread.rs) — and crease detection could seed
from spot/layer analysis (`find_spot_colorspaces` + the object tree). Suggested
phasing: **(1)** fold-spec model + XMP embed (core, testable) → **(2)** three.js
viewer reading the spec from a locally-opened PDF → **(3)** embed that viewer in the
`rbara-gui` webview → **(4)** hosting + share links. Keep **fold authoring** (the real
engineering risk) and **3D viewing** (the demo magnet) as separate milestones.

---

## 🗺️ Dependency notes

- **#12 (lazy parsing)** unblocks removing the hard size gate and underwrites large
  inputs for **#16 (imposer)** and **#18 (RIP)**.
- **#13 (PDF/X)** ↔ **#14 (preflight)** are a pair (write conformance ⇄ validate it)
  and share the object-tree + color + box model; both want **#3** and **#8** solid.
- **#1 (JPEG quality)** and **#9 (compress)** share the `raster`/`encode` machinery.
- **#8 (trim marks)** must be named to avoid colliding with the existing *Trim Marks*
  (content removal) and *Add Trim Box* actions.
- **#15 (automation)** rides on the now-uniform async command layer + command bar +
  action log; design already captured in `rbara-automation-plan.md`.
- **#19 (3D viewer)** pairs with **#16 (imposer)** — both answer "how does this flat
  file become a physical object" — reuses the XMP precedent ([`xmp.rs`](rustybara/src/xmp.rs))
  and the panel geometry from **#16**'s primitives. Captured as a forward idea; no
  XMP/fold work scheduled yet.
- **#20 (network print)** is OS-spooler *proofing* — distinct from **#18 (RIP)**
  (calibrated raster output). It can optionally reuse the export/raster path for
  image-only devices, but otherwise touches no core PDF code.

---

## 🐞 Bug fixes

| Bug | Area | Ease | Notes |
| --- | --- | :---: | --- |
| ✅ ~~rbv dock icon persists / zero-mem process after termination (macOS)~~ **fixed v0.1.9** | [`rbv/src/viewer.rs`](rbv/src/viewer.rs#L1383), [`rbara-gui/src/commands.rs`](rbara-gui/src/commands.rs#L1242) | 🟡 | **Resolved.** Two culprits fixed: (a) Escape now uses `event_loop.exit()` instead of `std::process::exit(0)` (no `process::exit` left in `rbv/src`); (b) `open_in_viewer` now spawns a detached reaper thread that blocks on `child.wait()`, reaping the `Child`. *Not* in `spawn_render`. Was **T1 #4**. |
| ✅ ~~rbv pops a console window on launch (Windows)~~ **fixed v0.1.9** | [`rbv/src/main.rs`](rbv/src/main.rs#L1) | 🟢 | **Resolved.** Added `#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]` to rbv's `main.rs` (mirroring [`rbara-gui/src/main.rs`](rbara-gui/src/main.rs#L1)), so the windowed `rbara-gui` no longer slides in a console when it spawns rbv. `--listen` unaffected (stdin is a piped handle, not a console). |

### 🔎 Investigation note — the "process multiplexer" lead (06/2026)

A research pass on the note's hypothesis (collapse the two threads in
`spawn_render` into one "mux" to fix both OS bugs) found the premise doesn't hold:

- **Threads ≠ processes ≠ windows.** `spawn_render` (`viewer.rs:290`) spawns two
  *OS threads inside the single rbv process*. Threads produce no dock tile, no
  taskbar entry, no console window — so collapsing them changes nothing about
  either OS bug. Both bugs live in the process/window lifecycle, not the render
  threads (see the re-diagnosed table rows above).
- **The `rmux` reference is a different tool.** [`Helvesec/rmux`](https://github.com/Helvesec/rmux)
  is a *terminal* multiplexer (tmux-style PTY/session daemon with a typed SDK) — it
  multiplexes terminal sessions, not render threads in a GUI process. Not applicable.
- **There *is* a worthwhile "render-mux" refactor — but it's not an OS-bug fix.**
  `spawn_render`, `spawn_plate_separation`, and the tile worker each detach
  independent threads, and `spawn_render` can finish a render for a page you've
  already navigated away from (handled safely by the `page == self.page` guards in
  `user_event`, but the work is wasted). A single long-lived render worker behind
  one command channel — with cancellation of superseded requests — would cut thread
  churn and kill stale renders. Worth doing as a **separate efficiency task**; it
  does **not** touch the dock icon or the console window.

---

<div align="center">
<sub>Living document — re-triage each release. Ratings are relative, not absolute.</sub>
</div>
