# Role Routing Guide

Roles are defined in `conductor.json`. Route tasks to the appropriate role based on task characteristics.

## Role Matrix

| Role | CLI | Use For | Strengths |
|------|-----|---------|-----------|
| `oracle` | codex (reasoning) | Deep analysis, architecture, security, algorithms | Extended thinking, trade-off analysis |
| `librarian` | gemini | Doc lookup, API reference, best practices research | Fast retrieval, broad knowledge |
| `explore` | gemini | Codebase navigation, file discovery, pattern finding | Quick scanning, parallel search |
| `frontend-ui-ux-engineer` | gemini-pro | UI/UX review, component design, accessibility | Visual reasoning, design patterns |
| `document-writer` | gemini | README, docs, comments, changelogs | Clear writing, formatting |
| `multimodal-looker` | gemini | Screenshot analysis, image review, visual debugging | Image understanding |

## Task → Role Mapping

### Use `oracle` when:
- Architecture decisions with trade-offs
- Root cause analysis for complex bugs
- Security vulnerability assessment
- Algorithm design / complexity analysis
- Refactoring strategy for legacy code
- Migration planning with risk assessment
- Any task requiring step-by-step reasoning

### Use `librarian` when:
- Looking up framework/library docs
- Finding best practices or patterns
- Researching external dependencies
- Checking API specifications

### Use `explore` when:
- Initial codebase discovery
- Finding relevant files for a task
- Understanding project structure
- Locating similar implementations

### Use `frontend-ui-ux-engineer` when:
- Reviewing UI component design
- Accessibility audit
- Layout/styling decisions
- Component architecture

### Use `document-writer` when:
- Writing/updating documentation
- Generating changelogs
- Creating code comments
- README improvements

### Use `multimodal-looker` when:
- Analyzing screenshots or mockups
- Visual regression review
- Image-based debugging
- UI comparison tasks

## Combining Roles

For complex tasks, combine roles in sequence:

1. **Discovery phase**: `explore` → find relevant files
2. **Analysis phase**: `oracle` → deep reasoning on findings
3. **Review phase**: `librarian` → verify against docs/best practices

Example (security audit):
```
explore → find auth-related files
oracle → analyze vulnerabilities + propose fixes
librarian → verify against OWASP guidelines
```
