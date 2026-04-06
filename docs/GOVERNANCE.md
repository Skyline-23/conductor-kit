# Governance

## Goal

Governance defines what the runtime is allowed to do, independent of transport or execution mechanics.

This must stay separate from runtime policy.

## Governance Rules

Initial governance rules:
- exactly one orchestrator authority per run
- one active run per session unless explicitly overridden
- workers may not spawn other workers directly
- workers may not mutate tasks they do not claim
- cleanup must not destroy active claimed work unless forced
- cross-session state mutation is forbidden

## Approval Boundaries

Some actions should require an explicit approval decision:
- force-cancelling an active run
- stealing or invalidating a live claim
- deleting session-scoped state
- replaying from a conflicting snapshot root

The approval result should be durable and evented.

## Mutation Ownership

Only these runtime actors may mutate these state families:
- orchestrator authority: run state, phase, worker assignment
- task/claim subsystem: task state and claims
- dispatch subsystem: dispatch records
- mailbox subsystem: mailbox records
- memory subsystem: project memory and notes

Observers may not mutate authoritative state.

## Cleanup Rules

Normal cleanup requires:
- no active authority lease, or explicit force
- no non-terminal claimed tasks, or explicit force
- dispatch backlog drained or marked failed
- mailbox state preserved for resume unless explicitly pruned

## Resume Rules

Resume is allowed only when:
- the run state is present
- the session scope is authoritative or no session scope exists
- state schema versions are compatible
- no conflicting active authority exists

## Nested Orchestration

V1 should disallow nested orchestration by default.

The orchestrator may delegate tasks to workers, but workers should hand upward, not create sub-runtimes.
