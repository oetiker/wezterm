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

Handoff commit: `dee161b`   Date: 2026-08-12   Reason: milestone — Tasks 7, 9 and 10 closed
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

As of handoff commit `dee161b` (re-derive merge/push state — see §8):

**Execution mode: subagent-driven development**, re-confirmed by the user this
session. The ledger at `.superpowers/sdd/2026-08-11-purecpu-fixes/progress.md` is
authoritative on task state — trust it and `git log` over anything here.

- **Tasks 1–10 and 15 complete and reviewed.** Suite at **116 tests**, 0 failed,
  re-derived by the controller against each commit, not taken on an agent's word.
- **Task 7 is now FULLY closed** (`db28ca3`). Its last deferred finding — build
  the `Quad` after the clip bail — landed, with the rect rule hoisted into
  `Quad::dest_rect_of` so there is exactly one copy. Solid-colour quads now build
  no `Quad` at all.
- **Task 9 (I4)** at `868159c`: the blink predicate resolved the *raw* pane cursor
  shape, which stays `Default` until DECSCUSR, so `default_cursor_style =
  "BlinkingBlock"` was inert. Now `purecpu_dirty::cursor_blinking`.
- **Task 10 (I3 + L3)** at `230117c`, reviewed, with two comment corrections at
  `dee161b`. Four animation paths (bell, blinking text, GIF frames, timer) each
  mark their own region; the idle skip stays.
- **Tasks 11–14 not started.** Task 11 is **blocked on the measurement round**
  (§3), not on code.
- **The version panic: root cause NOT found; the standing diagnosis and its
  workaround are both refuted** (§7). Unchanged this session — nobody touched it.

**Three open threads**: Tasks 11–14, the measurement round they depend on, and
the version panic.

## 3. Do this next

1. **Task 12 first — it is the only remaining task that needs neither a build nor
   a display.** Three font-data panics (M3, M4, M5) in
   `wezterm-font/src/rasterizer/skrifa_rasterizer.rs`, gate
   `cargo test -j4 -p wezterm-font`. Its reports must say **"panic site
   guarded"**, never "fixed" — no crafted font was ever built to fire them.
2. **Then the batched measurement round**, which is the gate on Task 11 and the
   honest closure of Tasks 9 and 10. One release build
   (`cargo build -j4 --release -p wezterm-gui`, from scratch and long — the
   release target dir is empty), then the parity harness on `:20`. **The Task 10
   review rewrote what this round must measure, and the naive version confirms
   nothing** — see §4's entry on scoped predictions. It must include:
   - the idle-cursor case (expect the subject **at the floor**),
   - an **SGR 5** case (expect it **near the ceiling** — that is NOT a failure),
   - a **static full-screen image** case (the one that tests the "a still image
     costs nothing, forever" claim, which is true in rects and false in CPU),
   - M2's wide-sixel label row, which is Task 11's precondition.
3. **Task 11 only after that.** The plan forbids fixing M2 before re-measuring
   it; it may have been closed outright by Tasks 4/7.
4. **Do not run a review and an implementation concurrently in this worktree.**
   Both edit the tree and build; they will corrupt each other. The **shared
   `CARGO_TARGET_DIR` is contended**, and a `cp` taken right after a successful
   build produced a different md5 minutes later and an unrunnable binary.
   `md5sum` every binary copy and re-hash after copying.
5. **`:20` was alive and validated at this commit** (1280x1024, 96x96 dpi, depth
   24 — verified this session). It is an ordinary user process and has died once
   already when the user's ThinLinc session restarted. Recipe:
   `plans/2026-08-10-purecpu-parity-review.md:768-782` — transcribe it, never
   improvise geometry/DPI, and **starting an X server is the USER'S decision**.

## 4. Lessons & traps  ← the irreplaceable part

Carried forward where still true, plus this session's.

- **A WRONG "WHY" NEXT TO RIGHT CODE IS ITS OWN DEFECT CLASS, and this session
  found two.** Neither `bell_region`'s NaN claim nor the `first_row` "safe
  direction" claim broke anything; both were *reasons* pinned in comments that
  were backwards. They outlive everyone who understood the code, and they are
  exactly how the next person talks themselves into replacing right code with a
  plausible shortcut. **Review comments as claims, not as prose.** Closed at
  `dee161b`.
- **A MUTANT'S VALUES CAN REFUTE A COMMENT, not just the code.** The Task 10
  reviewer disproved the NaN claim empirically: with `fade_out` forced to 0 the
  predicate returned `None` where the real code returns `PaneBackground` — which
  is *why* its mutant killed anything. It never argued the float semantics.
  Generalise: when a comment states a mechanism, a mutant that exercises that
  mechanism is a cheaper test of the comment than reading the library.
- **VERIFY THE CONTROLLER'S PREMISES, NOT JUST THE AGENTS'.** Thirteenth time,
  and this time the wrong premise was **mine, in a brief**: I wrote "an unfocused
  window still fades a bell" and told the implementer to consider dropping the
  `focused` gate. `render/paint.rs:121` gates the **GL path's own** rescheduling
  on `self.focused.is_some()`, so dropping it would have been a divergence from
  the renderer this pass exists to match. The agent refuted it with the citation
  and flagged it as "the first thing to re-derive". **Put "challenge the premise,
  with evidence, instead of executing it" in EVERY brief** — it has now prevented
  more waste than any other instruction in this pass.
- **A NEGATIVE RESULT NEEDS A CONTROL MOST OF ALL, AND THE CONTROL BELONGS
  INSIDE THE SAME EXPERIMENT.** Task 7's fix was proved by a `panic!` in
  `Quad::new` that did *not* fire for skipped quads — worthless on its own, since
  a test that never reaches the loop passes identically. The same run showed it
  *does* fire for a painted quad. That pairing is the evidence; either half alone
  is not. (The prior session's version of this lesson: an empty `strings | grep`
  is a claim about your regex until you prove it is a claim about the data.)
- **A PREDICTION IS SCOPED TO A CONFIG — NAME THE CONFIG.** Task 10's report
  predicted "the subject sits at the floor within noise". True, but only for the
  no-animation case. One SGR 5 cell makes the dirty list non-empty every frame,
  so the early exit does not fire and a whole `paint_pass` runs — ~250× the cost
  the report measured. **A measurement round run only on the idle case would have
  confirmed nothing about the paths the task added, while looking like a pass.**
- **CHECK THAT A MUTANT PERTURBS THE VALUE THE TARGET TEST READS.** Caught twice
  more: five of six `painted_y_span` fixtures have integer edges, so a
  `floor`→`ceil` mutation is a **no-op** on them; only the odd-cell-height
  fixture discriminates. Same shape as Task 15's `cy2.min(blit_y2 + 1)`.
- **CLASSIFY A KILL, DON'T COUNT IT.** Each mutant should kill **exactly one**
  test, at a **pinned-value assertion** rather than a count or a panic; name the
  discriminating **negative** too. A kill spread over many tests means the branch
  was already redundantly covered. Corollary learned this session: **a kill
  spread wide can be the point** — the `dest_rect_of` mutant killing three *other*
  tests is the positive evidence that the call site really routes through the
  hoisted function rather than a second copy.
- **ASK IMPLEMENTERS TO FLAG THEIR OWN WEAK POINTS. Eleven for eleven**, and a
  self-assessed weak point is a **hypothesis, not a finding — run it.** Task 10's
  agent ran one of its two (the scan cost) and killed its own planned cache as a
  result; the one it could not run is where its reviewer found a real error.
- **REASONING FROM A RESTATEMENT INSTEAD OF FROM THE THING.** The root shared by
  several errors in this pass. **When a finding says "X is safe because Y", ask
  what else reaches X.**
- **The plan's own text can be wrong, and a good implementer will follow it off a
  cliff.** Task 10 alone found four more defects in it, including that the bell
  is *not* a whole-window tint. **Keep running the pre-flight scan; it costs ten
  minutes.** Its line numbers are stale again after every task.
- **Do not restructure code that no test executes.**
- **A test that asserts no value discriminates almost nothing.** Pin values.
  Derive them *before* the first compile; a value copied out of a failure message
  asserts nothing. **Passing on the first run is the evidence it was a
  derivation.**
- **MODEL THE RESIDUAL INSTEAD OF ARGUING ABOUT IT.** When a number has no
  mechanism, compute the mechanism's prediction.
- **A REVIEWER'S ALGEBRA CAN BE WRONG IN THE PROJECT'S OWN KNOWN TRAP.**
  **Floating-point association order is part of "term for term"** — two forms can
  differ by up to 2 ulp, enough to flip a `ceil`. Compute, don't argue, about
  floats.
- **READ THE LIBRARY'S SEMANTICS, NOT THE PLAUSIBLE ONES.** `OnceLock::set`
  returns `Err` carrying **the value you passed**. `x/0.0` is `+inf`, not NaN;
  only `0.0/0.0` is NaN.
- **A BUILD LOG SAYING "Finished in 0.42s" MEANS NOTHING REBUILT.** An entire
  prior experiment was void on this.
- **Ask "which single test is the only guard on X?"** and **do not bank the first
  red you see — classify it.**
- **When a test compares against an oracle, decide which side is ground truth and
  put the slack on the OTHER one.** Keep the oracle a deliberate transcription
  that delegates to nothing, or it stops being independent.
- **Beware a case that passes for the wrong reason.** Ask *why* a run passed.
- **AN IDLE NOTIFICATION IS NOT A REPORT.** Both agents this session finished
  silently with their inline replies lost; both had completed and committed.
  Signatures: clean tree + commit → finished silently, go read the commit and the
  report file; dirty tree + no build → stalled; dirty tree + live `cargo` →
  deadlocked, `SendMessage` the *same* agent, never re-dispatch.
- **Never take an agent's gate result on its word — re-derive it against the
  commit**, and re-run at least one load-bearing fault injection yourself. Both
  agents' claims held up this session; that is the outcome, not the reason to
  stop checking.
- **A crash kills the subagent but not its work.** Read the tree and logs first.
- **Prior sessions leave evidence in their scratchpads** (`grep -rl` over
  `/tmp/claude-1003/`). The version-panic work lives almost entirely there.
- **`.superpowers/` is gitignored, so an agent cannot commit its report.** Say
  "write the file; it will not be committed, and that is expected."
- **Put `timeout: 600000` on every long Bash call, and tell the agent never to end
  a turn while a background shell is live.** The cause is the turn ending, not the
  timeout (anthropics/claude-code#50572, closed "not planned").
- **State process constraints as actions, not prohibitions.** "Redirect every
  command to a log file and Read it" lands; "don't pipe the gate" gets ignored.
- **Require the full report in a file, a short summary inline.** Nine sessions now.
- **`/scratch` on this box silently corrupts large writes.**
  `wezterm-gui-t4rev-subject` is a corrupt copy; do not use it.
- **`pkill` as a literal string in a Bash tool command kills the call with exit
  144.** Use `pgrep` and `kill -TERM`; `|| true` does **not** protect you.
- **A subagent's stdout lands in the human's real terminal.** Redirect everything
  to a log file and Read it; never `cat`/`head`/`tail` a binary or a PNG.
- **Focus state is a confound in any two-window X comparison.** Sequential capture.
- **llvmpipe is the reference and it works** — this GPU-less Xvnc box resolves
  OpenGL to Mesa llvmpipe 4.5, the only reason a GPU reference exists.
- `timeout N cat </dev/null` does **not** wait; use `timeout N tail -f /dev/null`.
- **Harness trivia:** the test module in `render/purecpu.rs` is `mod test`
  (**singular**, `pub(crate)`); `purecpu_sampler.rs`/`purecpu_dirty.rs` use
  `mod tests`. `wezterm-gui` is binary-only:
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
- **Starting an X server is the USER'S decision, not the controller's.**
- **Destructive commands need the user's explicit approval.**
- **Nothing you run may emit raw bytes to the terminal.**
- **Never more than 4 cores**, including cargo (`-j4`).
- **Never run the 65536-atlas case** — 16 GiB on a shared box.
- **`has_color == 2.0` stays off the sampler.** Settled — do not re-decide.
- **The idle skip stays.** Each animated thing marks its own region. The rejected
  alternative — skip the early exit whenever any animation is live — repaints the
  whole window at the blink rate forever. **Do not drift back toward it under
  pressure from an awkward case.**
- **`has_animation` is a CLOCK, never evidence that something is animating.** It
  is refreshed only by a paint, so it goes permanently stale; reading it as
  liveness reintroduces the forever-at-`animation_fps` loop. Verified to hold at
  every read as of `dee161b`.
- **The `focused` gate stays on animation rescheduling** — GL does the same
  (`render/paint.rs:121`). Removing it diverges from the renderer we are matching.
- **Do not collapse `last_seqno_by_pane` back to a single field.**
- **The horizontal extent rule lives in THREE places** — `render/pane.rs`
  :110-152, :606-646, and `purecpu_dirty::painted_x_span`. Touch one, touch all.
  `painted_y_span` is now the y twin and mirrors `render/pane.rs`'s `y` /
  `height_delta` term for term, association included.
- **Never raise `FUZZ`**, and never grade a case against another case's number.
- **Every config comes from `gen-config.sh`.** A hand-written config is the only
  way the bell guard can fire, and the only way a rejected config can silently
  make both windows use the same backend and report a triumphant `AE = 0`.
- **The OpenGL path must come out bit-identical** — Task 14 Step 2 checks it, and
  Task 13's new L3 half (§7) is the first change that touches GL, so it needs
  that check rather than a quiet landing.
- Verdict column in the matrix holds a **bare token**; the table is **6 cells per
  row** and a literal `|` in a cell silently breaks it — twice already.
- Do not commit `mise.toml` (untracked, intentional); `out/` and `.superpowers/`
  stay gitignored — the ledger is deliberately local-only.
- **Never overwrite `/scratch/oetiker/wezterm-builds/wezterm-gui-prefix`** — the
  pre-fix reference, not reproducible now that fixes have landed.
- **Do not go looking for `/scratch/oetiker/wezterm/.tag`.** Its absence is
  correct and harmless (§7).

## 6. Where the detail lives

- Change history: `git log dee161b..HEAD`; previous handoff
  `git show 08af818:docs/controller-handoff.md`
- **Spec:** `docs/superpowers/specs/2026-08-11-purecpu-fixes-design.md` — §3
  carries the settled B-vs-C argument
- **Plan:** `docs/superpowers/plans/2026-08-11-purecpu-fixes.md` — **15** tasks;
  Global Constraints at the top bind every task. **Treat its task text with
  active suspicion** (§4). Task 14 Step 7's hedge about the blit loop being
  unexecuted is **now resolved in our favour** — Task 15 landed and `db28ca3`
  proved the loop executes; delete the hedge rather than carry it.
- **Ledger:** `.superpowers/sdd/2026-08-11-purecpu-fixes/progress.md` —
  **authoritative on task state**, and it carries the full reasoning behind each
  review finding, each controller decision, and **three controller retractions**.
  The retractions are as load-bearing as the findings.
- Per-task briefs/reports/reviews: same directory,
  `task-N-{brief,report,review,review-brief,fix1-brief,fix1-report}.md`. Task 15's
  and Task 10's are the most complete artifacts in the pass.
- **The review this repairs:** `docs/purecpu-review/{README,parity-matrix,findings,noise-floor}.md`
- `wezterm-gui/src/termwindow/render/purecpu.rs` — `call_draw_purecpu`,
  `blit_vertex_buffer`, `blend_over`, `blend_over_masked`, `subpixel_mask`, and
  `mod test` with the `Rig`/`blit_probe`/`coverage_atlas` harness
- `wezterm-gui/src/termwindow/render/purecpu_sampler.rs` — the sampler unit,
  `Quad`, `dest_rect_of`, `walk()` and `legacy_oracle`
- `wezterm-gui/src/termwindow/purecpu_dirty.rs` — the dirty unit, now also the
  home of every animation **decision** (`cursor_blinking`, `bell_region`,
  `text_blink_animates`, `animation_timer_needed`, `animation_frame_due`,
  `painted_y_span`, `pane_rect`). `mod.rs` is plumbing that reads `self`.
- Binaries: `wezterm-gui-prefix` (pre-fix, **never overwrite**),
  `wezterm-gui-c5cc8ab-review`, `wezterm-gui-t7-773b845`, `wezterm-gui-t8-fd6142a`,
  `wezterm-gui-t4rev-subject` (**corrupt**). Harness selects via `WEZTERM_BIN`.
- Version-panic evidence: `/tmp/claude-1003/-scratch-oetiker-wezterm/47cc07f6-.../scratchpad/`

## 7. Open questions / pending decisions

- **THE MEASUREMENT ROUND IS THE NEXT REAL GATE** and §3 item 2 lists the four
  configs it must cover. Running only the idle case is the trap.
- **F4 — L3 IS ONLY HALF FIXED, and this half is Task 13's.**
  `colorease.rs:115` still has `1000 / fps as u64`, inside `intensity_one_shot` —
  the function that *produces* the `has_animation` instants Task 10 reuses as its
  clock. So the timer now runs at 16.667 ms while the ease it tracks asks for
  16 ms: **the two clocks disagree where before they agreed**, which qualifies
  Task 10's own "cadence identical to the GPU path" claim. Effect is benign
  (~0.7 ms late, one fewer wakeup). It touches the GL path, so it needs the
  GL-invariant check.
- **F1 — the `first_row` question, open with the experiment named.** The
  `AnimatedCellScan` maps lines by **stable row** while the renderer places them
  by **slice index**; they agree only while `first_row == viewport`. The comment
  is corrected at `dee161b` and the code deliberately unchanged (the assumption
  predates Task 10 and `force_full`-on-viewport-change covers the ordinary route
  in). **Unresolved: whether a viewport pinned in scrollback that output then
  TRIMS PAST reaches it** — `resolved` does not change in that case, so the guard
  does not fire. **The experiment that settles it:** drive
  `terminal_with_lines_mut` against a `Terminal` with a deliberately small
  `scrollback_size`, request a stable range that has aged out, assert the
  `first_row` the callback receives. Few lines, in a crate that has a harness.
- **Known and accepted, not hidden:** glyph overhang past a blinking cell is not
  dirtied (the **accepted** Task 9 cursor path has the same property); image-cell
  padding can push the extreme edge cell's quad a few px outside its cell.
- **THE VERSION PANIC — the standing diagnosis and its workaround are BOTH
  REFUTED; root cause still unknown.** Read the ledger's Phase 2 entry first.
  Binaries **are** tagged (the old `strings` check was faulty); `.tag`'s absence
  is harmless; the prior tag experiment rebuilt nothing; **the panic is not
  build-determined** — one build both panicked and did not. `assign_version_info`
  has one caller with no visible double-call route, so reading more source will
  not crack it. **The blocker is a reproduction, not instrumentation** — the
  panicking command was never recorded. Not fork-specific; nothing to report
  upstream yet. **Do not start by building an instrumented binary.**
- **What Task 15 still does NOT cover:** nothing in `call_draw_purecpu` itself is
  executed (clear-loop wiring, band offsets, `subpixel_aa = use_subpixel && idx
  == 1`, present paths); inside `blit_vertex_buffer`, the whole `subpixel_aa` arm,
  `apply_hsv`, per-vertex HSV, `mix_value != 0`, the degenerate-quad `continue`,
  `blit_end() == None`. **Nor does any test execute Task 10's `mod.rs` plumbing,
  `AnimatedCellScan`, or `image_next_frame_due`** — three of the Task 10 review's
  four findings live in exactly that unexecuted region, which is the argument for
  a harness rather than another round of careful reading.
- **A cheap, high-value experiment nobody has run:** assert that the
  pre-extraction body (`c1fc3cf`) and the current one produce bit-identical
  framebuffers over a corpus of random quads.
- **The four `parity-matrix.md` defects** are Task 13's to land, plus the
  **PureCpu 8192 vs GL 16384 atlas-cap asymmetry**, an unlisted finding wanting a
  matrix row.
- **`painted_y_span` — a scope question for the user, queued for Task 14.**
  **Ask, do not decide unilaterally.**
- **Does M2 survive Task 4?** Task 11 — re-measure before fixing.
- **Task 8's colour-emoji alpha ruling is unverifiable on this instrument.**
  Settling it needs a depth-32 `xcb::GetImage` or an assertion on
  `PureCpuState::frame_buffer` — the latter is possible via Task 15's rig.
- **Task 8's reviewer left one falsifiable prediction:** make `subpixel_mask`
  return `[f32; 4]` and re-measure `AE fuzz 0` on `plain.sh`; it predicts ≈2458.
- **Nobody has photographed Task 4's ≤1 px fringe defect or its fix.**
- **Is option C worthwhile after this pass?** Spec §3's condition is now met.
- **The user has still not read `docs/purecpu-review/README.md`.**
- **M3/M4/M5 will be fixed blind** — triggers never reproduced. Reports must say
  "panic site guarded", not "fixed".
- **Should focus change force a full repaint?** `inactive_pane_hsb` recolours
  panes on focus change and no `force_full` trigger covers it.

## 8. Staleness watch

- **Integration state must be re-derived, never inherited.** Whether this branch
  is merged, pushed or superseded is not knowable from this file:
  `git merge-base --is-ancestor HEAD main`, `git log --oneline HEAD..main`,
  `git branch -a --contains HEAD`. If this branch is merged, stop reading and go
  to the successor's handoff. Note `main` is upstream wezterm and legitimately
  runs ahead.
- **Sibling worktrees / other workstreams may exist that this file cannot name** —
  anything started after the handoff commit is invisible here.
- Task state in §2 reflects the ledger at this commit. **The ledger is
  authoritative; read it rather than trusting §2.**
- **`:20` is an ordinary user process and has died once already.** Verify with
  `DISPLAY=:20 XAUTHORITY=<path> xdpyinfo | grep dimensions` (expect `1280x1024`,
  `96x96 dpi`, depth 24), and validate any replacement against a documented
  control before trusting a measurement.
- The suite was **116 tests** at this commit (89 → 90 after Task 7's finding, 94
  after Task 9, 116 after Task 10). **Use the delta, not the absolute number.**
- Line numbers drift with every edit, and Task 10 moved ~770 lines through
  `mod.rs` and `purecpu_dirty.rs`. **Every line number in an older report is
  suspect — grep.**
- Binaries named in older reports may not exist; three were deleted with the
  user's approval when `/scratch` was tight.
