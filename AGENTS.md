# Project Instructions for Coding Agents

These instructions apply to the entire repository. This is an experimental demo
whose purpose is to crystallize good concepts quickly, not to preserve old APIs.

## Product priorities

1. **Do not preserve backward compatibility.** Breaking configuration, APIs,
   schemas, object layouts, names, or behavior is expected when it produces a
   cleaner design. Update all callers, examples, documentation, and tests in the
   same change. Delete aliases, migrations, deprecation shims, legacy readers,
   hypothesis-era options, stale names, and superseded implementations instead
   of carrying them forward.
2. Optimize for **speed of evolution, maximum runtime efficiency, and zero
   technical debt**. Prefer deletion and one clear source of truth over adapters,
   compatibility layers, duplicated contracts, or speculative abstractions.
3. Treat the principles in this file as strong defaults, not substitutes for
   engineering judgment. A principle may be violated when concrete evidence and
   common sense show that doing so is better. Keep the exception narrow and state
   the reason and trade-off in the handoff. The prohibition on silent user-visible
   transformations below is not covered by this exception.

## Maximum shift-left validation and explicit contracts

- **Move every check to the earliest boundary where its inputs are known.**
  Prefer compile-time constraints and valid-by-construction types; otherwise
  validate in constructors, factories, `TryFrom`, or a builder's `build` method.
  Do not publish an operational object and ask its consumers to remember a
  separate `validate()` call for its intrinsic invariants.
- An operational object's existence must guarantee that it is internally
  consistent, valid, and non-contradictory. Keep invariant-bearing fields private
  and expose only mutations that preserve the contract. Deserialization, alternate
  constructors, defaults, cloning, and conversions must not bypass that guarantee.
  Prefer types that make invalid combinations unrepresentable over boolean flags
  and repeated defensive checks.
- Explicitly distinguish raw input, UI drafts, wire DTOs, and partially resolved
  configuration from validated execution objects. Incomplete drafts are legitimate;
  they must cross a fallible validation boundary before use as operational state.
  Never turn invalid input into a valid object by silently substituting defaults,
  coercing values, dropping data, or panicking.
- Document each non-trivial contract next to its owning type or constructor:
  invariants, units and ranges, identity/null/absence semantics, validation timing,
  permitted mutations, errors, and what still depends on external state. Shared
  cross-component contracts should also have a discoverable architecture document.
- Apply judgment: do not perform network I/O in a pure constructor or repeat
  expensive checks on every accessor. Validate configuration-only constraints
  before connecting; validate discovery-dependent constraints when discovery
  completes, before destination preparation or worker startup. Validate new records
  when constructing Arrow/runtime objects, before buffering or side effects.
- Constructor validation does **not** replace checks for new untrusted input,
  schema drift, external state changes, or data-dependent destination constraints.
  Revalidate those at their owning runtime boundary before write/commit/acknowledge.
  Centralize each invariant rather than maintaining divergent validation copies.
- Every migrated contract needs regression coverage for rejected construction,
  alternate construction/deserialization paths, and invariant-preserving mutation;
  retain runtime-boundary coverage where the assumptions can change. Update all
  callers together. Record justified exceptions and unreviewed areas explicitly;
  a repository scan alone is not proof of full compliance.

## Data preservation is the highest priority

- **Above all else, do not lose user data.** When safety, convenience,
  performance, availability, or implementation simplicity conflict with data
  preservation, preserve the data and make the failure explicit.
- Every configuration default must be lossless. Defaults must retain source
  records and fields, preserve ordering and commit guarantees where promised,
  and preserve the full supported value, type, precision, scale, timezone, and
  encoding. A destructive behavior such as dropping records, ignoring unknown
  fields, skipping malformed input, or acknowledging unpersisted data must
  require an explicit, deliberate user choice and must never be the default.
- Never silently or quietly truncate, round, saturate, narrow, coerce, replace,
  reinterpret, or otherwise reduce the precision or fidelity of user data. This
  prohibition applies to values, identifiers, schemas, timestamps, numeric
  types, strings, binary payloads, offsets, and metadata at every stage of the
  delivery.
- Never conceal or continue through detected corruption. Fail closed before the
  next irreversible side effect, preserve replayability, and report which value,
  record, partition, or persisted state violated the contract without exposing
  secrets.
- Validate losslessness during configuration parsing or discovery whenever it
  can be known statically, and validate it again at runtime before buffering,
  writing, uploading, committing, or acknowledging data. Cover both validation
  boundaries with regression tests for every new conversion or destination
  constraint.

## Absolute user isolation

- Users are strict security and data-isolation boundaries. A user's subscriptions,
  workspaces, custom names, notes, extraction recipes, rules, article state,
  activity history, errors, exports, and all other user-visible or user-derived
  data must never be visible to, returned to, mutated by, or inferred by another
  user. Enforce ownership in repository queries and mutation boundaries; never
  rely on frontend filtering or caller-supplied ownership identifiers.
- Cross-user reuse is permitted only below the user-data boundary for fetching
  the same public source. The system may coalesce or cache an outbound fetch so
  that identical RSS or public-page URLs are requested once instead of once per
  user. Shared fetch artifacts must contain no account credentials, private
  headers, cookies, user configuration, notes, rules, read state, or other
  tenant-specific data. Fan-out from a shared fetch must recreate and persist
  each user's results independently under verified ownership.
- Equality of source URLs never merges user-owned subscriptions or their state.
  Unsubscribing, pausing, renaming, annotating, configuring, reading, saving,
  deleting, or applying rules in one account must have no effect on another
  account, even when both accounts consume the same fetched bytes.
- Every feature that reads or writes user-owned data needs cross-user isolation
  tests covering direct object lookup, list queries, mutations, exports, shared
  source URLs, and guessed or reused identifiers. Treat any cross-user data
  exposure or mutation as a release-blocking defect.

## User-visible semantics: never guess or silently transform

- **Never make a product or UX decision on the user's behalf by silently changing
  their identifiers or data.** This includes renaming tables or columns, hashing,
  truncating or escaping names or values, normalizing values, coercing types,
  changing precision or timezone, rewriting paths, substituting defaults for
  invalid input, dropping fields or rows, and any similar transformation visible
  at the source or destination.
- Do not invent an automatic fallback for a destination limitation. For example,
  if S3 imposes a key-length limit and a source value can exceed it, reject the
  configuration during discovery when possible and otherwise reject the offending
  runtime value. **Do not silently replace the value with a hash, shortened form,
  encoded alias, or generated name.**
- Prefer fail-fast behavior for every unsupported edge case. Validate static
  constraints while parsing the configuration or during delivery discovery,
  before connecting workers or creating destination state. Revalidate
  data-dependent constraints at runtime before buffering, INSERT, upload, commit,
  or another side effect.
- A transformation is allowed only when both conditions hold:
  1. the user explicitly requests it through a deliberate, documented
     configuration choice; and
  2. the exact transformation is explicitly implemented, named, validated, and
     covered by startup and runtime tests.
- An explicit transformation must have deterministic, documented semantics and
  must be observable in configuration and diagnostics. It must never be enabled
  implicitly for compatibility, convenience, robustness, or performance.
- When requirements call for a new transformation but the user has not selected
  its semantics, stop and ask rather than choosing a policy in code.

## Primary-key identity semantics

- When a dataset declares a primary key, the default record model has exactly
  one current logical row for each distinct complete primary-key value. Updates
  and deletes address that row identity; they must not create an implicit
  append-only history.
- Duplicate snapshot rows for one primary key, ambiguous change events, and
  other violations of that identity model must fail closed before destination
  commit or source acknowledgement. Never silently deduplicate them or choose a
  winner through last-write-wins behavior.
- Semantics that intentionally retain multiple rows, versions, or history
  entries for one primary key are allowed only through an explicit, documented,
  non-default configuration choice with startup and runtime validation.

## Frontend interaction stability: zero unexpected layout shift

- Before changing frontend appearance, read [UI style guide](docs/ui-style-guide.md).
  The approved light-theme direction is **A: cool slate + teal**. Reuse semantic
  theme tokens; do not invent component-local neutral palettes. Update the guide
  alongside any deliberately approved palette change. Preserve dark-theme and
  semantic status distinctions.

- In every selectable `oneOf`/`anyOf`, place the authored default branch first so
  it appears immediately after `Not selected`. When extensions contribute a
  preferred variant, schema composition must move that variant to the first
  branch and contract tests must verify the emitted catalog order.

- **Every user interaction must receive immediate visible feedback.** The
  pressed state must appear on pointer-down/keyboard activation, and the
  resulting state transition must be rendered synchronously or on the next
  animation frame. A click must never appear to have been ignored.
- If an operation waits for network, storage, discovery, validation, worker
  startup, or any other asynchronous work, render a pending state immediately,
  before awaiting it. Use a spinner, skeleton, progress state, or explicit
  status text appropriate to the control. Preserve the control's dimensions and
  surrounding layout across idle, pressed, pending, success, and error states.
- **Notification flicker, toast flicker, and flashes of transient state are
  forbidden.** Immediate feedback belongs on the initiating control or in a
  permanently reserved status region. Do not briefly mount a toast, banner, or
  verbose loading message for an operation that commonly completes before a
  person can read it. Delay such secondary indicators by a short, documented
  threshold; if the operation finishes first, never show them. Once a delayed
  indicator becomes visible, keep it visible for a short minimum readable
  duration unless an error supersedes it. The delay must never postpone the
  control's immediate pressed/pending feedback, and neither appearance nor
  disappearance may change layout.
- Prevent accidental duplicate activation while an operation is pending. Use a
  disabled/busy state, request deduplication, or an explicitly safe idempotent
  interaction model. Keep enough visible feedback to make it obvious that the
  first activation was accepted; disabling a control without explaining the
  pending state is not sufficient.
- Success and failure must also produce immediate, visible, accessible feedback.
  Associate it with the initiating control or stable status region, expose busy
  state through appropriate ARIA semantics, and restore focus deliberately.
- **“Delayed interaction feedback + layout shift causes an accidental
  second-click activation” is a forbidden failure mode.** Never allow delayed
  content, a popup, a notification, or a newly enabled action to appear beneath
  the pointer location where a user may repeat a seemingly ignored click. Delay
  the new hit target until pointer release/movement when necessary, or place it
  outside that interaction coordinate while preserving layout.
- **Unexpected layout shift is forbidden at any cost.** A situation where a
  notification or asynchronous update moves the interface and can cause an
  accidental click is absolutely unacceptable. Treat stable target coordinates
  during interaction as a correctness and safety property, not visual polish.
- Never insert an unexpected toast, snackbar, notification, validation message,
  or asynchronous status into normal document flow. Render transient feedback
  in a fixed overlay that does not move existing content.
- When a banner or message must participate in normal flow, reserve a stable,
  fixed-size region for it before the content appears. Showing, hiding, loading,
  success, and error states must occupy the same layout footprint.
- Do not change layout while the user has an active pointer press, focused
  control, drag, scroll, tap, or equivalent interaction. Defer non-critical UI
  updates until the interaction ends. If a change is unavoidable, preserve the
  position and hit target of the control being used.
- Declare dimensions for dynamic content in advance. Use explicit `width` and
  `height`, `aspect-ratio`, stable containers, and correctly sized placeholders
  or skeletons for images, previews, asynchronous blocks, and other late content.
- Never place destructive or irreversible actions where a moving layout can
  bring them under an existing pointer or touch target. `Delete`, `Buy`, `Send`,
  `Confirm`, and equivalent actions require an additional safety barrier such as
  confirmation, undo, or delayed enablement. This barrier supplements layout
  stability and must never be used as a substitute for it.
- Every frontend change that introduces conditional, asynchronous, lazy-loaded,
  expanded, collapsed, validated, or notification content must include a
  regression test proving that surrounding interactive controls do not move
  unexpectedly. Every new asynchronous interaction must additionally test its
  immediate pressed/pending feedback and duplicate-activation protection.

### Browser autofill must never interfere with editor fields

- Every new native `input`, `textarea`, or `select` must use a shared field
  primitive. Non-authentication fields must use the autofill-resistant
  primitive from `web/src/ui/AutofillResistantField.tsx`. The only exception
  is the application's own authentication fields described below. Raw editable
  HTML controls and `contenteditable` are prohibited outside the shared
  primitives in `web/src`; the frontend architecture check enforces this rule.
- The autofill-resistant primitive deliberately uses the browser-specific
  `autocomplete="none"` mitigation, an opaque per-mount `name`, and the common
  password-manager ignore attributes. Do not replace `none` with standard
  `off`, `new-password`, or a semantic autocomplete token: some browsers may
  ignore those values or route the field into password/autofill heuristics.
- Never expose configuration paths, labels, secret names, connector names, or
  other semantic identifiers through a native field's `name`. Radio controls
  that must share a native group name must obtain it from
  `useOpaqueFieldName()` and pass it as `opaqueGroupName`.
- Protection attributes must be stamped by the shared primitive after caller
  props so a feature cannot accidentally override them. Any intentional change
  to this contract must update its component tests and the architecture guard
  in the same change.
- **User-approved authentication exception:** support browser/password-manager
  autofill for the application's own username and password fields in sign-in,
  invitation registration, password change, and password reset forms. These
  fields must use a separate shared `AuthField` primitive with a closed set of
  roles: username, current password, and new password (including confirmation).
  It supplies the corresponding `autocomplete="username"`,
  `autocomplete="current-password"`, or `autocomplete="new-password"` value,
  stable purpose-specific names, and omits password-manager ignore attributes.
  Callers cannot override the role's native field type or protection attributes.
  Do not introduce a generic autofill opt-out on editor fields or extend this
  exception to source settings, external credentials, selectors, search, or
  other application editors. The architecture guard must restrict `AuthField`
  consumers to the application's authentication forms.
- When implementing these primitives, add component tests and the architecture
  guard together: verify auth roles and caller-override resistance, and retain
  regression coverage for opaque names and autofill suppression in every
  non-authentication field. This documentation-only exception does not claim
  that the primitives or checks already exist in this repository.
- For protected editor fields, this is the strongest application-side
  mitigation, not an absolute browser
  security boundary. Browser vendors document that a site cannot guarantee suppression
  in every environment; users who require an absolute local prohibition must
  disable form-autofill suggestions in browser settings.

## Outbound HTTP security

- Every production outbound HTTP request must use the repository's shared HTTP
  client wrapper. Do not construct or execute a connector-local `reqwest::Client`,
  `hyper` client, or another general-purpose HTTP client directly.
- Feed, web-page, and article fetches may follow HTTP(S) redirects through the
  shared outbound HTTP boundary. Every hop, including same-host redirects, must
  repeat URL, scheme, address, and DNS/IP validation before a connection is made.
  Reject local, private, link-local, and infrastructure metadata destinations.
  Enforce an explicit, validated configuration limit on redirect hops and the
  overall request deadline; reject loops and disallowed targets with typed errors.
  Never forward credentials, sensitive headers, or request bodies to a different
  origin. These protections also apply to browser-backed fetching.
- Keep URL allow-listing, scheme and address validation, DNS/IP protections,
  timeouts, TLS policy, and credential redaction centralized in the same outbound
  HTTP boundary. Connector code may add a narrower policy but must never weaken
  the shared one.
- Every new HTTP integration and every redirect-related bug fix needs regression
  coverage for allowed redirects, per-hop address validation, blocked destinations,
  redirect loops and configured hop limits, and prevention of cross-origin
  credential, sensitive-header, and request-body forwarding.

## External request observability

- Every production request to an external system must record its completion and
  elapsed time through the shared external-request instrumentation. This includes
  databases, queues, object storage, catalogs, control APIs, and transport-level
  discovery calls. Do not add connector-local timing formats.
- Direct HTTP calls inherit timing from `OutboundHttpRequest`. An SDK that only
  accepts an opaque transport must wrap each semantic request with
  `observe_external_request`; long-lived streaming requests must additionally
  expose their existing source/sink counters so transfer throughput remains
  observable.
- Use stable, credential-free system and operation names. Never log request or
  response bodies, authorization material, URL userinfo/query strings, or an SDK
  error value that may reproduce sensitive request data. Add a regression test
  whenever a new adapter or logging boundary is introduced.

## Performance and design

- Use non-cryptographic hashes for non-cryptographic work such as hash-table
  keys, partition assignment, sampling, and fingerprints that are always
  verified against their source value. Prefer MurmurHash3 x64 128-bit
  (Murmur3 128) unless measurements justify another non-cryptographic
  algorithm. Do not spend SHA-family or other cryptographic hashing cost where
  collision resistance is not required by an explicit, documented adversarial
  or cross-trust-boundary threat model. Long-lived or persisted identity alone
  is not such a requirement. Reproducibility manifests, benchmark provenance,
  build/configuration fingerprints, cache keys, and trusted-environment file
  identity are non-cryptographic tasks even when their fingerprints are stored
  permanently; use Murmur3 128 and retain the exact metadata needed to verify
  the source value. Do not infer a cryptographic requirement merely from words
  such as integrity, identity, provenance, durable, persistent, or verified.
  A hash must never silently replace user data or become the sole proof of
  equality where a collision could lose or corrupt data.
- Every operational or safety limit must come from an explicit user-visible
  configuration value and be validated before execution. A hardcoded constant
  must never reject, truncate, or otherwise break a delivery that satisfies its
  configured limits. Constants may define implementation capacities only when
  exceeding them is structurally impossible or the corresponding constraint is
  explicitly exposed and validated in configuration.
- Strive for the most efficient practical implementation: bounded memory,
  explicit backpressure, minimal copies and allocations, useful concurrency,
  deterministic behavior, and no blocking work on async executor threads.
- Do not trade throughput or memory efficiency for convenience. A measured
  regression of at most **5%** is acceptable only when it buys a materially
  better interface or substantially more readable and maintainable code. Measure
  representative hot paths before accepting such a regression; do not guess.
- Correctness, durability, and liveness remain mandatory. If one of them requires
  a larger performance trade-off, document the evidence and choose the safe
  design rather than hiding the trade-off.
- Prefer native Rust libraries and in-process implementations. Avoid `libffi`,
  language bindings, helper sidecars, and equivalent cross-runtime machinery.
  Use one only when a native Rust solution is demonstrably impractical and the
  operational and performance cost is explicitly justified.
- Keep interfaces sink-neutral and parser-neutral. Put destination constraints
  in the sink contract and validate them during discovery and again before side
  effects. Do not leak ClickHouse or S3 details into generic pipeline code.

## Testing rules

- **Never mix test bodies and production code in one file.** A production module
  may contain only a `#[cfg(test)] mod tests;` declaration. Put its tests in a
  sibling `tests.rs` or `tests/` subtree. Put cross-component tests in the root
  `tests/` directory. Test helpers shared by integration tests belong in a
  dedicated test-support module, not in production modules.
- When several production modules in one component have separate test files,
  collect them under that component's `tests/` directory (for example,
  `json_parser/tests/parser.rs`). Never create a directory named after a
  production file solely to hold its `tests.rs`.
- Every sink must have an automated end-to-end test that exercises its real
  wire/storage implementation. Use `testcontainers` or an equivalent hermetic,
  pinned service fixture. Fake transports are useful unit-test seams but do not
  satisfy this requirement.
- For external sinks, the E2E test must start the real compatible service (for
  example ClickHouse or an S3-compatible service), create required state, write
  through the production sink, verify destination data, and clean up. It must
  cover at least the durability/commit barrier and one representative failure or
  replay scenario. An in-process sink still needs a full pipeline integration
  test even when no container is meaningful.
- E2E tests must be part of the normal automated test command. Do not mark them
  ignored, silently skip them when Docker is absent, or claim completion without
  running them. If the required runtime is unavailable, report the task as not
  fully verified.
- Every bug fix needs a regression test that fails for the old behavior. Every
  destination-contract change needs startup-validation and runtime-validation
  coverage.
- Every constraint validated in the frontend must also be validated by the
  backend. Frontend validation is immediate UX feedback, never a trust or
  correctness boundary.

## Verification gates

Use `just check-affected` for quick iteration. It runs package-scoped
`cargo check` for affected Rust packages and their necessary dependents, plus
`tsc --noEmit` for frontend changes. Inspect its plan with
`just test-affected-dry` when the affected scope is unclear.

Tests, Clippy, formatting checks, browser E2E, Docker integration checks and
full builds may be run whenever they provide useful verification. Run
`just check-release` before claiming the implementation is release-ready:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets --all-features
```

The release test command must include the hermetic YDB/Chromium and
backup/restore Docker acceptance tests. These tests must fail clearly when
Docker or a required image is unavailable; they must never silently skip.
Do not claim that the release gate passed when only the compile-only development
gate ran.

## Fast development and Cargo cache hygiene

Development latency is a product requirement. Preserve the following setup and
workflow instead of compensating for slow builds with broader parallelism or
repeated full-workspace commands.

### Use the smallest stable compilation surface

- Start with `just test-affected-dry` when the affected scope is not obvious,
  then run `just check-affected` once on the final tree. Do not repeatedly run
  it after every small edit.
- Prefer `cargo check` to `cargo build`: checking avoids code generation and
  linking. Build a binary only when the user needs to execute that binary.
- Keep one canonical development profile and feature set. Alternating between
  check/build/test/clippy, different feature combinations, `RUSTFLAGS`, target
  triples, or profiles creates distinct Cargo fingerprints and duplicates most
  artifacts.
- Package-scoped checks must include only the changed crate and the necessary
  reverse-dependency closure. A `Cargo.lock` change alone is not a reason to
  compile every workspace target. Treat actual compiler configuration,
  toolchain, protocol, or vendor changes as cross-cutting.
- Rust and frontend compile checks may run concurrently when independent. Do
  not run multiple Cargo processes against the same target directory: they
  serialize on Cargo's build lock and make elapsed time harder to diagnose.
- Record command durations. The affected selector writes its current report to
  `target/affected-tests-timings.json` and history to the adjacent JSONL file;
  use this evidence before broadening or optimizing a gate.

### Preserve crate-local ownership

- Connector-specific integration and E2E targets belong under that connector
  crate's `tests/` directory. The root `tests/` directory is only for genuinely
  cross-crate behavior. Otherwise checking one connector reconstructs a large
  monolithic root test target.
- Builds and checks should consume generated artifacts without rewriting them.
  Regeneration is an explicit task performed only when the owning contract
  changes.

### Keep caches bounded and reusable

- The repository pins Rust in `rust-toolchain.toml`. Do not casually change the
  toolchain, dev/test profiles, global rustflags, or workspace-wide features:
  each change invalidates a large portion of both Cargo and compiler caches.
- The project uses `.cargo/rustc-wrapper.sh` and `sccache`. Keep the sccache
  budget bounded (currently 50 GiB). A normal developer shell should use
  sccache; the wrapper may bypass it only in an environment whose sandbox blocks
  sccache IPC.
- `target/` is not a bounded cache. Do not use `cargo clean` routinely because
  a healthy target directory is valuable. Clean it only after evidence of
  pathological fragmentation, incompatible build variants, or severe disk
  pressure. Never delete `~/.cargo` as a generic build-speed fix; registry and
  source caches are reusable.
- Keep at least roughly 15–20% filesystem capacity free. A nearly full APFS or
  equivalent filesystem plus a huge flat `target/*/deps` directory can turn
  metadata operations into minutes of I/O wait even when rustc consumes almost
  no CPU.
- Development profiles intentionally disable incremental compilation and split
  debug info so sccache can reuse outputs and Cargo does not create enormous
  populations of tiny files. Change this only with measured evidence from this
  workspace.

### Diagnose a slow or apparently hung build before retrying

- Do not start a second Cargo command. Inspect the existing Cargo process and
  its rustc child first. Cargo often appears idle because it is waiting for one
  compiler, linker, archiver, filesystem operation, or the build-directory
  lock.
- Cargo timing output identifies the slow crate but not necessarily the blocked
  syscall. On macOS, trace the actual rustc child PID, not the parent Cargo PID,
  with `sample <rustc-pid>` and `sudo fs_usage -w -f filesystem <rustc-pid>`;
  correlate it with `iostat` and `vm_stat`. A trace of only the Cargo parent
  cannot prove where rustc waited.
- Check `df -h`, `du -sh target`, the population of `target/*/deps`, duplicate
  `.fingerprint` variants, and `sccache --show-stats`. Near-zero rustc CPU plus
  a very large, slow-to-enumerate target directory is evidence of storage and
  metadata pressure, not insufficient Cargo job parallelism.
- Cargo already schedules work across available CPUs. Raising `jobs` far above
  the CPU count does not shorten a dependency-chain critical path and can worsen
  memory and I/O contention. Optimize dependency boundaries and cache reuse
  before changing parallelism.

## Change hygiene

- In every non-empty Rust configuration struct and struct-like configuration
  enum variant, separate adjacent fields with exactly one blank line. A field's
  doc comments and attributes belong to that field: keep them together without
  blank lines, and put the separator before the comments or attributes of the
  following field. Apply this convention to shared, nested, connector, parser,
  sink, source, and internal configuration types alike, even when a helper type
  does not have a `Config` suffix.
- Search for and remove obsolete names, options, comments, tests, and docs after
  each conceptual change.
- Keep the working tree's unrelated user changes intact.
- Make failures explicit and typed; deterministic configuration/schema errors
  fail fast, while genuinely transient external failures use bounded, observable
  retry behavior unless the process-level policy explicitly says otherwise.
- Keep credentials out of logs and require an explicit trust decision for
  plaintext transports.
- Handoffs must state what changed, what was deleted, performance implications,
  exact verification commands, and any principle exception or unverified risk.
