# Contributing

## Changesets

Every PR requires a changeset. CI will fail without one.

**User-facing changes** (features, fixes, breaking changes):

```bash
npx changeset
```

Pick the bump level (`patch`, `minor`, or `major`) and write a short summary. This becomes the CHANGELOG entry.

**Non-user-facing changes** (CI, docs, refactors, tests):

```bash
npx changeset --empty
```

This satisfies CI without triggering a version bump.
