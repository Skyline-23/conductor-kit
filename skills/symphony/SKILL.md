---
name: symphony
description: Switch the current conductor flow into the wider orchestration mode.
---

# Symphony

Use `$symphony` when the task needs broader orchestration across multiple threads of work.

Run:
- `conductor-symphony`

Rules:
- widen only when the task justifies it
- keep parallel work explicit
- converge back to one verified result
