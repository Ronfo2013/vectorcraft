# Type parity

> **Last reviewed:** 2026-10-10 · **Last updated:** 2026-10-13 · **Change:** minor (Optical Margin Alignment and Roman Hanging Punctuation done: `type.opticalMarginAlignment`, `text.romanHangingPunctuation`, type core 82% → 83%; Hyphenation… and Justification… options done: they act in the line breaker and justified lines, `text.hyphenation`, `text.justification`) · **Target:** Adobe Illustrator 2026 (30.x)

Type is Illustrator's largest single area by use (weight 13 of 105 across "Type core" and "Type advanced" in
[target-app-parity.md](target-app-parity.md)). This checklist lists the features one by one. Status is from the
code and tests (`crates/text`, `crates/engine/src/cmd/type*.rs`, `textstyles.rs`, `crates/tools/src/text.rs`)
against Illustrator's public documentation; **estimated** unless noted.

## Summary

| Part | % | Remaining (h) |
|---|---:|---:|
| Type core (point, area, path type, Character/Paragraph, styles, threading, OpenType, fonts) | 83% | 8–12 |
| Type advanced (CJK composition, Middle Eastern, spelling, Touch Type, Retype, variable fonts) | 50% | 20–28 |

## Checklist

| Feature | Status | Notes |
|---|---|---|
| Point, area and type-on-a-path type; vertical variants | done / partial | vertical point, area and path type have initial support |
| Area Type Options (rows, columns, inset, first baseline, Auto Size, Shrink Text to Fit) | done | |
| Threaded text, Fit Headline, Text Wrap | done | |
| Type on a Path effects and options; brackets | done | #429 |
| Character and Paragraph panels, styles with overrides | done | mixed-value display for differing paragraphs missing |
| Character size and horizontal scale after object/group scaling | done | #1034: fields and setters compensate the stored affine; point size follows the transformed em-height axis, not the glyph ink bounds. Range edits, keyboard increments, leading and baseline shift use document points; rotation, reflection and shear remain editable. |
| Preserve font sizes while scaling a group | missing | Separate enhancement; scaling still scales text with the other artwork. |
| OpenType panel, Glyphs panel, alternates | done | Highlight Alternate Glyphs preference unused (#394) |
| Font menu: samples, live preview, filters, favourites; font matching by any name; Find Font | done | #339 |
| Variable fonts | partial | named instances as styles; axis sliders missing |
| Colour fonts (OpenType SVG / COLR) | partial | |
| Every-line and Single-line composers | done | tuned by eye, never compared side by side |
| Hyphenation… and Justification… options | done | Paragraph panel menu dialogs, `text.hyphenation` / `text.justification`, per paragraph, saved in the file. Hyphenation: Words Longer Than, After First, Before Last, Hyphen Limit (both composers), Hyphenation Zone (Single-line Composer), Hyphenate Capitalized Words ([helpx](https://helpx.adobe.com/illustrator/using/hyphenation-line-breaks.html)). Justification: Desired word/letter spacing and glyph scaling on every line, justified lines fill within Minimum/Maximum (word spacing, then letter spacing, then glyph scaling, then the word spaces), Auto Leading (also the Character panel's Leading and the Alt+↑/↓ leading nudge), Single Word Justification (new type: Full Justify; paragraphs from files saved before the option keep their layout: a single word spreads only in Justify All Lines, and the dialog shows the choice that matches) ([helpx](https://helpx.adobe.com/illustrator/desktop/design-with-text/edit-format-text/adjust-word-and-letterspacing-in-justified-text.html)). Horizontal type only (vertical type keeps the defaults, Auto Leading applies); the hyphenator is VectorCraft's own pattern-free English one (no language dictionaries); not compared side by side with the reference app |
| Optical Margin Alignment | done | Type menu, per story (every threaded frame), `type.opticalMarginAlignment` with query mode, saved in the file. The punctuation listed below hangs by the same amounts, and the sides of letters such as W, A, V, Y, T and O stand slightly outside: amounts are VectorCraft's own, derived from the letter shapes (the documentation gives none), never compared side by side. Left and right edges, point and area type; justified lines fill to the hung edges. Not in right-to-left paragraphs or type on a path; one character hangs per edge ([helpx](https://helpx.adobe.com/illustrator/using/formatting-paragraphs.html)) |
| Roman Hanging Punctuation | done | Paragraph panel menu, per paragraph, `text.romanHangingPunctuation` with query mode, saved in the file: quotation marks, hyphens, full stops and commas hang wholly outside the margins; asterisks, tildes, ellipses, en and em dashes, colons and semicolons hang half ([helpx](https://helpx.adobe.com/illustrator/using/formatting-paragraphs.html)). Both edges, point and area type, both composers; justified lines fill to the hung edge |
| Tab stops and leaders; Tabs panel | partial | Tabs panel and stops with leaders in the model (`text.tabs.set`); leader drawing not verified against the docs |
| Hidden characters (Show Hidden Characters) | done | `type.hiddenCharacters` |
| Spell check (Check Spelling, Auto Spell Check, custom dictionary) | missing | menu stubs; needs an openly licensed dictionary |
| Smart Punctuation, Change Case, Find & Replace | done | |
| Touch Type tool | missing | listed in the toolbar, not implemented |
| Retype (identify fonts in outlines/images) | missing | Illustrator's uses a cloud model |
| Snap to Glyph | missing | menu stub |
| Middle Eastern: bidirectional layout, Paragraph Direction | done | |
| Middle Eastern: Character Direction, digit types, kashidas, Middle Eastern composers, split caret | missing | |
| CJK: input methods (marked text, candidate window) | done | |
| CJK: kinsoku (Hard, Soft, None), Line-end Punctuation Half Width, tate-chu-yoko (auto) | done / partial | custom kinsoku sets, the Mojikumi Settings dialog and the other mojikumi sets missing |
| CJK: Proportional Metrics (`palt` in horizontal type, `vpal` in vertical type) | done | #966 |
| CJK: ruby, manual tate-chu-yoko | missing | #633 |
| Inline symbols in text | beyond Illustrator | not scored |

## Revision history

| Date | Change | Summary |
|---|---|---|
| 2026-10-13 | minor | Optical Margin Alignment and Roman Hanging Punctuation done (layout at both edges in point and area type, both composers, commands, Type menu and Paragraph panel menu, file format); Type core 82% → 83%, 10–15 h → 8–12 h (estimated) |
| 2026-10-13 | minor | Hyphenation… and Justification… options done (composition, commands, dialogs, file format); Type core 80% → 82%, 14–19 h → 10–15 h (estimated); review fixes: old files keep their single-word lines, Auto Leading in the Character panel and leading nudge, contradictory or unknown options refused |
| 2026-10-11 | minor | Proportional Metrics done: `palt` in horizontal type, `vpal` in vertical type (#966, #1082); ruby and manual tate-chu-yoko still missing |
| 2026-10-10 | minor | #1034: transformed character controls and regression coverage; preserving group font sizes remains a separate enhancement. Scores/hours unchanged at this rounding. |
| 2026-10-10 | major | First checklist, split out of the parity table's Type core and Type advanced rows |
