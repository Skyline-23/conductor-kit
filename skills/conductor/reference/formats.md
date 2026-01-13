# Output Format Standards

All delegate outputs should follow these formats for consistent parsing.

## Standard Response Format

```json
{
  "summary": "one-line conclusion",
  "confidence": "high|medium|low",
  "findings": [
    {
      "type": "issue|suggestion|info",
      "file": "path/to/file.ts",
      "line": 42,
      "message": "description",
      "severity": "critical|high|medium|low"
    }
  ],
  "suggested_actions": [
    {
      "action": "edit|create|delete|run",
      "target": "path or command",
      "description": "what to do"
    }
  ]
}
```

## Analysis Response

For `oracle` and analytical tasks:

```json
{
  "summary": "one-line conclusion",
  "confidence": "high|medium|low",
  "analysis": {
    "problem": "problem description",
    "root_cause": "identified cause",
    "impact": "affected areas",
    "trade_offs": [
      {"option": "A", "pros": ["..."], "cons": ["..."]}
    ]
  },
  "recommendation": "recommended approach",
  "reasoning": "step-by-step reasoning"
}
```

## Search Response

For `explore` and `librarian` tasks:

```json
{
  "summary": "what was found",
  "confidence": "high|medium|low",
  "results": [
    {
      "file": "path/to/file.ts",
      "lines": [10, 25],
      "relevance": "high|medium|low",
      "snippet": "code or text excerpt",
      "note": "why this is relevant"
    }
  ],
  "patterns": ["identified patterns"],
  "next_steps": ["suggested follow-up searches"]
}
```

## Patch Response

For edit suggestions:

```json
{
  "summary": "what changes are proposed",
  "confidence": "high|medium|low",
  "patches": [
    {
      "file": "path/to/file.ts",
      "hunks": [
        {
          "start_line": 10,
          "old_lines": ["original line 1", "original line 2"],
          "new_lines": ["new line 1", "new line 2"],
          "description": "what this hunk does"
        }
      ]
    }
  ],
  "verification": {
    "commands": ["npm test", "npm run typecheck"],
    "expected": "all pass"
  }
}
```

## Checklist Response

For review and validation tasks:

```json
{
  "summary": "overall assessment",
  "confidence": "high|medium|low",
  "checklist": [
    {
      "item": "check description",
      "status": "pass|fail|warn|skip",
      "details": "explanation",
      "file": "path/to/file.ts",
      "line": 42
    }
  ],
  "passed": 5,
  "failed": 1,
  "warnings": 2
}
```

## Confidence Levels

| Level | Meaning | When to use |
|-------|---------|-------------|
| `high` | Strong evidence, clear conclusion | Verified against code/tests |
| `medium` | Reasonable inference, some uncertainty | Based on patterns, not verified |
| `low` | Speculation, needs verification | Incomplete context, edge cases |

## Error Response

When delegate cannot complete:

```json
{
  "error": true,
  "type": "timeout|parse_error|insufficient_context|unsupported",
  "message": "description of failure",
  "partial_result": { },
  "suggestion": "how to recover"
}
```
