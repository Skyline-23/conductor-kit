---
description: "Release mode: release checklist and validation."
argument-hint: "version or release scope"
---

Respect disabled state. If Conductor is disabled, do not enable it automatically; inform the user and proceed without Conductor unless they explicitly request enabling.
If enabled, load conductor skill. Activate release mode.

Checklist: version bump, changelog, validation, secret scan.

Refer to skills/conductor/SKILL.md for detailed instructions.
