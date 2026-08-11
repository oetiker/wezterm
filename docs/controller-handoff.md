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

Handoff commit: `773b845`   Date: 2026-08-11   Reason: milestone (Task 7 closed) + a mid-session crash proved this file is the only crash insurance
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
  `GL_CLAMP_TO_EDGE`, checked). So the sampler is nearest-neighbour and
  **bit-exact parity on those quads is reachable** — and as of Task 7 it is
  *achieved*, not merely reachable (§2). `Linear` serves only the
  `has_color == 2.0` background-image branch, which PureCpu **does** draw
  (`purecpu.rs:440`) and which stays deliberately off the sampler.
- **The LCD subpixel data is already in the atlas** — per-channel sRGB coverage
  in R/G/B with max-alpha in A (`skrifa_rasterizer.rs:695-701`). PureCpu's
  `IS_GLYPH` branch reads only `tex_a`. So subpixel AA (Task 8, next) is a
  compositing fix, not new rasterisation.

## 2. Where we are now

As of handoff commit `773b845` (re-derive merge/push state — see §8):

**Execution mode: subagent-driven development**, at the user's explicit request.
The ledger at `.superpowers/sdd/2026-08-11-purecpu-fixes/progress.md` is
authoritative on task state — trust it and `git log` over anything here.

- **Tasks 1–7 complete.** Suite at **69 tests**, 0 failed, 0 ignored.
- **Task 7 (`773b845`) is the biggest parity win of the pass**, verified on a
  restored instrument and re-derived independently by the controller:
  - iTerm2 OSC 1337 non-native: PAE 61423 → **0** (bit-identical to GL)
  - Sixel 300×300: PAE 1542 → **0** (bit-identical)
  - DECDWL/DECDHL: 45232 → **257**, with the single-width control lines unmoved
    at 257 *in the same capture*; the DECDHL bottom band that was never drawn
    now matches GL row-for-row
  - `wide-sixel`: **60652 → 1285, a 47× improvement** — but only visible against
    the right control (see §4, the most important lesson in this file)
  - Step 4: all eight bit-identical crops still AE=0/PAE=0. No regression.
- **`has_color == 2.0` deliberately keeps the old truncating rect and 1:1 crop**,
  held there by `legacy_bg_image_quads_are_untouched`. Settled; see §5.
- **Tasks 8–14 not started.** Task 8 is subpixel AA — the last large piece of
  renderer work in the plan.

**Not yet done on Task 7, and it is the one open loop:** the pass's pattern is
implement → review, and **every review in this pass has found at least one real
defect**. Task 7 has had a verification pass but **not a reviewer pass**. The
user was asked and had not answered when this was written.

**The user has the Task 7 report and intends to read it personally**
(`task-7-report.md`, ~8700 words, self-contained). Their reaction is pending.

**The verification turned up four defects in `docs/purecpu-review/parity-matrix.md`**
— conflated attribution on wide-sixel, a wrong-axis characterisation of its
residual, an unfounded `degraded` verdict on scaled glyphs, and two rows that now
understate the result. All are Task 13's to land; report §10 lists them.

## 3. Do this next

1. **Ask the user about the Task 7 reviewer pass if they have not answered** —
   it is the only thing between Task 7 and closed. Recommend it: reviewers have
   earned their cost every single time in this pass, and Task 7 is the largest
   behaviour change in it.
2. **Then Task 8** (subpixel AA, plan line ~1300). It is a compositing fix:
   `blend_over_masked` with per-channel coverage, mirroring GL's dual-source
   blend `dst = src0*src1 + dst*(1-src1)`. The plan's first step is a failing
   test and its expectations were hand-checkable last time anyone looked — but
   **run the pre-flight scan anyway** (§4).
3. **`:20` is running and is a bit-exact instrument** (§4). It is an ordinary
   user process and will die with the user's ThinLinc session again. Recreation
   recipe: `plans/2026-08-10-purecpu-parity-review.md:768-782` — transcribe it,
   never improvise geometry/DPI, and **validate any replacement against a
   documented control before trusting a row** (`ctl-chrome` → 2070 / 674 / 257).
4. **Do not run a review and an implementation concurrently in this worktree.**
   Both edit the tree and build; they will corrupt each other. One at a time.
5. **Nothing else needs the user until Task 14**, except the `painted_y_span`
   scope question (§7). They chose straight-through execution with a single
   review at the end.

## 4. Lessons & traps  ← the irreplaceable part

Carried forward from prior sessions where still true, plus this one's.

- **A CONTROL BINARY MUST SHARE EVERY PRIOR COMMIT IN THE PASS.** The single
  most expensive lesson of this session. `wide-sixel` looked like a *regression*
  for two full rounds — differing pixels doubled — because the control
  (`wezterm-gui-prefix`) predates `c5cc8ab`'s atlas cap. Against a control that
  shares the cap, the same commit is a **47× improvement** (60652 → 1285).
  Nothing about the subject changed; the baseline was measuring a different
  delta. `wezterm-gui-prefix` is a valid baseline **for the whole pass, never
  for one task inside it** — later tasks must isolate against the immediately
  preceding build.
- **A delta without a held-constant check is a correlation.** What licensed the
  attribution above was not the delta but confirming from the isolation run's
  *own logs* that PureCpu hit `max 8192 → Scale(4)` and GL `max 16384 →
  Scale(2)`, identical to the subject run — cap, sprite resolution and corpus
  constant, so Task 7 was the only variable.
- **Pin a reference binary by its embedded CI tag, not its filename or md5.**
  `wezterm-gui-c5cc8ab-review` carries `20260811-113714-c5cc8abf`, tying it to
  the *commit*. md5 only proves the file is intact; a filename proves nothing.
  This project already has one corrupt binary and one case of numbers taken from
  a binary built at the wrong HEAD.
- **An unchecked control is just another number.** The controller read a
  control's non-zero value as a control *failing* and dispatched an agent to
  redo a sound measurement — without opening the document that records what that
  control is supposed to print. All seven controls had reproduced their
  documented values exactly. **Inverted evidence is worse than no evidence.**
- **Reasoning from a document that describes a measurement, instead of from the
  measurement.** The root shared by *both* of the controller's wrong reframings
  of `wide-sixel`. The same error in a different costume as the trace-log trap
  below: trusting an artifact that describes the code instead of reading what
  the code does next.
- **A trace that fires on the way into a fix is not evidence the defect survives
  it.** Task 1 concluded `glyph.scale != 1` was live from the `log::trace!` at
  `glyphcache.rs:858-867` — which is emitted from *inside* the branch that
  resets scale to 1.0 on the very next line. It proves the branch was *entered*.
  All four `CachedGlyph` construction sites were then enumerated: the only one
  that can hold a non-unit scale (`:824`, whitespace) has `texture: None` and
  emits no quad, so **no `CachedGlyph` carrying a texture can have
  `scale != 1.0`.** That corpus cannot exercise I5; the matrix row is unfounded.
- **BRIEF AGENTS TO CHALLENGE THE PREMISE, NOT JUST EXECUTE IT.** The recovery
  brief asserted an instrument failure that did not exist. The only reason it
  did not cost a wasted round is that the agent checked the premise and pushed
  back with evidence. Twice more it disproved a controller reframing by testing
  it. **"Verify the reviewer's arithmetic" applies to the controller's too** —
  and that direction is the one nobody tests.
- **Ask implementers to flag their own weak points. Seven for seven.** Every
  task in this pass has had its real finding there. Task 7's verifier named
  "every number comes from captures I did not take" — and the fix was to
  reproduce one, which came back byte-identical. Make the section mandatory and
  **spend the next agent's effort on what it names.**
- **Mutation testing is this pass's highest-yield instrument**, and it caught
  something in every task that used it. Make every new test watch a mutant die.
- **But check the mutant perturbs the value the target test reads**, and **pick
  the mutation that ISOLATES the assertion under test.** Made and caught four
  times. Task 6's fix round rejected two candidate value sets as *vacuous*
  pre-emptively: at 2:1 magnification a quarter-pixel shift is an eighth of a
  texel and every sample floors identically. Minification is where a fractional
  destination origin has leverage.
- **A test that asserts no value discriminates almost nothing.** Pin values.
- **Ask "which single test is the only guard on X?"** Task 6's review found
  three single-test guards that collapsed into one added assertion.
- **Do not bank the first red you see — classify it.** A failure in the wrong
  direction is not confirmation.
- **When a test compares against an oracle, decide which side is ground truth
  and put the slack on the OTHER one.** Slack on the assertion that encodes the
  defect is how a suite goes quietly blind.
- **Floating-point association order is part of "term for term".**
  `pane.rs` builds `x + ((cols*cw) + width_delta)`; a rearrangement is a
  different `f32` by up to an ulp, enough to flip a `ceil`.
- **The plan's own text can be wrong, and a good implementer will follow it off
  a cliff.** Eleven times now. Task 7's text would have routed
  `has_color == 2.0` through the sampler — silent scope expansion — because the
  branch lives *inside* the loop the plan said to replace wholesale. **Keep
  running the pre-flight scan; it costs ten minutes.**
- **Verify library and shader semantics from source, not from a restatement.**
  Task 6's review checked that glium's `Clamp` is `GL_CLAMP_TO_EDGE` and that
  `to_texture_coords` uses exact sprite edges with no half-texel inset. Either
  could have made the unit confidently wrong with every test agreeing.
- **Verify runtime preconditions; a static trace is not an observation.** Every
  verdict must answer *"could this run have failed, and what would that have
  looked like?"*
- **The control experiment is the strongest instrument this project has** — and
  **put the control INSIDE the capture** so a null result cannot pass as
  success. The DECDWL row is the model: its single-width control lines read 257
  in the same frame as the fixed double-width lines.
- **Beware a case that passes for the wrong reason.** Ask *why* a run passed.
- **Never take a resumed or crashed agent's result on its word — re-derive it
  against the commit.** One command each time; it has corrected a real error
  twice and confirmed a good one many times.
- **An idle notification is not a report.** Established by looking, every time.
  Signatures: clean tree + commit → finished silently; dirty tree + no build →
  stalled; dirty tree + live `cargo` → deadlocked, `SendMessage` the *same*
  agent. Other users' `cargo`/`rustc` are routinely visible — check the command
  line before concluding one is ours.
- **A crash kills the subagent but not its work.** This session lost an agent
  mid-verification: the commit, the built binary and the harness logs all
  survived in the worktree and the scratchpad. **Go read the tree and the logs
  before re-dispatching anything** — the recovery brief only needed to cover
  what was genuinely missing.
- **Messages cross constantly when an agent is working.** Three times this
  session a controller instruction arrived after the agent had already done the
  thing. Before pressing an agent on an "unrun" item, check whether its last
  report already contains it.
- **Put `timeout: 600000` on every long Bash call, and tell the agent never to
  end a turn while a background shell is live.** The cause is the turn ending,
  not the timeout (anthropics/claude-code#50572, closed "not planned").
- **State process constraints as actions, not prohibitions.** "Redirect every
  command to a log file and Read it" lands; "don't pipe the gate" gets ignored.
- **Subagents terminate on idle and their inline replies are frequently lost.**
  Require the full report in a file, a short summary inline. Six sessions now.
- **`/scratch` on this box silently corrupts large writes.** `md5sum` every
  binary copy. `wezterm-gui-t4rev-subject` is a corrupt copy; do not use it.
- **Build AFTER committing**, so the embedded CI tag pins evidence to a real SHA.
- **`pkill` as a literal string in a Bash tool command kills the call with exit
  144 before anything runs.** Use `pgrep` and `kill -TERM`. **Related, and now
  confirmed as a harness defect:** `lib.sh`'s `kill_class` uses
  `pkill -f -- "--class $1"`, which matches the *calling shell* whenever the
  class name appears in its own command line. `kill_class ... || true` does
  **not** protect you — the shell dies before `||` runs, and in one case it
  fired *before* the launch it was meant to clean up after, so nothing ran and
  the expected log did not exist. Workaround: `setsid … &` + `kill -TERM -$PGID`,
  and `pgrep -af "partrace[c]pu"` (character class breaks the self-match).
- **A subagent's stdout lands in the human's real terminal.** Redirect every
  harness/build/wezterm command to a log file and Read it; never `cat`/`head`/
  `tail` a binary (`out/*.png`, `*.xwd`); never run a corpus script directly.
- **Focus state is a confound in any two-window X comparison.** Sequential
  capture is mandatory.
- **llvmpipe is the reference and it works** — this GPU-less Xvnc box resolves
  OpenGL to Mesa llvmpipe 4.5, the only reason a GPU reference exists.
- `timeout N cat </dev/null` does **not** wait; use `timeout N tail -f /dev/null`.
- `local a="$1" b="...$a..."` fails under `set -u`; split multi-variable `local`.
- **Harness trivia:** the test module in `render/purecpu.rs` is `mod test`
  (**singular**); `purecpu_dirty.rs` uses `mod tests`. `wezterm-gui` is
  binary-only: `cargo test -j4 -p wezterm-gui --bin wezterm-gui` (`--lib` fails).

## 5. Don'ts & constraints

The plan's Global Constraints block is authoritative — read it. The ones that
matter most, plus this session's additions:

- **Never touch X displays `:10`–`:14`** — other users, and the user's own
  ThinLinc session. All testing on `:20`. Xauthority:
  `/tmp/claude-1003/-scratch-oetiker-wezterm/659fab62-7b17-4059-a502-42815bcb9732/scratchpad/Xauthority-20`
  (belongs to a *previous* session's scratchpad and is what the running Xvnc
  authenticates against — do not "fix" the path; verify with `xdpyinfo`).
- **Starting an X server is the USER'S decision, not the controller's.** They
  authorised `:20`'s restoration explicitly this session. Ask again next time.
- **Nothing you run may emit raw bytes to the terminal.** Same standing as the
  display rule.
- **Never more than 4 cores**, including cargo (`-j4`).
- **Never run the 65536-atlas case** — 16 GiB on a shared ~25 GiB box.
- **`has_color == 2.0` stays off the sampler.** GL filters it bilinearly and
  background image is out of scope by the user's own scope decision. Routing it
  would expand scope *and* manufacture parity diffs that look like sampler bugs.
  Held by `legacy_bg_image_quads_are_untouched`. Settled — do not re-decide.
- **Do not collapse `last_seqno_by_pane` back to a single field.** A capture
  proves the collapsed version leaves a background pane frozen.
- **The horizontal extent rule lives in THREE places** — `render/pane.rs`
  :110-152, :606-646, and `purecpu_dirty::painted_x_span` — with nothing tying
  them together. If you touch one, touch all three.
- **Never raise `FUZZ`**, and never grade a case against another case's number.
- **Every config comes from `gen-config.sh`.** A hand-written config is the only
  way the bell guard can fire, and the only way a rejected config can silently
  make both windows use the same backend and report a triumphant `AE = 0`.
- **The OpenGL path must come out bit-identical.** Verified in Task 14, not
  assumed.
- **Background image, opacity and blur/HSB tint are settled as out of scope.**
- Verdict column in the matrix holds a **bare token**; the table is **6 cells
  per row** and a literal `|` in a cell silently breaks it — twice already.
- Do not commit `mise.toml` (untracked, intentional); `out/` and `.superpowers/`
  stay gitignored — the ledger is deliberately local-only.
- **Never overwrite `/scratch/oetiker/wezterm-builds/wezterm-gui-prefix`** — the
  pre-fix reference, not reproducible now that fixes have landed.

## 6. Where the detail lives

- Change history: `git log 773b845..HEAD`; previous handoff
  `git show 24ac5dc:docs/controller-handoff.md`
- **Spec:** `docs/superpowers/specs/2026-08-11-purecpu-fixes-design.md` — §3
  carries the settled B-vs-C argument
- **Plan:** `docs/superpowers/plans/2026-08-11-purecpu-fixes.md` — 14 tasks;
  Global Constraints at the top bind every task. **Treat its task text with
  active suspicion** (§4).
- **Ledger:** `.superpowers/sdd/2026-08-11-purecpu-fixes/progress.md` —
  **authoritative on task state**, and it carries the full reasoning behind each
  review finding and controller decision. Includes a retraction of a controller
  finding that was wrong; the retraction is as load-bearing as the findings.
- **Task 7's report** — `task-7-report.md`, ~8700 words, the most complete
  artifact in the pass. §10 lists the four `parity-matrix.md` defects; §11 is
  its weakest-point section.
- Per-task briefs/reports/reviews: same directory,
  `task-N-{brief,report,review}.md`, plus `task-N-fix1-*.md`
- **The review this repairs:** `docs/purecpu-review/{README,parity-matrix,findings,noise-floor}.md`
- `wezterm-gui/src/termwindow/purecpu_dirty.rs` — the dirty unit (Tasks 3, 4)
- `wezterm-gui/src/termwindow/render/purecpu_sampler.rs` — the sampler unit plus
  the `Quad` facade that joins coverage to sampling (Tasks 6, 7)
- `wezterm-gui/src/termwindow/render/purecpu.rs` — the quad loop; `blend_over`
- `wezterm-gui/src/termwindow/mod.rs` — `do_paint_purecpu`, the dirty-rect walk,
  the cursor-blink block (Task 8), the idle skip
- `wezterm-font/src/rasterizer/skrifa_rasterizer.rs:695-701` — per-channel LCD
  coverage already in the atlas (Task 8's input)
- `wezterm-gui/src/glyphcache.rs:857-871` — where `scale` is reset to 1.0
- Binaries: `wezterm-gui-prefix` (pre-fix, **never overwrite**),
  `wezterm-gui-c5cc8ab-review` (cap, no Task 7 — the correct isolation control),
  `wezterm-gui-t7-773b845`, `wezterm-gui-t4rev-seqno-control`,
  `wezterm-gui-t4rev-subject` (**corrupt**). Harness selects via `WEZTERM_BIN`.

## 7. Open questions / pending decisions

- **Task 7's reviewer pass — unresolved, asked, unanswered.** §3.
- **The four `parity-matrix.md` defects** found by Task 7's verification are
  Task 13's to land. Report §10.
- **PureCpu's atlas cap (8192) is half GL's (16384)**, so any image needing
  `AllowImage::Scale` renders permanently at half GL's resolution. `c5cc8ab`'s
  price for memory safety, by construction. Unlisted finding; wants a matrix row.
- **`painted_y_span` — a scope question for the user, queued for Task 14.**
  Window padding, the half-cell vertical overshoot, a `Top`/`Bottom` divider row
  and descender overhang are covered by no band. Pre-existing, but the exact
  mirror on y of what Task 4 fixed on x. **Ask, do not decide unilaterally.**
- **Does M2 survive Task 4?** Deferred to Task 11; re-measure before fixing.
  Task 4's Finding 4 goes with it: viewport-scroll `force_full` is still
  active-pane-only (`mod.rs`), never reproduced at runtime by anyone.
- **Nobody has photographed Task 4's ≤1 px fringe defect or its fix.** All
  evidence is arithmetic. Assigned to Task 14.
- **Is option C worthwhile after this pass?** Spec §3 said revisit once the
  sampler and dirty units have tests. That condition is now met.
- **The user has still not read `docs/purecpu-review/README.md`** — only
  summaries. They *are* reading `task-7-report.md`, which is the first time this
  workstream's output has gone in front of them directly.
- **M3/M4/M5 will be fixed blind** — triggers never reproduced (`fontTools`
  unavailable). Reports must say "panic site guarded", not "fixed".
- **Should focus change force a full repaint?** `inactive_pane_hsb` recolours
  panes on focus change and no `force_full` trigger covers it. Decide explicitly
  rather than letting Task 9 discover it.

## 8. Staleness watch

- **Integration state must be re-derived, never inherited.** Whether this branch
  is merged, pushed or superseded is not knowable from this file:
  `git merge-base --is-ancestor HEAD main`, `git log --oneline HEAD..main`,
  `git branch -a --contains HEAD`. If this branch is merged, stop reading and go
  to the successor's handoff. Note `main` is upstream wezterm and legitimately
  runs ahead.
- **Sibling worktrees / other workstreams may exist that this file cannot
  name** — anything started after the handoff commit is invisible here.
- **The user's answer on the Task 7 reviewer pass may have arrived** since this
  was written. The ledger and `git log` are the truth.
- Task state in §2 reflects the ledger at this commit. **The ledger is
  authoritative; read it rather than trusting §2.**
- **`:20` is an ordinary user process and died once already today** when the
  user's ThinLinc session restarted. Verify with
  `DISPLAY=:20 XAUTHORITY=<path> xdpyinfo | grep dimensions` (expect
  `1280x1024`, `96x96 dpi`, depth 24), and validate any replacement against a
  documented control before trusting a measurement.
- The Noto Color Emoji `glyph.scale = 0.159` result is font-installation
  dependent — though as of Task 7 that path is known to be neutralised anyway.
- The suite was **69 tests** at this commit (66 before Task 7). Use the delta,
  not the absolute number.
- Line numbers in §6 drift with every edit. Grep, don't trust them.
