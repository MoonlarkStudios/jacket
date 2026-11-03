# Jacket themes

The header's Theme control switches between Jacket Dark, Jacket Light and
Catppuccin Mocha. Appearance changes apply to the entire interface, including
fields, menus, previews, editors and artwork controls. They preserve the current
review, selected tracks, edits and active operation.

Dark and Light share Resonant v2's charcoal and neutral surface hierarchy, with
Jacket's copper accent. Mocha uses the shared Catppuccin colors. Geist typography
and the same semantic control roles keep the two products visually related.

Choose **Load theme file…** to import a TOML definition. Jacket validates the
complete file before replacing the current theme. Cancelling or importing an
invalid file preserves the active theme. The imported definition is copied into
the application preference, so moving or deleting the original file does not
remove the selected appearance.

## Theme format

Jacket accepts Resonant v2's version 1 semantic theme format:

```toml
version = 1
base = "dark"

[colors]
accent = "#A7D9FF"
accent_text = "#101C28"
selection = "#A7D9FF"
```

`base` is `dark`, `light`, or `catppuccin_mocha`. Omitted colors inherit that
preset. Definitions do not include other files or run code. Files must contain
UTF-8 text and be at most 64 KiB.

Every color is an opaque six-digit sRGB `#RRGGBB` value. Alpha, shorthand, named
colors, unknown fields and unsupported versions are rejected with a useful
error. Authoring controls semantic roles rather than individual widgets or
interaction states. Hover, pressed, selection and scrim colors are derived once
when a theme is selected. Overriding `accent` also changes `accent_strong` unless
the file supplies an explicit strong accent.

Jacket uses `background`, `chrome`, `panel`, `raised`, `border`, `text`,
`text_muted`, `accent`, `accent_strong`, `accent_text`, `success`, `warning`,
`danger` and `selection`. The shared schema also accepts Resonant's `grid`,
`playhead`, `loop_region`, `annotation`, `source`, `drums`, `bass`, `other`,
`vocals`, `keys`, `guitar`, `meter_low`, `meter_high` and `meter_clip` roles. Those
audio roles make a definition portable between the products; Jacket retains
their literal values without assigning them to unrelated interface elements.

The built-in themes test text and primary-action contrast. Custom colors remain
as authored, including low-contrast choices. Use at least 4.5:1 contrast for
control text and secondary labels, and at least 3:1 for interactive boundaries
against their actual surfaces. The operating system owns native window chrome
and file dialogs; these remain outside the application's theme palette.


## Keyboard controls

Open Appearance with Ctrl/Cmd+Shift+, or choose the current theme in the header.
While Appearance is open, 1 selects Dark, 2 selects Light and 3 selects Mocha.
Enter confirms the current choice; Escape returns to the workspace. Switching
palettes does not discard an open review or stop a download.

Metadata forms focus the first editable field when opened. Tab and Shift+Tab move
between fields, and Ctrl/Cmd+S applies the form. Invalid values keep the form open
with the error. In the workspace Ctrl/Cmd+S opens the file preview; Enter in that
preview opens the destination dialog.
