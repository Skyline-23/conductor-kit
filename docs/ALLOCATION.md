# Allocation

## Goal

Allocation decides where new work goes.

Rebalance decides whether existing work should move.

These must stay as policy seams, not scattered runtime heuristics.

## Inputs

Allocation should consider:
- task readiness
- task dependencies
- worker current state
- worker current load
- worker last heartbeat
- worker kind
- verification backlog

## Allocation Outputs

Each decision should produce:
- `selected_worker`
- `fallback_workers`
- `reason`

The reason string is part of the contract.

## V1 Allocation Policy

Start simple:
1. prefer idle workers
2. prefer workers already handling related follow-up work
3. avoid assigning to blocked or stale workers
4. keep verifier work separate when possible
5. fall back to least-loaded healthy worker

## Rebalance Triggers

Rebalance is allowed when:
- a worker is stale
- a worker died
- a claim expired
- a task is blocked too long
- verification backlog is starving completion

## Rebalance Restrictions

Rebalance may not:
- steal a task with a valid active claim
- silently drop mailbox history
- rewrite unrelated worker state

## V1 Rebalance Policy

Start conservative:
- reclaim only expired or released tasks
- requeue dead-worker tasks
- nudge blocked workers before reassigning
- surface a recommendation before automatic reassignment where possible

## Future Expansion

Possible later additions:
- priority classes
- affinity groups
- workload classes
- verification lane reservation
