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

Handoff commit: `f67139c`   Date: 2026-08-12   Reason: context budget, after closing Task 15 and refuting the version-panic diagnosis
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
  quads (`render/draw.rs:215-218`). So the sampler is nearest-neighbour and
  bit-exact parity on those quads is **achieved**, not merely reachable (§2).
  `Linear` serves only the `has_color == 2.0` background-image branch, which
  PureCpu **does** draw and which stays deliberately off the sampler.
- **The LCD subpixel data is already in the atlas** — per-channel coverage in
  R/G/B with max-alpha in A (`skrifa_rasterizer.rs:695-701`). Task 8 used it.
  That RGB is **sRGB-encoded** and must be **linearised** by PureCpu, because
  GL's atlas is an `SrgbTexture2d` that converts on read while `colorMask` is the
  one shader output never re-encoded by `to_srgb`. The plan said the opposite;
  only measurement caught it. See §4.

## 2. Where we are now

As of handoff commit `f67139c` (re-derive merge/push state — see §8):

**Execution mode: subagent-driven development**, at the user's explicit request.
The ledger at `.superpowers/sdd/2026-08-11-purecpu-fixes/progress.md` is
authoritative on task state — trust it and `git log` over anything here.

- **Tasks 1–8 and 15 complete and reviewed.** Suite at **89 tests**, 0 failed,
  re-derived by the controller against the commit, not taken on an agent's word.
- **Task 15** (the framebuffer harness) is **closed**. `call_draw_purecpu`'s quad
  walk was extracted into a free `blit_vertex_buffer` taking plain borrowed data
  — a **pure move**, verified by normalised diff (exactly five differences: one
  dropped borrow re-acquisition and four `&mut state.frame_buffer` → `&mut *fb`).
  Three mutants now die where a `panic!` used to leave the suite green.
  `purecpu_sampler`'s `walk()` no longer duplicates the loop; it drives the
  shipping one via `test::blit_probe`.
- **Tasks 9–14 not started.**
- **The version panic: Phase 2 done, root cause NOT found, but the standing
  diagnosis AND its workaround are both refuted** (§7). This is the single most
  important correction in this handoff, because the previous handoff told you to
  go restore `.tag`, and that would have been wasted work.

**Two open threads**: Tasks 9–14 (ordinary plan work) and the version panic,
which is now blocked on **finding a reproduction**, not on instrumentation.

## 3. Do this next

1. **Tasks 9–14 in plan order.** This is the main line and nothing blocks it.
   Nothing needs the user until Task 14 except the `painted_y_span` scope
   question (§7). Task 11 must **re-measure M2 before fixing it**.
2. **Task 7 review Finding 3 is now unblocked** — build the `Quad` *after* the
   clip bail so an incremental repaint stops paying for a discarded `Quad`. It
   was deferred until something executed that loop; Task 15 delivered that, and
   `blit_skips_a_quad_that_misses_every_dirty_rect` is the test that keeps the
   restructure honest. **Do not duplicate the coverage rule at the call site** —
   hoist it into an associated function so there is exactly one copy.
3. **The version panic only if the user asks for it.** It is not on the critical
   path, and the honest next step is "find a reproduction", which may not be
   cheap. Do **not** start by building an instrumented binary (§7).
4. **Do not run a review and an implementation concurrently in this worktree.**
   Both edit the tree and build; they will corrupt each other. One at a time.
   The **shared `CARGO_TARGET_DIR` is contended**, and a `cp` taken right after a
   successful build produced a different md5 minutes later and an unrunnable
   binary. `md5sum` every binary copy and re-hash after copying.
5. **`:20` was alive and validated at this commit** (1280x1024, 96x96 dpi, depth
   24). It is an ordinary user process and has died once already when the user's
   ThinLinc session restarted. Recreation recipe:
   `plans/2026-08-10-purecpu-parity-review.md:768-782` — transcribe it, never
   improvise geometry/DPI, and **starting an X server is the USER'S decision**.

## 4. Lessons & traps  ← the irreplaceable part

Carried forward from prior sessions where still true, plus this one's.

- **A NEGATIVE RESULT NEEDS A CONTROL MOST OF ALL.** This session's sharpest
  lesson, and it invalidated an entire prior investigation. `strings BIN | grep
  -E "^<tag>$"` returned nothing and was read as "this binary has no version
  tag". The anchors cannot match: the tag is concatenated into a larger string
  blob, so `strings` emits it *inside* a longer line. **The controller made the
  identical mistake first** and caught it only by grepping for a string known to
  be present. Dropping the anchors turned an empty result into two tags. An empty
  grep is a claim about your regex until you prove it is a claim about the data.
- **VERIFY THE CONTROLLER'S PREMISES, NOT JUST THE AGENTS'.** Put "challenge the
  premise, with evidence, instead of executing it" in **every** brief. It has now
  prevented more waste than any other instruction in this pass, and it caught a
  controller error again this session (§ the plan note below).
- **A SELF-ASSESSED WEAK POINT IS A HYPOTHESIS, NOT A FINDING — RUN IT.** Ten
  tasks have named their own weakest point; Task 15's reviewer was the first to
  *execute* one, and it was wrong in an instructive direction. The report
  predicted `blit_probe`'s `written == (alpha != 0)` predicate would under-report;
  it does not (a zero-alpha pixel is never written, so `false` is correct). What
  actually breaks is the **coordinate encoding**: at `fg` alpha 0.5 the probe
  decodes texel (10,20) as (5,10), because the RGB blended toward a cleared
  framebuffer instead of replacing. Same walk *length*, so the rect and count
  checks still pass. The report's proposed sentinel fix would not have caught it.
- **REASONING FROM A RESTATEMENT INSTEAD OF FROM THE THING.** The root shared by
  several errors in this pass, and the controller committed it again: I wrote a
  plan note from Task 15's report and inherited its scope without asking "what
  *else* builds this rect?". `cell_width` is integral, but
  `experimental_pixel_positioning` does not use `cell_width` at all. Corrected at
  `ee609e3`. **When a finding says "X is safe because Y", ask what else reaches X.**
- **CHECK THAT A MUTANT PERTURBS THE VALUE THE TARGET TEST READS.** Task 15's
  `cy2.min(blit_y2 + 1)` is a **no-op on sampled quads** (`blit_y2 == dest_y2 >=
  cy2`, so the `min` swallows it). Only `IS_BG_IMAGE`, where the crop actually
  bites, discriminates. The obvious glyph-shaped test would have passed against
  the mutant and been reported as a kill.
- **CLASSIFY A KILL, DON'T COUNT IT.** The strong form: each mutant should kill
  **exactly one** test, at a **pinned-value assertion** rather than a count or a
  panic. Kills spread over many tests mean the branch was already redundantly
  covered; a death at a count means the test detects "something changed", not
  "this value is wrong". Also name the discriminating **negative** — the test
  that correctly does *not* die.
- **ASK IMPLEMENTERS TO FLAG THEIR OWN WEAK POINTS. Ten for ten.** Every task has
  had its real finding there. Task 15's fix round named a genuine one-assertion
  hole no gate would ever have caught, *and specified the fix it did not write*.
  The controller closed it directly (`f67139c`) rather than spending a whole agent
  round on one assertion — that trade is usually right.
- **A BUILD LOG SAYING "Finished in 0.42s" MEANS NOTHING REBUILT.** An entire
  prior experiment was void on this: it "rebuilt with a tag", cargo did nothing,
  and the resulting binary was byte-identical to the one it was being compared
  against. Read the build log before trusting what you just built.
- **READ THE LIBRARY'S SEMANTICS, NOT THE PLAUSIBLE ONES.** `OnceLock::set`
  returns `Err` carrying **the value you passed**, not the value already stored.
  The version-panic payload had been read as evidence of two *different* version
  strings; it says nothing at all about the first call.
- **MODEL THE RESIDUAL INSTEAD OF ARGUING ABOUT IT.** Task 8's reviewer explained
  6489 differing pixels by enumerating every source, destination and attainable
  atlas byte — deriving a 1-LSB bound *and* predicting the count to 2.4%. Task
  15's used the same move on float edges. When a number has no mechanism, compute
  the mechanism's prediction.
- **NAME WHICH NUMBER CARRIES THE CLAIM.** Task 8's fault injection did *not*
  discriminate what the report leaned on it for. Both the load-bearing evidence
  and a decoy were in the report; only one carried the claim.
- **ENUMERATE BY RESOLVED ARGUMENT, NOT BY LITERAL CALL TEXT.** Four helpers
  forward a `layer_num` parameter, so a grep over call text cannot see it.
- **A DELTA WITHOUT A HELD-CONSTANT CHECK IS A CORRELATION**, and **a control
  binary must share every prior commit in the pass.**
- **A REVIEWER'S ALGEBRA CAN BE WRONG IN THE PROJECT'S OWN KNOWN TRAP.** Task 7's
  reviewer proved adjacent solid quads cannot seam "because both carry the same
  edge `e`" — and the premise was **false**; the two forms differ by up to 2 ulp.
  The conclusion survived for a different reason. **Floating-point association
  order is part of "term for term".** Compute, don't argue, about floats.
- **The plan's own text can be wrong, and a good implementer will follow it off a
  cliff.** Twelve times now. **Keep running the pre-flight scan; it costs ten
  minutes.**
- **Do not restructure code that no test executes.** Task 7's perf finding waited
  for Task 15 for exactly this reason — and is now unblocked.
- **A test that asserts no value discriminates almost nothing.** Pin values.
  Derive them *before* the first compile; a value copied out of a failure message
  asserts nothing. Passing on the first run is the evidence it was a derivation.
- **Ask "which single test is the only guard on X?"**
- **Do not bank the first red you see — classify it.**
- **When a test compares against an oracle, decide which side is ground truth and
  put the slack on the OTHER one.** And keep the oracle a deliberate
  transcription that delegates to nothing (`legacy_oracle`), or it stops being
  independent.
- **Verify library and shader semantics from source, not from a restatement.**
- **The control experiment is the strongest instrument this project has** — and
  **put the control INSIDE the capture** so a null result cannot pass as success.
- **Beware a case that passes for the wrong reason.** Ask *why* a run passed.
- **AN IDLE NOTIFICATION IS NOT A REPORT.** In this session all three agents
  finished silently with their inline replies lost; every time the work was
  complete and committed. Signatures: clean tree + commit → finished silently, go
  read the commit and the report file; dirty tree + no build → stalled; dirty
  tree + live `cargo` → deadlocked, `SendMessage` the *same* agent.
- **Never take an agent's gate result on its word — re-derive it against the
  commit.** One command each time. Re-run at least one load-bearing fault
  injection yourself.
- **A crash kills the subagent but not its work.** Read the tree and the logs
  before re-dispatching anything.
- **Prior sessions leave evidence in their scratchpads** (`grep -rl` over
  `/tmp/claude-1003/`). The version-panic work lives almost entirely there.
- **`.superpowers/` is gitignored, so an agent cannot commit its report.** Do not
  put "commit only your report" in a brief — it is unsatisfiable. Say "write the
  file; it will not be committed, and that is expected."
- **Put `timeout: 600000` on every long Bash call, and tell the agent never to end
  a turn while a background shell is live.** The cause is the turn ending, not the
  timeout (anthropics/claude-code#50572, closed "not planned").
- **State process constraints as actions, not prohibitions.** "Redirect every
  command to a log file and Read it" lands; "don't pipe the gate" gets ignored.
- **Require the full report in a file, a short summary inline.** Eight sessions now.
- **`/scratch` on this box silently corrupts large writes.** `wezterm-gui-t4rev-subject`
  is a corrupt copy; do not use it.
- **`pkill` as a literal string in a Bash tool command kills the call with exit
  144.** Use `pgrep` and `kill -TERM`. `lib.sh`'s `kill_class` has the same defect
  and `|| true` does **not** protect you — the shell dies before `||` runs.
- **A subagent's stdout lands in the human's real terminal.** Redirect everything
  to a log file and Read it; never `cat`/`head`/`tail` a binary or a PNG.
- **Focus state is a confound in any two-window X comparison.** Sequential capture.
- **llvmpipe is the reference and it works** — this GPU-less Xvnc box resolves
  OpenGL to Mesa llvmpipe 4.5, the only reason a GPU reference exists.
- `timeout N cat </dev/null` does **not** wait; use `timeout N tail -f /dev/null`.
- **Harness trivia:** the test module in `render/purecpu.rs` is `mod test`
  (**singular**, and now `pub(crate)` so `purecpu_sampler` can reach
  `blit_probe`); `purecpu_sampler.rs`/`purecpu_dirty.rs` use `mod tests`.
  `wezterm-gui` is binary-only: `cargo test -j4 -p wezterm-gui --bin wezterm-gui`.
  The release target dir is **empty** — a release build is from scratch and long.

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
- **Do not collapse `last_seqno_by_pane` back to a single field.**
- **The horizontal extent rule lives in THREE places** — `render/pane.rs`
  :110-152, :606-646, and `purecpu_dirty::painted_x_span`. Touch one, touch all.
- **Never raise `FUZZ`**, and never grade a case against another case's number.
- **Every config comes from `gen-config.sh`.** A hand-written config is the only
  way the bell guard can fire, and the only way a rejected config can silently
  make both windows use the same backend and report a triumphant `AE = 0`.
- **The OpenGL path must come out bit-identical.** Verified in Task 14.
- Verdict column in the matrix holds a **bare token**; the table is **6 cells per
  row** and a literal `|` in a cell silently breaks it — twice already.
- Do not commit `mise.toml` (untracked, intentional); `out/` and `.superpowers/`
  stay gitignored — the ledger is deliberately local-only.
- **Never overwrite `/scratch/oetiker/wezterm-builds/wezterm-gui-prefix`** — the
  pre-fix reference, not reproducible now that fixes have landed.
- **Do not go looking for `/scratch/oetiker/wezterm/.tag`.** Its absence is
  correct and harmless (§7). The previous handoff said to restore it; that
  instruction is **withdrawn**.

## 6. Where the detail lives

- Change history: `git log f67139c..HEAD`; previous handoff
  `git show c1fc3cf:docs/controller-handoff.md`
- **Spec:** `docs/superpowers/specs/2026-08-11-purecpu-fixes-design.md` — §3
  carries the settled B-vs-C argument
- **Plan:** `docs/superpowers/plans/2026-08-11-purecpu-fixes.md` — **15** tasks;
  Global Constraints at the top bind every task. **Treat its task text with
  active suspicion** (§4). Task 7's section carries the deferred perf finding;
  Task 15 is authorised and done; the Open Questions list carries the corrected
  cell-width / pixel-positioning note.
- **Ledger:** `.superpowers/sdd/2026-08-11-purecpu-fixes/progress.md` —
  **authoritative on task state**, and it carries the full reasoning behind each
  review finding, each controller decision, and **three controller retractions**.
  The retractions are as load-bearing as the findings.
- Per-task briefs/reports/reviews: same directory,
  `task-N-{brief,report,review,review-brief,fix1-brief,fix1-report}.md`.
  Task 15's report and review are the two most complete artifacts in the pass.
- **The review this repairs:** `docs/purecpu-review/{README,parity-matrix,findings,noise-floor}.md`
- `wezterm-gui/src/termwindow/render/purecpu.rs` — `call_draw_purecpu` (thin now),
  `blit_vertex_buffer` (the extracted loop), `blend_over`, `blend_over_masked`,
  `subpixel_mask` (whose doc carries the sRGB correction and the 1-LSB bound),
  and `mod test` with the `Rig`/`blit_probe`/`coverage_atlas` harness
- `wezterm-gui/src/termwindow/render/purecpu_sampler.rs` — the sampler unit,
  `Quad`, `walk()` (now delegating) and `legacy_oracle` (deliberately not)
- `wezterm-gui/src/termwindow/purecpu_dirty.rs` — the dirty unit
- Binaries: `wezterm-gui-prefix` (pre-fix, **never overwrite**),
  `wezterm-gui-c5cc8ab-review`, `wezterm-gui-t7-773b845`,
  `wezterm-gui-t8-fd6142a` (md5 `06f12cab5cbfff71bbac495ef9f9889c`),
  `wezterm-gui-t4rev-subject` (**corrupt**). Harness selects via `WEZTERM_BIN`.
- Version-panic evidence: `/tmp/claude-1003/-scratch-oetiker-wezterm/47cc07f6-.../scratchpad/`
  — `bt.log` (the only captured panic), `build-tag.log`, `tagstart.log`,
  `wezterm-gui-tagtest`.

## 7. Open questions / pending decisions

- **THE VERSION PANIC — the standing diagnosis and its workaround are BOTH
  REFUTED; root cause still unknown.** Read the ledger's Phase 2 entry before
  touching this. In short:
  - Binaries **are** tagged. "Every binary this pass is untagged" was a faulty
    `strings` check (§4). `wezterm-gui-t8-fd6142a` carries `20260811-210625-fd6142a4`.
  - `.tag`'s absence is **harmless**: `build.rs` falls to a git-derive branch that
    works, and `rerun-if-changed` on the branch ref re-bakes a tag every commit.
    **Do not restore `.tag`.** The previous handoff's instruction is withdrawn.
  - The prior "tag experiment" rebuilt nothing (0.42s) and proved nothing.
  - **The panic is not build-determined.** `wezterm-gui-tagtest` carries exactly
    the tag in the panic payload and afterwards printed help and started cleanly.
    One build both panicked and did not. Every tag-centred hypothesis is dead.
  - Statically, `assign_version_info` has one caller (`bootstrap()`), which has
    three callers, all `fn main` in three **separate** binaries; `wezterm-gui`'s
    `main()` calls `run()` once with no retry. There is no visible route to a
    double call, which is why reading more source will not crack it.
  - **The blocker is a reproduction, not instrumentation.** The panicking command
    was never recorded. An instrumented build may simply never panic. Treat
    "find a reproduction" as the task.
  - Not fork-specific (`config/src/version.rs`, `env-bootstrap` are upstream).
    Nothing worth reporting upstream yet.
- **Task 7 review Finding 3** (build the `Quad` after the clip bail) — unblocked
  by Task 15, not done. §3 item 2.
- **What Task 15 still does NOT cover**, and a reader will over-read it: nothing
  in `call_draw_purecpu` itself is executed by any test (the clear-loop wiring,
  the band offset arithmetic, `subpixel_aa = use_subpixel && idx == 1`, the
  present paths); and inside `blit_vertex_buffer`, the whole `subpixel_aa` arm,
  `apply_hsv`, per-vertex HSV, `mix_value != 0`, the degenerate-quad `continue`
  and `blit_end() == None`. IS_GLYPH and IS_GRAY_SCALE **are** now covered.
- **A cheap, high-value experiment Task 15 made possible and nobody has run:**
  assert that the pre-extraction body (`c1fc3cf`) and the current one produce
  bit-identical framebuffers over a corpus of random quads. That is a stronger
  statement than the textual pure-move diff.
- **The four `parity-matrix.md` defects** from Task 7's verification are Task 13's
  to land, plus the **PureCpu 8192 vs GL 16384 atlas-cap asymmetry**, which is an
  unlisted finding that wants a matrix row.
- **`painted_y_span` — a scope question for the user, queued for Task 14.**
  **Ask, do not decide unilaterally.**
- **Does M2 survive Task 4?** Deferred to Task 11; re-measure before fixing.
- **Task 8's colour-emoji alpha ruling is unverifiable on this instrument.**
  Settling it needs a depth-32 `xcb::GetImage` or an assertion on
  `PureCpuState::frame_buffer` — the latter is now possible via Task 15's rig.
- **Task 8's reviewer left one falsifiable prediction:** make `subpixel_mask`
  return `[f32; 4]` and re-measure `AE fuzz 0` on `plain.sh`; it predicts ≈2458.
  Materially above that means a second mechanism and its primary ruling is wrong.
- **Nobody has photographed Task 4's ≤1 px fringe defect or its fix.**
- **Is option C worthwhile after this pass?** Spec §3's condition (sampler and
  dirty units tested) is now met, and the blit loop is tested too.
- **The user has still not read `docs/purecpu-review/README.md`** — only
  summaries and per-task reports.
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
- The suite was **89 tests** at this commit (75 before Task 15, 85 after it, 89
  after its fix round). Use the delta, not the absolute number.
- Line numbers drift with every edit — and Task 15 moved ~330 lines of
  `purecpu.rs`, so **every line number in an older report is wrong**. Grep.
- Binaries named in older reports may not exist; three were deleted with the
  user's approval when `/scratch` was tight.
