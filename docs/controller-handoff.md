# Controller Handoff — PureCpu parity review (wezterm fork)

> Starter pack for the next controller session. This handoff lives in ONE
> worktree — run `git worktree list` first and confirm this is the workstream
> you're resuming. Read this first, then `git log <handoff-commit>..HEAD` for
> everything that changed since. Detail is NOT here — it's in git + the
> superpowers plan/ledger/docs (§6). Before you rewrite this file at your own
> handoff: read the previous version (`git show HEAD:docs/controller-handoff.md`)
> and carry forward any lesson in §4/§5 that is still true. Fresh synthesis,
> not blank page. On merge into another branch, rewrite that branch's handoff
> to the merged reality — do not merge or preserve this text.

Handoff commit: c057c04   Date: 2026-08-11   Reason: plan complete
Worktree / branch: `/scratch/oetiker/wezterm` (primary checkout) @ `update-optimization-rebased`
Trunk at time of writing: `main` @ 05343b3 — **reader: if trunk has moved, §2 is provisionally stale; if trunk now contains this branch's HEAD, this file is a tombstone** (`git merge-base --is-ancestor HEAD main`). `main` here is upstream wezterm and runs ahead of this branch by ordinary upstream commits; that is normal and is NOT a merge signal.
Sibling worktrees: `/scratch/oetiker/claude-worktrees/wezterm-osc52-upstream` @ `osc52-x11-fix` — the two-commit upstream PR (wezterm/wezterm#8043), unrelated to this review; leave it alone until that PR resolves. This line cannot see worktrees created later; check yourself.

## 1. Mission

The user's fork carries two large unreviewed changes on top of upstream
`e723cf5`: a pure-Rust font stack (skrifa/harfrust/fontdb) and a new `PureCpu`
software renderer for GPU-less operation. The job is to **review the patch and
establish where PureCpu falls short of the GPU backend** — producing a defect
findings list and an evidence-backed parity matrix, *not* fixes. The user
decides what gets fixed after reading both.

The mental model that matters: PureCpu is not a separate pipeline. It consumes
the **same layer/quad stream** the GPU path emits, from the **same single glyph
atlas**. So parity gaps are not "missing features" — they are places where the
hand-written rasteriser cannot reproduce what the GPU's fixed function does.
**Three** root causes now explain nearly every confirmed parity defect: no
texture rescaling (`purecpu.rs:346` blits 1:1 and crops), the idle-skip
(`termwindow/mod.rs:1436-1447` returns early, so nothing time-driven repaints),
and the raw-vs-resolved cursor shape (`mod.rs:1383`). The most severe defect in
the whole review is none of those: the atlas has no size ceiling, so a ~1.5 KB
sixel sequence allocates 4 GiB (findings C1), and that arm is fork-introduced.

Scope is narrow by decision: personal fork, Linux/X11 only, no macOS/Windows/
Wayland, no upstreamability. Background image and transparency are out of scope.

## 2. Where we are now

As of handoff commit c057c04 (re-derive merge/push state — see §8):

**All eight tasks are complete.** Each was implemented by a subagent, reviewed
by a second, and fix-looped to a clean re-review. The ledger is authoritative.

- **Tasks 1–3** — static audit, harness, noise-floor gate: PROCEED for the
  terminal body (PAE=257 = 1 LSB), **STOP for the tab strip**, which therefore
  has no calibrated noise floor to grade against.
- **Task 4 (inline images)** — 4 fix rounds. Sixel, iTerm2 and animated GIF all
  `degraded`.
- **Task 5 (chrome)** — 2 fix rounds. Fancy tab bar `degraded` (two classes: 37
  columns a clean 1px shift, 99 columns sub-pixel divergence ~77 LSB); retro tab
  bar, window buttons, rounded corners, split dividers all `parity`.
- **Task 6 (cursor/animation)** — 2 fix rounds. Cursor static `parity` (block,
  bar and underline all cell-bit-identical); cursor blink `missing` for
  config-driven blink, with the app-driven DECSCUSR path separately `degraded`;
  blink-attribute text `missing`; visual bell `missing`. Established the third
  root cause.
- **Task 7 (defect findings)** — 1 fix round, closed clean. `findings.md`:
  1 Critical, 5 Important, 7 Medium, 5 Low, plus seven documented non-findings.
  Ids were **renumbered** during the fix round (now C1 / I1–I5 / M1–M7 / L1–L5);
  ledger lines written before that use the old scheme.
- **Task 8 (consolidation)** — 2 fix rounds, closed clean. Produced
  `docs/purecpu-review/README.md`, the summary the user actually asked for.

**THE PLAN IS COMPLETE.** All eight tasks closed, each implemented by one
subagent, reviewed by an independent second, and fix-looped to a clean
re-review. Deliverables: `docs/purecpu-review/{README,parity-matrix,findings,
noise-floor}.md` plus the `tools/purecpu-parity/` harness. Matrix: 21 rows
(14 measured / 2 by-reading / 5 out-of-scope; 6 `parity`, 7 `degraded`,
3 `missing`, 5 `known gap`). Findings: 18 defects (1 Critical, 5 Important,
7 Medium, 5 Low) plus 7 documented non-findings.

Task 8 also carried an authorised addendum: **DECDWL/DECDHL was measured**,
because two by-reading verdicts had already been overturned by measurement and
a third was about to ship unmeasured. It **confirmed** the prediction (body
PAE=45232 against a 257 gate; GL lays down exactly 2.01x the ink) — the first
by-reading prediction in the plan that measurement upheld — and turned up an
unpredicted sub-case: the DECDHL *bottom* half is not drawn at all.

Task 7 produced findings the parity harness was structurally blind to: **dirty
tracking consults only the active pane** (other panes of a split never repaint
until something forces a full repaint), and **dirty-rect geometry omits
`pos.top`/`pos.left`** (even the active pane freezes when not at the window's
top-left). For a user of this fork these may matter more than the matrix does.
The blindness is **not** that captures were settled — Task 4 added a
non-settling sampler. It is that every case put the content under test in a
single full-width pane, where both offsets are zero and the active pane is the
only one with live output (`findings.md:244-245`). A corpus can be blind along
an axis nobody thought to vary, not only along the axis a tool cannot see.

The wide-sixel label-row anomaly the plan carried since Task 4 is **diagnosed**:
PureCpu composites cell 0's glyph pixels twice (a coverage model predicts all 87
differing pixels within 1.91 LSB). The *mechanism* that can do it is in the code
by construction; the specific route was attributed, then **tested and
eliminated** across two deliberate reproductions. The remaining evidence needed
is an instrumented dump of `dirty_pixel_rects` at the frame that produced it.

## 3. Do this next

**Nothing is in flight. Do not start work here without asking the user first** —
the plan is done and what follows is all their call.

1. **The user has still read none of it.** Point them at
   `docs/purecpu-review/README.md` and let them drive. Lead with the two things
   they'd act on: findings C1 (a ~1.5 KB sixel sequence allocates 4 GiB, on a
   fork-introduced arm) and I1/I2 (split panes stop repainting).
2. **Nothing has been fixed — by design.** The plan produced documents only.
   What gets fixed, and in what order, has never been put to the user. If they
   ask, the cheap high-value ones are C1 (a bounds check on one arm) and the 1:1
   blit (nearest-neighbour scaling in one loop would address inline images,
   double-width/height lines and scaled bitmap emoji together). **Fixing means a
   new plan — the constraints in §5 assume a read-only source tree.**
3. A whole-branch review pass — the fork's patch as a whole, beyond the
   PureCpu/font/X11 scope Task 7 covered — remains undone. The ledger's ~12
   deferred minors are its natural input.

## 4. Lessons & traps  ← the irreplaceable part

- **The dominant failure mode of this entire plan is a silent false verdict** —
  a measurement reporting a result the instrument or corpus could not actually
  see. It has now appeared eight or nine distinct ways: a corpus requesting
  images at native size so the crop was a no-op; `capture_settled` defining
  success as "nothing changed" and so structurally excluding animation; a split
  divider never on screen; a blink rate with a non-blinking cursor shape; two
  blank captures; a rejected config that took down `front_end` so both windows
  ran the same backend; and — Task 6's inversion — a **false `missing`**, where a
  flat sample sequence is exactly what a window that never rendered also
  produces. **Every verdict must answer "could this run have failed, and what
  would that have looked like?"** Put it in every dispatch; it is what earns the
  review rounds.
- **The control experiment is the strongest instrument this plan found.** Task
  6's reviewer settled three verdicts at once by setting
  `purecpu_force_full_repaint = true` on an otherwise identical config and
  showing the bell and blink text return at the GPU path's own levels. Prefer
  "change one thing and watch the defect disappear" over accumulating more
  observations of the defect. It converts "we observed X" into "X is caused by Y".
- **A printed command that nobody re-runs is a lie waiting to happen.** This has
  now happened **five** times, and twice the wrong numbers were the reassuring
  ones. The latest: the sole Critical's reproduce block used `pgrep | head -1`,
  selecting the parent process, so it printed 4 MiB for a 4 GiB finding. Make
  both implementer and reviewer copy commands out of the *rendered* file and run
  them. Any env var the command needs goes IN the command.
- **A fix can create the very failure it repaired.** Task 6's I1 fix promoted a
  probe to load-bearing evidence while a section elsewhere still said the probe
  "is not itself matrix evidence" — the document then contradicted itself, with
  no runnable command behind its new numbers. Scope re-reviews to include "what
  did the repair break?", not only "was the finding addressed?".
- **Findings can be right about the phenomenon and wrong about its geometry.**
  Task 7's M2 measured a real double-composite but binned it in 8-px bins on
  10-px cells, concluding "two cells" and building an attribution on it. Check
  that a measurement's *bins* align with the thing being measured.
- **The plan's own text can be wrong, and a good implementer will follow it off a
  cliff.** Five times now (FUZZ=3, the native-size corpus, Task 6's brief steps,
  the `sed`-edited config, Task 7's `cargo test -p wezterm-gui --lib` against a
  binary crate). When a review objects to a *plan-mandated* choice, the plan is
  usually what needs fixing. Scan each brief against the Global Constraints
  **before** dispatching and resolve conflicts in the dispatch itself.
- **Amend the plan's Global Constraints when a lesson generalises.** Rules added
  there are inherited by every later task for free — the cheapest leverage the
  controller has. Latest addition: *grade a row against its own named feature,
  not the mechanism behind it* (which is why animated GIF is `degraded` while
  cursor blink is `missing`, despite one shared root cause).
- **Ask implementers to flag their own weak points.** Every task that did so
  closed faster, and the self-flagged items were repeatedly where the real
  findings were. Task 7 listed seven; the reviewer's Critical was adjacent to two.
- **Subagents terminate on idle and their inline replies are frequently lost.**
  Always require the full report in a file and only a short summary inline. This
  held for every dispatch across two sessions.
- **Resuming the same implementer for fix rounds and the same reviewer for scoped
  re-reviews works well** — they keep their probe scripts and prior reasoning,
  and reviewers verified their own findings honestly rather than rubber-stamping.
  Hold an idle implementer in reserve for its own fix round rather than spending
  it on the next task.
- **Reviewers here earn their cost.** Every single review across both sessions
  found at least one Critical or Important defect that would have shipped a wrong
  claim into a deliverable. Do not skip or downgrade them near the end.
- `timeout N cat </dev/null` does **not** wait (cat hits EOF); use
  `timeout N tail -f /dev/null` — `pause` in `lib.sh` does this.
- **Focus state is a confound in any two-window X comparison.** Sequential
  capture is mandatory; an unfocused wezterm draws a hollow cursor.
- Captures are deterministic once focus is held constant (AE=0 for a backend
  against itself), so any non-zero self-diff means the *method* is wrong.
- **llvmpipe is the reference and it works** — this GPU-less Xvnc box resolves
  OpenGL to Mesa llvmpipe 4.5, which is the only reason a GPU reference exists.
- `local a="$1" b="...$a..."` fails under `set -u`; split multi-variable `local`.

## 5. Don'ts & constraints

- **Never touch X displays `:10`–`:14`** — other users on this shared machine.
  All testing is on `:20`. Xauthority:
  `/tmp/claude-1003/-scratch-oetiker-wezterm/659fab62-7b17-4059-a502-42815bcb9732/scratchpad/Xauthority-20`
  (session-scoped, but still alive at this commit; if gone, recreate per the
  plan's "Environment setup" section and restart Xvnc).
- **Never more than 4 cores.**
- **Never run the 65536-atlas case** — 16 GiB on a shared ~25 GiB box. The 32768
  case (4 GiB) is already measured; the next step is arithmetic, not execution.
- **This plan produces documents, not fixes.** All Rust source is read-only.
  Harness scripts under `tools/purecpu-parity/` *may* be fixed — that
  distinction was ruled on explicitly and matters.
- **Never raise `FUZZ`, and never grade a case against another case's number.**
  Grade against a stated basis.
- Do not rebuild wezterm; `/scratch/oetiker/wezterm-builds/wezterm-gui-rebased`
  is the artifact under test and matches this branch.
- Shared harness files (`lib.sh`, `gen-config.sh`, `compare-case.sh`,
  `sample-case.sh`) are load-bearing for Tasks 1–6's committed results: any
  change must keep default output byte-identical, verified.
- Verdict column holds a **bare token** (`parity` / `degraded` / `missing` /
  `known gap`); caveats go in Evidence. The matrix table is **6 cells per row** —
  a literal `|` inside a cell silently breaks this and has done so twice.
- Background image, opacity and blur/HSB tint are **settled as out of scope**.
- Do not commit `mise.toml` (untracked, intentional); `out/` and `.superpowers/`
  stay gitignored — the ledger is deliberately local-only.

## 6. Where the detail lives

- Change history: `git log c057c04..HEAD`
- Spec: `docs/superpowers/specs/2026-08-10-purecpu-parity-review-design.md`
- Plan: `docs/superpowers/plans/2026-08-10-purecpu-parity-review.md` — the
  Global Constraints block at the top carries every generalised lesson
- Progress ledger: `.superpowers/sdd/2026-08-10-purecpu-parity-review/progress.md`
  — **authoritative on task completion**, and carries all deferred minors that
  Task 8 and the final whole-branch review must triage
- Per-task briefs/reports/reviews: same directory,
  `task-N-{brief,report,review,rereview*,fix-round-*}.md`
- `docs/purecpu-review/parity-matrix.md` — the parity deliverable
- `docs/purecpu-review/findings.md` — the defect deliverable (C1/I1–I5/M1–M7/L1–L5)
- `docs/purecpu-review/noise-floor.md` — gate reasoning, thresholds, limits
- `wezterm-gui/src/termwindow/render/purecpu.rs:346` — the 1:1 blit
- `wezterm-gui/src/termwindow/mod.rs:1436-1447` — the idle-skip
- `wezterm-gui/src/termwindow/mod.rs:1383` — raw-vs-resolved cursor shape
- `wezterm-gui/src/renderstate.rs:78-118` — the unbounded atlas arm (findings C1)

## 7. Open questions / pending decisions

- **Does any of this get fixed, and in what order?** Not part of this plan. The
  user has not been asked. Worth knowing when they are: adding nearest-neighbour
  scaling to the one blit loop would address inline images, double-width/height
  lines and scaled bitmap emoji together; and C1 (the 4 GiB allocation) is a
  bounds check on one arm.
- **The wide-sixel double-composite's route** is undetermined — see §2. The only
  thing that will settle it is an instrumented `dirty_pixel_rects` dump, which
  needs a source edit this plan forbids.
- **Upstream PR wezterm/wezterm#8043** (OSC 52) is open. If it merges, this
  branch's `ef7b636` — the same fix as one commit — conflicts on the next
  rebase; dropping it is the right resolution.
- The user has seen no review output yet beyond my summaries. Task 8 produces
  the document they actually asked for.

## 8. Staleness watch

- **Integration state must be re-derived, never inherited.** Whether this branch
  is merged, pushed or superseded is not knowable from this file:
  `git merge-base --is-ancestor HEAD main`, `git log --oneline HEAD..main`,
  `git branch -a --contains HEAD`. If merged, stop and go to the successor's
  handoff. Note `main` is upstream wezterm and legitimately runs ahead.
- **Sibling worktrees / other workstreams may exist that this file cannot name** —
  anything started after the handoff commit is invisible here.
- **Task 8 was in flight at this commit.** Its state is in the ledger, not here.
- The `:20` X server and `marco` are ordinary user processes and may be gone.
  Verify with `DISPLAY=:20 XAUTHORITY=<path> xdpyinfo | grep dimensions`.
- The scratchpad Xauthority path in §5 belongs to an earlier session; it was
  still present at this commit, but its absence is expected eventually.
- Task counts in §2 reflect the ledger at this commit. The ledger is append-only
  and authoritative — read it rather than trusting §2.
- Task 7's finding ids were renumbered mid-task; older ledger lines use the old
  scheme. Cite `findings.md`'s current headings, not the ledger's ids.
