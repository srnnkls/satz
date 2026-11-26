---
description: Names marked outdated, with their replacements.
data:
  outdated: {}
---

# Links

## outdated-reference

Names listed in the `outdated` table, mapped to their replacements, should not
appear in text, code spans or link destinations.

```tq
(rule outdated-reference
  :message "\"{@swap.key}\" is marked outdated; use \"{@swap.value}\""
  :at @text
  :focus ?old
  (row :table "outdated" :key ?old) @swap
  [(paragraph :text (contains ?old)) (heading :text (contains ?old)) (table :text (contains ?old))] @text)
```

### Matches

```md outdated.legacy-model=current-model
Use legacy-model here.
```

### Passes

```md outdated.legacy-model=current-model
Use current-model here.
```
