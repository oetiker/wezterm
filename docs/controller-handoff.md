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

Handoff commit: see git log for this file (written at `ed7ba9e`)   Date: 2026-08-11   Reason: context budget
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

- **The GPU samples the atlas `Nearest`** for glyphs, emoji and images
  (`render/draw.rs:215-218`); `Linear` is only for the window background
  attachment (out of scope). So the missing resampler is nearest-neighbour and
  **bit-exact parity on scaled quads is reachable**, not approximate.
- **The LCD subpixel data is already in the atlas** — per-channel sRGB coverage
  in R/G/B with max-alpha in A (`skrifa_rasterizer.rs:695-701`). PureCpu's
  `IS_GLYPH` branch reads only `tex_a`. So subpixel AA is a compositing fix, not
  new rasterisation.

## 2. Where we are now

As of handoff commit `ed7ba9e` (re-derive merge/push state — see §8):

**Execution mode: subagent-driven development**, at the user's explicit request.
The ledger at `.superpowers/sdd/2026-08-11-purecpu-fixes/progress.md` is
authoritative on task state — trust it and `git log` over anything here.

- **Tasks 1, 2, 3, 5 — complete**, each reviewed clean or Approved-with-minors
  with all minors closed.
  - Task 1: harness prerequisites, bell guard, and the scaled-glyph font
    question answered YES from runtime evidence (Noto Color Emoji,
    `glyph.scale = 0.159`).
  - Task 2: the atlas ceiling. **C1 confirmed fixed at runtime**, not by
    argument: peak RSS 637 MiB against a pre-fix 4170 MiB, `AllowImage::Scale`
    fallback observed firing (I re-derived 172 where the reviewer said 168).
  - Task 3: the `dirty` unit (`purecpu_dirty.rs`) — `PanePlacement`,
    `pane_placement`, `row_band`, `cell_rect`, 8 tests, no callers by design.
  - Task 5: M6 + M7. `clear_rect` made total, `clamp_band` extracted, present
    loop routed through it, plus a straddle test and saturating arithmetic in
    `collect_clip_rects`/`coalesce_to_bands`.
- **Task 4 — IN FLIGHT at this commit.** Dispatched to subagent `task4-impl`
  (agent name `task4-impl-2`) against brief
  `.superpowers/sdd/2026-08-11-purecpu-fixes/task-4-brief.md`. **See §3.1 — it
  needs a specific kind of check on arrival, and it may already have landed.**
- **Tasks 6–14 — not started.** Suite is at 46 tests, 0 failed, 0 ignored.

**I changed the task order.** Task 5 now runs *before* Task 4. Task 4 is what
first makes an out-of-framebuffer rect reachable (its origins come from
`PositionedPane`, the mux's view of the tab, which is not synchronised to the
framebuffer size), and Task 5 is what makes `clear_rect` total. They touch
disjoint files, so the swap was free and it removes the window in which a panic
is reachable in committed code. The plan document was not rewritten — the tasks
are unchanged, only their order.

## 3. Do this next

1. **Deal with Task 4 first — and check the worktree, do not infer from
   silence.** An idle notification is not a report (§4). Three signatures,
   three opposite remedies: clean tree + commit → finished silently, go read
   `task-4-report.md`; dirty tree + no build running → stalled, run the gates
   and commit for it; dirty tree + a live `cargo` → deadlocked on a pending
   build, `SendMessage` the *same* agent to resume, never re-dispatch. Note
   other users' `cargo`/`rustc` processes are routinely visible on this box —
   check the command line before concluding one is ours.
2. **Task 4's report needs its §D runtime evidence re-derived, not read.** It is
   the first task in this pass whose core claim (a non-active pane repaints)
   cannot be shown by a unit test. Confirm the control capture from
   `wezterm-gui-prefix` actually shows the defect; if the pre-fix and fixed
   captures look the same, the corpus did not exercise the defect and the pass
   is a null result wearing a green hat.
3. **Then Task 6** (the `sampler` unit), and keep the 6/7 split — units are
   deliberately separated from their integrations so a reviewer can reject the
   arithmetic without rejecting the wiring.
4. **Nothing needs the user until Task 14.** They chose straight-through
   execution with a single review at the end. Do not check in between tasks.

## 4. Lessons & traps  ← the irreplaceable part

Carried forward from the prior sessions where still true, plus this one's.

- **A subagent's stdout lands in the human's real terminal.** The one that
  actually bit. Guarding wezterm's `audible_bell` config was the wrong layer:
  the exposure is *any* `0x07` byte in *any* command output. The rules that
  hold: redirect every harness/build/wezterm command to a log file and Read it;
  never `cat`/`head`/`tail` a binary file (`out/*.png`, `*.xwd` are full of
  BEL); never run a corpus script directly (`corpus/cursor.sh` rings the bell by
  design); guard first, before the first wezterm launch. Redirection is the
  durable rule because it does not depend on any config being correct at the
  moment a command runs — precisely the assumption that failed.
- **When the user interjects, answer on the next turn — not two tool calls
  later.** A user interjection outranks whatever is mid-flight.
- **Mutation testing is this pass's highest-yield instrument, and it caught
  something in every task that used it.** A suite that has only ever been seen
  going green proves the code compiles, not that the assertions discriminate.
  Task 3 shipped 8 tests that all passed first try; three of my mutations died
  correctly and a *fourth* — deleting `cell_rect`'s entire row guard — survived,
  which the reviewer found and I re-derived. Make every new test watch a mutant
  die before you accept it.
- **But check that the mutant actually perturbs the value the target test
  reads.** I specified a mutation (`x0.clamp(0, fb_w - 1)`) to validate Task 5's
  new straddle test; the implementer ran it and showed it does not touch that
  test's input at all — the failure came from a *pre-existing* test. Had it
  simply obeyed, the new test would have looked justified for the wrong reason.
  A mutation that goes red somewhere is not evidence about the test you are
  validating.
- **The plan's own text can be wrong, and a good implementer will follow it off
  a cliff.** Now **ten** times across three sessions. Three in this session:
  - Task 5's red gate was **unobservable as written** — it added all six tests
    at once and then expected to see both a runtime panic *and* a compile
    failure, which cannot both happen. Following it literally would have filed a
    compile error as the red gate and never witnessed M7. Splitting it into two
    stages is what produced the actual panic.
  - Task 5's prose said the negative-extent half of M7 "silently skips the row".
    It **panics** in debug (`attempt to multiply with overflow`). `findings.md`
    had this right; the *plan* lost the distinction and a test comment inherited
    it.
  - Task 4's Step 1 code **narrows repaint coverage** (see §5).
  Run a pre-flight scan of every task's plan text against the tree before
  dispatching. It has paid for itself every single time.
- **Ask implementers to flag their own weak points — it has been where the real
  finding was on every task so far.** Task 3's implementer named mutation
  testing as the check it had not run (correct, and the hole was there). Task
  5's named the untested call site (I re-derived it by reading; it was faithful).
  Task 2's named its static-trace argument, which is exactly what the runtime
  verification then settled.
- **Verify runtime preconditions; a static trace is not an observation.** The
  project's dominant failure mode is a measurement reporting a result the
  instrument or corpus could not actually see — now seen **ten** distinct ways.
  Every verdict must answer *"could this run have failed, and what would that
  have looked like?"*
- **Never take a resumed agent's gate result on its word — re-derive it against
  the commit.** A resumed subagent can report a stale pre-edit log as a fresh
  green run (obra/superpowers#2113). Generalise it: re-derive every load-bearing
  claim. It has cost one command each time and corrected a real error twice.
- **An idle notification is not a report.** Never conclude "done" or "stalled"
  from silence — inspect the tree. See §3.1 for the three signatures.
  **Deadlocked ≠ lost:** the edits are still in the worktree; `SendMessage` the
  same agent to resume.
- **Put `timeout: 600000` on every long Bash call in a dispatch prompt, and tell
  the agent never to end a turn while a background shell is live.** The cause is
  the subagent ending its turn, not the timeout: Claude Code backgrounds the
  command rather than killing it, the agent ends its turn anyway, and the shell
  dies with the turn (anthropics/claude-code#50572, closed "not planned").
  Banning backgrounding just converts this into a synchronous timeout.
- **State process constraints as actions, not prohibitions.** "Don't pipe the
  gate" gets ignored; "redirect every command to a log file and Read it" lands.
- **Reviewers earn their cost — every review in this pass has found at least one
  real defect.** Task 5's ran 85,800 `clear_rect` calls plus an exhaustive
  invariant sweep, which is what let it call a branch *dead* rather than assert
  it. Do not skip or downgrade them, especially near the end.
- **Reviewers are also wrong sometimes — verify their arithmetic too.** Task 5's
  claimed a specific wrong fix "passes the committed suite"; it did not. I
  passed that claim through to the implementer unchecked and the implementer
  caught it. Both directions need checking.
- **The control experiment is the strongest instrument this project has.**
  Prefer "change one thing and watch the defect disappear" over accumulating
  observations of the defect. **Put the control INSIDE the capture** so a null
  result cannot pass as success.
- **Subagents terminate on idle and their inline replies are frequently lost.**
  Always require the full report in a file and only a short summary inline.
  Held across four sessions.
- **Hold an idle implementer in reserve for its own fix round** rather than
  spending it on the next task; resuming keeps its context and probe scripts.
  Used for both Task 3 and Task 5 fix rounds; both landed in one turn.
- **`pkill` as a literal string in a Bash tool command kills the call with exit
  144 before anything runs.** Content-independent, 100% reproducible. Use
  `pgrep` and `kill -TERM`.
- `timeout N cat </dev/null` does **not** wait (cat hits EOF); use
  `timeout N tail -f /dev/null` — `pause` in `lib.sh` does this.
- **Focus state is a confound in any two-window X comparison.** Sequential
  capture is mandatory; an unfocused wezterm draws a hollow cursor, injecting a
  full character cell of difference that survives 12% fuzz. `focus-probe.sh` is
  the one deliberate exception.
- **llvmpipe is the reference and it works** — this GPU-less Xvnc box resolves
  OpenGL to Mesa llvmpipe 4.5, the only reason a GPU reference exists.
- `local a="$1" b="...$a..."` fails under `set -u`; split multi-variable `local`.
- **Harness trivia worth keeping:** the test module in `render/purecpu.rs` is
  `mod test` (**singular**); `purecpu_dirty.rs` uses `mod tests`. A brief that
  greps for `mod tests` in `purecpu.rs` will silently miss.

## 5. Don'ts & constraints

The plan's Global Constraints block is authoritative — read it. The ones that
matter most, plus this session's additions:

- **Task 4 must NOT use plain `row_band` for its dirty bands.** This is the
  single most important carried decision. `row_band` is exactly `cols * cell_w`
  wide; today's code emits `x: 0, width: fb_width`; the pane's *painted*
  background is wider than the former (`render/pane.rs:110-152` oversizes by
  half a cell into the split gutter, runs the right-most pane to
  `dimensions.pixel_width`, and starts at `x = 0` when `pos.left == 0`). So the
  plan's code silently **narrows** repaint coverage — stale window padding, a
  stale half-cell gutter, and clipped-but-uncleared glyph overhang past the last
  column, on ordinary text frames. The brief (`task-4-brief.md` §A) specifies a
  tested `painted_x_span` + `row_band_painted` in the unit, rounding **outward**
  so the band is always a superset of the painted rect. In the ordinary
  single-pane case this reproduces today's `x: 0, width: pixel_width` exactly.
  **Keep `row_band`** — it is still right for anything scoped to the text area
  and Task 3's tests cover it.
- **Task 9's bell fade inherits this too.** Any animation that changes a pane
  *background* inside an incremental frame must cover the painted extent or go
  through `force_full`. Note `force_full` is currently set for config/shape/quad
  generation changes, viewport scroll, selection change, `Exposed`, scroll info
  and tab-bar changes — **not** focus change.
- **Never touch X displays `:10`–`:14`** — other users on this shared machine.
  All testing on `:20`. Xauthority:
  `/tmp/claude-1003/-scratch-oetiker-wezterm/659fab62-7b17-4059-a502-42815bcb9732/scratchpad/Xauthority-20`
  (belongs to a *previous* session's scratchpad but is still alive and is what
  the running Xvnc authenticates against — do not "fix" the path to the current
  session; verify with `xdpyinfo` instead).
- **Nothing you run may emit raw bytes to the terminal.** See §4. Same standing
  as the display rule.
- **Never more than 4 cores**, including cargo (`-j4`).
- **Never run the 65536-atlas case** — 16 GiB on a shared ~25 GiB box.
- **Shared harness files** (`lib.sh`, `gen-config.sh`, `compare-case.sh`,
  `sample-case.sh`, `focus-probe.sh`) are load-bearing for the review's
  committed numbers: any change must keep default output byte-identical.
- **Never raise `FUZZ`**, and never grade a case against another case's number.
- **Every config comes from `gen-config.sh`.** If it refuses a combination you
  need, extend it — a hand-written config is the only way the bell guard can
  fire, and the only way a rejected config can silently make both windows use
  the same backend and report a triumphant `AE = 0`.
- **The OpenGL path must come out bit-identical.** Verified in Task 14, not
  assumed. Note the Task 5 review established that `clear_rect` is **not** on
  the full-repaint path (only the incremental branch), one less shared surface
  than I had assumed.
- **Background image, opacity and blur/HSB tint are settled as out of scope.**
- Verdict column in the matrix holds a **bare token**; the table is **6 cells per
  row** and a literal `|` in a cell silently breaks it — twice already.
- Do not commit `mise.toml` (untracked, intentional); `out/` and `.superpowers/`
  stay gitignored — the ledger is deliberately local-only.
- **Never overwrite `/scratch/oetiker/wezterm-builds/wezterm-gui-prefix`** — the
  pre-fix reference binary, and not reproducible now that fixes have landed.

## 6. Where the detail lives

- Change history: `git log ed7ba9e..HEAD`
- **Spec:** `docs/superpowers/specs/2026-08-11-purecpu-fixes-design.md` — §3
  carries the settled B-vs-C argument
- **Plan:** `docs/superpowers/plans/2026-08-11-purecpu-fixes.md` — 14 tasks;
  Global Constraints at the top bind every task. **Its Task 4 code is wrong —
  see §5.**
- **Ledger:** `.superpowers/sdd/2026-08-11-purecpu-fixes/progress.md` —
  **authoritative on task state**, and it now carries the full reasoning behind
  each review finding and controller decision, not just status lines. The
  sibling `.superpowers/sdd/2026-08-10-purecpu-parity-review/` belongs to the
  review plan; do not write to it.
- Per-task briefs/reports/reviews and diff packages: same directory,
  `task-N-{brief,report,review}.md`, `review-<base>..<head>.diff`
- **The review this repairs:** `docs/purecpu-review/{README,parity-matrix,findings,noise-floor}.md`
- `wezterm-gui/src/termwindow/purecpu_dirty.rs` — the dirty unit (Task 3)
- `wezterm-gui/src/termwindow/render/purecpu.rs` — `clear_rect`/`clamp_band` now
  total (Task 5); `:152-528` the 376-line quad loop; `:529` `blend_over`
- `wezterm-gui/src/termwindow/mod.rs:1272-1375` — the dirty-rect block Task 4
  rewrites; `:1377-1400` cursor blink (Task 8); `:1436-1447` the idle skip
- `wezterm-gui/src/termwindow/render/pane.rs:110-152` — the painted background
  extent rules Task 4 must mirror
- `wezterm-gui/src/termwindow/render/draw.rs:215-218` — the GPU's Nearest sampler
- `wezterm-font/src/rasterizer/skrifa_rasterizer.rs:695-701` — per-channel LCD
  coverage already in the atlas
- Binaries: `/scratch/oetiker/wezterm-builds/wezterm-gui-prefix` (pre-fix
  reference, **never overwrite**), `wezterm-gui-rebased`, `wezterm-gui-fixed`
  (rebuilt by later tasks). Harness selects via `WEZTERM_BIN`.

## 7. Open questions / pending decisions

- ~~Does the C1 bail reach the `AllowImage::Scale` fallback at runtime?~~
  **Settled: yes, observed; peak RSS 637 MiB.**
- **Does M2 survive Task 4?** Deferred on purpose (Task 11). Its attribution to
  a specific rect overlap was tested and eliminated twice; Task 4 rewrites the
  dirty-rect geometry that is the suspected source, so M2 may simply not exist
  afterwards. Re-measure before fixing.
- **Is option C worthwhile after this pass?** Spec §3: revisit once the sampler
  and dirty units have tests, which is what makes a rewrite safe.
- **The user has still not read `docs/purecpu-review/README.md`** — only
  summaries in conversation. Their reaction remains the one piece of feedback
  this workstream has never had.
- **M3/M4/M5 will be fixed blind** — their triggers were never reproduced
  (`fontTools` unavailable). The plan requires the report to say "panic site
  guarded", not "fixed". Hold that line.
- **Should focus change force a full repaint?** `inactive_pane_hsb` recolours
  panes on focus change, and no `force_full` trigger covers it. It is not a
  regression from this pass (a focus change produces no dirty rows today
  either), so it is unlisted-finding territory rather than in scope — but it is
  real. Decide explicitly rather than letting Task 9 discover it.

## 8. Staleness watch

- **Integration state must be re-derived, never inherited.** Whether this branch
  is merged, pushed or superseded is not knowable from this file:
  `git merge-base --is-ancestor HEAD main`, `git log --oneline HEAD..main`,
  `git branch -a --contains HEAD`. If merged, stop and go to the successor's
  handoff. Note `main` is upstream wezterm and legitimately runs ahead.
- **Sibling worktrees / other workstreams may exist that this file cannot name** —
  anything started after the handoff commit is invisible here.
- **Task 4 was in flight when this was written.** By the time you read this it
  has almost certainly resolved one way or another. `git log ed7ba9e..HEAD` and
  the ledger are the truth; §2 is not. If its commit is absent and the tree is
  dirty, read §3.1 before doing anything.
- Task state in §2 reflects the ledger at this commit. **The ledger is
  authoritative; read it rather than trusting §2.**
- The `:20` X server and its Xauthority are ordinary user processes/files and may
  be gone. Verify with
  `DISPLAY=:20 XAUTHORITY=<path> xdpyinfo | grep dimensions`.
- The Noto Color Emoji `glyph.scale = 0.159` result is font-installation
  dependent. It was observed on this machine at this commit; a font change
  invalidates it and Task 7's verification would silently become a null result.
- The suite was 46 tests at this commit. Task 4 adds to it; use the delta, not
  the absolute number, when grading later tasks.
