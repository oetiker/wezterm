# Controller Handoff — PureCpu fix pass (wezterm fork)

> Starter pack for the next controller session. This handoff lives in ONE
> worktree — run `git worktree list` first and confirm this is the workstream
> you're resuming. Read this first, then `git log <handoff-commit>..HEAD` for
> everything that changed since. Detail is NOT here — it's in git + the
> superpowers plan/ledger/docs (§6). Before you rewrite this file at your own
> handoff: read the previous version (`git show HEAD:docs/controller-handoff.md`)
> and carry forward any lesson in §4/§5 that is still true. Fresh synthesis,
> not blank page. On merge into another branch, rewrite that branch's handoff
> to the merged reality — do not merge or preserve this text.

Handoff commit: `fde15d5`   Date: 2026-08-12   Reason: milestone — Task 12 closed, the measurement round run, Task 11 closed without code
Worktree / branch: `/scratch/oetiker/wezterm` (primary checkout) @ `update-optimization-rebased`
Trunk at time of writing: `main` @ 05343b3 — **reader: if trunk has moved, §2 is provisionally stale; if trunk now contains this branch's HEAD, this file is a tombstone** (`git merge-base --is-ancestor HEAD main`). `main` here is upstream wezterm and legitimately runs ahead of this branch by ordinary upstream commits; that is NOT a merge signal.
Sibling worktrees: `/scratch/oetiker/claude-worktrees/wezterm-osc52-upstream` @ `osc52-x11-fix` — the two-commit upstream PR (wezterm/wezterm#8043), unrelated; leave it alone until that PR resolves. This line cannot see worktrees created later; check yourself.

## 1. Mission

A predecessor session **reviewed** this fork's PureCpu software renderer against
the GPU backend and produced documents only (`docs/purecpu-review/`). This
workstream is the **fix pass**: repair everything that review found, so
`front_end = "PureCpu"` on a GPU-less Linux/X11 box loses as little as possible
against `front_end = "OpenGL"`.

Scope, set by the user in brainstorming: **all 18 findings** (C1, I1–I5, M1–M7,
L1–L5), **plus** subpixel-antialiased text and the scaled fallback/bitmap glyph
row. The five `known gap` rows (background image, opacity, blur/HSB tint) stay
**out of scope** — never examined, so they are investigation, not repair.

The structural decision that shapes the code (spec §3): **option B — point fixes
plus two extracted units.** The user pushed back hard and asked why not option C
(a full shader-equivalence rewrite of the quad loop) — **that argument is settled
in spec §3; do not relitigate it, but do read it.** B is a strict *prefix* of C,
so C stays available afterwards with tests in place.

Two load-bearing facts, both verified in code rather than assumed:

- **The GPU samples the atlas `Nearest`** for glyphs, colour emoji and grayscale
  quads (`render/draw.rs:215-218`). So bit-exact parity on those quads is
  **achieved**, not merely reachable. `Linear` serves only the `has_color == 2.0`
  background-image branch, which PureCpu **does** draw and which stays
  deliberately off the sampler.
- **The LCD subpixel data is already in the atlas** — per-channel coverage in
  R/G/B with max-alpha in A (`skrifa_rasterizer.rs`). That RGB is **sRGB-encoded**
  and must be **linearised** by PureCpu, because GL's atlas is an `SrgbTexture2d`
  that converts on read while `colorMask` is the one shader output never
  re-encoded by `to_srgb`. The plan said the opposite; only measurement caught it.

## 2. Where we are now

As of handoff commit `fde15d5` (re-derive merge/push state — see §8):

**Execution mode: subagent-driven development**, confirmed by the user. The
ledger at `.superpowers/sdd/2026-08-11-purecpu-fixes/progress.md` is
authoritative on task state — trust it and `git log` over anything here.

- **Tasks 1–12 and 15 complete. Only Tasks 13 and 14 remain.**
- **Task 12 (M3, M4, M5)** at `b10c3a5`: three font-data panic sites guarded.
  `cargo test -j4 -p wezterm-font` = **16 passed** (delta +15; the crate had
  exactly one test before). Gate re-derived by the controller, one fault
  injection re-run independently, and the agent's own weak point closed with a
  `cargo check -p wezterm-gui`.
- **THE MEASUREMENT ROUND IS RUN** (`fde15d5` carries the harness it needed).
  All four required configs covered. Results and their verdicts are in the
  ledger — **read them there, not here.** Headline: idle sits at the floor
  (1.1% of the way to the ceiling); the SGR 5 case is **16%**, not "near the
  ceiling"; a static image costs **0.57 ms/frame** forever; llvmpipe GL costs
  **30–120× the subject** on this box.
- **Task 11 (M2) CLOSED WITHOUT A CODE CHANGE.** The pre-fix control
  reproduced M2's signature to the digit and the subject shows `n=0`. Task 11's
  Steps 2–4 are satisfied; nothing to implement.
- **The version panic: root cause NOT found; the standing diagnosis and its
  workaround are both refuted** (§7). Untouched this session — but note the new
  binary prints `someone forgot to call assign_version_info` and **does not
  panic**, which is one more data point that the panic is not build-determined.

**Two open threads**: Tasks 13 and 14, and the version panic.

## 3. Do this next

1. **Task 13 — it is the only implementation task left.** L1 (band copy / X GC
   churn), L2 (per-glyph `HintingInstance`), L4 (`SetSelectionOwner`
   `CURRENT_TIME`), L5 (removed backends silently substituted). **Plus two items
   that are now Task 13's and are not in its plan text:**
   - **F4 — the other half of L3.** `colorease.rs:115` still has
     `1000 / fps as u64` inside `intensity_one_shot` (§7). It touches the GL
     path, so it needs the GL-invariant check.
   - **`findings.md` M5 needs a correction** (§7) — Task 12 refuted its
     "M5 reaches M3" chain.
   **L1 and L2 both say "measure".** You now have the instrument:
   `tools/purecpu-parity/measure-round.sh` and `cpu-case.sh`. Use them rather
   than inventing a method. L1's effect should show on the `static-image` and
   `blink-text` cases; L2 is a glyph-miss path, so steady state will NOT show
   it — measure first paint, or record that you could not.
2. **Then Task 14** — the single review checkpoint, plus the doc rewrites. Its
   inputs are all in hand now: the measurement round's numbers, M2's closure,
   and the GL-cost result that belongs in `README.md`.
3. **Do not run a review and an implementation concurrently in this worktree.**
   Both edit the tree and build; the **shared `CARGO_TARGET_DIR` is contended**,
   and a `cp` taken right after a successful build produced a different md5
   minutes later and an unrunnable binary. `md5sum` every binary copy and
   re-hash after copying — done every time this session, always matched.
4. **`:20` was alive and validated at this commit** (1280x1024, 96x96 dpi, depth
   24, `marco` running). It is an ordinary user process and has died once when
   the user's ThinLinc session restarted. Recipe:
   `plans/2026-08-10-purecpu-parity-review.md` "Environment setup" — transcribe
   it, never improvise geometry/DPI. **The user has authorised starting a
   replacement if it is dead**; that authorisation was given for the measurement
   round and went unused. Validate any replacement against the documented
   control before trusting a number from it.

## 4. Lessons & traps  ← the irreplaceable part

Carried forward where still true, plus this session's.

- **A PREDICTION'S MECHANISM AND ITS MAGNITUDE ARE SEPARATELY FALSIFIABLE, AND
  ONLY MEASUREMENT SEPARATES THEM.** The Task 10 review predicted the SGR 5 case
  "near the ceiling, ~250×". The mechanism was right — the early exit is
  defeated, a `paint_pass` runs every animation frame — and the magnitude was
  wrong by ~6×. **Both halves were in one sentence, and accepting or rejecting
  the sentence whole would have been wrong either way.** Settled by modelling,
  not arguing: the ceiling is 6.03 ms/full repaint, so the subject's 6.00%
  is 1.00 ms/frame if it paints every frame versus an *impossible* 24 ms on each
  of 75 transitions. **Ruling a branch out by impossibility is cheaper than
  instrumenting it.**
- **THE CONTROL BELONGS INSIDE THE SAME EXPERIMENT — and this session is the
  clean demonstration.** M2's subject result is `n=0`: worthless alone, since a
  broken crop, a blank capture or a wrong offset all produce `n=0` too. Run in
  the same round, the pre-fix control reproduced `n=87, max 1.91, mean -0.05`
  **to the digit**, which proves the instrument on that box on that day and only
  then makes the zero mean something. **A negative result needs a control most
  of all.**
- **VERIFY THE CONTROLLER'S PREMISES, NOT JUST THE AGENTS'. Fourteenth time**,
  and this time the implementer refuted `findings.md` itself with a citation:
  M5's infinite scale does **not** reach M3's panic, because
  `glyph_outline_draw_ops` draws with `Size::unscaled()` and the scale lives in
  a `tiny_skia::Transform`, never in the path points. **Put "challenge the
  premise, with evidence, instead of executing it" in EVERY brief** — it has now
  prevented more waste than any other instruction in this pass.
- **THE PRE-FLIGHT SCAN CAUGHT A PLAN THAT NAMED THE WRONG FILE.** Task 12's
  Files list and its Step 1 grep pointed at `skrifa_rasterizer.rs` for M3; that
  grep returns **zero hits** and M3 is in `paint_ops.rs`. M5 was four sites, not
  "both". **A plan step that tells you to grep is a testable claim — run the
  grep yourself before dispatching.** Ten minutes, every task, still paying.
- **A SELF-ASSESSED WEAK POINT IS A HYPOTHESIS; RUN THE ONE THE AGENT COULDN'T.**
  Twelve for twelve on asking implementers to flag their own. Task 12's #5 was
  "this `pub` signature change rests on grep, not a compile, because I declined
  to spend a wezterm-gui build". That is exactly the one worth the controller's
  eight minutes: `cargo check -j4 -p wezterm-gui`, exit 0. **The agent picked
  the right weak point and could not afford to close it; you can.**
- **A WRONG "WHY" NEXT TO RIGHT CODE IS ITS OWN DEFECT CLASS.** Neither
  `bell_region`'s NaN claim nor the `first_row` "safe direction" claim broke
  anything; both were *reasons* pinned in comments that were backwards. They
  outlive everyone who understood the code. **Review comments as claims.**
  The same applies to source comments about cost: **"a still image costs nothing
  here, forever" was true in rects and false in CPU by 0.57 ms every frame** —
  a comment can be true in the dimension its author was thinking about and false
  in the one that matters.
- **A MUTANT'S VALUES CAN REFUTE A COMMENT, not just the code.** When a comment
  states a mechanism, a mutant that exercises it is a cheaper test of the comment
  than reading the library.
- **PRESERVE EVIDENCE BEFORE A HARNESS OVERWRITES IT.** `compare-case.sh` writes
  fixed filenames (`out/wide-sixel-{gl,cpu}.png`) — the stored M2 evidence lived
  at exactly those paths. Copied to `wide-sixel-prefix-era-*` first. **Check
  what a rerun clobbers before you rerun it.**
- **DON'T ATTRIBUTE FROM A CONTROL BINARY.** Task 4-vs-7 attribution for M2 was
  left undetermined on purpose: the only Task-4-era binaries carry deliberate
  review modifications, so probing with one would have produced a confident
  wrong answer. **Name the experiment that would settle it and move on.**
- **A REFUSAL CAN BE A FEATURE.** `gen-config.sh` now refuses
  `FORCE_FULL_REPAINT=true` for OpenGL, where the option is inert. An inert
  ceiling reads as "the subject already reaches the ceiling" — the same class of
  false pass as a rejected config making both windows use one backend and
  reporting a triumphant `AE = 0`. **Verify a new knob in BOTH directions before
  using it in a measurement.**
- **A NEGATIVE RESULT NEEDS A CONTROL, AND AN EMPTY GREP IS A CLAIM ABOUT YOUR
  REGEX** until you prove it is a claim about the data.
- **CHECK THAT A MUTANT PERTURBS THE VALUE THE TARGET TEST READS**, and
  **CLASSIFY A KILL, DON'T COUNT IT** — exactly one test, at a pinned-value
  assertion, with the discriminating **negative** named. A kill spread wide can
  still be the point when the property really is shared.
- **REASONING FROM A RESTATEMENT INSTEAD OF FROM THE THING.** When a finding
  says "X is safe because Y", ask what else reaches X.
- **The plan's own text can be wrong, and a good implementer will follow it off a
  cliff.** Its line numbers are stale after every task. Also: **its shell
  commands are wrong for this box** — Task 14 Step 1's
  `cp target/release/wezterm-gui` does not exist, because `CARGO_TARGET_DIR` is
  `/home/oetiker/scratch/cargo-target`.
- **Do not restructure code that no test executes.**
- **A test that asserts no value discriminates almost nothing.** Pin values, and
  derive them *before* the first compile; **passing on the first run is the
  evidence it was a derivation.**
- **READ THE LIBRARY'S SEMANTICS, NOT THE PLAUSIBLE ONES.** `PathBuilder::finish`
  returns `None` for a **one-verb** builder, which is why a `move_to`-only
  "empty path" fallback could never succeed. `OnceLock::set` returns `Err`
  carrying the value you passed. `x/0.0` is `+inf`, not NaN.
- **A REVIEWER'S ALGEBRA CAN BE WRONG IN THE PROJECT'S OWN KNOWN TRAP** —
  floating-point association order is part of "term for term". Compute, don't
  argue, about floats.
- **A BUILD LOG SAYING "Finished in 0.42s" MEANS NOTHING REBUILT.**
- **When a test compares against an oracle, decide which side is ground truth and
  put the slack on the OTHER one.**
- **Beware a case that passes for the wrong reason.** Ask *why* a run passed.
- **AN IDLE NOTIFICATION IS NOT A REPORT.** Task 12's agent finished silently
  with its inline reply lost, as every agent in this pass has. Signatures: clean
  tree + commit → finished silently, go read the commit and the report file;
  dirty tree + no build → stalled; dirty tree + live `cargo` → deadlocked,
  `SendMessage` the *same* agent, never re-dispatch.
- **Never take an agent's gate result on its word — re-derive it against the
  commit**, and re-run at least one load-bearing fault injection yourself.
- **A crash kills the subagent but not its work.** Read the tree and logs first.
- **Prior sessions leave evidence in their scratchpads** (`grep -rl` over
  `/tmp/claude-1003/`). The version-panic work lives almost entirely there.
- **`.superpowers/` is gitignored, so an agent cannot commit its report.** Say
  "write the file; it will not be committed, and that is expected."
- **Put `timeout: 600000` on every long Bash call, and tell the agent never to end
  a turn while a background shell is live.** As controller you may background a
  build and poll it — the harness re-invokes you on exit — but a *subagent*
  cannot. The cause is the turn ending, not the timeout
  (anthropics/claude-code#50572, closed "not planned").
- **State process constraints as actions, not prohibitions.** "Redirect every
  command to a log file and Read it" lands; "don't pipe the gate" gets ignored.
- **Require the full report in a file, a short summary inline.** Ten sessions now.
- **`/scratch` on this box silently corrupts large writes.**
  `wezterm-gui-t4rev-subject` is a corrupt copy; do not use it.
- **`pkill` as a literal string in a Bash tool command kills the call with exit
  144.** Use `pgrep` and `kill -TERM`; `|| true` does **not** protect you.
  Related: `pgrep -c -f <pattern>` counts your own shell — it reported 2 leaked
  windows when there were none.
- **A subagent's stdout lands in the human's real terminal.** Redirect everything
  to a log file and Read it; never `cat`/`head`/`tail` a binary or a PNG.
- **Focus state is a confound in any two-window X comparison.** Sequential capture.
- **llvmpipe is the reference and it works** — and it costs 176–249% CPU, 30–120×
  the PureCpu subject. The GPU reference exists only because of it.
- `timeout N cat </dev/null` does **not** wait; use `timeout N tail -f /dev/null`.
- **Harness trivia:** the test module in `render/purecpu.rs` is `mod test`
  (**singular**, `pub(crate)`); `purecpu_sampler.rs`/`purecpu_dirty.rs` and all
  four new `wezterm-font` modules use `mod tests`. `wezterm-gui` is binary-only:
  `cargo test -j4 -p wezterm-gui --bin wezterm-gui`. `cargo fmt --check` **fails
  repo-wide and did before this pass** — do not "fix" it. Three
  unused-placeholder warnings are likewise pre-existing.

## 5. Don'ts & constraints

The plan's Global Constraints block is authoritative — read it. The ones that
matter most:

- **Never touch X displays `:10`–`:14`** — other users, and the user's own
  ThinLinc session. All testing on `:20`. Xauthority:
  `/tmp/claude-1003/-scratch-oetiker-wezterm/659fab62-7b17-4059-a502-42815bcb9732/scratchpad/Xauthority-20`
  (belongs to a *previous* session's scratchpad and is what the running Xvnc
  authenticates against — do not "fix" the path; verify with `xdpyinfo`).
- **Destructive commands need the user's explicit approval.**
- **Nothing you run may emit raw bytes to the terminal.**
- **Never more than 4 cores**, including cargo (`-j4`).
- **Never run the 65536-atlas case** — 16 GiB on a shared box.
- **`has_color == 2.0` stays off the sampler.** Settled — do not re-decide.
- **The idle skip stays.** Each animated thing marks its own region. The rejected
  alternative — skip the early exit whenever any animation is live — repaints the
  whole window at the blink rate forever. **Measured this session: that is the
  36–72% ceiling, against a 1.4% idle subject. Do not drift back toward it.**
- **`has_animation` is a CLOCK, never evidence that something is animating.**
  Reading it as liveness reintroduces the forever-at-`animation_fps` loop.
- **The `focused` gate stays on animation rescheduling** — GL does the same
  (`render/paint.rs:121`).
- **Do not collapse `last_seqno_by_pane` back to a single field.**
- **The horizontal extent rule lives in THREE places** — `render/pane.rs`
  :110-152, :606-646, and `purecpu_dirty::painted_x_span`. Touch one, touch all.
  `painted_y_span` is the y twin and mirrors `render/pane.rs` term for term,
  association included.
- **Never raise `FUZZ`**, and never grade a case against another case's number.
- **Every config comes from `gen-config.sh`.** A hand-written config is the only
  way the bell guard can fire, and the only way a rejected config can silently
  make both windows use the same backend and report a triumphant `AE = 0`.
  **This is why `FORCE_FULL_REPAINT` was added to that script rather than
  hand-written for the ceiling arm.**
- **The OpenGL path must come out bit-identical** — Task 14 Step 2 checks it, and
  Task 13's L3 half (§7) touches GL, so it needs that check rather than a quiet
  landing.
- Verdict column in the matrix holds a **bare token**; the table is **6 cells per
  row** and a literal `|` in a cell silently breaks it — twice already.
- Do not commit `mise.toml` (untracked, intentional); `out/` and `.superpowers/`
  stay gitignored — the ledger is deliberately local-only.
- **Never overwrite `/scratch/oetiker/wezterm-builds/wezterm-gui-prefix`** — the
  pre-fix reference and the floor/control arm of every measurement. Verified
  byte-identical after this session's whole round.
- **Do not go looking for `/scratch/oetiker/wezterm/.tag`.** Its absence is
  correct and harmless (§7).

## 6. Where the detail lives

- Change history: `git log fde15d5..HEAD`; previous handoff
  `git show 4a0e566:docs/controller-handoff.md`
- **Spec:** `docs/superpowers/specs/2026-08-11-purecpu-fixes-design.md` — §3
  carries the settled B-vs-C argument
- **Plan:** `docs/superpowers/plans/2026-08-11-purecpu-fixes.md` — **15** tasks;
  Global Constraints at the top bind every task. **Treat its task text with
  active suspicion** (§4).
- **Ledger:** `.superpowers/sdd/2026-08-11-purecpu-fixes/progress.md` —
  **authoritative on task state**, and it carries the full reasoning behind each
  review finding, each controller decision, **three controller retractions**, and
  **the measurement round's four verdicts with their arithmetic**. Read the
  round's entry there rather than trusting §2's summary.
- Per-task briefs/reports/reviews: same directory,
  `task-N-{brief,report,review,...}.md`. Task 15's, Task 10's and Task 12's are
  the most complete artifacts in the pass.
- **The review this repairs:** `docs/purecpu-review/{README,parity-matrix,findings,noise-floor}.md`
- **The harness:** `tools/purecpu-parity/` — `compare-case.sh` (pixels),
  `sample-case.sh` (time-driven content), **`cpu-case.sh` + `measure-round.sh`
  (cost, new at `fde15d5`)**, `gen-config.sh` (every config), `lib.sh`
  (launch/find/cleanup contract), `corpus/`.
- `wezterm-gui/src/termwindow/render/purecpu.rs`, `purecpu_sampler.rs`,
  `purecpu_dirty.rs` — the two extracted units and the blit loop; `mod.rs` is
  plumbing that reads `self`.
- `wezterm-font/src/rasterizer/{paint_ops,skia_colr,skrifa_rasterizer}.rs` and
  `shaper/harfrust_shaper.rs` — Task 12's guards and the crate's first tests.
- Binaries: `wezterm-gui-prefix` (pre-fix floor/control, **never overwrite**),
  **`wezterm-gui-b10c3a5` (this round's subject, md5 `a9745eed…`)**,
  `wezterm-gui-t7-773b845`, `wezterm-gui-t8-fd6142a`,
  `wezterm-gui-t4rev-subject` (**corrupt**). Harness selects via `WEZTERM_BIN`.
- Measurement artifacts: `tools/purecpu-parity/out/m2-{control,subject}/` and
  `out/wide-sixel-prefix-era-*.png` (preserved originals).
- Version-panic evidence: `/tmp/claude-1003/-scratch-oetiker-wezterm/47cc07f6-.../scratchpad/`

## 7. Open questions / pending decisions

- **F4 — L3 IS ONLY HALF FIXED, and this half is Task 13's.**
  `colorease.rs:115` still has `1000 / fps as u64` inside `intensity_one_shot` —
  the function that *produces* the `has_animation` instants Task 10 reuses as its
  clock. The timer runs at 16.667 ms while the ease asks for 16 ms: **the two
  clocks disagree where before they agreed.** Benign (~0.7 ms late, one fewer
  wakeup), but it touches GL, so it needs the GL-invariant check.
- **`findings.md` M5 is WRONG and Task 13/14 must correct it.** Its claim that
  the infinite scale reaches M3's panic does not hold (§4). M3 and M5 are
  independent defects.
- **A static image costs 0.57 ms every frame while any animation holds the timer
  open** — `attrs.images()` deep-clones every `ImageCell` per frame. Measured,
  quantified, **not a regression from this pass and not in any task's scope.**
  Decide at Task 14 whether it becomes a new finding or a follow-up.
- **F1 — the `first_row` question, open with the experiment named.** The
  `AnimatedCellScan` maps lines by **stable row** while the renderer places them
  by **slice index**; they agree only while `first_row == viewport`. Code
  deliberately unchanged. **Unresolved: whether a viewport pinned in scrollback
  that output then TRIMS PAST reaches it.** The experiment: drive
  `terminal_with_lines_mut` against a `Terminal` with a small `scrollback_size`,
  request a stable range that has aged out, assert the `first_row` the callback
  receives.
- **Task 12's residue, recorded not hidden:** `num_palette_entries` is still raw
  `u16` font data — a font declaring 65535 entries allocates a 65535-element
  `Vec` per COLR glyph render (no panic, out of M4's scope). And "unclipped vs
  invisible" for a blank COLR clip layer is a **judgement**, unfalsifiable
  without a real font; the escape hatch is one line and its single guarding test
  is named in `task-12-report.md`.
- **M3/M4/M5 are GUARDED, NOT REPRODUCED** — no font reaching any of them was
  ever built. Never let a doc say "fixed".
- **THE VERSION PANIC — the standing diagnosis and its workaround are BOTH
  REFUTED; root cause still unknown.** Read the ledger's Phase 2 entry first.
  Binaries **are** tagged; `.tag`'s absence is harmless; **the panic is not
  build-determined**. The new `wezterm-gui-b10c3a5` prints
  `someone forgot to call assign_version_info` and runs fine — one more data
  point. **The blocker is a reproduction, not instrumentation.** Do not start by
  building an instrumented binary.
- **M2's attribution to Task 4 vs Task 7 is undetermined by choice.** Settle it,
  if ever, by building at `b7936ac` and re-running the control/subject pair.
- **What Task 15 still does NOT cover:** nothing in `call_draw_purecpu` itself;
  inside `blit_vertex_buffer`, the whole `subpixel_aa` arm, `apply_hsv`,
  per-vertex HSV, `mix_value != 0`, the degenerate-quad `continue`. **Nor does
  any test execute Task 10's `mod.rs` plumbing, `AnimatedCellScan`, or
  `image_next_frame_due`.**
- **A cheap, high-value experiment nobody has run:** assert that the
  pre-extraction body (`c1fc3cf`) and the current one produce bit-identical
  framebuffers over a corpus of random quads.
- **The four `parity-matrix.md` defects** are Task 13/14's to land, plus the
  **PureCpu 8192 vs GL 16384 atlas-cap asymmetry**, an unlisted finding wanting a
  matrix row. The sixel strip's residual `PAE 1285` is that asymmetry, not M2.
- **`painted_y_span` — a scope question for the user, queued for Task 14.**
  **Ask, do not decide unilaterally.**
- **Task 8's colour-emoji alpha ruling is unverifiable on this instrument**;
  settling it needs a depth-32 `xcb::GetImage` or an assertion on
  `PureCpuState::frame_buffer` (possible via Task 15's rig).
- **Task 8's reviewer left one falsifiable prediction:** make `subpixel_mask`
  return `[f32; 4]` and re-measure `AE fuzz 0` on `plain.sh`; it predicts ≈2458.
- **Nobody has photographed Task 4's ≤1 px fringe defect or its fix.**
- **Is option C worthwhile after this pass?** Spec §3's condition is now met.
- **The user has still not read `docs/purecpu-review/README.md`.**
- **Should focus change force a full repaint?** `inactive_pane_hsb` recolours
  panes on focus change and no `force_full` trigger covers it.

## 8. Staleness watch

- **Integration state must be re-derived, never inherited.**
  `git merge-base --is-ancestor HEAD main`, `git log --oneline HEAD..main`,
  `git branch -a --contains HEAD`. If this branch is merged, stop reading and go
  to the successor's handoff. Note `main` is upstream wezterm and legitimately
  runs ahead.
- **Sibling worktrees / other workstreams may exist that this file cannot name.**
- Task state in §2 reflects the ledger at this commit. **The ledger is
  authoritative; read it rather than trusting §2.**
- **`:20` is an ordinary user process and has died once already.** Verify with
  `DISPLAY=:20 XAUTHORITY=<path> xdpyinfo | grep dimensions` (expect `1280x1024`,
  `96x96 dpi`, depth 24) and check `marco` is on `:20` before any capture.
- **The measurement round's numbers are tied to `wezterm-gui-b10c3a5`
  (md5 `a9745eed…`) and to a shared, busy 128-core box.** Absolute percentages
  will drift with machine load; **the floor→subject→ceiling *ratios* are the
  result, not the percentages.** Re-run the round rather than comparing a new
  number against these.
- Suite sizes at this commit: `wezterm-gui` **116**, `wezterm-font` **16**.
  **Use the delta, not the absolute number.**
- Line numbers drift with every edit. **Every line number in an older report is
  suspect — grep.**
- Binaries named in older reports may not exist; three were deleted with the
  user's approval when `/scratch` was tight.
