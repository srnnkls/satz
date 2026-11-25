---
description: Heading structure of a document.
---

# Structure

## duplicate-heading

A heading repeated in one document makes its anchors ambiguous.
`(repeated-heading @first @repeat ?text)` matches each repeat with the first
heading of its text in the same file.

```hq
(define (repeated-heading @first @repeat ?text)
  (file :node ?f (heading :norm ?text :text /\S/ :node ?n) @first)
  (file :node ?f (heading :norm ?text :node (after ?n)) @repeat)
  (not (file :node ?f (heading :norm ?text :node (before ?n)))))

(rule duplicate-heading
  :message "heading repeated; first occurrence on line {@first.line}"
  :at @repeat
  :related @first
  (repeated-heading @first @repeat ?text))
```

### Matches

```md
# Usage

## Setup

## setup
```

### Passes

```md
# Usage

## Setup

## Teardown
```
