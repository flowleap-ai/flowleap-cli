# Search Statement block

A **Search Statement** is the Boolean query built from the concept-synonym table and the
classification mapping, shown to the user as one block they can copy, open in Espacenet, or ask
you to change. Show it whenever you turn a claim or an invention into a query the user may run
themselves. Write the block in exactly this shape, so every surface renders it the same.

## Before you show it

1. Build the statement from the concept table: OR the synonyms inside one concept, AND the
   concepts together, AND the classification. Field names come from
   [cql-reference.md](cql-reference.md) only.
2. Probe it: run the statement once with a small range and read the total. Record the run with a
   purpose of `Search Statement v<N>` (v1 for the first, +1 for each change). The working record
   lists every query that ran, so each version the user saw is kept. When the search tool is not
   available (no sign-in, backend error), still show the block, write `not run — <reason>` in
   place of the hit count, and tell the user the statement is not in the working record.
3. Pick the **Discriminating Term**: the concept that separates this invention from its
   technology area. A CPC or IPC code is never discriminating. A broad category word
   ("artificial intelligence", "sensor") is never discriminating.

## The block

````markdown
**Search Statement v1** — <one line: what the statement looks for>, <total> hits

| Concept | Synonyms | CPC |
|---|---|---|
| <concept> | <term>, <term>, <term> | <code or —> |

```cql
<the statement exactly as it ran>
```

[Open in Espacenet](https://worldwide.espacenet.com/patent/search?q=<encoded statement>)

Discriminating Term: <term>

Ask me to change a concept and I regenerate the statement; the working record keeps every version that ran.
````

Keep the parts in this order. Keep the table headers, the `Discriminating Term:` label and the
last line word for word; put your notes after the block, not inside it.

Rules for each part:

- **Fence tag**: `cql` for EPO OPS / Espacenet. `lucene` for a USPTO statement, only when the
  user asks for USPTO. The chat code block has a copy button; do not add another.
- **Espacenet link**: Espacenet runs OPS CQL unchanged for `ti`, `ab`, `ta`, `txt`, `pa`, `in`,
  `cpc`, `ic`, `pd` (checked 2026-10-05: same totals on both). Encode the statement with
  `encodeURIComponent`, then also replace `(` with `%28` and `)` with `%29` — an unencoded
  parenthesis ends the markdown link early. Never put a USPTO `lucene` statement in an Espacenet
  link; omit the link line for it.
- **No Discriminating Term**: replace the `Discriminating Term:` line with this line, word for
  word, and say which concept the user could add:

  `⚠ No Discriminating Term — this statement returns the field, not the invention`

- **Editing**: when the user asks to add, remove or change a concept or a term, rebuild the
  statement, probe it again as the next version, and show the whole block again with the new
  version number. Never edit an earlier block in place.

## Example (a wireless charger that detects metal on the pad)

````markdown
**Search Statement v1** — wireless charging with foreign-object detection, published before 2016, 56 hits

| Concept | Synonyms | CPC |
|---|---|---|
| wireless charging | "wireless charging", "inductive charging", "wireless power" | H02J50/60 |
| foreign object | "foreign object", "metal object" | H02J50 (IPC) |

```cql
ta=("wireless charging" OR "inductive charging" OR "wireless power") AND ta=("foreign object" OR "metal object") AND (cpc=H02J50/60 OR ic=H02J50) AND pd<2016
```

[Open in Espacenet](https://worldwide.espacenet.com/patent/search?q=ta%3D%28%22wireless%20charging%22%20OR%20%22inductive%20charging%22%20OR%20%22wireless%20power%22%29%20AND%20ta%3D%28%22foreign%20object%22%20OR%20%22metal%20object%22%29%20AND%20%28cpc%3DH02J50%2F60%20OR%20ic%3DH02J50%29%20AND%20pd%3C2016)

Discriminating Term: foreign object

Ask me to change a concept and I regenerate the statement; the working record keeps every version that ran.
````

## On the CLI

The text above is shared with the FlowLeap app, word for word. On the CLI, read it with these
mappings:

- **Field names**: `cql-reference.md` is the app's file. On the CLI the field reference is
  "Step 2 — write the query" in the `flowleap-patent` skill.
- **Probe**: `flowleap --json patent search --query '<statement>' --count-only` runs the
  statement once and returns `{ query, total }`. Use `total` as the hit count. For a USPTO
  statement, `flowleap --json uspto search --query '<statement>' --count-only` returns
  `{ query, count }`.
- **Record**: the CLI has no `purpose` tag and keeps no log of its own. Write each version that
  ran into the working record of the task (the recipe's query log, or the command log of
  `recipe-audit-report`): `Search Statement v<N>`, the exact command, and its `total`.
- **Not run**: when the probe fails (no sign-in, a key gate, a backend error), still show the
  block with `not run — <reason>`. A key gate is a user-action stop: follow `flowleap-keys`.
