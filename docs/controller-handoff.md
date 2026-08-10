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

Handoff commit: c6b700b   Date: 2026-08-10   Reason: context budget
Worktree / branch: `/scratch/oetiker/wezterm` (primary checkout) @ `update-optimization-rebased`
Trunk at time of writing: `main` @ 05343b3 — **reader: if trunk has moved, §2 is provisionally stale; if trunk now contains this branch's HEAD, this file is a tombstone** (`git merge-base --is-ancestor HEAD main`)
Sibling worktrees: `/scratch/oetiker/claude-worktrees/wezterm-osc52-upstream` @ `osc52-x11-fix` — holds the two-commit upstream PR (wezterm/wezterm#8043), unrelated to this review; leave it alone until that PR resolves. This line cannot see worktrees created later; check yourself.

## 1. Mission

The user's fork of wezterm carries two large unreviewed changes on top of
upstream `e723cf5`: a pure-Rust font stack (skrifa/harfrust/fontdb, replacing
FreeType/HarfBuzz/C COLR) and a new `PureCpu` software renderer for running
without a GPU. The job is to **review the patch and establish where PureCpu
falls short of the GPU backend** — producing two documents (a defect findings
list and an evidence-backed parity matrix), *not* fixes. Fixes are a separate
decision the user makes after reading both.

The mental model that matters: PureCpu is not a separate renderer pipeline. It
consumes the **same layer/quad stream** the GPU path emits, from the **same
single glyph-cache atlas**. So parity gaps are not "missing features" — they are
places where the hand-written rasteriser cannot reproduce what the GPU's fixed
function does. The dominant one is **texture sampling**: the GPU interpolates
texture coordinates across a quad (i.e. scales), while `call_draw_purecpu` blits
1:1 and crops. Nearly every confirmed `degraded` verdict traces back to that one
missing capability.

Scope is deliberately narrow: personal fork, Linux/X11 only. No macOS/Windows/
Wayland, no docs, no upstreamability. Parity is measured for inline images, tab
bar/window chrome, and cursor/text animation. Background image and transparency
are explicitly out of scope by the user's decision.

## 2. Where we are now

As of handoff commit c6b700b (re-derive anything about merge/push state — see §8):

**Tasks 1–3 of 8 are complete**, each implemented by a subagent, reviewed by a
second subagent, and fix-looped where needed.

- **Task 1 (static audit)** — `docs/purecpu-review/parity-matrix.md`. Refuted the
  original hypothesis (that PureCpu's single-texture read means content silently
  fails): the GPU path is *also* single-texture, so no quad can reference a
  texture PureCpu lacks. Found the real gap instead — no rescaling in the
  rasteriser — plus no subpixel-AA analogue, and an idle-skip that starves
  time-driven animation. Also established `FrontEndSelection::Software` is just
  OpenGL + `prefer_swrast()`, unrelated to PureCpu.
- **Task 2 (harness)** — `tools/purecpu-parity/` with `lib.sh`, `gen-config.sh`,
  `corpus/plain.sh`, `calibrate.sh`, `focus-probe.sh`. Proves which backend each
  instance actually ran via `/proc/<pid>/maps` (libEGL present for OpenGL, absent
  for PureCpu) — stronger than logs, which never name the front-end.
- **Task 3 (noise-floor GATE)** — `docs/purecpu-review/noise-floor.md`. **Gate
  outcome: PROCEED for the terminal body, STOP for the tab strip.** The body
  noise floor is textbook: PAE = 257 (exactly one 8-bit step, so no body pixel
  differs by more than 1 LSB), background AE = 0 across 501000 px, no channel
  bias.

**Tasks 4–8 remain**: inline images, chrome, cursor/animation, the defect
findings review, and consolidation.

Two live results Tasks 4–6 must respect, both already propagated into the plan:

1. **Sequential capture is mandatory** (plan commit 8ebd59d). Simultaneous
   capture leaves one window unfocused; wezterm draws a hollow cursor when
   unfocused vs solid when focused, injecting a whole cell of difference that
   survives 12% fuzz and mimics a real defect.
2. **`FUZZ = 1`, plus assert body `PAE <= 257` at fuzz 0** (plan commit 6a10ec2).

**One real PureCpu defect is already confirmed and belongs to Task 5**: some
fancy tab-bar title glyph runs render exactly 1px left of OpenGL (bit-identical
after a 1px shift). No fuzz value absorbs it — 43px still differ at 50%.

## 3. Do this next

1. Resume the SDD loop at **Task 4 (inline images)**. Generate the brief with
   `scripts/task-brief <plan> 4`, record BASE (`git rev-parse HEAD`), dispatch an
   implementer, then a reviewer. The ledger at
   `.superpowers/sdd/2026-08-10-purecpu-parity-review/progress.md` is the
   authority on what is done — trust it over any recollection.
2. Task 4 is the highest-value remaining measurement: Task 1 predicts `degraded`
   for all three image kinds by reading, and Task 4 either confirms that with
   pixels or overturns it. Make sure the implementer treats sixel, iTerm2 and
   animated GIF as **three separate verdicts** — they may diverge.
3. Then Tasks 5, 6, 7, 8 in order. Task 7 (defect review of the Rust source) is
   independent of the harness and could be dispatched at any time if you want
   parallel progress — but never run two *implementers* at once against this
   tree.

## 4. Lessons & traps  ← the irreplaceable part

- **Subagents here terminate on idle and their replies are frequently lost.**
  Always require every dispatched agent to write its output to a file and return
  only a short summary. One review had to be re-run from scratch because it only
  replied inline. This is the single biggest tax on this workflow.
- **`timeout N cat </dev/null` does not wait — it returns in ~0s** (cat hits EOF
  immediately). Use `timeout N tail -f /dev/null`. This burned both me and the
  harness: every retry loop silently became a spin, and my own "waiting" loops
  were no-ops that made working agents look stalled. Verified on this machine.
- **Generate the review package from the BASE you recorded *and the current
  HEAD*.** I pinned a package to a stale HEAD and the reviewer duly reported an
  already-fixed bug as Critical, costing a round trip. Check `git log` right
  before packaging.
- **The plan's own rules can be wrong, and a good implementer will follow them
  off a cliff.** `FUZZ = 3` came from mechanically applying my brief's rule; the
  reviewer proved by injecting synthetic defects that it hides a uniform +6/255
  body-wide brightness error entirely. When a review objects to a *plan-mandated*
  choice, the plan is usually what needs fixing.
- **Prefer PAE over fuzzed pixel counts.** The calibrated fact is "no body pixel
  differs by more than 1 LSB", which a pixel count cannot express. PAE rises at
  every injected defect level including +1/255, where both fuzz thresholds report
  zero.
- **Focus state is a confound in any two-window X comparison**, not just for the
  cursor. Verified in source: `render/mod.rs:709-718` maps a block cursor to
  `CursorShape::Default` only `if focused_and_active`; `customglyph.rs:5080-5100`
  fills the cell for `Default` and draws an outline for `SteadyBlock`.
- **Captures are genuinely deterministic once focus is held constant** — the same
  backend reproduces itself byte-identically across runs (AE = 0). So any
  non-zero diff between runs of the same backend means something is wrong with
  the method, not with rendering.
- **llvmpipe is the reference and it works.** This is a GPU-less ThinLinc Xvnc
  box; OpenGL resolves to Mesa llvmpipe 4.5. That is the only reason a GPU
  reference exists at all here.
- The four `focus-{A,B}-{gl,cpu}.png` captures underpinning the focus finding live
  in gitignored `out/`; `tools/purecpu-parity/focus-probe.sh` regenerates them.

## 5. Don'ts & constraints

- **Never touch X displays `:10`–`:14`** — other users on this shared machine.
  All testing happens on `:20` (Xvnc + marco, started this session; recreate per
  the plan's "Environment setup" section if gone). Xauthority:
  `/tmp/claude-1003/-scratch-oetiker-wezterm/659fab62-7b17-4059-a502-42815bcb9732/scratchpad/Xauthority-20`.
  Note that scratchpad path is session-scoped and may vanish — if so, recreate the
  cookie with `xauth -f <new path> add :20 MIT-MAGIC-COOKIE-1 <32 hex>` and restart Xvnc.
- **Never more than 4 cores** for any build or test.
- **This plan produces documents, not fixes.** Do not "helpfully" fix the
  rasteriser. The user decides what gets fixed after reading the findings.
- **Never raise `FUZZ` to make a case pass.** The tab-strip defect must stay a
  finding, not be dissolved into a threshold.
- Background image, `window_background_opacity`, `text_background_opacity` and
  background blur/HSB tint are **settled as out of scope** — `known gap` rows,
  not investigated. Do not relitigate.
- Do not rebuild the wezterm binary; `/scratch/oetiker/wezterm-builds/wezterm-gui-rebased`
  is the artifact under test and matches this branch.
- Do not commit `mise.toml` (untracked, intentionally kept) and do not sweep
  `out/` into git (gitignored on purpose).

## 6. Where the detail lives

- Change history: `git log c6b700b..HEAD`
- Spec: `docs/superpowers/specs/2026-08-10-purecpu-parity-review-design.md`
- Plan: `docs/superpowers/plans/2026-08-10-purecpu-parity-review.md` — Global
  Constraints at the top now carry the sequential-capture and FUZZ=1 rules
- Progress ledger: `.superpowers/sdd/2026-08-10-purecpu-parity-review/progress.md`
  — **the authority on task completion**; also carries all deferred minors that
  the final whole-branch review must triage
- Per-task briefs/reports/reviews: same directory, `task-N-{brief,report,review,rereview}.md`
- `docs/purecpu-review/parity-matrix.md` — the deliverable being filled in
- `docs/purecpu-review/noise-floor.md` — gate reasoning, thresholds, limits
- `wezterm-gui/src/termwindow/render/purecpu.rs:346` — the 1:1 blit (`blit_w = tex_w.min(dest_w)`)
  that causes every scaling-related `degraded` verdict
- `wezterm-gui/src/termwindow/mod.rs:1438` — the idle-skip early return that starves animation

## 7. Open questions / pending decisions

- **Does the scaling gap get fixed, and how?** Not part of this plan, but it is
  the obvious follow-on: adding nearest-neighbour scaling to the one blit loop in
  `purecpu.rs` would address inline images, double-width/height lines and scaled
  bitmap emoji together. The user has not been asked yet.
- **Animated GIF, blinking text and visual bell** are predicted broken by reading
  (nothing marks them dirty, so the idle-skip never repaints) but were left
  `needs-measurement` because an incidental repaint could mask it. Task 6 settles
  them.
- **Upstream PR wezterm/wezterm#8043** (the OSC 52 fix) is open and unreviewed. If
  it merges, this branch's top commit `ef7b636` — the same fix as one commit —
  will conflict on the next rebase; dropping it is the right resolution.
- The user has not seen any of the review output yet. Tasks 7 and 8 produce the
  documents they actually asked for.

## 8. Staleness watch

- **Integration state must be re-derived, never inherited.** Whether this branch
  is merged, pushed or superseded is not knowable from this file:
  `git merge-base --is-ancestor HEAD main`, `git log --oneline HEAD..main`,
  `git branch -a --contains HEAD`. If this branch is merged, stop reading and go
  to the successor's handoff.
- **Sibling worktrees / other workstreams may exist that this file cannot name** —
  anything started after the handoff commit is invisible here.
- The `:20` X server and `marco` were started as ordinary user processes this
  session. They may be gone. Verify with
  `DISPLAY=:20 XAUTHORITY=<path> xdpyinfo | grep dimensions` before assuming the
  harness can run.
- The scratchpad Xauthority path in §5 is session-scoped and is likely to
  disappear before the next session; treat its absence as expected, not as a fault.
- Task counts in §2 reflect the ledger at the handoff commit. The ledger is
  append-only and authoritative — read it rather than trusting §2's numbers.
- The parity matrix still contains `needs-measurement` rows. Task 8 has a check
  that must come back empty; if it does not, rows were skipped.
