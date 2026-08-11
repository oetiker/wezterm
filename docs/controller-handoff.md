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

Handoff commit: written at `bb52ba3`, refreshed after Task 6 (see `git log bb52ba3..HEAD`)   Date: 2026-08-11   Reason: context budget
Worktree / branch: `/scratch/oetiker/wezterm` (primary checkout) @ `update-optimization-rebased`
Trunk at time of writing: `main` @ 05343b3 — **reader: if trunk has moved, §2 is provisionally stale; if trunk now contains this branch's HEAD, this file is a tombstone** (`git merge-base --is-ancestor HEAD main`). `main` here is upstream wezterm and legitimately runs ahead of this branch by ordinary upstream commits; that is NOT a merge signal.
Sibling worktrees: `/scratch/oetiker/claude-worktrees/wezterm-osc52-upstream` @ `osc52-x11-fix` — the two-commit upstream PR (wezterm/wezterm#8043), unrelated; leave it alone until that PR resolves. This line cannot see worktrees created later; check yourself.

## 1. Mission

A predecessor session **reviewed** this fork's PureCpu software renderer against
the GPU backend and produced documents only (`docs/purecpu-review/`). This
session is the **fix pass**: repair everything that review found, so
`front_end = "PureCpu"` on a GPU-less Linux/X11 box loses as little as possible
against `front_end = "OpenGL"`.

Scope, set by the user in brainstorming: **all 18 findings** (C1, I1–I5, M1–M7,
L1–L5), **plus** subpixel-antialiased text and the scaled fallback/bitmap glyph
row. The five `known gap` rows (background image, opacity, blur/HSB tint) stay
**out of scope** — never examined, so they are investigation, not repair.

The structural decision that shapes the code (spec §3): **option B — point fixes
plus two extracted units.** Three of the four root causes are *absences* (no
resampler, no pane origin in the dirty rect, no per-channel coverage), and an
absence fixed inline is invisible to the next reader. So a `dirty` unit and a
`sampler` unit get extracted and tested; everything else stays a point fix. The
user pushed back hard and asked why not option C (a full shader-equivalence
rewrite of the quad loop) — **that argument is settled in spec §3; do not
relitigate it, but do read it**: C restructures the site of only 5 of 18
findings and spends its risk where the code is already bit-identical to the GPU.
B is a strict *prefix* of C, so C stays available afterwards with tests in place.

Two load-bearing facts, both verified in code rather than assumed:

- **The GPU samples the atlas `Nearest`** for glyphs, colour emoji and grayscale
  quads (`render/draw.rs:215-218`, and glium's `Clamp` really is
  `GL_CLAMP_TO_EDGE`, checked). So the missing resampler is nearest-neighbour and
  **bit-exact parity on those quads is reachable**, not approximate — with the
  caveat in §2 about exact-ratio minification.
  **Correction, established by Task 6's review:** `Linear` serves the
  `has_color == 2.0` background-image branch, and **PureCpu does draw that
  branch** (`purecpu.rs:440`) — earlier versions of this handoff and the sampler's
  module doc both said it does not. It stays out of scope, but "out of scope"
  means *leave it alone*, not *it isn't there*. See §2's rulings.
- **The LCD subpixel data is already in the atlas** — per-channel sRGB coverage
  in R/G/B with max-alpha in A (`skrifa_rasterizer.rs:695-701`). PureCpu's
  `IS_GLYPH` branch reads only `tex_a`. So subpixel AA is a compositing fix, not
  new rasterisation.

## 2. Where we are now

As of handoff commit `bb52ba3` (re-derive merge/push state — see §8):

**Execution mode: subagent-driven development**, at the user's explicit request.
The ledger at `.superpowers/sdd/2026-08-11-purecpu-fixes/progress.md` is
authoritative on task state — trust it and `git log` over anything here.

- **Tasks 1, 2, 3, 5 — complete** (see the previous handoff at `ed7ba9e` via
  `git show ed7ba9e:docs/controller-handoff.md` for their detail; nothing about
  them has changed).
- **Task 4 — COMPLETE and closed.** Commit `bc05562`, review Approved with
  minors, fix rounds `a5c7639` (F1+F2) and `bb52ba3` (round 1b). Suite 46 → 54.
  What matters going forward:
  - Its **per-pane seqno map** (`last_seqno_by_pane`) was a deviation from the
    brief and is **vindicated by a capture**: a collapsed-baseline control binary
    goes flat for 14 samples while the subject tracks the GL reference
    sample-for-sample. Do not "simplify" it back to one field.
  - `painted_x_span` is `f32`, **term for term with `render/pane.rs`, including
    association order** (`x + ((cols*cw) + width_delta)`). See §4 — this is
    load-bearing and was got wrong once.
  - Two findings were recorded, not fixed: **no `painted_y_span`** (the vertical
    mirror of what Task 4 fixed on x) and **viewport-scroll `force_full` is still
    active-pane-only**. See §5 and §7.
- **Task 6 — landed at `e3d2c07`, reviewed Approved with minors, FIX ROUND 1 IN
  FLIGHT** with `task6-impl-2` (resumed) against `task-6-fix1-brief.md`. Suite
  54 → 63. **See §3.1 — it may already have landed.** The unit is
  `render/purecpu_sampler.rs`; no caller by design.
- **Tasks 7–14 — not started.** Suite is at 63 tests, 0 failed, 0 ignored.

**Two rulings from Task 6's review that Task 7 must inherit — do not
re-litigate, do read the reasoning in the ledger:**
- **Task 7 must NOT route `has_color == 2.0` (the background-image branch,
  `purecpu.rs:440`) through the sampler.** GL bilinearly filters that branch, and
  background image is out of scope by the user's own scope decision. Routing it
  is silent scope expansion *and* manufactures parity diffs that look like
  sampler bugs. The module doc claimed PureCpu "does not draw" that branch; it
  does, and that claim is being corrected in fix round 1.
- **Exact-ratio minification (2:1, 3:1, …) is the expected home of 1-texel parity
  diffs**, because every sample lands precisely on a texel boundary where
  hardware interpolation precision decides the tie. Triage a diff there as
  interpolation precision, not as a sampler defect.

**Task order was changed once** (Task 5 before Task 4) and that is done and
absorbed; tasks are otherwise in plan order. The plan document was never
rewritten — only the order changed.

## 3. Do this next

1. **Deal with Task 6's fix round first — and check the worktree, do not infer
   from silence.** An idle notification is not a report (§4). Three signatures,
   three opposite remedies: clean tree + commit → finished silently, go read the
   "Fix round 1" section of `task-6-report.md`; dirty tree + no build running →
   stalled, run the gates and commit for it; dirty tree + a live `cargo` →
   deadlocked on a pending build, `SendMessage` the *same* agent to resume, never
   re-dispatch. Note other users' `cargo`/`rustc` processes are routinely visible
   on this box — check the command line before concluding one is ours (the
   `mdmost-semantic-selection` worktree ran its own `cargo test` throughout this
   session).
2. **Re-derive the fix round's two mutant results rather than reading them:**
   with the strengthened identity axis (`Axis::new(64.0, 10.0, 100.25, 10.0, 4096)`),
   both M10 (identity special-cased) and M5 (`+ 0.5` dropped) must fail *on the
   identity test itself* at `63 → 64`. That is the whole point of the round — it
   converts three single-test guards into two-test guards.
3. **Then Task 7** (wire the sampler into the quad loop, plan line 1133). Keep
   the 6/7 split — units are deliberately separated from their integrations so a
   reviewer can reject the arithmetic without rejecting the wiring. Task 7's brief
   **must** require:
   - the two inherited rulings in §2 (no `has_color == 2.0` routing; exact-ratio
     minification triage);
   - a **composition test for the seam**. `cover_start`/`cover_end` and `texel`
     have never been used together — every Task 6 test drives `texel` with a
     hand-written pixel range. An off-by-one in how Task 7 joins them (iterating
     `cover_start..=cover_end` instead of `..`) is invisible to the current suite.
     This is the largest known gap, and it is precisely what Task 7 writes;
   - awareness that **mirrored background tiles** (negative `src_extent`) go from
     *silently dropped* to *drawn* — correct, but a behaviour change nobody has
     written down.
   Task 7 is where the four bit-identical parity rows are at risk.
4. **Do not run a review and an implementation concurrently in this worktree.**
   Both edit the tree and build; they will corrupt each other. One at a time.
5. **Nothing needs the user until Task 14**, with one exception now queued: the
   `painted_y_span` scope question (§7). They chose straight-through execution
   with a single review at the end. Do not check in between tasks.

## 4. Lessons & traps  ← the irreplaceable part

Carried forward from the prior sessions where still true, plus this one's.

- **A subagent's stdout lands in the human's real terminal.** Guarding wezterm's
  `audible_bell` config was the wrong layer: the exposure is *any* `0x07` byte in
  *any* command output. The rules that hold: redirect every harness/build/wezterm
  command to a log file and Read it; never `cat`/`head`/`tail` a binary file
  (`out/*.png`, `*.xwd` are full of BEL); never run a corpus script directly
  (`corpus/cursor.sh` rings the bell by design); guard first, before the first
  wezterm launch. Redirection is the durable rule because it does not depend on
  any config being correct at the moment a command runs.
- **When the user interjects, answer on the next turn — not two tool calls
  later.** A user interjection outranks whatever is mid-flight.
- **Ask implementers to flag their own weak points. This has now been where the
  real finding was on EVERY task in the pass — six for six.** Task 4 is the
  strongest case: its implementer named the per-pane seqno map as reasoning it
  had verified by reading rather than measuring, and the reviewer, told to attack
  exactly that, built the discriminating control that turned it into the most
  valuable change in the commit. Make the weakest-point section mandatory, and
  **spend the next agent's effort on what it names.**
- **Mutation testing is this pass's highest-yield instrument, and it caught
  something in every task that used it.** Make every new test watch a mutant die
  before you accept it.
- **But check that the mutant actually perturbs the value the target test
  reads.** Made and caught three times now. Corollary learned this session, when
  re-deriving a fix round myself: **pick the mutation that ISOLATES the assertion
  under test.** Task 4's MT1 (truncate everywhere) also moves the left edge, so
  the *tightness* assertion fires first and the superset assertion — the one
  encoding the finding — is never exercised. MT2 (truncate only where the value
  feeds the right edge) is the one that proves it. A mutation that goes red is
  not evidence about the assertion you care about.
- **Do not bank the first red you see — classify it.** Task 4's fix round ran its
  extended sweep and the first failure was an *over*-cover, not the under-cover
  the finding predicted. The implementer replaced the assertions with a collector
  and counted by direction (4086 under-right, 0 under-left) instead of accepting
  a red as confirmation. That is the standard.
- **When a test compares against an oracle, decide explicitly which side is
  ground truth — and put the slack on the OTHER one.** Settled this session for
  `painted_x_span`: what gets rasterised is the `f32` rect `render/pane.rs`
  actually computes, so the `f32` term-for-term oracle is ground truth and its
  superset assertion is **strict, no epsilon**; the `f64` evaluation is a
  *different approximation* and it is the one that takes slack. Inverting it
  pushes the slack onto the correctness assertion, which is precisely what lets a
  real 1 px miss through. Generalise: slack on the assertion that encodes the
  defect is how a suite goes quietly blind.
- **Floating-point association order is part of "term for term".** Task 4 round 1
  claimed its `f32` expression matched `pane.rs` — the entire justification for
  choosing `f32` over `f64` — and it did not: `pane.rs` builds a rect of
  `(x, width)` so its right edge is `x + ((cols*cw) + width_delta)`, while the
  implementation computed `(x + cols*cw) + width_delta`. Different `f32` by up to
  an ulp, enough to flip a `ceil`. Never mattered in practice, but the *claim*
  was false. Related and more useful: because the span is computed from the same
  `f32` value the paint pass uses, **drift moves both together and cannot by
  itself cause an under-cover, at any magnitude** — the risk was never "f32
  drifts", it was "our expression is not their expression".
- **The plan's own text can be wrong, and a good implementer will follow it off a
  cliff — but not always.** Ten times across three sessions, and then **Task 6's
  pre-flight came back clean** (I recomputed all eight test expectations by hand;
  they are internally consistent). Keep running the scan — it has paid for itself
  every time it found something, and a clean result costs ten minutes. Do not
  generalise either way.
- **A test that asserts no value discriminates almost nothing.** Task 6's
  `degenerate_extents_do_not_panic` is `let _ = a.texel(5)`, so deleting the guard
  it exists to protect left the plan's entire eight-test suite green while the
  function returned **three different answers** depending on which unwritten
  language rule caught it (guard → 10; `NaN → 0`; `inf → i32::MAX → 4095`). Pin
  values, not the absence of a panic.
- **Ask "which single test is the only guard on X?" — the answer is usually
  alarming.** Task 6's review found the half-texel `+ 0.5` guarded by
  `minifies_two_to_one` alone, `dest_origin` guarded by the identity test alone,
  and the identity test unable to kill its own named mutant. All three collapsed
  into **one added assertion** with a sub-pixel origin. Single-test guards are
  invisible until someone enumerates them; enumerate them.
- **A reviewer that RUNS its proposed fix is worth several that argue for one.**
  Task 6's review did not merely claim the identity test could be strengthened —
  it wrote the axis, ran both mutants against it, and pasted the `63 → 64`
  failures. That is the standard to brief reviewers toward.
- **Verify library semantics, not just your own code.** The same review checked
  that glium's `SamplerWrapFunction::Clamp` maps to `GL_CLAMP_TO_EDGE` rather than
  legacy `GL_CLAMP` (which blends toward a border colour), and that
  `to_texture_coords` uses exact sprite edges rather than the very common
  `(i+0.5)/w` half-texel inset. Either could have made the whole unit
  confidently wrong with every test agreeing.
- **Verify runtime preconditions; a static trace is not an observation.** The
  project's dominant failure mode is a measurement reporting a result the
  instrument or corpus could not actually see. Every verdict must answer *"could
  this run have failed, and what would that have looked like?"*
- **The control experiment is the strongest instrument this project has.** Prefer
  "change one thing and watch the defect disappear" over accumulating
  observations of the defect. **Put the control INSIDE the capture** so a null
  result cannot pass as success. Task 4's review is the model: it built a control
  binary differing *only* in the mechanism under test.
- **Beware a case that passes for the wrong reason.** Task 4's review found the
  pre-fix binary *passing* the seqno case — because pre-fix full-width bands
  incidentally covered the background pane's row 0. That is I2 masking I1, and it
  disqualifies that case as an I1 control. Ask *why* a run passed, not just
  whether.
- **Never take a resumed agent's gate result on its word — re-derive it against
  the commit.** Generalise it: re-derive every load-bearing claim. It has cost one
  command each time and corrected a real error twice.
- **An idle notification is not a report.** Three times this session, all three
  were "finished silently" (clean tree + commit + report file) — but that was
  established by looking, not assumed. See §3.1 for the signatures.
  **Deadlocked ≠ lost:** the edits are still in the worktree; `SendMessage` the
  same agent to resume.
- **"Hold an idle implementer in reserve for its own fix round" only works
  WITHIN a session.** An agent from a previous session is not addressable —
  `ListAgents` shows only in-session subagents and peer sessions. Task 4's
  implementer had finished and gone; its fix round needed a fresh agent briefed
  from the report. Plan the round while the agent is still live, or accept the
  re-briefing cost. Resuming *within* the session worked well (round 1b landed in
  one turn, with the sharpest finding of the task).
- **Put `timeout: 600000` on every long Bash call in a dispatch prompt, and tell
  the agent never to end a turn while a background shell is live.** The cause is
  the subagent ending its turn, not the timeout: Claude Code backgrounds the
  command rather than killing it, the agent ends its turn anyway, and the shell
  dies with the turn (anthropics/claude-code#50572, closed "not planned").
- **State process constraints as actions, not prohibitions.** "Don't pipe the
  gate" gets ignored; "redirect every command to a log file and Read it" lands.
- **Reviewers earn their cost — every review in this pass has found at least one
  real defect.** Do not skip or downgrade them, especially near the end.
- **Reviewers are also wrong sometimes — verify their arithmetic too.** Both
  directions need checking.
- **Subagents terminate on idle and their inline replies are frequently lost.**
  Always require the full report in a file and only a short summary inline.
  Held across five sessions.
- **`/scratch` on this box silently corrupts large writes.** Copying a fresh
  binary into `/scratch/oetiker/wezterm-builds/` produced 10.1 MB of zeros from
  offset ~73.6 MB, **twice**, with `cp` reporting success and size/mtime matching.
  It fails *loudly* at runtime (panic at `config/src/version.rs:7`, no window), so
  it cannot fake a green result — but it cost a reviewer an hour. **`md5sum` every
  binary copy before using it.** `wezterm-gui-t4rev-subject` is one of the corrupt
  copies; do not use it.
- **Build AFTER committing**, so the embedded CI tag pins the evidence to a real
  SHA. Task 4's original §D numbers came from a binary built while HEAD was still
  the previous commit — legitimate, but unverifiable after the fact.
- **`pkill` as a literal string in a Bash tool command kills the call with exit
  144 before anything runs.** Use `pgrep` and `kill -TERM`.
- `timeout N cat </dev/null` does **not** wait (cat hits EOF); use
  `timeout N tail -f /dev/null` — `pause` in `lib.sh` does this.
- **Focus state is a confound in any two-window X comparison.** Sequential
  capture is mandatory; an unfocused wezterm draws a hollow cursor, injecting a
  full character cell of difference that survives 12% fuzz.
- **llvmpipe is the reference and it works** — this GPU-less Xvnc box resolves
  OpenGL to Mesa llvmpipe 4.5, the only reason a GPU reference exists.
- `local a="$1" b="...$a..."` fails under `set -u`; split multi-variable `local`.
- **Harness trivia worth keeping:** the test module in `render/purecpu.rs` is
  `mod test` (**singular**); `purecpu_dirty.rs` uses `mod tests`.

## 5. Don'ts & constraints

The plan's Global Constraints block is authoritative — read it. The ones that
matter most, plus this session's additions:

- **Never touch X displays `:10`–`:14`** — other users on this shared machine.
  All testing on `:20`. Xauthority:
  `/tmp/claude-1003/-scratch-oetiker-wezterm/659fab62-7b17-4059-a502-42815bcb9732/scratchpad/Xauthority-20`
  (belongs to a *previous* session's scratchpad but is still alive and is what
  the running Xvnc authenticates against — do not "fix" the path to the current
  session; verify with `xdpyinfo` instead). Confirmed alive at 1280x1024 this
  session.
- **Nothing you run may emit raw bytes to the terminal.** See §4. Same standing
  as the display rule.
- **Never more than 4 cores**, including cargo (`-j4`).
- **Never run the 65536-atlas case** — 16 GiB on a shared ~25 GiB box.
- **Do not collapse `last_seqno_by_pane` back to a single field.** It looks like
  redundant bookkeeping and it is not; a capture proves the collapsed version
  leaves a background pane frozen. §2.
- **The horizontal extent rule now lives in THREE places** — `render/pane.rs`
  :110-152 (`paint_pane`), :606-646 (`build_pane`), and `purecpu_dirty::painted_x_span`
  — with nothing tying them together. They agree today (verified term for term
  this session). Nothing in the build will notice when they stop. If you touch one,
  touch all three.
- **Task 9's bell fade inherits the y-axis gap.** Any animation that changes a
  pane *background* inside an incremental frame must cover the painted extent or
  go through `force_full`. Note `force_full` is set for config/shape/quad
  generation changes, viewport scroll, selection change, `Exposed`, scroll info
  and tab-bar changes — **not** focus change.
- **Shared harness files** (`lib.sh`, `gen-config.sh`, `compare-case.sh`,
  `sample-case.sh`, `focus-probe.sh`) are load-bearing for the review's committed
  numbers: any change must keep default output byte-identical. `gen-config.sh`
  gained `SPLIT_DIR`/`SPLIT_FIRST`/`SPLIT_SECOND` in Task 4, verified
  byte-identical on default output by two independent `cmp` runs.
- **Never raise `FUZZ`**, and never grade a case against another case's number.
- **Every config comes from `gen-config.sh`.** If it refuses a combination you
  need, extend it — a hand-written config is the only way the bell guard can fire,
  and the only way a rejected config can silently make both windows use the same
  backend and report a triumphant `AE = 0`.
- **The OpenGL path must come out bit-identical.** Verified in Task 14, not
  assumed. `clear_rect` is **not** on the full-repaint path (only the incremental
  branch). Task 4's diff was confirmed to permit no GL behaviour change at all.
- **Background image, opacity and blur/HSB tint are settled as out of scope.**
- Verdict column in the matrix holds a **bare token**; the table is **6 cells per
  row** and a literal `|` in a cell silently breaks it — twice already.
- Do not commit `mise.toml` (untracked, intentional); `out/` and `.superpowers/`
  stay gitignored — the ledger is deliberately local-only.
- **Never overwrite `/scratch/oetiker/wezterm-builds/wezterm-gui-prefix`** — the
  pre-fix reference binary, and not reproducible now that fixes have landed.

## 6. Where the detail lives

- Change history: `git log bb52ba3..HEAD`; previous handoff `git show ed7ba9e:docs/controller-handoff.md`
- **Spec:** `docs/superpowers/specs/2026-08-11-purecpu-fixes-design.md` — §3
  carries the settled B-vs-C argument
- **Plan:** `docs/superpowers/plans/2026-08-11-purecpu-fixes.md` — 14 tasks;
  Global Constraints at the top bind every task. Task 6 begins at line 913,
  Task 7 at 1133. **Its Task 4 code was wrong** (used plain `row_band`); that is
  now resolved in code, but treat the remaining task text with the same suspicion.
- **Ledger:** `.superpowers/sdd/2026-08-11-purecpu-fixes/progress.md` —
  **authoritative on task state**, and it carries the full reasoning behind each
  review finding and controller decision, not just status lines. The sibling
  `.superpowers/sdd/2026-08-10-purecpu-parity-review/` belongs to the review plan;
  do not write to it.
- Per-task briefs/reports/reviews and diff packages: same directory,
  `task-N-{brief,report,review}.md`, `task-4-fix1-{brief,report}.md`,
  `review-<base>..<head>.diff`
- **The review this repairs:** `docs/purecpu-review/{README,parity-matrix,findings,noise-floor}.md`
- `wezterm-gui/src/termwindow/purecpu_dirty.rs` — the dirty unit (Tasks 3, 4)
- `wezterm-gui/src/termwindow/render/purecpu_sampler.rs` — the sampler unit (Task 6)
- `wezterm-gui/src/termwindow/render/purecpu.rs` — `clear_rect`/`clamp_band` total
  (Task 5); `:152-528` the 376-line quad loop; `:529` `blend_over`
- `wezterm-gui/src/termwindow/mod.rs` — `do_paint_purecpu` spans :1183-1497; the
  dirty-rect walk, the cursor-blink block at :1408-1445 (Task 8), the idle skip
- `wezterm-gui/src/termwindow/render/pane.rs:110-152` and `:606-646` — the painted
  background extent rules, twice
- `wezterm-gui/src/termwindow/render/draw.rs:215-218` — the GPU's Nearest sampler
- `wezterm-font/src/rasterizer/skrifa_rasterizer.rs:695-701` — per-channel LCD
  coverage already in the atlas
- Binaries: `/scratch/oetiker/wezterm-builds/wezterm-gui-prefix` (pre-fix
  reference, **never overwrite**), `wezterm-gui-fixed`, `wezterm-gui-t4rev-seqno-control`
  (the Task 4 review's collapsed-baseline control, verified good),
  `wezterm-gui-t4rev-subject` (**corrupt, do not use**). Harness selects via `WEZTERM_BIN`.

## 7. Open questions / pending decisions

- **Does M2 survive Task 4?** Deferred on purpose (Task 11). Its attribution to a
  specific rect overlap was tested and eliminated twice; Task 4 rewrote the
  dirty-rect geometry that is the suspected source, so M2 may simply not exist
  now. Re-measure before fixing. **Task 4's review Finding 4 goes with it:**
  viewport-scroll `force_full` is still active-pane-only (`mod.rs:1233-1241`,
  a single `Option`, resolved for the active pane alone), so scrolling a
  non-active pane's viewport produces no dirty rows and no repaint. Same family
  as I1, not a regression, **never reproduced at runtime by anyone.**
- **`painted_y_span` — a scope question for the user, queued for Task 14.**
  Bands are exactly `cell_h` tall, so the window's top/bottom padding, the
  half-cell vertical overshoot of `background_rect`, the divider row of a
  `Top`/`Bottom` split, and descender/box-drawing overhang past the bottom of a
  cell are covered by no band. Pre-existing (Task 3's `row_band` and the older
  code had it too), so not a regression — but it is the exact mirror on y of what
  Task 4 just fixed on x, and it is newly reachable in a shape that matters now
  that per-pane y origins are correct. It is an unlisted finding, i.e. scope
  expansion. **Ask, do not decide unilaterally.**
- **Nobody has photographed Task 4's ≤1 px fringe defect or its fix.** All
  evidence for review Finding 1 is arithmetic. The measurement that would settle
  it — a fractional `window_padding` knob in `gen-config.sh` plus an I2-B re-run —
  is **assigned to Task 14** by controller decision. Do not let it evaporate: it
  is the last unexamined link between "the numbers are right" and "the screen is
  right".
- **Is option C worthwhile after this pass?** Spec §3: revisit once the sampler
  and dirty units have tests, which is what makes a rewrite safe. Task 6/7 is the
  moment that condition is met.
- **The user has still not read `docs/purecpu-review/README.md`** — only
  summaries in conversation. Their reaction remains the one piece of feedback this
  workstream has never had.
- **M3/M4/M5 will be fixed blind** — their triggers were never reproduced
  (`fontTools` unavailable). The plan requires the report to say "panic site
  guarded", not "fixed". Hold that line.
- **Should focus change force a full repaint?** `inactive_pane_hsb` recolours
  panes on focus change, and no `force_full` trigger covers it. Not a regression
  from this pass, so unlisted-finding territory rather than in scope — but real.
  Decide explicitly rather than letting Task 9 discover it.

## 8. Staleness watch

- **Integration state must be re-derived, never inherited.** Whether this branch
  is merged, pushed or superseded is not knowable from this file:
  `git merge-base --is-ancestor HEAD main`, `git log --oneline HEAD..main`,
  `git branch -a --contains HEAD`. If merged, stop and go to the successor's
  handoff. Note `main` is upstream wezterm and legitimately runs ahead.
- **Sibling worktrees / other workstreams may exist that this file cannot name** —
  anything started after the handoff commit is invisible here.
- **Task 6 was in flight when this was written.** By the time you read this it has
  almost certainly resolved one way or another. `git log bb52ba3..HEAD` and the
  ledger are the truth; §2 is not. If its commit is absent and the tree is dirty,
  read §3.1 before doing anything.
- Task state in §2 reflects the ledger at this commit. **The ledger is
  authoritative; read it rather than trusting §2.**
- The `:20` X server and its Xauthority are ordinary user processes/files and may
  be gone. Verify with
  `DISPLAY=:20 XAUTHORITY=<path> xdpyinfo | grep dimensions`.
- The Noto Color Emoji `glyph.scale = 0.159` result is font-installation
  dependent. A font change invalidates it and Task 7's verification would silently
  become a null result.
- The suite was **63 tests** after Task 6 (54 before it). Use the delta, not the absolute
  number, when grading later tasks.
- Line numbers in §6 were accurate at this commit and drift with every edit to
  `mod.rs`. Grep, don't trust them.
