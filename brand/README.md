# Strata brand

<p>
  <img src="png/strata-icon-blue-128.png" width="64" alt="Strata app icon">
  <img src="png/strata-icon-dark-128.png" width="64" alt="Strata icon on Night">
</p>

## The idea

One strip folded into three layers. It is an S for Strata, the folded strip is the strata themselves (a board is built up in layers: pages, layers, frames, notes), and the blue dot where the strip ends is a node, because every item on a board is a node that can be connected.

Strata (once called Overlay) is a sibling of Graphsight, so the mark follows Graphsight's rules: a 100-unit box, 10-unit round strokes, and one blue dot. As with Graphsight, the app icon is the white mark on a Hub Blue tile, so the two sit together on the taskbar; the Night tile is the alternate.

## Construction

| Part | Regular (40 px and up) | Small (32 px and down) |
|---|---|---|
| Strip | stroke 10, round ends and turns | stroke 12 |
| Layers | at y 28, 50 and 72; the turns are half circles of radius 11 | at y 27, 50 and 73, radius 11.5 |
| Path | from (74.5, 28) left to x 41.5, down, right to x 63.5, down, left to x 44.5 | from (75, 27), the same turns, ending at x 46 |
| Dot | at (28.5, 72), radius 8, Hub Blue | at (28, 73), radius 9 |
| Icon | 100 x 100 square, corner radius 22.4, mark scaled 0.9 and centred | same |

The gap between the end of the strip and the dot is about a third of the stroke, so the dot reads as its own node. Colours: Hub Blue `#0A84FF`, Night `#14161C`, Paper `#F5F5F7`. The wordmark is "strata" in lowercase Manrope ExtraBold. In the horizontal lockup the mark stands a little taller than the lowercase letters, like a capital S.

## Files

| Folder | What | Use |
|---|---|---|
| `svg/strata-mark-*` | The mark alone: `color-on-dark`, `color-on-light`, `white`, `ink` | Docs, slides, anywhere the mark sits on a plain background |
| `svg/strata-icon-blue*`, `png/strata-icon-blue-*` | The app icon: white mark on Hub Blue | Window, taskbar, store listings, favicons. Use `-small` at 32 px and below |
| `svg/strata-icon-dark*`, `png/strata-icon-dark-*` | Alternate: Paper mark and blue dot on Night | Dark surfaces where a blue tile is too loud |
| `ico/strata.ico`, `ico/strata-dark.ico` | Windows icons, 16 to 256 px (blue, and Night) | `strata.ico` is embedded in `strata.exe` by `build.rs` |
| `svg/strata-wordmark-*` | "strata" alone | Rare; prefer a lockup |
| `svg/strata-lockup-horizontal-*`, `svg/strata-lockup-stacked-*` | Mark and wordmark together, `on-dark` and `on-light` | Headers, about screens, splash |
| `svg/strata-readme-header-*` | Lockup and tagline, transparent | The top of the README (dark and light GitHub themes) |
| `png/strata-social-banner-1280x640.png` | Lockup and tagline on Night | Repository social preview, posts |
| `svg/favicon.svg` | Small blue icon | Web pages |

All text in the SVGs is outlined (paths), so they look the same without the fonts installed.

## Rules

- Keep one blue dot. Never add a second accent colour to the mark.
- On dark backgrounds use Paper strokes; on light ones use Night. The dot stays Hub Blue, except in single-colour versions (`white`, `ink`).
- Leave clear space around the mark of at least the dot's diameter.
- Don't rotate or mirror the mark, join the dot to the strip, or add a fourth layer.
- Use the small cut at 32 px and below.

## Regenerating

Everything here is generated from code, so changes stay consistent across every size:

```
cargo run --release --manifest-path tools/brand/Cargo.toml
```

Run it from the repository root. It reads the fonts in `assets/fonts/` and writes into `brand/`. The mark's geometry is at the top of `tools/brand/src/main.rs`.
