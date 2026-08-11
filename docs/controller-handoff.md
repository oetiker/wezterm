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

Handoff commit: see git log for this file (written at c5cc8ab, amended after Task 2 review landed)   Date: 2026-08-11   Reason: context budget
Worktree / branch: `/scratch/oetiker/wezterm` (primary checkout) @ `update-optimization-rebased`
Trunk at time of writing: `main` @ 05343b3 — **reader: if trunk has moved, §2 is provisionally stale; if trunk now contains this branch's HEAD, this file is a tombstone** (`git merge-base --is-ancestor HEAD main`). `main` here is upstream wezterm and legitimately runs ahead of this branch by ordinary upstream commits; that is NOT a merge signal.
Sibling worktrees: `/scratch/oetiker/claude-worktrees/wezterm-osc52-upstream` @ `osc52-x11-fix` — the two-commit upstream PR (wezterm/wezterm#8043), unrelated; leave it alone until that PR resolves. This line cannot see worktrees created later; check yourself.

## 1. Mission

The predecessor session **reviewed** this fork's PureCpu software renderer
against the GPU backend and produced documents only (`docs/purecpu-review/`).
This session is the **fix pass**: repair everything that review found, so
`front_end = "PureCpu"` on a GPU-less Linux/X11 box loses as little as possible
against `front_end = "OpenGL"`.

Scope was set by the user in brainstorming: **all 18 findings** (C1, I1–I5,
M1–M7, L1–L5), **plus** subpixel-antialiased text and the scaled fallback/bitmap
glyph row — two things the review had ruled outside its own findings list. The
five `known gap` rows (background image, opacity, blur/HSB tint) stay **out of
scope**: they were never examined, so they are investigation work, not repair.

The structural decision that shapes the code (spec §3): **option B — point fixes
plus two extracted units.** Three of the four root causes are *absences* (no
resampler, no pane origin in the dirty rect, no per-channel coverage), and an
absence fixed inline is invisible to the next reader. So a `dirty` unit and a
`sampler` unit get extracted and tested; everything else stays a point fix. The
user pushed back hard on this and asked why not option C (a full
shader-equivalence rewrite of the quad loop) — **that argument is settled and
written down in spec §3; do not relitigate it, but do read it**, because the
reasoning is the honest kind: C restructures the site of only 5 of 18 findings,
and it spends its risk exactly where the code is already bit-identical to the
GPU. B is a strict *prefix* of C, so C stays available afterwards with tests in
place.

Two facts discovered while designing, both load-bearing and both verified in
code rather than assumed:

- **The GPU samples the atlas `Nearest`** for glyphs, emoji and images
  (`render/draw.rs:215-218`); `Linear` is used only for the window background
  attachment (`glyph-frag.glsl:121`), which is out of scope. So the missing
  resampler is nearest-neighbour, and **bit-exact parity on scaled quads is
  reachable** rather than approximate.
- **The LCD subpixel data is already in the atlas** — per-channel sRGB coverage
  in R/G/B with max-alpha in A (`skrifa_rasterizer.rs:695-701`). PureCpu's
  `IS_GLYPH` branch reads only `tex_a` and throws the rest away. So subpixel AA
  is a compositing fix (a per-channel `blend_over` mirroring GL's dual-source
  blend), not new rasterisation.

## 2. Where we are now

As of handoff commit c5cc8ab (re-derive merge/push state — see §8):

**Execution mode: subagent-driven development**, at the user's explicit request.
The ledger at `.superpowers/sdd/2026-08-11-purecpu-fixes/progress.md` is
authoritative on task state — trust it and `git log` over anything here.

- **Task 1 — complete** (`415b6c0..4c5488b`, review clean after 1 fix round).
  Harness prerequisites: the `check_bell_disabled` guard wired into all five
  wezterm-launch sites, `wezterm-gui-prefix` preserved as the pre-fix reference
  binary, and the scaled-glyph font question **answered YES from runtime
  evidence** — Noto Color Emoji yields `glyph.scale = 0.159`, seen through
  wezterm's own `log::trace!` at `glyphcache.rs`'s `scale != 1.0` branch, 4/4
  hits, no instrumentation added. That closes spec §10's first open question and
  means Task 7's resampler can be verified against a real trigger instead of a
  null result. Corpus: `tools/purecpu-parity/corpus/scaled-glyph.sh`.
- **Task 2 — complete** (`4c5488b..c5cc8ab`, review clean, 2 deferred minors).
  The atlas ceiling (`PURECPU_MAX_TEXTURE_SIZE = 8192`) plus a `bail!` on the
  PureCpu arm. **C1 is confirmed fixed at runtime, not by argument:** peak RSS
  **637 MiB against a pre-fix 4170 MiB**, and the `AllowImage::Scale` fallback
  fired **168 times** (`Not enough texture space … max 8192 … will retry render
  with Scale(2)/Scale(4)`). No crash; the sixel gradient still renders, degraded
  rather than blank. That settles the implementer's self-flagged static-trace
  weak point — the review's only Critical finding is closed with an observation
  behind it.
- **Tasks 3–14 — not started.**

**No renderer behaviour beyond C1 has changed yet.** Everything else in the plan
— the two units, the pane-walk, the resampler, subpixel, the animation dirty
rects — is untouched code.

**One incident worth knowing about, because it shaped the constraints.** The
first Task 1 implementer caused an **audible bell in the user's own terminal**
and was killed mid-run (nothing committed, no report). The cause is not
recoverable — it was in-process and stopped before its transcript flushed — but
every config on disk carried `audible_bell = 'Disabled'`, so wezterm was not the
source: the BEL arrived through the **subagent's stdout**, which lands in a real
person's terminal. The constraint had been written at the wrong layer. See §5.

## 3. Do this next

1. **Start at Task 3.** Nothing is in flight and nothing is outstanding — Task 2's
   review landed clean just after this file was first written (§2). Read the
   ledger first to confirm, then dispatch Task 3's implementer.
2. **Tasks 3/6 (the units) are deliberately split from Tasks 4/7 (their
   integrations)** so a reviewer can reject the geometry or the sampling
   arithmetic without rejecting the wiring. Keep that split; do not merge them
   to save a round.
3. **Nothing needs the user until Task 14.** They chose straight-through
   execution with a single review at the end. Do not check in between tasks.
4. **Carry the runtime-verification bar forward.** Task 2's review was worth far
   more than a code read because it *ran* the thing: it turned "the fallback
   should be reached" into "it fired 168 times and peak RSS was 637 MiB". Every
   later task has an equivalent — Task 7 has the six no-drift rows, Task 10 has
   the idle-CPU measurement and the `purecpu_force_full_repaint` control.

## 4. Lessons & traps  ← the irreplaceable part

Carried forward from the review session where still true, plus this session's.

- **A subagent's stdout lands in the human's real terminal.** This is the one
  that actually bit. Guarding wezterm's `audible_bell` config was the wrong
  layer: the exposure is *any* `0x07` byte in *any* command output. The rules
  that hold, now in the plan's Global Constraints: redirect every
  harness/build/wezterm command to a log file and Read it; never
  `cat`/`head`/`tail` a binary file (`out/*.png`, `*.xwd` are full of BEL);
  never run a corpus script directly (`corpus/cursor.sh` rings the bell by
  design); and put the guard in place *before* the first wezterm launch.
  Redirection is the durable rule because it does not depend on any config being
  correct at the moment a command runs — which is precisely the assumption that
  failed.
- **When the user interjects, answer on the next turn — not two tool calls
  later.** During the bell incident the user said "I am hearing the bell", then
  had to say "you did not immediately pickup my comment". Both times I was
  working on the wrong layer while they were telling me something that would
  have corrected it. A user interjection outranks whatever is mid-flight.
- **An idle notification is not a report.** Never conclude "done" or "stalled"
  from silence — inspect the tree. This happened here: a reviewer went idle
  having written nothing, I concluded it had died, and then found the `cargo`
  build it had launched still running. It later finished and delivered a clean
  review. Three signatures, three opposite remedies:
  - clean tree + commit present → **finished silently**; go read the commit.
  - dirty tree + no build process → **stalled**; run the gates and commit for it.
  - dirty tree + a live build → **deadlocked on a pending build** (it ended its
    turn while cargo ran and will never see the result).
  **Deadlocked ≠ lost:** the edits are still in the worktree. `SendMessage` the
  same agent to resume — do not re-dispatch, never restart from scratch.
- **Put `timeout: 600000` on every long Bash call in a dispatch prompt, and tell
  the agent never to end a turn while a background shell is live.** The cause of
  the deadlock above is the subagent ending its turn, not the timeout: on
  timeout Claude Code backgrounds the command rather than killing it, the agent
  ends its turn anyway, and the background shell dies with the turn
  (anthropics/claude-code#50572, closed "not planned"). Banning backgrounding
  does not help — it just converts this into a synchronous timeout. Have the
  agent poll `BashOutput` until the command exits. Release builds of this crate
  are long enough to hit this every time.
- **Never take a resumed agent's gate result on its word — re-derive it against
  the commit.** A resumed subagent can report a *stale* log from before its last
  edits as a fresh green run, and the loop cannot tell (obra/superpowers#2113).
  Task 2's runtime verification came from a resumed reviewer, so it was
  re-derived independently: binary built 11:48:50 and artifacts written
  11:49–11:54, so not stale; `grep -c` gives **172** fallback firings where the
  reviewer reported 168 (86 `Scale(2)` + 86 `Scale(4)`); peak RSS `653304 kB`
  matches its 637 MiB exactly; and `identify` gives stddev 3222 over 188 colours
  on the screenshot, so it is genuinely not blank. Conservative in the one place
  it was off. **Do this for every load-bearing claim** — the check cost one
  command.
- **Verify a subagent's arithmetic, not its adjectives.** "All green" is a
  claim; the count, the delta, and what accounts for it are the evidence.
- **State process constraints as actions, not prohibitions.** "Don't pipe the
  gate" gets ignored; "run it bare, wait for it in the same turn, read the
  output as it comes" lands. Same shape as the bell rule that finally worked:
  "redirect every command to a log file and Read it" beat "don't ring the bell".
- **Verify runtime preconditions; a static trace is not an observation.** Both
  by-reading verdicts the predecessor review overturned failed on preconditions,
  not mechanisms. Task 2 reproduced the same shape: the fallback argument is
  type-level and probably right, and it still needs to be watched firing. This
  is the project's dominant failure mode — a measurement reporting a result the
  instrument or corpus could not actually see, now seen in **nine or ten**
  distinct ways. Every verdict must answer *"could this run have failed, and
  what would that have looked like?"*
- **The plan's own text can be wrong, and a good implementer will follow it off
  a cliff.** This has now happened **eight** times across the two sessions.
  Three more in this one: the brief named `corpus/text.sh`, which does not exist
  (real file: `corpus/plain.sh`); `check_bell_disabled "$CONFIG"` referenced a
  variable that does not exist; and the C1 test used `expect_err`, which needs
  `T: Debug` that `Rc<dyn Texture2d>` does not have. All three were caught and
  worked around by implementers — but only because they were told to flag
  deviations. Keep asking for that.
- **The pre-flight scan of your own plan pays for itself.** Before Task 1 I
  found four defects in my own plan, two of them serious: the M7 test would have
  **passed before the fix** (a vacuous TDD cycle), and the M6 test asserted
  arithmetic inside the test rather than production code. Both are now real
  tests against real defects — an unclamped `x0` that slices backwards and
  panics, and an extracted `clamp_band`. Run that scan; do not assume a plan you
  just wrote is sound.
- **Reviewers earn their cost — again.** Task 1's reviewer found the one
  wezterm-launch site (`focus-probe.sh:37,39`) that neither my brief nor the
  implementer's own sweep caught. Every review across all three sessions has
  found at least one Critical or Important defect. Do not skip or downgrade
  them, especially not near the end.
- **The control experiment is the strongest instrument this project has.**
  Prefer "change one thing and watch the defect disappear" over accumulating
  observations of the defect. For Task 10 the control already exists:
  `purecpu_force_full_repaint = true` is the known-good ceiling.
- **Put the control INSIDE the capture.** The DECDWL corpus printed the doubled
  and single-width line in one frame, so the control read `PAE = 257` in the
  same capture the subject read 45232. A null result then cannot pass as
  success. Cheaper and stronger than any after-the-fact argument.
- **Ask implementers to flag their own weak points.** Every task that did so
  closed faster and the self-flagged item was repeatedly where the real finding
  was — Task 2's implementer flagged its static-trace argument itself, which is
  exactly what §3.1 now exists to settle.
- **Subagents terminate on idle and their inline replies are frequently lost.**
  Always require the full report in a file and only a short summary inline.
  Held across three sessions.
- **Hold an idle implementer in reserve** for its own fix round rather than
  spending it on the next task; resuming it keeps its context and its probe
  scripts.
- **`pkill` as a literal string in a Bash tool command kills the call with exit
  144 before anything runs.** Content-independent, 100% reproducible, cost the
  Task 1 implementer significant time. Use `pgrep` and `kill -TERM`.
- `timeout N cat </dev/null` does **not** wait (cat hits EOF); use
  `timeout N tail -f /dev/null` — `pause` in `lib.sh` does this.
- **Focus state is a confound in any two-window X comparison.** Sequential
  capture is mandatory; an unfocused wezterm draws a hollow cursor, injecting a
  full character cell of difference that survives 12% fuzz. `focus-probe.sh` is
  the one deliberate exception — it launches both at once *on purpose*.
- **llvmpipe is the reference and it works** — this GPU-less Xvnc box resolves
  OpenGL to Mesa llvmpipe 4.5, which is the only reason a GPU reference exists.
- `local a="$1" b="...$a..."` fails under `set -u`; split multi-variable `local`.

## 5. Don'ts & constraints

The plan's Global Constraints block is the authoritative copy — read it. The
ones that matter most:

- **Never touch X displays `:10`–`:14`** — other users on this shared machine.
  All testing is on `:20`. Xauthority:
  `/tmp/claude-1003/-scratch-oetiker-wezterm/659fab62-7b17-4059-a502-42815bcb9732/scratchpad/Xauthority-20`
  (session-scoped, alive at this commit; if gone, recreate per the review plan's
  "Environment setup" and restart Xvnc).
- **Nothing you run may emit raw bytes to the terminal.** See §4. This is a
  shared-machine courtesy rule with the same standing as the display rule.
- **Never more than 4 cores**, including cargo (`-j4`).
- **Never run the 65536-atlas case** — 16 GiB on a shared ~25 GiB box.
- **Shared harness files** (`lib.sh`, `gen-config.sh`, `compare-case.sh`,
  `sample-case.sh`, `focus-probe.sh`) are load-bearing for the review's
  committed numbers: any change must keep default output byte-identical,
  verified.
- **Never raise `FUZZ`**, and never grade a case against another case's number.
- **Every config comes from `gen-config.sh`.** If it refuses a combination you
  need, extend it rather than hand-writing around it — a hand-written config is
  the only way the bell guard can fire, and the only way a rejected config can
  silently make both windows use the same backend and report a triumphant
  `AE = 0`.
- **The OpenGL path must come out bit-identical.** Two fixes touch shared code
  (C1 in `renderstate.rs`, the pane iteration in `mod.rs`). Verified in Task 14,
  not assumed.
- **Background image, opacity and blur/HSB tint are settled as out of scope.**
- Verdict column in the matrix holds a **bare token**; the table is **6 cells per
  row** and a literal `|` in a cell silently breaks it — twice already.
- Do not commit `mise.toml` (untracked, intentional); `out/` and `.superpowers/`
  stay gitignored — the ledger is deliberately local-only.

## 6. Where the detail lives

- Change history: `git log c5cc8ab..HEAD`
- **Spec:** `docs/superpowers/specs/2026-08-11-purecpu-fixes-design.md` — §3
  carries the settled B-vs-C argument
- **Plan:** `docs/superpowers/plans/2026-08-11-purecpu-fixes.md` — 14 tasks; the
  Global Constraints block at the top binds every task
- **Ledger:** `.superpowers/sdd/2026-08-11-purecpu-fixes/progress.md` —
  **authoritative on task state**. The sibling directory
  `.superpowers/sdd/2026-08-10-purecpu-parity-review/` belongs to the review
  plan; do not write to it.
- Per-task briefs/reports/reviews and diff packages: same directory,
  `task-N-{brief,report,review}.md`, `review-<base>..<head>.diff`
- **The review this repairs:** `docs/purecpu-review/{README,parity-matrix,findings,noise-floor}.md`
- `wezterm-gui/src/termwindow/render/purecpu.rs:152-528` — the 376-line quad
  loop, no test coverage; `:346` the 1:1 blit; `:529` `blend_over`
- `wezterm-gui/src/termwindow/mod.rs:1300-1375` — dirty rects, active pane only,
  no `pos.top`/`pos.left`; `:1383` raw cursor shape; `:1436-1447` the idle skip
- `wezterm-gui/src/termwindow/render/draw.rs:215-218` — the GPU's Nearest sampler
- `wezterm-font/src/rasterizer/skrifa_rasterizer.rs:695-701` — per-channel LCD
  coverage already in the atlas
- Binaries: `/scratch/oetiker/wezterm-builds/wezterm-gui-prefix` (pre-fix
  reference, do not overwrite), `wezterm-gui-rebased` (original), and
  `wezterm-gui-fixed` (built by later tasks). Harness selects via `WEZTERM_BIN`.

## 7. Open questions / pending decisions

- ~~Does the C1 bail reach the `AllowImage::Scale` fallback at runtime?~~
  **Settled: yes, observed 168 times, peak RSS 637 MiB.** See §2.
- **Does M2 survive Task 4?** Deferred on purpose (Task 11). Its attribution to
  a specific rect overlap was tested and eliminated twice; Task 4 rewrites the
  dirty-rect geometry that is the suspected source, so M2 may simply not exist
  afterwards. Re-measure before fixing.
- **Is option C worthwhile after this pass?** Spec §3 says: revisit once the
  sampler and dirty units have tests, which is what makes a rewrite safe.
- **The user has still not read `docs/purecpu-review/README.md`** — only
  summaries in conversation. Their reaction remains the one piece of feedback
  this workstream has never had.
- **M3/M4/M5 will be fixed blind** — their triggers were never reproduced
  (`fontTools` unavailable). The plan requires the report to say "panic site
  guarded", not "fixed". Hold that line.

## 8. Staleness watch

- **Integration state must be re-derived, never inherited.** Whether this branch
  is merged, pushed or superseded is not knowable from this file:
  `git merge-base --is-ancestor HEAD main`, `git log --oneline HEAD..main`,
  `git branch -a --contains HEAD`. If merged, stop and go to the successor's
  handoff. Note `main` is upstream wezterm and legitimately runs ahead.
- **Sibling worktrees / other workstreams may exist that this file cannot name** —
  anything started after the handoff commit is invisible here.
- **Nothing was in flight when this file was finalised.** An earlier draft said
  the Task 2 review was abandoned mid-run; that reviewer then completed and
  wrote `task-2-review.md` (Approved). §2, §3 and §7 were corrected before this
  commit. The ledger records both the abandonment and the landing, in order —
  read it if the two ever appear to disagree.
- Task state in §2 reflects the ledger at this commit. **The ledger is
  authoritative; read it rather than trusting §2.**
- The `:20` X server and its Xauthority are ordinary user processes/files and may
  be gone. Verify with
  `DISPLAY=:20 XAUTHORITY=<path> xdpyinfo | grep dimensions`.
- The Noto Color Emoji `glyph.scale = 0.159` result is font-installation
  dependent. It was observed on this machine at this commit; a font change
  invalidates it and Task 7's verification would silently become a null result
  again.
