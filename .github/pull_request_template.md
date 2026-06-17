# Pull Request — Multi-Agent Governance Committee Review

> Every pull request that touches `typefix` source, tests, CI, or
> dependencies MUST obtain **unanimous APPROVED** verdicts from all four
> committee agents (Architect, Developer, QA, Security) before it can be
> merged into `main`.
>
> A single `REJECTED` verdict from any agent blocks the merge.
> See [`docs/governance.md`](../docs/governance.md) for the full process
> and [`docs/plan-implementacion.md` §9](../docs/plan-implementacion.md)
> for the mandate.

---

## Summary

<!-- One- or two-sentence description of the change. -->

**What does this PR do, and why?**

## Linked Issues

<!-- Reference any tracked issue, ticket, or design doc. -->

- Closes #
- Related to #

## Change Type

<!-- Pick the ones that apply. -->

- [ ] Bug fix
- [ ] New feature
- [ ] Performance improvement
- [ ] Refactor (no behavior change)
- [ ] Documentation only
- [ ] CI / governance / tooling
- [ ] Dependency upgrade

## Affected Modules

<!-- Check every directory this PR modifies. CODEOWNERS uses this signal
     to route the review to the right agent. -->

- [ ] `src/core/`        (Architect lead)
- [ ] `src/language/`    (Architect + QA lead)
- [ ] `src/correction/`  (Developer + QA)
- [ ] `src/hooks/`       (Security owns FFI / unsafe)
- [ ] `src/pipeline.rs`  (Architect)
- [ ] `src/memory.rs`    (Security + Developer)
- [ ] `benches/`         (Developer)
- [ ] `tests/`           (QA owns the gate)
- [ ] `docs/`            (Architect)
- [ ] `.github/`         (Architect + Security)
- [ ] `Cargo.toml` / `Cargo.lock`  (Developer + Security)

## Risk & Rollback

- **Risk level:** <!-- Low / Medium / High -->
- **Blast radius:** <!-- Which user-facing paths or invariants are touched? -->
- **Rollback plan:** <!-- How do we revert cleanly? Revert commit, feature flag, drain, etc. -->

---

## Committee Verdict Checklist

> The four agent roles below MUST all tick **APPROVED** before this PR
> can be merged.  In the comment thread, the named agent must reply with
> `VERDICT: APPROVED` (or `VERDICT: REJECTED`) together with the
> technical analysis mandated by §9.3 of the implementation plan.

- [ ] **Agent-Architect** — DDD constraints, Rust low-level, memory layout
  - VERDICT: ☐ APPROVED  ☐ REJECTED
  - Notes:
- [ ] **Agent-Developer** — Idiomatic Rust, clippy clean, complexity
  - VERDICT: ☐ APPROVED  ☐ REJECTED
  - Notes:
- [ ] **Agent-QA** — Boundary conditions, error paths, no happy-path assumptions
  - VERDICT: ☐ APPROVED  ☐ REJECTED
  - Notes:
- [ ] **Agent-Security** — `unsafe` blocks, memory leaks, OS-hook attack surface
  - VERDICT: ☐ APPROVED  ☐ REJECTED
  - Notes:

> If any box above is `REJECTED`, the PR **fails** the committee gate
> and must be rewritten per the rejection analysis.

---

## Pre-Commit Hooks (Local)

> Confirm the author has run these locally before opening the PR.
> CI will re-run them and **fail** if any box is unchecked.

- [ ] `cargo fmt --all -- --check`  (no diff)
- [ ] `cargo clippy --all-targets --all-features -- -D warnings`  (zero warnings)
- [ ] `cargo build --release`  (succeeds)
- [ ] `cargo test --all-features`  (all green)
- [ ] `cargo llvm-cov --all-features --summary-only`  (≥ 90% line coverage)
- [ ] `cargo bench`  (no regression vs. baseline in `docs/`)

## Pull-Request Gates (CI)

> These run automatically on every push and PR to `main`.
> They mirror the §9.2 / §9.4 acceptance criteria.

- [ ] CI `fmt` job is green
- [ ] CI `clippy` job is green (warnings = errors)
- [ ] CI `build` job is green (debug + release)
- [ ] CI `test` job is green (unit + integration + stress)
- [ ] CI `coverage` job is green (≥ 90% line coverage enforced)
- [ ] CI `committee-rules` job is green (zero `unwrap()` / `expect()` and zero `FIXME` / `TODO` in `src/`)

---

## Code Quality Self-Audit

> The author must tick these before requesting review.  They are also
> covered by the `committee-rules` CI job.

- [ ] No new `unwrap()` in production code (`src/`); every fallible call
      uses `?`, `match`, or `if let` with a typed error.
- [ ] If `unwrap()` is unavoidable, an inline `// SAFETY:` or
      `// UNWRAP-JUSTIFICATION:` comment explains why and links an issue.
- [ ] No new `FIXME` or `TODO` in production code.
- [ ] No new `unsafe` block, OR a new `// SAFETY:` comment justifies it
      and the Security agent signs off below.
- [ ] No new dependency added, OR `Cargo.toml` change is justified in
      the PR description and the Developer + Security agents approve.
- [ ] All new public APIs are documented with rustdoc.
- [ ] All new modules / functions have at least one unit test.
- [ ] Boundary / UTF-8 edge cases are covered by a test.

## Performance & Memory

- [ ] No allocation introduced in the keystroke hot path
      (`CharBuffer::push` and downstream).
- [ ] No new heap growth on the steady-state path.
- [ ] Benchmark deltas (vs. `main`) reported in the PR description
      when the change is in the hot path.
- [ ] Memory profile attached when `dhat` / `jemalloc` was re-run.

---

## Test Plan

<!-- What did the author run, and on what hardware? -->

- [ ] `cargo test --all-features` is green locally
- [ ] `cargo test --release --test stress_test -- --nocapture` is green
- [ ] New unit tests added for the change
- [ ] New integration / boundary tests added for the change
- [ ] `cargo bench` regression within ±5% of `main` (or justified)

## Documentation

- [ ] `docs/plan-implementacion.md` updated if the change affects scope
- [ ] `README.md` updated if the change is user-facing
- [ ] `docs/governance.md` updated if the change affects the committee
- [ ] rustdoc added / updated for any new public API

## Out-of-Scope

<!-- What did you intentionally NOT touch?  Calling this out prevents scope creep. -->

-

---

## Final Statement

> By submitting this PR, the author confirms:
> 1. The change has been self-audited against the checklist above.
> 2. The local pre-commit hooks (fmt, clippy, test, coverage) are green.
> 3. The four committee verdicts in this template are all
>    `APPROVED` before merge, or this PR will be marked draft / blocked.
