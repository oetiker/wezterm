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

Handoff commit: 5d4ad21   Date: 2026-08-10   Reason: context budget
Worktree / branch: `/scratch/oetiker/wezterm` (primary checkout) @ `update-optimization-rebased`
Trunk at time of writing: `main` @ 05343b3 — **reader: if trunk has moved, §2 is provisionally stale; if trunk now contains this branch's HEAD, this file is a tombstone** (`git merge-base --is-ancestor HEAD main`)
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
Two root causes now explain nearly every confirmed defect: **no texture
rescaling** (`purecpu.rs:346` blits 1:1 and crops where the GPU interpolates)
and **the idle-skip** (`termwindow/mod.rs:1438` returns early, so nothing
time-driven repaints).

Scope is narrow by decision: personal fork, Linux/X11 only, no macOS/Windows/
Wayland, no upstreamability. Background image and transparency are explicitly
out of scope.

## 2. Where we are now

As of handoff commit 5d4ad21 (re-derive merge/push state — see §8):

**Tasks 1–5 of 8 are complete.** Each was implemented by a subagent, reviewed
by a second, and fix-looped to a clean re-review. The ledger is authoritative.

- **Tasks 1–3** (static audit, harness, noise-floor gate) — as before: gate is
  PROCEED for the terminal body (PAE=257 = 1 LSB), **STOP for the tab strip**,
  which therefore has no calibrated noise floor to grade against.
- **Task 4 (inline images)** — 4 fix rounds. All three kinds `degraded`: sixel
  (at scale via `AllowImage::Scale`; native 64x64 is a parity sub-case), iTerm2
  (non-native sizes crop instead of scale; native AE=0), animated GIF (frames
  never advance). Task 1's by-reading prediction was never refuted — the first
  corpus simply could not exercise it.
- **Task 5 (chrome)** — 2 fix rounds. Fancy tab bar `degraded`; retro tab bar,
  window buttons, rounded corners, split dividers all `parity`, each re-measured
  on hermetic captures and separately re-checked for false parity. The fancy
  defect is **two** classes, not one: 37 columns a clean 1px shift, 99 columns
  sub-pixel divergence (~77 LSB) that shifting does not help.
- **Task 6 (cursor/animation) is dispatched and running** as of this commit —
  implementer `task6-impl`, BASE `5d4ad21`. Check the ledger before assuming it
  is unstarted or complete.

The harness gained real guards across Tasks 4–5, all of which Tasks 6–8 inherit:
`check_no_config_error` (a rejected config silently takes down `front_end` and
makes both windows the same backend), a blank-capture (Gray) warning,
`PARITY_SETTLE_WARMUP`, `sample_region`/`sample-case.sh` for time-driven rows,
and `gen-config.sh` env overrides with a two-directional blink-pairing guard.

**Tasks 7 (defect findings review of the Rust source) and 8 (consolidation)
remain.** Task 7 is independent of the harness and could be dispatched at any
time — but never run two implementers at once against this tree.

## 3. Do this next

1. Land Task 6: review it, fix-loop it, close it. Then Task 7, then Task 8.
2. **Task 7 has a lot of confirmed input waiting for it** — do not let it start
   from a blank page. The ledger holds: the `renderstate.rs:78-118` asymmetry
   (Glium caps at `caps.max_texture_size` and bails; the PureCpu arm has no cap
   and no fallible path, so PureCpu can *never* enter the `AllowImage::Scale`
   fallback), the unexplained label-row text-antialiasing anomaly from the
   wide-sixel case, and the two-class tab-strip characterisation.
3. Task 8 must check that no `needs-measurement` row survives, and must triage
   the deferred-minor list in the ledger (~9 entries) rather than discard it.

## 4. Lessons & traps  ← the irreplaceable part

- **The dominant failure mode of this entire plan is a silent false `parity`** —
  a measurement reporting "identical" because the instrument or corpus could not
  see the phenomenon. It has now appeared six or seven distinct ways: a corpus
  that requested images at native size so the crop was a no-op; `capture_settled`
  defining success as "nothing changed" and so structurally excluding animation;
  a split divider never on screen; a blink rate with a non-blinking cursor shape;
  two blank captures; and a rejected config that took down `front_end` so both
  windows ran the same backend. **Every `parity` verdict must answer "could this
  run have failed, and what would that have looked like?"** Put that question in
  every implementer and reviewer dispatch — it is what earns the review rounds.
- **A printed regeneration command that nobody re-runs is a lie waiting to
  happen.** Three times a command in the matrix contradicted the evidence cell it
  was attached to, and *twice the wrong numbers were the reassuring ones*. Make
  both implementer and reviewer copy commands out of the rendered file and run
  them. Any env var the command needs goes IN the command, never in prose beside it.
- **The plan's own text can be wrong, and a good implementer will follow it off a
  cliff.** This has now happened four times (FUZZ=3, the native-size corpus, the
  simultaneous-launch step in Task 6's brief, the `sed`-edited config). When a
  review objects to a *plan-mandated* choice, the plan is usually what needs
  fixing. Scan each brief against the Global Constraints **before** dispatching
  and resolve the conflicts in the dispatch itself.
- **Amend the plan's Global Constraints when a lesson generalises.** Rules added
  there (sequential capture, FUZZ=1, "a corpus must be able to trigger its
  defect", "`capture_settled` cannot measure animation") are inherited by every
  later task for free. This is the cheapest leverage the controller has.
- **Subagents terminate on idle and their replies are frequently lost.** Always
  require the full report in a file and only a short summary inline. This has held
  for every dispatch this session and saved several re-runs.
- **Resuming the same reviewer for scoped re-reviews works well** — it keeps its
  probe scripts and prior reasoning, and it verified its own findings honestly
  rather than rubber-stamping. Resuming the implementer for fix rounds 1–3
  likewise. The round-4 escalation to a fresh, stronger implementer also behaved
  exactly as the skill predicts.
- **Reviewers here earn their cost.** Every single review this session found at
  least one Critical or Important defect that would have shipped a wrong verdict
  into the deliverable. Do not be tempted to skip or downgrade them late in the plan.
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
  (session-scoped; if gone, recreate with `xauth -f <path> add :20 MIT-MAGIC-COOKIE-1 <32 hex>` and restart Xvnc).
- **Never more than 4 cores.**
- **This plan produces documents, not fixes.** `wezterm-gui/` source is
  read-only. Harness scripts under `tools/purecpu-parity/` *may* be fixed — that
  distinction was ruled on explicitly and matters.
- **Never raise `FUZZ`, and never grade a case against another case's number.**
  Grade against a stated basis. The tab-strip defect stays a finding.
- Do not rebuild wezterm; `/scratch/oetiker/wezterm-builds/wezterm-gui-rebased`
  is the artifact under test and matches this branch.
- Shared harness files (`lib.sh`, `gen-config.sh`, `compare-case.sh`,
  `sample-case.sh`) are load-bearing for Tasks 1–5's committed results: any
  change must keep default output byte-identical, verified.
- Verdict column holds a **bare token** (`parity` / `degraded` / `missing` /
  `known gap`); caveats go in Evidence.
- Background image, opacity and blur/HSB tint are **settled as out of scope**.
- Do not commit `mise.toml` (untracked, intentional); `out/` stays gitignored.

## 6. Where the detail lives

- Change history: `git log 5d4ad21..HEAD`
- Spec: `docs/superpowers/specs/2026-08-10-purecpu-parity-review-design.md`
- Plan: `docs/superpowers/plans/2026-08-10-purecpu-parity-review.md` — the
  Global Constraints block at the top now carries every generalised lesson
- Progress ledger: `.superpowers/sdd/2026-08-10-purecpu-parity-review/progress.md`
  — **authoritative on task completion**, and carries all deferred minors that
  Task 8 and the final whole-branch review must triage
- Per-task briefs/reports/reviews: same directory, `task-N-{brief,report,review,rereview*}.md`
- `docs/purecpu-review/parity-matrix.md` — the deliverable being filled in
- `docs/purecpu-review/noise-floor.md` — gate reasoning, thresholds, limits
- `wezterm-gui/src/termwindow/render/purecpu.rs:346` — the 1:1 blit behind every
  scaling `degraded` verdict
- `wezterm-gui/src/termwindow/mod.rs:1438` — the idle-skip behind every
  animation defect
- `wezterm-gui/src/renderstate.rs:78-118` — the max-texture-size asymmetry

## 7. Open questions / pending decisions

- **Does the scaling gap get fixed, and how?** Not part of this plan. Adding
  nearest-neighbour scaling to the one blit loop in `purecpu.rs` would address
  inline images, double-width/height lines and scaled bitmap emoji together. The
  user has not been asked.
- **The unexplained label-row text-antialiasing anomaly** in the wide-sixel case
  is characterised where, not why. Task 7 material.
- **Upstream PR wezterm/wezterm#8043** (OSC 52) is open. If it merges, this
  branch's `ef7b636` — the same fix as one commit — conflicts on the next
  rebase; dropping it is the right resolution.
- The user has seen no review output yet beyond my summaries. Tasks 7 and 8
  produce the documents they actually asked for.

## 8. Staleness watch

- **Integration state must be re-derived, never inherited.** Whether this branch
  is merged, pushed or superseded is not knowable from this file:
  `git merge-base --is-ancestor HEAD main`, `git log --oneline HEAD..main`,
  `git branch -a --contains HEAD`. If merged, stop and go to the successor's handoff.
- **Sibling worktrees / other workstreams may exist that this file cannot name** —
  anything started after the handoff commit is invisible here.
- **Task 6 was in flight at this commit.** Its state is in the ledger, not here.
- The `:20` X server and `marco` are ordinary user processes and may be gone.
  Verify with `DISPLAY=:20 XAUTHORITY=<path> xdpyinfo | grep dimensions`.
- The scratchpad Xauthority path in §5 is session-scoped; its absence is expected.
- Task counts in §2 reflect the ledger at this commit. The ledger is append-only
  and authoritative — read it rather than trusting §2.
- The parity matrix still contains `needs-measurement` rows for Task 6's area;
  Task 8 has a check that must come back empty.
