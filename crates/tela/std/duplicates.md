---
description: Paragraphs copied verbatim across the corpus.
params:
  min-words: 12
---

# Duplicates

## duplicate-content

A paragraph of at least `min-words` words copied verbatim elsewhere; Markdown
syntax counts, so equal labels with different destinations differ.

```hq
(rule duplicate-content
  :message "paragraph duplicates {@original}"
  :at @copy
  :related @original
  (paragraph :norm ?t :position ?p :words (>= $min-words) :dynamic (not "true")) @original
  (paragraph :norm ?t :position (after ?p)) @copy
  (not (paragraph :norm ?t :position (before ?p))))
```

### Matches

```md min-words=3
One two three four.

One two three four.
```

### Passes

```md min-words=3
One two three four.

Five six seven eight.
```
