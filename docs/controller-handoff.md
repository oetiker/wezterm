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

Handoff commit: `2427446`   Date: 2026-08-11   Reason: context budget, before starting Task 15 (a large refactor) on a tired context
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
  **Correction established by Task 8, and the plan said the opposite:** that RGB
  is **sRGB-encoded** and must be **linearised** by PureCpu, because GL's atlas
  is an `SrgbTexture2d` that converts on read while `colorMask` is the one shader
  output never re-encoded by `to_srgb`. See §4.

## 2. Where we are now

As of handoff commit `2427446` (re-derive merge/push state — see §8):

**Execution mode: subagent-driven development**, at the user's explicit request.
The ledger at `.superpowers/sdd/2026-08-11-purecpu-fixes/progress.md` is
authoritative on task state — trust it and `git log` over anything here.

- **Tasks 1–8 complete and reviewed.** Suite at **75 tests**, 0 failed, re-derived
  by the controller against the commit, not taken on an agent's word.
- **Task 7** (sampler wiring) — iTerm2 OSC 1337 and Sixel 300×300 both went
  **bit-identical** to GL (PAE 61423 → 0, 1542 → 0); DECDWL/DECDHL 45232 → 257;
  `wide-sixel` 60652 → 1285 (47×, against the *right* control). Reviewed
  (approved with minors), fix round `a0ee332`, majors recorded in the plan.
- **Task 8** (subpixel AA) — body **PAE 29041 → 257**, the noise floor, `AE` 0 at
  1% fuzz, with a fault injection and an inertness row. Reviewed: **approved, no
  code defect**; fix round `2427446`.
- **Tasks 9–14 not started. Task 15 is authorised by the user and is next** (§3).

**Two open threads the user has explicitly greenlit**, both in §3: Task 15 (the
framebuffer harness) and the version-panic root cause (Phase 1 done, refuted the
existing diagnosis, not solved).

## 3. Do this next

1. **Task 15 — the framebuffer harness.** Authorised by the user. The plan's
   Task 15 section has the shape, what it unlocks in priority order, and the two
   mutants that must die (`cy2.min(blit_y2 + 1)`, and an unconditional `panic!`
   in the clip loop — **both survive today**). It is the single highest-leverage
   piece of work left: it converts three separate "argued on paper" results into
   assertions, including the review's own equivalence proof.
2. **Then finish the version-panic debug** (§7). Phase 1 is done and is written
   up in the ledger; the next decisive experiment is named there. **Do not run it
   while a review or another implementation holds this worktree.**
3. **Then Tasks 9–14** in plan order. Nothing else needs the user until Task 14
   except the `painted_y_span` scope question (§7).
4. **Do not run a review and an implementation concurrently in this worktree.**
   Both edit the tree and build; they will corrupt each other. One at a time.
   This now has teeth beyond the tree: the **shared `CARGO_TARGET_DIR` is
   contended**, and a `cp` taken right after a successful build produced a
   different md5 minutes later and an unrunnable binary.
5. **`:20` is alive and validated** (1280x1024, 96x96 dpi, depth 24). It is an
   ordinary user process and has died once already when the user's ThinLinc
   session restarted. Recreation recipe:
   `plans/2026-08-10-purecpu-parity-review.md:768-782` — transcribe it, never
   improvise geometry/DPI, and **starting an X server is the USER'S decision**.

## 4. Lessons & traps  ← the irreplaceable part

Carried forward from prior sessions where still true, plus this one's.

- **VERIFY THE CONTROLLER'S PREMISES, NOT JUST THE AGENTS'.** This session's
  dominant theme: **three controller premises were wrong in one day**, each caught
  before it cost anything, twice by an agent instructed to push back and once by
  the controller re-checking itself. Put "challenge the premise, with evidence,
  instead of executing it" in **every** brief. It has now prevented more waste
  than any other instruction in this pass.
- **ENUMERATE BY RESOLVED ARGUMENT, NOT BY LITERAL CALL TEXT.** The controller
  concluded `has_color == 3.0` could not run under subpixel by grepping
  `allocate(1)`. Four helpers *forward* a `layer_num` parameter, so `borders.rs`
  (which passes 1) was invisible. A grep over call text cannot see a parameter.
- **A DELTA WITHOUT A HELD-CONSTANT CHECK IS A CORRELATION**, and **a control
  binary must share every prior commit in the pass.** `wide-sixel` read as a
  regression for two rounds because the control predated the atlas cap.
- **NAME WHICH NUMBER CARRIES THE CLAIM.** Task 8's fault injection (PAE 29041)
  does *not* discriminate what the report leaned on it for — turning on
  dual-source blending over a *grayscale* atlas would look the same. The number
  that actually proves per-channel data reached the atlas is a pixel dump showing
  a **non-neutral** GL pixel from **neutral** endpoints. Both were in the report;
  only one carries the claim.
- **MODEL THE RESIDUAL INSTEAD OF ARGUING ABOUT IT.** Task 8's reviewer explained
  6489 unexplained differing pixels by writing a standalone program that
  enumerated every source, destination and attainable atlas byte — deriving a
  1-LSB bound *and* predicting the observed count to 2.4%. Reusable technique:
  when a number has no mechanism, compute the mechanism's prediction.
- **A STRING THAT LOOKS LIKE EVIDENCE MAY BE MEASURING SOMETHING ELSE.**
  `--version` printing "someone forgot to call assign_version_info" was read as
  "this binary has no version tag". clap handles `--version` *before* `bootstrap()`
  runs, so every build prints it. The inference built on it was void.
- **A trace that fires on the way into a fix is not evidence the defect survives
  it** (`glyphcache.rs:858-867` logs from inside the branch that resets scale).
- **An unchecked control is just another number**, and **a non-zero control is not
  a failing control** — open the document that says what it should print. Both a
  controller and an implementer nearly voided good sweeps this way.
- **Reasoning from a document that describes a measurement, instead of from the
  measurement.** The root shared by several errors in this pass.
- **A REVIEWER'S ALGEBRA CAN BE WRONG IN THE PROJECT'S OWN KNOWN TRAP.** Task 7's
  reviewer proved adjacent solid quads cannot seam because both carry the same
  edge `e` — but `screen_line.rs` builds one as `(left + i*cw) + (w*cw)` and the
  next as `left + ((i+w)*cw)`. **Floating-point association order is part of
  "term for term"**; an ulp across a pixel centre is a 1px seam. Unmeasured.
- **Ask implementers to flag their own weak points. Eight for eight.** Every task
  has had its real finding there. Task 8's named the one decision no instrument
  here can check, and proved it unverifiable rather than asserting it.
- **Mutation testing is this pass's highest-yield instrument.** Make every new
  test watch a mutant die; **check the mutant perturbs the value the target test
  reads**, and **pick the mutation that ISOLATES the assertion under test**.
- **Do not restructure code that no test executes**, even for a real improvement.
  Task 7's perf finding was deferred into Task 15 for exactly this reason.
- **A test that asserts no value discriminates almost nothing.** Pin values.
- **Ask "which single test is the only guard on X?"**
- **Do not bank the first red you see — classify it.** A failure in the wrong
  direction is not confirmation.
- **When a test compares against an oracle, decide which side is ground truth and
  put the slack on the OTHER one.**
- **The plan's own text can be wrong, and a good implementer will follow it off a
  cliff.** Twelve times now, and Task 8's was the worst: "no colour-space
  conversion applies to the mask" was **false**, and only measurement caught it —
  the plan-faithful implementation measured PAE 14135 and looked like a big win.
  **Keep running the pre-flight scan; it costs ten minutes.**
- **Verify library and shader semantics from source, not from a restatement.**
- **The control experiment is the strongest instrument this project has** — and
  **put the control INSIDE the capture** so a null result cannot pass as success.
- **Beware a case that passes for the wrong reason.** Ask *why* a run passed.
- **AN IDLE NOTIFICATION IS NOT A REPORT — and in this session all three agents
  finished silently with their inline replies lost.** Every time, the work was
  complete and committed. Signatures: clean tree + commit → finished silently,
  go read the commit and the report file; dirty tree + no build → stalled; dirty
  tree + live `cargo` → deadlocked, `SendMessage` the *same* agent.
- **Never take an agent's gate result on its word — re-derive it against the
  commit.** One command each time.
- **A crash kills the subagent but not its work.** Read the tree and the logs
  before re-dispatching anything.
- **Prior sessions leave evidence in their scratchpads.** The version-panic
  investigation found an earlier session's captured panic *and* its workaround
  (`grep -rl` over `/tmp/claude-1003/`). Look there before reproducing anything.
- **Put `timeout: 600000` on every long Bash call, and tell the agent never to end
  a turn while a background shell is live.** The cause is the turn ending, not the
  timeout (anthropics/claude-code#50572, closed "not planned").
- **State process constraints as actions, not prohibitions.** "Redirect every
  command to a log file and Read it" lands; "don't pipe the gate" gets ignored.
- **Subagents terminate on idle and their inline replies are frequently lost.**
  Require the full report in a file, a short summary inline. Seven sessions now.
- **`/scratch` on this box silently corrupts large writes**, and the shared
  `CARGO_TARGET_DIR` is contended. `md5sum` every binary copy and re-hash after
  copying. `wezterm-gui-t4rev-subject` is a corrupt copy; do not use it.
- **`pkill` as a literal string in a Bash tool command kills the call with exit
  144.** Use `pgrep` and `kill -TERM`. `lib.sh`'s `kill_class` has the same defect
  and `|| true` does **not** protect you — the shell dies before `||` runs.
  Workaround: `setsid … &` + `kill -TERM -$PGID`, and `pgrep -af "partrace[c]pu"`.
- **A subagent's stdout lands in the human's real terminal.** Redirect everything
  to a log file and Read it; never `cat`/`head`/`tail` a binary or a PNG.
- **Focus state is a confound in any two-window X comparison.** Sequential capture.
- **llvmpipe is the reference and it works** — this GPU-less Xvnc box resolves
  OpenGL to Mesa llvmpipe 4.5, the only reason a GPU reference exists.
- `timeout N cat </dev/null` does **not** wait; use `timeout N tail -f /dev/null`.
- **Harness trivia:** the test module in `render/purecpu.rs` is `mod test`
  (**singular**); `purecpu_sampler.rs`/`purecpu_dirty.rs` use `mod tests`.
  `wezterm-gui` is binary-only: `cargo test -j4 -p wezterm-gui --bin wezterm-gui`.

## 5. Don'ts & constraints

The plan's Global Constraints block is authoritative — read it. The ones that
matter most:

- **Never touch X displays `:10`–`:14`** — other users, and the user's own
  ThinLinc session. All testing on `:20`. Xauthority:
  `/tmp/claude-1003/-scratch-oetiker-wezterm/659fab62-7b17-4059-a502-42815bcb9732/scratchpad/Xauthority-20`
  (belongs to a *previous* session's scratchpad and is what the running Xvnc
  authenticates against — do not "fix" the path; verify with `xdpyinfo`).
- **Starting an X server is the USER'S decision, not the controller's.**
- **Destructive commands need the user's explicit approval** — deletions were
  flagged and left undone by an agent, correctly, until the user authorised them.
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
  Task 8 added a `RENDER_TARGET` knob there; its first draft *was* rejected and
  `check_no_config_error` caught it — the guard is demonstrated, not assumed.
- **The OpenGL path must come out bit-identical.** Verified in Task 14.
- Verdict column in the matrix holds a **bare token**; the table is **6 cells per
  row** and a literal `|` in a cell silently breaks it — twice already.
- Do not commit `mise.toml` (untracked, intentional); `out/` and `.superpowers/`
  stay gitignored — the ledger is deliberately local-only.
- **Never overwrite `/scratch/oetiker/wezterm-builds/wezterm-gui-prefix`** — the
  pre-fix reference, not reproducible now that fixes have landed.

## 6. Where the detail lives

- Change history: `git log 2427446..HEAD`; previous handoff
  `git show 0ebbbdf:docs/controller-handoff.md`
- **Spec:** `docs/superpowers/specs/2026-08-11-purecpu-fixes-design.md` — §3
  carries the settled B-vs-C argument
- **Plan:** `docs/superpowers/plans/2026-08-11-purecpu-fixes.md` — now **15**
  tasks; Global Constraints at the top bind every task. **Treat its task text
  with active suspicion** (§4). Task 7's section carries a post-implementation
  note on the solid-quad scope expansion and the seam caveat; Task 15 is the
  harness; Task 14 Step 7 now refuses to claim the suite covers the blit loop.
- **Ledger:** `.superpowers/sdd/2026-08-11-purecpu-fixes/progress.md` —
  **authoritative on task state**, and it carries the full reasoning behind each
  review finding, each controller decision, and **two controller retractions**.
  The retractions are as load-bearing as the findings.
- Per-task briefs/reports/reviews: same directory,
  `task-N-{brief,report,review,review-brief}.md`, plus `task-N-fix1-*.md`.
  Task 7's and Task 8's reports are the two most complete artifacts in the pass.
- **The review this repairs:** `docs/purecpu-review/{README,parity-matrix,findings,noise-floor}.md`
- `wezterm-gui/src/termwindow/render/purecpu.rs` — the quad loop, `blend_over`,
  `blend_over_masked`, `subpixel_mask` (whose doc comment carries the sRGB
  correction, the sub-layer-1 enumeration and the 1-LSB bound)
- `wezterm-gui/src/termwindow/render/purecpu_sampler.rs` — the sampler unit + `Quad`
- `wezterm-gui/src/termwindow/purecpu_dirty.rs` — the dirty unit
- Binaries: `wezterm-gui-prefix` (pre-fix, **never overwrite**),
  `wezterm-gui-c5cc8ab-review`, `wezterm-gui-t7-773b845`,
  `wezterm-gui-t8-fd6142a` (md5 `06f12cab5cbfff71bbac495ef9f9889c`),
  `wezterm-gui-t4rev-subject` (**corrupt**). Harness selects via `WEZTERM_BIN`.

## 7. Open questions / pending decisions

- **Task 15 is authorised and not started.** Plan §Task 15.
- **The version panic — Phase 1 done, root cause NOT found.** The existing
  diagnosis ("assign_version_info called twice") is **refuted** in the ledger:
  `OnceLock::set` fails regardless of value, so a double call would panic for
  untagged builds too, and they demonstrably do not. **`/scratch/oetiker/wezterm/.tag`
  was moved away by an earlier session and never restored** — that, not build-script
  caching, is why every binary this pass is untagged. Next experiment is named in
  the ledger. Not fork-specific; worth reporting upstream once understood.
- **The solid-quad seam question is unmeasured** — see the Task 7 note in the plan.
  Task 15 answers it more cheaply than a capture.
- **Task 8's colour-emoji alpha ruling is unverifiable on this instrument** and
  every number in that report is consistent with it being wrong. Settling it needs
  a depth-32 `xcb::GetImage` or an assertion on `PureCpuState::frame_buffer` —
  i.e. Task 15 again.
- **Task 8's reviewer left one falsifiable prediction:** make `subpixel_mask`
  return `[f32; 4]` and re-measure `AE fuzz 0` on `plain.sh`; it predicts ≈2458.
  Materially above that means a second mechanism and its primary ruling is wrong.
- **The four `parity-matrix.md` defects** from Task 7's verification are Task 13's
  to land, plus the **PureCpu 8192 vs GL 16384 atlas-cap asymmetry**, which is an
  unlisted finding that wants a matrix row.
- **`painted_y_span` — a scope question for the user, queued for Task 14.**
  **Ask, do not decide unilaterally.**
- **Does M2 survive Task 4?** Deferred to Task 11; re-measure before fixing.
- **Nobody has photographed Task 4's ≤1 px fringe defect or its fix.**
- **Is option C worthwhile after this pass?** Spec §3's condition (sampler and
  dirty units tested) is now met.
- **The user has still not read `docs/purecpu-review/README.md`** — only
  summaries and `task-7-report.md`.
- **M3/M4/M5 will be fixed blind** — triggers never reproduced. Reports must say
  "panic site guarded", not "fixed".
- **Should focus change force a full repaint?** `inactive_pane_hsb` recolours
  panes on focus change and no `force_full` trigger covers it.

## 8. Staleness watch

- **Integration state must be re-derived, never inherited.**
  `git merge-base --is-ancestor HEAD main`, `git log --oneline HEAD..main`,
  `git branch -a --contains HEAD`. Note `main` is upstream wezterm and
  legitimately runs ahead.
- **Sibling worktrees / other workstreams may exist that this file cannot name.**
- Task state in §2 reflects the ledger at this commit. **The ledger is
  authoritative; read it rather than trusting §2.**
- **`:20` is an ordinary user process and has died once already.** Verify with
  `DISPLAY=:20 XAUTHORITY=<path> xdpyinfo | grep dimensions` (expect `1280x1024`,
  `96x96 dpi`, depth 24), and validate any replacement against a documented
  control before trusting a measurement.
- The suite was **75 tests** at this commit (69 after Task 7, 70 after its review,
  75 after Task 8). Use the delta, not the absolute number.
- Line numbers drift with every edit. Grep, don't trust them.
- Three binaries and a 1.9 GB diagnostic target dir were deleted this session with
  the user's approval; `/scratch` was at 59%. Do not assume any binary named in an
  older report still exists.
