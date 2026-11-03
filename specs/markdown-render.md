---
id: MDR
status: accepted
adrs: []
---

# Markdown render of the edge cases

## Intent

The render writes the functional docs from the artifacts of the passes, which an LLM fills. This spec covers the
inputs an LLM can produce that the layout must stay robust to: a multi-line description, a weak score in a list,
an inverted line range, a slug that is not a safe path, two slugs that give one file. It does not describe the
whole layout of the pages.

## Requirements

- **MDR-1** (deterministic) The list item of a feature on a domain page, and of a use case on a feature page, must show its description on one line, as the agent index (`llms.txt`) does.
- **MDR-2** (deterministic) The list item of a feature on a domain page, and of a use case on a feature page, must put ⚠️ before a score below the low confidence threshold, as the agent index does.
- **MDR-3** (deterministic) A cited range whose end line comes before its start line must be shown with the lower line first (`path:3-9`).
- **MDR-4** (deterministic) The render must fail with an error naming the path when two items render to the same file.
- **MDR-5** (deterministic) A slug must not give a path outside the docs dir.

## Scenarios

- (MDR-1) Given a feature whose description is `Customers pay.\n\nBy card.`, when the docs are rendered, then its
  item on the domain page reads `Customers pay. By card.` and no free paragraph follows the item.
- (MDR-3) Given a step citing `src/pay.rs` from line 9 to line 3, when the docs are rendered, then the step shows
  `src/pay.rs:3-9`.
- (MDR-4) Given the domains `orders` and `Orders`, when the docs are rendered, then the render fails naming
  `functional/orders/README.md` and writes no page.
- (MDR-5) Given a domain slug `../../etc`, when the docs are rendered, then its page is
  `functional/etc/README.md`.

## Non-goals

- The layout of the pages beyond these edge cases (headings, sections, order of the items).
- Two paths that differ only by case on a case-insensitive file system (a feature `readme` next to the domain's
  `README.md`).
- Making the slugs distinct: the passes already do it (features and use cases through `unique_slug`, domains merged
  by slug); a collision comes from an edited or an older artifact.
