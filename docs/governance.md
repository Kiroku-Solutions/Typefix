# Multi-Agent Governance & CI/CD

> Reference: [`docs/plan-implementacion.md` §9](./plan-implementacion.md#9-multi-agent-governance--cicd)
>
> Mandate: **Every change to `typefix` source, tests, CI, or
> dependencies must be approved unanimously by the four-agent
> committee before it can land on `main`.**

This document is the operational counterpart to §9 of the implementation
plan.  It explains who the committee is, how an approval round works,
what the CI/CD pipeline enforces automatically, and how the §9.4
acceptance criteria are measured.

---

## 1. Committee Structure

The committee is composed of four specialized agent roles.  Each role
owns a distinct domain of the codebase and produces a single
`VERDICT: [APPROVED / REJECTED]` per pull request.

| Agent             | Domain                                                                                  | Verdict scope                                                            |
|-------------------|-----------------------------------------------------------------------------------------|--------------------------------------------------------------------------|
| **Agent-Architect** | DDD constraints, Rust low-level design, memory layout & FFI safety                    | `src/core/`, `src/language/`, `src/pipeline.rs`, `docs/`, `.github/`    |
| **Agent-Developer** | Idiomatic Rust, clippy compliance, algorithmic complexity, DRY                       | `src/correction/`, `benches/`, `Cargo.toml`, `Cargo.lock`                |
| **Agent-QA**       | Boundary conditions, error handling, no happy-path assumptions, UTF-8 / multi-language | `src/language/`, `src/correction/`, `tests/`                             |
| **Agent-Security** | `unsafe` audit, memory leaks, OS-hook attack surface, dependency supply-chain          | `src/hooks/`, `src/memory.rs`, `.github/`, `Cargo.toml`, `Cargo.lock`    |

> The current deployment uses `@camilo` as the placeholder human owner
> behind all four roles (see [`.github/CODEOWNERS`](../.github/CODEOWNERS)).
> In a multi-agent production deployment, replace the placeholder with
> the real per-agent handles (e.g. `@typo-architect`,
> `@typo-developer`, `@typo-qa`, `@typo-security`).

### 1.1 Authority and Tie-Breaking

- **Unanimity is required.**  A single `REJECTED` verdict blocks the merge.
- The human owner (`@camilo`) acts as the **final tie-breaker** and may
  override a rejection only with an explicit, written justification in
  the PR thread.  Overrides are recorded in the meeting notes.
- The committee may be temporarily reduced to three agents only if a
  quorum waiver is recorded in the same PR; the missing agent must
  sign off retroactively before the next release tag.

---

## 2. Approval Process

### 2.1 Author flow

1. Author opens a **draft** pull request against `main`.
2. Author runs the **local pre-commit hooks** listed in §3.1 and ticks
   the **Code Quality Self-Audit** in
   [`.github/pull_request_template.md`](../.github/pull_request_template.md).
3. Author converts the PR from draft to **ready for review** and
   requests review from CODEOWNERS (`@camilo`).
4. CI runs the **automated gates** listed in §3.2 in parallel.  CI is a
   hard precondition for committee review: a red CI = no review.
5. Each of the four committee agents replies in the PR thread with
   `VERDICT: APPROVED` or `VERDICT: REJECTED` **plus** the technical
   analysis mandated by §9.3 of the implementation plan.
6. The PR template's committee checklist is updated to reflect every
   verdict.
7. Once all four verdicts are `APPROVED` and CI is green, the PR may be
   merged (squash or rebase) by `@camilo`.

### 2.2 Committee flow

For every PR:

```
              ┌────────────────────────────┐
              │  Author opens / updates PR │
              └─────────────┬──────────────┘
                            │
              ┌─────────────▼──────────────┐
              │   CI gates (automated)     │
              │   fmt / clippy / build /   │
              │   test / coverage / rules  │
              └─────────────┬──────────────┘
                            │ green
              ┌─────────────▼──────────────┐
              │  Four committee verdicts   │
              │  Architect / Developer /   │
              │  QA / Security             │
              └─────────────┬──────────────┘
                            │ all APPROVED
              ┌─────────────▼──────────────┐
              │  Human owner merges        │
              └────────────────────────────┘
```

### 2.3 Rejection handling

A rejection **must** include:

- The line(s) of code or configuration that violate the rule.
- The specific clause of §9 of the implementation plan (or the
  governing `docs/governance.md` section) that the change breaches.
- A concrete suggestion for the rewrite.

Rejections are not personal; the author reopens the conversation, fixes
the issue, and re-requests review.  Re-rejection on the same issue
escalates to a synchronous committee meeting.

---

## 3. CI/CD Pipeline

The pipeline is defined in [`.github/workflows/ci.yml`](../.github/workflows/ci.yml).

### 3.1 Pre-commit hooks (local, mandatory before push)

The following checks **must** be green on the author's machine before
pushing:

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all-features`
- `cargo llvm-cov --all-features --summary-only`  (line coverage ≥ 90%)
- Boundary tests against UTF-8 edge cases (covered by the unit /
  integration test suite).

A `pre-commit` configuration (`.pre-commit-config.yaml`) wiring these
into a `pre-commit` framework is recommended but **not** enforced by
CI; CI re-runs the same commands authoritatively.

### 3.2 Pull-request gates (CI, blocking)

CI runs in parallel on every `push` and `pull_request` against `main`
using the `stable` Rust toolchain.  It uses
[`Swatinem/rust-cache@v2`](https://github.com/Swatinem/rust-cache) to
cache the `target/` directory and the cargo registry between runs.

| Job                | Command                                                              | Purpose                                                                 |
|--------------------|----------------------------------------------------------------------|-------------------------------------------------------------------------|
| `fmt`              | `cargo fmt --all -- --check`                                         | Enforce canonical formatting.                                            |
| `clippy`           | `cargo clippy --all-targets --all-features -- -D warnings`           | Lint with warnings treated as errors.                                    |
| `build`            | `cargo build --all-targets` + `cargo build --release`                | Confirm compilation in both profiles.                                    |
| `test`             | `cargo test --all-features` + integration + release-mode stress       | Exercise unit, integration, and stress tests.                            |
| `coverage`         | `cargo llvm-cov --all-features --workspace`                          | Enforce the §9.4 90% line-coverage gate.                                 |
| `committee-rules`  | `rg` scan for `.unwrap()` / `.expect()` / `FIXME` / `TODO` in `src/` | Enforce the §9.2 and §9.4 "zero unwrap, zero FIXME" rules.              |
| `all-gates`        | Aggregator                                                           | Fails the run if **any** upstream job failed or was cancelled.            |

All jobs must be green before the PR is eligible for committee review.

### 3.3 Integration & performance gates (manual / scheduled)

These are not part of every PR — they run on `main` and on release
branches:

- **Stress test** with extreme inputs (`tests/stress_test.rs`):
  rapid-fire keystrokes, very long words, multi-language interleaving.
- **Memory profiling** with [`dhat`](https://github.com/nnethercote/dhat)
  and/or [`jemalloc`](https://github.com/jemalloc/jemalloc) to confirm
  the steady-state footprint stays within the `< 10MB` target.
- **Latency profiling** with [`criterion`](https://github.com/bheisler/criterion.rs)
  (`benches/benchmarks.rs`); regressions beyond ±5% versus `main` block
  the release.

### 3.4 Branch protection (recommended)

Configure GitHub branch protection on `main` with:

- Require status checks: `fmt`, `clippy`, `build`, `test`, `coverage`,
  `committee-rules`, `all-gates`.
- Require review from Code Owners.
- Require linear history.
- Disallow force pushes and branch deletion.

---

## 4. §9.4 Acceptance Criteria — How Each Is Enforced

| §9.4 criterion                          | Enforcement mechanism                                                                                                | Status                          |
|-----------------------------------------|----------------------------------------------------------------------------------------------------------------------|---------------------------------|
| **100% PRs pass through committee review** | CODEOWNERS file forces `@camilo` review on every path; PR template mandates all four verdicts before merge.            | Enforced by tooling + template   |
| **0 `unwrap()` in production code**     | `committee-rules` CI job greps `src/` for `.unwrap(` / `.expect(` and fails the build on any hit.                     | Enforced by CI                   |
| **Coverage > 90% enforced in CI**        | `coverage` job runs `cargo llvm-cov` and fails if line coverage is below 90.0%.                                      | Enforced by CI                   |
| **0 security warnings from Agent-Security** | Security agent reviews every PR; rejection blocks merge; CI does not duplicate the manual review but `.cargo/audit` (optional) catches known-vulnerable dependencies. | Enforced by committee + optional tooling |

### 4.1 Tracking & reporting

- The PR template's committee checklist is the unit of record for
  every PR.
- A monthly roll-up of all `REJECTED` verdicts and the criteria they
  cited is appended to `docs/governance-audit.md` (to be created in a
  later phase).
- The `coverage` job's JSON output is uploaded as a CI artifact and
  archived for trend analysis.

---

## 5. System Prompt — Code-Review Committee

The exact system prompt that all four agent roles share is reproduced
verbatim from §9.3 of the implementation plan.  It is the contract the
committee operates under; do not paraphrase it when invoking a
review.

```markdown
SYSTEM ROLE: Multi-Agent Code Review & Governance Committee
ENVIRONMENT: Production-Grade Rust Core Engine Integration

You are a highly collaborative, rigorous Multi-Agent Committee
consisting of an Architect, a Senior Developer, a Security Engineer,
and a QA Lead. Your collective objective is to audit incoming code
changes for a hyper-lightweight, zero-latency typo correction engine.

CRITICAL INSTRUCTIONS FOR THE COMMITTEE:
1. ABSOLUTE CONSENSUS: No pull request, code modification, or feature
   branch can be merged without unanimous approval from all four
   specialized agent roles.
2. NO SHORTCUTS: Reject any code that utilizes unhandled 'unwrap()'
   statements, assumes happy paths, lacks comprehensive edge-case
   handling for multi-byte UTF-8, or introduces bloated dependencies.
3. AGENT SKILL UTILIZATION: Leverage specialized tools to actively
   trace execution graphs, check performance overhead profiles, and
   execute structural validation tests.

INDIVIDUAL AGENT MANDATES:
- Architect: Enforce strict modular separation and zero-cost
  abstractions. Ensure FFI layers are memory-safe.
- Developer: Ensure idiomatic Rust (clippy compliant), optimal
  algorithmic complexity (O(1) mappings or O(N) bounded Tries),
  and DRY principles.
- QA Lead: Look for boundary conditions, buffer overflows, rapid
  hardware inputs, multi-language dictionary collision bugs, and
  missing unit/integration tests.
- Security: Audit unsafe blocks, pointer manipulations in hardware
  hooks, and OS level process privileges.

OUTPUT FORMAT:
Each agent must provide a distinct 'VERDICT: [APPROVED / REJECTED]'
accompanied by an explicit, technical analysis of their domain.
If any single agent rejects, the entire review process fails and
requires a rewrite.
```

---

## 6. Adding or Removing Committee Members

- A proposal to add or remove a committee role is itself a PR and must
  pass the existing committee (chicken-and-egg is resolved by the
  human owner `@camilo` casting the deciding vote).
- The new role must (a) own a non-empty subset of paths in
  `CODEOWNERS`, (b) be listed in §1 of this document, and (c) add its
  verdict block to the PR template.

---

## 7. Related Documents

- [`docs/plan-implementacion.md` §9](./plan-implementacion.md) — the
  mandate this governance implements.
- [`.github/CODEOWNERS`](../.github/CODEOWNERS) — code-owner routing.
- [`.github/pull_request_template.md`](../.github/pull_request_template.md)
  — the per-PR checklist.
- [`.github/workflows/ci.yml`](../.github/workflows/ci.yml) — the
  automated gate pipeline.
- [`docs/sec-9-analysis.md`](./sec-9-analysis.md) — analysis of how
  the files created in this section map to the §9.4 criteria.
