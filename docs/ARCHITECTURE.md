# Architecture

## Goal

`conductor-kit` should be a thin orchestration layer for CLI-native agents.

It is not a terminal automation framework.
It is not a tmux control plane.
It is not a giant prompt pack.

## What We Keep

From `oh-my-codex`, only these ideas survive:

- orchestrator and worker split
- a resumable loop instead of one-shot prompting
- persistent project memory
- explicit runtime state

## What We Reject

- overgrown prompt instructions
- hidden control flow
- tmux as required runtime infrastructure
- terminal key injection as a primary transport
- a separate role taxonomy added on top of the runtime

## Core Components

### 1. Skill Layer

`skills/conductor/SKILL.md` should answer only:
- when to use the orchestrator loop
- when to spawn or continue workers
- how to converge findings
- how to verify before acting

### 2. Config Layer

`config/conductor.json` defines:
- runtime loop policy
- preferred transport
- memory policy
- worker defaults

### 3. Helper CLI

`src/main.rs` is intentionally small.

Current responsibilities:
- config discovery
- health reporting
- config validation

Planned responsibilities:
- session registry
- direct transport broker
- state ledger
- memory persistence

### 4. State Ledger

The runtime should eventually persist:
- task id
- worker fan-out
- current phase
- last verified summary
- continuation pointers

This replaces ad hoc terminal state.

## Transport Strategy

Preferred order:

1. `stdio`
2. `unix_socket`
3. `tcp`

`tmux` is deliberately absent from the core.

## Loop Model

Each orchestration run follows:

1. `discover`
   Gather repository facts and external facts separately.
2. `spawn`
   Start or continue the right workers.
3. `converge`
   Merge outputs into a single working understanding.
4. `verify`
   Check against the repository, tests, and actual outputs.
5. `continue`
   Either spawn another iteration or close the run.

The loop must be resumable from persisted state.

## Memory Model

Memory should be:
- project-scoped
- bounded
- invalidated on git HEAD changes
- readable without replaying entire session history

## Runtime Vocabulary

Use these terms consistently:

- `orchestrator`
- `worker`
- `loop`
- `state`
- `memory`
- `resume`

Do not add a separate role taxonomy on top of them.
