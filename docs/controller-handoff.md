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

Handoff commit: `6311e97`   Date: 2026-08-12   Reason: context budget — Tasks 13 and 16 closed, Task 14 dispatched and IN FLIGHT
Worktree / branch: `/scratch/oetiker/wezterm` (primary checkout) @ `update-optimization-rebased`
Trunk at time of writing: **`origin/main`** @ e723cf5 (merge base) — **reader: if trunk has moved, §2 is provisionally stale; if trunk now contains this branch's HEAD, this file is a tombstone** (`git merge-base --is-ancestor HEAD origin/main`). **Do NOT check against local `main`: it has NO common ancestor with this branch** (`git merge-base main HEAD` exits 1), so `--is-ancestor HEAD main` always reports "not merged" and is not a real check. `origin` is upstream wezterm (`wez/wezterm`) and legitimately runs ahead by ordinary upstream commits — that is NOT a merge signal. `fork` (`oetiker/wezterm`) is this fork's own remote and is where a merge would actually show up.
Sibling worktrees: `/scratch/oetiker/claude-worktrees/wezterm-osc52-upstream` @ `osc52-x11-fix` — the two-commit upstream PR (wezterm/wezterm#8043), unrelated to this pass but **now load-bearing for it** (§4, the L4 story); leave it alone until that PR resolves. This line cannot see worktrees created later; check yourself.

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

As of handoff commit `6311e97` (re-derive merge/push state — see §8):

**Execution mode: subagent-driven development**, confirmed by the user. The
ledger at `.superpowers/sdd/2026-08-11-purecpu-fixes/progress.md` is
authoritative on task state — trust it and `git log` over anything here.

- **Tasks 1–13, 15 and 16 are COMPLETE. Only Task 14 remains, and it is IN
  FLIGHT** (agent `task14-impl`, dispatched at `6311e97`). **Your first move is
  to find out what happened to it — §3.**
- **Task 13** (`bce58ea`): L2 `HintingInstance` cache (3.0× on the glyph-miss
  path, debug A/B), F4 hoisted into `colorease::animation_frame_interval` shared
  with `schedule_animation_timer_if_needed`, L1 built/measured/**null**/reverted,
  two `findings.md` corrections. **L5 closed as a non-defect** (§4).
- **Task 16** (`fbbb433`, fixed at `6311e97`) — **created by the controller, not
  in the plan.** Glyph ink escaping a dirtied cell is now repainted, by growing
  dirty rects to overlapping quads' own `dest_rect_of`. **The first defect in this
  pass reproduced in pixels before being fixed.** One fix round; review clean.
- **Suite at this commit: `wezterm-gui` 127, `wezterm-font` 18.** Both re-derived
  by the controller against the commit, not taken on report. **Use the delta.**
- **The version panic: root cause NOT found; the standing diagnosis and its
  workaround are both refuted** (§7). Untouched for two sessions.

**Two open threads**: Task 14, and the version panic.

## 3. Do this next

1. **Establish what Task 14 did — it was live when this was written.** An idle
   notification is not a report. Three signatures, three remedies: clean tree +
   commit → finished silently, go read the commit and `task-14-report.md`; dirty
   tree + no `cargo` → stalled, run the gates and commit for it; dirty tree +
   live `cargo` → deadlocked on a pending build, `SendMessage` **the same agent**
   (`task14-impl`), never re-dispatch. Its brief is `task-14-brief.md` and it
   **supersedes the plan's Task 14 text** — the plan's version is stale in three
   ways (§4).
2. **If Task 14 reports the GL invariant non-zero, that outranks everything.** It
   was told to stop and escalate rather than self-fix. A leak means a PureCpu fix
   reached the GPU path; the suspects are Task 13's F4 (`colorease` +
   `schedule_animation_timer_if_needed`, genuinely shared) and Task 16 (judged
   PureCpu-only by two reviewers, but new). Trace before reporting anything else.
3. **Then review Task 14** (`review-package` + `task-reviewer-prompt`), adjudicate,
   and the pass's task list is done. **Four Minors from Task 13's review were
   ROUTED INTO TASK 14** and its review is their gate — check they landed:
   the `colorease` `.max(1)` comment, the `skrifa_rasterizer` key comment, the
   `findings.md` L1/L5 records, and an optional `debug_assert` in
   `hinting_instance`. §7 lists what else was deferred to the final review.
4. **Then the final whole-branch review**, on the most capable model, pointed at
   the ledger's deferred-minor and parked lines — then
   `superpowers:finishing-a-development-branch`.
5. **`:20` was alive and validated at this commit** (1280x1024, 96x96 dpi, depth
   24, `marco` running, Xvnc pid 3438443). It is an ordinary user process and has
   died once when the user's ThinLinc session restarted. Recipe:
   `plans/2026-08-10-purecpu-parity-review.md` "Environment setup" — transcribe
   it, never improvise geometry/DPI. **The user has authorised starting a
   replacement if it is dead.** Validate any replacement against the documented
   control before trusting a number from it.

## 4. Lessons & traps  ← the irreplaceable part

Carried forward where still true, plus this session's.

- **A FINDING IS A CLAIM ABOUT THE CODE AT THE TIME IT WAS WRITTEN, AND THE TREE
  MOVES UNDER IT. `git log -S` ON THE EXACT LINE THE FINDING ATTACKS IS THE
  CHEAPEST POSSIBLE CHECK.** L4 said to replace `CURRENT_TIME` in
  `SetSelectionOwner` with the event timestamp. That line carries a comment
  arguing the opposite, and `git log -S` dates it to `ef7b636` — **the fork's own
  OSC 52 fix, the open upstream PR #8043 in the sibling worktree.** Implementing
  L4 would have reverted it and reintroduced the bug. One command turned a
  dispatch into a one-line ruling. **Make this standard before dispatching any
  finding-derived task.**
- **READ THE FINDING'S OWN CITATIONS BEFORE REWRITING A STEP AROUND THEM — a
  pre-flight scan that improves a plan can also break one.** L5 said "removed
  backends silently substituted". I grepped for the *subject* and found
  `front_end = "Software"` falling into an `enable_opengl()` arm, and wrote a
  brief sending the implementer there. **`findings.md` L5 names the three FONT
  backends by line number, all of which already warn**, and `Software` means
  "OpenGL with a software rasterizer" — a warning there would have been actively
  misleading. **My corrected brief was more confidently wrong than the plan text
  it replaced.** The implementer refuted it with citations.
- **A HANDOFF'S OPEN-QUESTIONS SECTION IS THE PART THAT ROTS FASTEST**, because
  closing a question requires someone to actively delete it. **The previous
  handoff contradicted itself** — §5 said "`painted_y_span` is the y twin", §7
  asked the user whether to build `painted_y_span` — and I read §7, put the
  question to the user, and **got authorisation for work that was already done.**
  It exists at `purecpu_dirty.rs:228` with six tests. **Before putting any §7
  question to the user, grep for the thing it claims is missing.** Same one-line
  check as the two lessons above.
- **A CONFIG-REACHABLE PERFORMANCE CLIFF IS STILL A DEFECT, AND NO MEASUREMENT
  ROUND FINDS ONE UNLESS SOMEONE VARIES THE CONFIG THAT REACHES IT.** Task 16's
  first version unioned dirty rects against everything on vertex buffer 1 — which
  carries **window borders** as well as glyphs. The border strips are *full window
  height*, and a lone pane's row bands already span `0..window_width`, so with any
  non-zero border width **every incremental frame becomes a full repaint**. The
  measured round used a zero-border config and could never have seen it.
- **A SELF-ASSESSED WEAK POINT IS A HYPOTHESIS; RUN THE ONE THE AGENT COULDN'T.**
  Fourteen for fourteen. Task 13's was "my instrument samples only the wezterm-gui
  pid, and `CreateGc` is a **server**-side operation" — a blind spot nobody else
  had noticed. I sampled the `:20` Xvnc across both arms: 7,7 vs 8,7 ticks over
  1800 GC pairs. Null on both sides, so the reverted fix stays reverted. Task 16's
  named weak point was, in the reviewer's own words, "the right one — it is
  exactly where the two findings live".
- **VERIFY THE CONTROLLER'S PREMISES, NOT JUST THE AGENTS'. Fifteen, sixteen and
  seventeen times now** — and this session produced the first three where the
  wrong premise was one *I* introduced while "correcting" the plan. **Put
  "challenge the premise, with evidence, instead of executing it" in EVERY
  brief.** It has now prevented more waste than any other instruction in this pass.
- **A BRIEF THAT NAMES ITS OWN FALLIBILITY GETS BETTER WORK BACK.** Task 16's
  brief said the bound might not exist and that proving so was a complete outcome.
  The implementer proved no bound is derivable at the dirty walk (`AnimatedCellScan`
  walks *cells* — no shaping, no `CachedGlyph`, no bearings), then found the exact
  extent one stage later: a quad's destination rect **is** the pixels it writes,
  via the blit's own `dest_rect_of`. **Not an approximation of the ink; the ink.**
  It also worked out that **both frames' quads are needed** — this frame's ink
  drawn, last frame's erased.
- **RULING A BRANCH OUT BY IMPOSSIBILITY IS CHEAPER THAN INSTRUMENTING IT** — but
  **a prediction's mechanism and its magnitude are separately falsifiable, and
  only measurement separates them.** The Task 10 review predicted the SGR 5 case
  "near the ceiling, ~250×": mechanism right, magnitude wrong by ~6×.
- **THE CONTROL BELONGS INSIDE THE SAME EXPERIMENT.** M2's subject result is
  `n=0`, worthless alone — a broken crop or wrong offset gives `n=0` too. The
  pre-fix control, run in the same round, reproduced `n=87, max 1.91, mean -0.05`
  **to the digit**. **A negative result needs a control most of all.**
- **REVERSE THE ARM ORDER BETWEEN REPETITIONS.** Task 13's L1 round 1 ran the
  arms in a fixed order and appeared to say the fix was slower; reversing showed
  **position in the sequence moved the number more than the code did.** Task 16
  then did this by default and found the same. Any A/B on this box without order
  reversal is uninterpretable.
- **INJECT INTO A COPY, OR COMMIT BEFORE INJECTING.** Task 16's implementer ran a
  fault injection by editing the shipping file and reverting with
  `git checkout -- <file>`, **discarding every uncommitted edit of the round**. It
  re-applied all five and wrote the incident down when nothing forced it to — a
  green gate afterwards would have hidden it completely.
- **A WRONG "WHY" NEXT TO RIGHT CODE IS ITS OWN DEFECT CLASS**, and this session
  found three more. The best: `animation_frame_interval`'s `.max(1)` is documented
  as belt-and-braces, but the call site reads `animation_fps` with no clamp and
  `0` is a legal `u8` — **the clamp removes a real config-reachable
  divide-by-zero panic**, and the hoist fixed a panic nobody set out to fix. The
  wrong comment is exactly what would have lost that.
- **CLASSIFY A KILL, DON'T COUNT IT** — exactly one test, at a pinned-value
  assertion, with the discriminating **negative** named. And **check that a mutant
  perturbs the value the target test reads**: five of six `painted_y_span`
  fixtures have integer edges, so `floor`→`ceil` is a no-op on them.
- **AN EDIT THAT MAKES A TEST PASS BY REMOVING WHAT IT TESTED IS THE THING TO
  LOOK FOR** when a fix round touches existing tests. Task 16's re-reviewer
  re-derived a hardcoded literal to prove an edited test kept its geometry.
- **THE PLAN'S OWN TEXT CAN BE WRONG AND A GOOD IMPLEMENTER WILL FOLLOW IT OFF A
  CLIFF.** Its line numbers are stale after every task; **its shell commands are
  wrong for this box** (`cp target/release/wezterm-gui` — `CARGO_TARGET_DIR` is
  `/home/oetiker/scratch/cargo-target`, there is no `target/` here); and its
  Task 14 Step 2 list of GL-touching code predates Tasks 13 and 16.
- **A REFUSAL CAN BE A FEATURE.** `gen-config.sh` refuses `FORCE_FULL_REPAINT=true`
  for OpenGL, where the option is inert. **Verify a new knob in BOTH directions
  before using it in a measurement.**
- **READ THE LIBRARY'S SEMANTICS, NOT THE PLAUSIBLE ONES.** `PathBuilder::finish`
  returns `None` for a one-verb builder. `OnceLock::set` returns `Err` carrying
  the value. `x/0.0` is `+inf`, not NaN. `Size::new(0.0).ppem()` is `Some(0.0)`,
  so it does **not** differ from an unscaled fold.
- **A REVIEWER'S ALGEBRA CAN BE WRONG IN THE PROJECT'S OWN KNOWN TRAP** —
  floating-point association order is part of "term for term". Compute, don't
  argue, about floats.
- **A BUILD LOG SAYING "Finished in 0.42s" MEANS NOTHING REBUILT.**
- **When a test compares against an oracle, decide which side is ground truth and
  put the slack on the OTHER one.** And **beware a case that passes for the wrong
  reason** — ask *why* a run passed.
- **AN IDLE NOTIFICATION IS NOT A REPORT.** Every agent in this pass has finished
  silently with its inline reply lost. Signatures in §3.1.
- **Never take an agent's gate result on its word — re-derive it against the
  commit**, and re-run at least one load-bearing fault injection yourself. Done
  for both tasks this session; both held.
- **`/scratch` CORRUPTS LARGE COPIES AND THE TWO DIAGNOSES CONTRADICT.** Task 13:
  `sync` between `cp` and `md5sum` fixed it every time. Task 16: two of three
  copies corrupted, ~10.2M differing bytes, and **`sync` did not fix it** — only a
  fresh `cp` did. **Unresolved. The only rule both support: md5-verify every copy
  against its source in a retry loop and re-copy on mismatch.**
  `wezterm-gui-t4rev-subject` is a known corrupt copy; do not use it.
- **Prior sessions leave evidence in their scratchpads** (`grep -rl` over
  `/tmp/claude-1003/`). The version-panic work lives almost entirely there.
- **`.superpowers/` is gitignored, so an agent cannot commit its report.** Say
  "write the file; it will not be committed, and that is expected."
- **Put `timeout: 600000` on every long Bash call, and tell the agent never to end
  a turn while a background shell is live.** As controller you may background and
  poll; a *subagent* cannot. The cause is the turn ending, not the timeout
  (anthropics/claude-code#50572, closed "not planned").
- **State process constraints as actions, not prohibitions.** "Redirect every
  command to a log file and Read it" lands; "don't pipe the gate" gets ignored.
- **Require the full report in a file, a short summary inline.** Eleven sessions.
- **A subagent's stdout lands in the human's real terminal.** Never `cat`/`head`/
  `tail` a binary or a PNG.
- **`pkill` as a literal string in a Bash tool command kills the call with exit
  144.** Use `pgrep` and `kill -TERM`; `|| true` does **not** protect you.
  `pgrep -c -f <pattern>` counts your own shell.
- **Focus state is a confound in any two-window X comparison.** Sequential capture.
- **llvmpipe is the reference and it works** — and it costs 176–249% CPU, 30–120×
  the PureCpu subject. The GPU reference exists only because of it.
- `timeout N cat </dev/null` does **not** wait; use `timeout N tail -f /dev/null`.
- **Harness trivia:** the test module in `render/purecpu.rs` is `mod test`
  (**singular**, `pub(crate)`); `purecpu_dirty.rs`, `purecpu_sampler.rs` and the
  `wezterm-font` modules use `mod tests`. `wezterm-gui` is binary-only:
  `cargo test -j4 -p wezterm-gui --bin wezterm-gui`. `cargo fmt --check` **fails
  repo-wide and did before this pass** — do not "fix" it. Three
  unused-placeholder warnings and one `fields start and end are never read` are
  likewise pre-existing.

## 5. Don'ts & constraints

The plan's Global Constraints block is authoritative — read it. The ones that
matter most:

- **Never touch X displays `:10`–`:14`** — other users, and the user's own
  ThinLinc session. All testing on `:20`. Xauthority:
  `/tmp/claude-1003/-scratch-oetiker-wezterm/659fab62-7b17-4059-a502-42815bcb9732/scratchpad/Xauthority-20`
  (belongs to a *previous* session's scratchpad and is what the running Xvnc
  authenticates against — do not "fix" the path; verify with `xdpyinfo`).
- **Destructive commands need the user's explicit approval.**
- **Nothing you run may emit raw bytes to the terminal.**
- **Never more than 4 cores**, including cargo (`-j4`).
- **Never run the 65536-atlas case** — 16 GiB on a shared box.
- **`has_color == 2.0` stays off the sampler.** Settled — do not re-decide.
- **The idle skip stays.** Each animated thing marks its own region. The rejected
  alternative — skip the early exit whenever any animation is live — repaints the
  whole window at the blink rate forever. **Measured: that is the 36–72% ceiling,
  against a 1.4% idle subject. Do not drift back toward it.**
- **`has_animation` is a CLOCK, never evidence that something is animating.**
- **The `focused` gate stays on animation rescheduling** — GL does the same
  (`render/paint.rs:121`).
- **Do not collapse `last_seqno_by_pane` back to a single field.**
- **The horizontal extent rule lives in THREE places** — `render/pane.rs`
  :110-152, :606-646, and `purecpu_dirty::painted_x_span`. Touch one, touch all.
  `painted_y_span` is the y twin (**it EXISTS** — `purecpu_dirty.rs:228`, six
  tests) and mirrors `render/pane.rs` term for term, association included.
- **Only a SAMPLED quad has ink whose extent is not its own rect.** That is why
  `collect_quad_dest_rects` skips `IS_SOLID_COLOR` and `IS_BG_IMAGE`. Do not
  "simplify" it into unioning every quad — that is the border cliff in §4.
- **Never raise `FUZZ`**, and never grade a case against another case's number.
- **Every config comes from `gen-config.sh`.** A hand-written config is the only
  way the bell guard can fire, and the only way a rejected config can silently
  make both windows use the same backend and report a triumphant `AE = 0`.
- **The OpenGL path must come out bit-identical** — Task 14 Step 2 checks it.
- Verdict column in the matrix holds a **bare token**; the table is **6 cells per
  row** and a literal `|` in a cell silently breaks it — twice already.
- Do not commit `mise.toml` (untracked, intentional); `out/` and `.superpowers/`
  stay gitignored — the ledger is deliberately local-only.
- **Never overwrite `/scratch/oetiker/wezterm-builds/wezterm-gui-prefix`** — the
  pre-fix reference and the floor/control arm of every measurement.
  **`wezterm-gui-fixed` in that directory is STALE** (pre-dates Tasks 12, 13, 16);
  do not use it as a reference and do not overwrite it without the user's say-so.
- **Do not go looking for `/scratch/oetiker/wezterm/.tag`.** Its absence is
  correct and harmless (§7).

## 6. Where the detail lives

- Change history: `git log 6311e97..HEAD`; previous handoff
  `git show 321b7ea:docs/controller-handoff.md`
- **Spec:** `docs/superpowers/specs/2026-08-11-purecpu-fixes-design.md` — §3
  carries the settled B-vs-C argument
- **Plan:** `docs/superpowers/plans/2026-08-11-purecpu-fixes.md` — 15 tasks;
  Global Constraints at the top bind every task. **Treat its task text with
  active suspicion** (§4). **Task 16 is not in it** — controller-authored.
- **Ledger:** `.superpowers/sdd/2026-08-11-purecpu-fixes/progress.md` —
  **authoritative on task state**, and it carries the reasoning behind every
  review finding and controller decision, **four controller retractions**, the
  measurement round's four verdicts with their arithmetic, and this session's
  L4/L5/`painted_y_span` premise failures written up with their mechanisms.
- Per-task briefs/reports/reviews: same directory,
  `task-N-{brief,report,review,rereview}.md`. **Tasks 15, 16 and 13 are the most
  complete artifacts in the pass.** Task 14's brief supersedes the plan's text.
- **The review this repairs:** `docs/purecpu-review/{README,parity-matrix,findings,noise-floor}.md`
- **The harness:** `tools/purecpu-parity/` — `compare-case.sh` (pixels),
  `sample-case.sh` (time-driven), `cpu-case.sh` + `measure-round.sh` (cost),
  `gen-config.sh` (every config), `lib.sh` (launch/find/cleanup contract),
  `corpus/` — including **`blink-descender.sh`, new in Task 16**, the only corpus
  entry that demonstrates a defect in pixels.
- `wezterm-gui/src/termwindow/purecpu_dirty.rs` (**note: NOT under `render/`**),
  and `render/purecpu.rs`, `render/purecpu_sampler.rs` — the units and the blit
  loop; `mod.rs` is plumbing that reads `self`.
- Binaries: `wezterm-gui-prefix` (pre-fix floor/control, **never overwrite**),
  `wezterm-gui-b10c3a5`, `wezterm-gui-t13-*` (the L1 arms),
  `wezterm-gui-t16-*` (Task 16's arms), `wezterm-gui-fixed` (**STALE**),
  `wezterm-gui-t4rev-subject` (**corrupt**). Harness selects via `WEZTERM_BIN`.
- Version-panic evidence: `/tmp/claude-1003/-scratch-oetiker-wezterm/47cc07f6-.../scratchpad/`

## 7. Open questions / pending decisions

- **Task 14's outcome is unknown at this commit** — see §3.1.
- **DEFERRED TO THE FINAL WHOLE-BRANCH REVIEW, from Task 13's review:** the
  `remain` inversion in `colorease` (`remain` is the phase *since* the last frame
  boundary and the short branch waits that phase rather than the time *to* the
  next boundary — pre-existing, faithfully preserved, but the new comment makes
  the oddity look intentional); and **`SkrifaRasterizer` lost `Sync`** (was
  `Send + Sync` at BASE, now `Send` only via `RefCell`) — unobservable in tree,
  but an auto-trait loss on a public type.
- **DEFERRED, from Task 16's review:** the pixel demonstration is **not
  re-runnable from committed tooling** — `blink-descender.sh` needs
  `line_height = 0.75` and `gen-config.sh` has no such knob. **This is in tension
  with "every config comes from `gen-config.sh`"**: the one demonstration that
  photographed a defect cannot be reproduced under the rule the pass enforces.
  Also: a per-frame `Vec` allocation for `last_text_quads`, and a note that the
  `IS_BG_IMAGE` skip is dead on the only production path today.
- **ANIMATED-IMAGE INK ESCAPING ITS CELL IS STILL NOT REPAINTED.** Image quads go
  to buffer 0 and are shifted by padding; covering them means admitting buffer 0,
  which is the border cliff. A real fix needs a filter separating image quads from
  background quads, which does not exist today. Stated in a doc comment at
  `collect_quad_dest_rects`; **not** a silent gap.
- **F1 — the `first_row` question, open with the experiment named.** The
  `AnimatedCellScan` maps lines by **stable row** while the renderer places them
  by **slice index**; they agree only while `first_row == viewport`. **Unresolved:
  whether a viewport pinned in scrollback that output then TRIMS PAST reaches it.**
  The experiment: drive `terminal_with_lines_mut` against a `Terminal` with a
  small `scrollback_size`, request a stable range that has aged out, assert the
  `first_row` the callback receives.
- **Task 12's residue, recorded not hidden:** `num_palette_entries` is still raw
  `u16` font data — a font declaring 65535 entries allocates a 65535-element `Vec`
  per COLR glyph render (no panic, out of M4's scope). And "unclipped vs
  invisible" for a blank COLR clip layer is a **judgement**, unfalsifiable without
  a real font.
- **M3/M4/M5 are GUARDED, NOT REPRODUCED** — no font reaching any of them was
  ever built. **Never let a doc say "fixed".**
- **THE VERSION PANIC — the standing diagnosis and its workaround are BOTH
  REFUTED; root cause still unknown.** Read the ledger's Phase 2 entry first.
  Binaries **are** tagged; `.tag`'s absence is harmless; **the panic is not
  build-determined**. **The blocker is a reproduction, not instrumentation.** Do
  not start by building an instrumented binary.
- **M2's attribution to Task 4 vs Task 7 is undetermined by choice.** Settle it,
  if ever, by building at `b7936ac` and re-running the control/subject pair.
- **What no test covers:** nothing in `call_draw_purecpu` itself; inside
  `blit_vertex_buffer`, the whole `subpixel_aa` arm, `apply_hsv`, per-vertex HSV,
  `mix_value != 0`, the degenerate-quad `continue`. Nor Task 10's `mod.rs`
  plumbing, `AnimatedCellScan`, or `image_next_frame_due`.
- **A cheap, high-value experiment nobody has run:** assert that the
  pre-extraction body (`c1fc3cf`) and the current one produce bit-identical
  framebuffers over a corpus of random quads.
- **`cpu-case.sh` should grow an optional second pid to sample.** Its client-only
  blind spot was invisible until an implementer named it (§4).
- **Task 8's colour-emoji alpha ruling is unverifiable on this instrument**;
  settling it needs a depth-32 `xcb::GetImage` or an assertion on
  `PureCpuState::frame_buffer` (possible via Task 15's rig).
- **Task 8's reviewer left one falsifiable prediction:** make `subpixel_mask`
  return `[f32; 4]` and re-measure `AE fuzz 0` on `plain.sh`; it predicts ≈2458.
- **Nobody has photographed Task 4's ≤1 px fringe defect or its fix.**
- **Is option C worthwhile after this pass?** Spec §3's condition is now met.
- **The user has still not read `docs/purecpu-review/README.md`.**
- **Should focus change force a full repaint?** `inactive_pane_hsb` recolours
  panes on focus change and no `force_full` trigger covers it.

## 8. Staleness watch

- **Integration state must be re-derived, never inherited.** Whether this branch
  is merged, pushed or superseded is not knowable from this file:
  `git merge-base --is-ancestor HEAD origin/main`, `git log --oneline HEAD..origin/main`,
  `git branch -a --contains HEAD`. If this branch is merged, stop reading and go
  to the successor's handoff.
  **Use `origin/main`, not local `main`.** Local `main` has NO common ancestor
  with this branch — `git merge-base main HEAD` exits 1 — so
  `--is-ancestor HEAD main` reports "not merged" unconditionally and looks like a
  passing check while testing nothing. This cost the controller a wrong reading at
  the end of the pass. `origin` is upstream wezterm and legitimately runs ahead;
  `fork` (`oetiker/wezterm`) is where a merge of this work would actually appear.
- **Sibling worktrees / other workstreams may exist that this file cannot name** —
  anything started after the handoff commit is invisible here.
- **Task 14 was IN FLIGHT when this was written.** §2's task state and the suite
  counts below are pre-Task-14. The ledger is authoritative; read it rather than
  trusting §2.
- **This file's §7 is the part most likely to be wrong** — see §4's lesson on
  open-questions rot, which cost a user decision on false premises this session.
  **Grep before putting any §7 item to the user.**
- **`:20` is an ordinary user process and has died once already.** Verify with
  `DISPLAY=:20 XAUTHORITY=<path> xdpyinfo | grep dimensions` (expect `1280x1024`,
  `96x96 dpi`, depth 24) and check `marco` is on `:20` before any capture. Xvnc
  pid was 3438443; pids do not survive a restart.
- **All measured percentages are tied to specific binaries and to a shared, busy
  128-core box.** Absolute numbers drift with load; **the floor→subject→ceiling
  *ratios* are the result.** Re-run a round rather than comparing against a stored
  number, and reverse arm order between repetitions.
- Suite sizes at this commit: `wezterm-gui` **127**, `wezterm-font` **18**.
  **Use the delta, not the absolute number.**
- Line numbers drift with every edit. **Every line number in an older report is
  suspect — grep.**
- Binaries named in older reports may not exist; several were deleted with the
  user's approval when `/scratch` was tight.
