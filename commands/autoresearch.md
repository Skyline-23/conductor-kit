# Autoresearch

Use this command when the current task needs repeated measurable experiments.

What it should do:
- treat autoresearch as a lightweight Ralph-compatible loop
- require a concrete metric command and extraction regex
- keep edits inside an explicit in-scope set
- keep `results.tsv` and `run.log` in the repo root
- discard regressions automatically

Core commands:
- `conductor autoresearch setup --goal "<goal>" --metric-command "<command>" --metric-regex "<regex>" --direction lower|higher --in-scope <path>`
- `conductor autoresearch step "<short experiment description>"`
- `conductor autoresearch summary`
