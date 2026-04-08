# Ralph

Use this command when the task needs the resumable Ralph-style loop that keeps iterating until one verified outcome is accepted.

Loop:
1. initialize or resume the current conductor run
2. open the ops surface
3. stay in the operator lane by default and keep iterating
4. only widen to a team when a worker count is explicitly requested
5. automatically re-enter the operator lane when wakeable events, idle convergence, or verification pressure appear
6. keep progress observable through the HUD and worker panes
7. continue or close based on verification

Default shape:
- `conductor ralph`
- `conductor ralph <run_id>`

Optional:
- `conductor ralph <run_id> <worker_count>`
