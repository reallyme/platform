# Formal verification

Run the same checks as the [formal methods workflow](../../.github/workflows/formal-methods.yml)
with Kani 0.68.0, a Java 21 JDK, and the TLA+ 1.7.4 tools JAR.

```sh
cargo +stable install --locked kani-verifier --version 0.68.0
cargo +stable kani setup
TLA2TOOLS_JAR=/path/to/tla2tools.jar bash scripts/conformance/verify-formal-methods.sh
```

The [Kani installer](https://model-checking.github.io/kani/install-guide.html)
uses its own Rust toolchain. Download the JAR from the
[TLA+ 1.7.4 release](https://github.com/tlaplus/tlaplus/releases/tag/v1.7.4)
and verify its SHA-256 against the value in the workflow. The runner builds a
temporary copy of the harnesses and production modules so the verifier can use its
own compiler without building the server kit or writing into the source tree.

## Listener route policy

[The Kani harness](policy_path.rs) checks the production escape decoder used
by the [path canonicalizer](../../kits/server/src/http/layers/route_visibility/policy_path.rs).
It checks all 65,536 pairs of escape bytes against the decoder's acceptance
policy, including mixed-case hex digits, malformed escapes, and decoded bytes
that would introduce another escape, a separator, or NUL.
[Middleware tests](../../kits/server/src/http/layers/route_visibility/path_canonicalization_tests.rs)
cover whole paths, including malformed escapes and multibyte UTF-8.

The [route admission harness](route_admission.rs) checks the production
[admission decision](../../kits/server/src/http/visibility/admission.rs).
An explicit rule can admit a request only when both path spellings select
the same rule and both matches permit the listener. The default can admit
only two unmatched paths. The proof quantifies over the classifications,
the default, and rule equality. Rule matching and listener restrictions
are exercised through the [middleware tests](../../kits/server/src/http/layers/route_visibility/encoded_carve_out_tests.rs).

The [route selection harness](route_selection.rs) checks the production
[ranking function](../../kits/server/src/http/visibility/selection.rs) with
zero, one, or two matching candidates. For two candidates it quantifies over
all representable lengths and both match kinds. The longer match wins and
exact wins at equal length. The [policy tests](../../kits/server/src/http/visibility/tests.rs)
exercise segment boundaries and verify that a denied winner blocks admission
even when a less specific rule and the default would both allow the listener.

## FoundationDB tenant deletion

[The TLA+ model](TenantDeletion.tla) follows the
[tenant admin operations](../../kits/foundationdb/src/fdb/tenant/admin.rs).
The model starts with one tenant incarnation and explores both empty and
nonempty tenants, partial metadata, a concurrent application write, an
uncertain commit, and interruption or failure before recovery. It checks that
deletion preserves application data and foreign metadata, a successful delete
requires tenant absence, and explicit recovery initializes both kit metadata
keys only when the reserved namespace is empty.

Each modeled commit is atomic. The model assumes that the non-snapshot range
read conflicts with writes before the metadata clear, and that FoundationDB
checks tenant emptiness at deletion. Each uncertain commit resolves before
the model advances to lookup or restoration. The model represents the
automatic restoration attempts by their final outcome. It checks safety for
a single deletion and operator recovery with a matching recorded ID. The
model abstracts metadata values and retry progress.
Use the FoundationDB integration suite to validate the production adapter
against the database.

## Tenant-map delete retries

[TenantMapDeletion.tla](TenantMapDeletion.tla) starts after the original
tenant's metadata has been cleared. It separates the transactional ID read,
staged clear, commit submission, database outcome, and caller observation.
It checks the adapter's three-attempt budget with a concurrent application
write, an independent deletion of the original, and one reuse of its name.
`commit_unknown_result` can lead to a retry after either a commit or an
abort. A deadline can end the retry loop while a submitted clear can still
commit or abort. The model assumes unique IDs, a non-snapshot read that
conflicts with changes to the tenant-map entry, and an emptiness check at
commit. Reconciliation models a successful lookup and leaves metadata
restoration to the separate restoration model.
[FoundationDB special-key transactions](https://apple.github.io/foundationdb/special-keys.html#read-write-modules),
[FoundationDB commit errors](https://apple.github.io/foundationdb/developer-guide.html#non-retryable-errors)

| Configuration | TLC checks |
| --- | --- |
| [Check the ID on every attempt](TenantMapDeletion.cfg) | A replacement survives, application data survives, and success requires the original mapping to be absent. |
| [Skip the ID comparison on retry](TenantMapDeletionSkipRetryId.cfg) | The original delete commits, the caller receives an ambiguous result, and a retry deletes the empty replacement. The runner requires `ReplacementSurvives` to fail. The competing delete is disabled so that the counterexample must go through the original commit. |

The [adapter](../../kits/foundationdb/src/fdb/tenant/admin.rs) reads and clears
the entry in the same transaction on every attempt. The [live regression](../../kits/foundationdb/src/fdb/tenant/admin_tests.rs)
checks the conflict after name reuse and exercises the retry runner with an
injected `commit_unknown_result` against an empty replacement.

## Automatic metadata restoration after a delete failure

[TenantRestoration.tla](TenantRestoration.tla) starts with a submitted
metadata clear and a saved copy of its values. It separates each database
commit from the caller's observation. A clear, tenant-map delete, or restore
attempt can settle after a timeout. The model explores three restore
attempts, including attempts that settle after a retry or after the caller
returns, as well as a concurrent application write, deletion by another
operator, and one reuse of the original tenant name.

| Configuration | TLC checks |
| --- | --- |
| [ID-bound restore](TenantRestoration.cfg) | Only the original tenant receives the saved metadata. An acknowledged restore preserves the snapshot while the original exists. Delete success requires the original to be absent. |
| [Restore by name](TenantRestorationByName.cfg) | The runner requires a counterexample in which the second phase commits after a timeout, the name is reused, and automatic restoration writes into the replacement. The replacement can be empty or hold its own metadata. |
| [Skip the write if values already match](TenantRestorationSkipWrite.cfg) | The runner requires a counterexample in which an older clear commits after the restore observes matching values. A real restore write adds the conflict that prevents the clear from committing afterward. |

The adapter saves the ID of the handle that reads and clears the metadata.
Every [automatic restore](../../kits/foundationdb/src/fdb/tenant/admin.rs)
attempt checks the newly opened handle against that ID before writing the
snapshot unconditionally. The [live regressions](../../kits/foundationdb/src/fdb/tenant/admin_tests.rs)
exercise name reuse after a delete commits and the conflict between a restore
write and an in-flight clear.

The model distinguishes a reported timeout from a server-side abort. It
assumes a non-snapshot clear conflicts with subsequent writes to its read
range, and a delete commits only against an empty original tenant. It models
an individual tenant-map delete. The [tenant-map retry model](TenantMapDeletion.tla)
covers repeated attempts separately. Metadata bytes, arbitrary third-party
metadata edits, and recovery after retry exhaustion are outside the
restoration model.
[FoundationDB transaction timeouts and conflict rules](https://apple.github.io/foundationdb/developer-guide.html#non-retryable-errors)

## Delete retries and restoration together

[TenantDeleteRecovery.tla](TenantDeleteRecovery.tla) starts after the
metadata clear is submitted, with a nonempty snapshot of the original
tenant's metadata. It reuses the tenant-map retry model and adds the adapter's
reconciliation lookup and restoration. After a delete error, a successful
lookup that finds the original absent completes the delete. An unavailable
lookup or one that finds the original leads to restoration. Each restore
attempt verifies a handle again. Submitted clears, deletes, and restores
can settle after the caller stops waiting, including after the final error.
The model checks three delete attempts and three restore attempts. It
represents saved metadata with one marker and does not model external
metadata edits, tenant renames, or process restarts.

| Configuration | TLC checks |
| --- | --- |
| [Combined safety checks](TenantDeleteRecovery.cfg) | A replacement survives and never receives the snapshot. Application data survives, and delete success requires the original mapping to be absent. A committed restore preserves the original and its saved metadata by conflicting with a pending clear and making the tenant nonempty for a pending delete. |
| [Retry exhaustion](TenantDeleteRecoveryExhaustion.cfg) | The delete budget is exhausted, the lookup is unavailable, and all restores fail. The original survives without its metadata. The runner requires `ExhaustionPreservesMetadata` to fail. |
| [Late delete](TenantDeleteRecoveryLateDelete.cfg) | The delete deadline expires with a commit in flight. The lookup is unavailable and the caller returns after exhausting restore attempts. The pending delete then commits against the empty original. The runner requires `NoDeleteCommitAfterExhaustion` to fail. |

## FoundationDB recovery after name reuse

[TenantRecovery.tla](TenantRecovery.tla) starts after an interrupted delete
has cleared the kit metadata. It models one recovery attempt, one tenant
name, and two tenant IDs. The name can disappear and then resolve to the
replacement ID. The model separates the management lookup, opening by name,
and resolving the keyspace for the recovery transaction. It also covers
concurrent population of the reserved metadata namespace and a commit that
may complete after the caller times out.

[The lookup-only configuration](TenantRecoveryLookupOnly.cfg) represents a
management-lookup-only check for recovery.
The lookup can accept the recorded ID and the later name-based open can
resolve to the replacement. If its metadata namespace is empty, recovery
can initialize the replacement. The runner requires TLC to report precisely
that invariant violation.

[The bound-ID configuration](TenantRecovery.cfg) models the ID check in
[recover_interrupted_delete](../../kits/foundationdb/src/fdb/tenant/admin.rs).
The adapter checks the resolved ID of the opened handle and creates its
transaction from that handle without a separate management lookup.
It checks that committed writes target only the recorded tenant, success
requires a commit, and recovery writes both kit keys only when the namespace
is empty. The model assumes that a bound transaction cannot be retargeted,
that deletion invalidates writes to its old ID, and that the non-snapshot
metadata read conflicts with concurrent population. The model omits
cross-cluster identity, retries, and repeated name reuse. The
[live regression](../../kits/foundationdb/src/fdb/tenant/admin_tests.rs)
checks that a verified handle stays bound to the deleted tenant when the
name is reused and that the replacement remains untouched.

FoundationDB assigns IDs and keyspaces at tenant creation. In the 7.3.77
client, a name-based tenant object has its own asynchronous ID lookup
and supplies that tenant object to transactions.
The operator connector uses the native client directly because the
multiversion client can reopen a tenant by name on a client switch.
[Tenant documentation](https://apple.github.io/foundationdb/tenants.html),
[native client](https://github.com/apple/foundationdb/blob/7.3.77/fdbclient/NativeAPI.actor.cpp#L2735-L2746),
[thread-safe client](https://github.com/apple/foundationdb/blob/7.3.77/fdbclient/ThreadSafeTransaction.cpp#L227-L243),
[multiversion client](https://github.com/apple/foundationdb/blob/7.3.77/fdbclient/MultiVersionTransaction.actor.cpp#L1853-L1881)

## Durable deletion record

[TenantDeletionRecord.tla](TenantDeletionRecord.tla) models the operator
protocol. It persists an intent with the original tenant ID before
submitting the metadata clear. A crash discards the worker's progress while
the record and submitted commits survive. A restarted worker reconciles a
pending record by ID. A missing mapping or a different ID completes the
record as `original-absent`. An unavailable lookup leaves it pending.

If the original ID still matches, the worker verifies the handle used for the
repair transaction. The transaction initializes an empty reserved metadata
namespace and preserves compatible metadata. When metadata is already
compatible, it commits a write that conflicts with any older pending clear.
The worker records `metadata-safe` only after it acknowledges that commit.
A pending tenant-map delete still requires emptiness, so repaired metadata
also protects the original from that delete. Incompatible metadata leaves the
record pending for operator review. An unacknowledged repair can commit after
a crash and be confirmed by a later worker.

| Configuration | TLC checks |
| --- | --- |
| [Durable record](TenantDeletionRecord.cfg) | Destructive work requires a durable intent. Completed records stay stable and accurate. Repairs stay on the recorded ID and preserve existing metadata, application data, and any replacement. |
| [Trust the lookup alone](TenantDeletionRecordLookupOnly.cfg) | The original is present during lookup, then its name is reused before repair opens the handle. The runner requires a replacement write to violate `RepairTargetsRecordedId`. |
| [Accept compatible metadata without a conflicting write](TenantDeletionRecordSkipFence.cfg) | A restarted worker records `metadata-safe` and an older clear commits afterward. The runner requires `SafeMetadataRecordIsAccurate` to fail. |

The model checks safety for one cluster, one record, one name reuse, and up
to two repair submissions across repeated restarts. It assumes atomic durable
record updates, non-reused tenant IDs, transactional metadata conflict checks,
and no new destructive operation after reconciliation begins. The delete
commit abstracts the tenant-map retry loop checked in [TenantMapDeletion.tla](TenantMapDeletion.tla).
The [operator CLI](../../kits/foundationdb/src/bin/fdb-tenant-admin.rs) stores
the cluster UID alongside the tenant name and ID, and locks the record path
across destructive work and reconciliation. The [live regression](../../kits/foundationdb/src/bin/fdb-tenant-admin/live_tests.rs)
kills workers before and after the clear and after the delete, then runs
`resume-delete` in a new process. It also checks that an older clear
conflicts with the resumed write and that an empty replacement stays empty.

## Cluster-side tenant reservation

[TenantReservation.tla](TenantReservation.tla) models two operators with
distinct local records for the same tenant name in one cluster. Both can
request a free reservation. A conditional commit grants it to one operator
and durably records the tenant ID. Grant acknowledgement and process state
are separate from the durable reservation.

After expiry, an operator can claim a higher epoch and reconcile that same
intent using the cluster's recorded ID. Recovery can repair the original or
record it absent after name reuse. Every destructive transaction depends on
the reservation epoch and phase it read. A takeover or completion conflicts
with a pending mutation from the old epoch. The model treats expiry as
eligibility for takeover and revokes the old grant when takeover commits.

| Configuration | TLC checks |
| --- | --- |
| [Transactional reservation](TenantReservation.cfg) | At most one current grant, stable completion, no stale commits, and no writes to a replacement from the reserved operation. A recovery worker that keeps its grant and remains available eventually completes under weak fairness. |
| [Preflight check only](TenantReservationPreflightOnly.cfg) | An old metadata clear commits after takeover. The runner requires `StaleTransactionsCannotCommit` to fail. |
| [Cross-host crash recovery](TenantReservationCrashRecovery.cfg) | Two claims overlap. The winner crashes with its tenant-map delete in flight. The other operator takes over and repairs the original. The runner requires this path to violate `NoCrossHostCrashRecovery`. |

The model covers one reservation lifecycle, up to three epochs, and one
reuse of the tenant name. It assumes transactional checks on the reservation
and tenant ID, the tenant-map emptiness check, and compatible or empty kit
metadata. Recovery is modeled as one transaction that verifies the grant and
records the outcome with any metadata write. The model abstracts lease timing
and repeated delete attempts within an epoch.

The [live transaction test](../../kits/foundationdb/src/fdb/tenant/reservation_tests.rs)
checks the transaction layout with native FoundationDB clients in
separate processes. A cluster transaction uses `RAW_ACCESS` to read an epoch
in a dedicated control tenant and clear metadata inside the verified target
tenant prefix. The second phase reads the reservation key in the transaction
that checks and clears the tenant-map entry. In each phase the peer commits
an epoch change before the pending transaction attempts to commit. The test
requires `not_committed` and checks that FoundationDB reports the reservation
key in the conflicting range. It also commits both operations at the current
epoch to verify that each transaction layout can succeed. [FoundationDB conflict ranges](https://apple.github.io/foundationdb/developer-guide.html#conflict-ranges),
[FoundationDB tenant transaction scope](https://apple.github.io/foundationdb/tenants.html#raw-access),
[FoundationDB 7.3.77 tenant prefix](https://github.com/apple/foundationdb/blob/7.3.77/fdbclient/NativeAPI.actor.cpp#L2747-L2753)

The [reservation adapter](../../kits/foundationdb/src/fdb/tenant/reservations.rs)
implements acquisition and takeover for the operator CLI. It keeps the
per-name active reservation and completed operation records in the control
tenant. Every guarded attempt verifies the control ID and the current epoch.
An expired reservation can be transferred using FoundationDB read versions.
Recovery commits the outcome with any metadata write. The tenant-map delete
observes the reservation in its transaction and finalizes in a later one
because required tenant mode rejects combining a tenant-map mutation with
ordinary data writes. The [live recovery test](../../kits/foundationdb/src/fdb/tenant/reservation_recovery_tests.rs)
checks that a stale delete retry stops, a superseded recovery leaves both
the metadata and outcome unchanged, and the current recovery commits both.
The [CLI test](../../kits/foundationdb/src/bin/fdb-tenant-admin/live_tests.rs)
exercises independent record paths and takeover without a local record.
[FoundationDB 7.3.77 transaction validation](https://github.com/apple/foundationdb/blob/7.3.77/fdbserver/CommitProxyServer.actor.cpp#L1301-L1311)

## Pruning completed reservations

[TenantReservationPruning.tla](TenantReservationPruning.tla) follows two
operation IDs across two lifecycles of a tenant name. The model replaces a
completed record with a tombstone at the same operation key in one
conditional transaction.
The tombstone keeps the tenant ID and outcome and discards the grant. The
transaction leaves the active pointer alone so a newer operation can own it.
Acquisition and resume read the operation key in their transaction before
using the missing-record path. A pending local intent can read a completed
outcome from the tombstone without acquiring a grant.

The local file can still say pending after completion commits. Pruning can
also commit while its caller loses the reply. A failed attempt that makes no
durable change is represented by stuttering. The model assumes strictly
serializable transactions and stable operation IDs across retries.
[FoundationDB transactions with unknown results](https://apple.github.io/foundationdb/developer-guide.html#transactions-with-unknown-results)

| Configuration | TLC checks |
| --- | --- |
| [Atomic tombstone](TenantReservationPruning.cfg) | A completed intent never reacquires ownership, its recorded outcome remains replayable, and the active pointer agrees with the active operation. |
| [Erasure after ambiguous commits](TenantReservationPruningAmbiguous.cfg) | The operation finishes while its local file stays pending. Prune erases the row and loses its reply. Resuming the stale file then reacquires the slot for the same tenant ID. The runner requires `NoReacquisitionAfterAmbiguousCommits` to fail. |
| [Erasure across name reuse](TenantReservationPruningNameReuse.cfg) | After the replacement tenant has its own completed operation, a stale local file can reclaim the slot in recovery mode. The runner requires `NoReacquisitionAfterNameReuse` to fail. |
| [Replay during a new reservation](TenantReservationPruningReplay.cfg) | The runner requires a witness that prunes a completed record, retries the ambiguous prune, and reads the tombstone's outcome while a new operation owns the reused name. It checks for `NoRetiredReplayDuringNewReservation` to fail. |

The pruning model abstracts metadata clear and tenant-map delete into one
effect and bounds takeover to one transfer per operation. The reservation
model covers their separate commit and epoch checks. Tombstones remain
durable for as long as an old local file could be resumed, so this design
reduces record size and retains one key per operation.

The [reservation adapter](../../kits/foundationdb/src/fdb/tenant/reservation_pruning.rs)
reads existing full records and compacts them through `prune-delete`. The
[live pruning test](../../kits/foundationdb/src/fdb/tenant/reservation_recovery_tests.rs)
commits a prune through a second FoundationDB handle and injects the
unknown-result error into the first handle's retry loop. It checks that
the retry leaves the tombstone intact, that an old intent replays its saved
outcome, and that a replacement keeps its active grant.

## Reclaiming reservation tombstones

[TenantReservationCutoff.tla](TenantReservationCutoff.tla) models a durable
admission generation for each tenant name, scoped to the recorded control
tenant. New local intents capture the current generation with the tenant ID.
The file keeps that generation even if it is persisted after the cutoff
moves. Existing records without a generation start at zero. An admission
generation applies to the operation's whole lifecycle. The reservation epoch
still identifies its current grant.

With the active slot empty, an operator can commit an advance to a fixed
generation. A retry for the same target observes that generation or a later
one without advancing again. The claim transaction and each retry read the
cutoff alongside the active key and operation record. A collector clears a
tombstone only after comparing its stored generation with the durable cutoff
inside that transaction. This lets an advance commit once and reclamation
proceed in later transactions.

Retained completions remain readable across an advance. Once the key is
gone, a pending file below the cutoff is expired and has no cluster outcome
to replay. A locally saved outcome is still available to that file. The
model assumes all writers enforce the generation and abstracts the lease
and tenant mutations already covered by the earlier reservation models.
It bounds the search to two operation IDs, two advances, and one reuse of
the tenant name. An uncommitted attempt can stutter.

| Configuration | TLC checks |
| --- | --- |
| [Durable cutoff](TenantReservationCutoff.cfg) | The cutoff is monotone, a captured generation is immutable, an older or completed intent cannot hold a grant, and retained outcomes remain accurate. |
| [Preflight-only admission](TenantReservationCutoffPreflight.cfg) | A prepared resume retakes the slot after an unacknowledged cutoff advance, tombstone reclamation, and a completed operation for the replacement tenant. The runner requires `NoStaleClaimAfterNameReuse` to fail. |
| [Reclamation before the cutoff](TenantReservationCutoffEarlyReclaim.cfg) | A lost reclaim reply leaves the old operation key missing while its local file is still eligible to claim. The runner requires `NoSameGenerationClaimAfterReclaim` to fail. |
| [Progress after the cutoff](TenantReservationCutoffProgress.cfg) | The runner requires a witness where a retry observes a later advance, an old reclaim is retried, the stale file expires, and a current-generation operation completes for the replacement. It checks for `NoProgressAfterCutoff` to fail. |

The [Rust adapter](../../kits/foundationdb/src/fdb/tenant/reservation_cutoff.rs)
captures the generation with the tenant ID and uses the cutoff inside
acquisition, recovery, and guarded grant transactions. The
[live cutoff test](../../kits/foundationdb/src/fdb/tenant/reservation_cutoff_tests.rs)
forces both commit orders between a claim and an advance. It also tests
a resume racing an advance, an ambiguous advance retry, and name reuse.
