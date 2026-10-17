# Overlay brand

<p>
  <img src="png/overlay-icon-blue-128.png" width="64" alt="Overlay app icon">
  <img src="png/overlay-icon-dark-128.png" width="64" alt="Overlay icon on Night">
</p>

## The idea

An "O" for Overlay, left open at the top right. The blue dot closes it. A board is finished when things connect: the dot is a node, and it completes the shape the way a connection completes a graph.

Overlay is a sibling of Grapht, so the mark follows Grapht's rules exactly. It is drawn in the same 100-unit box with the same 10-unit round strokes, with one blue dot. As with Grapht, the app icon is the white mark on a Hub Blue tile, so the two sit together on the taskbar; the Night tile is the alternate.

## Construction

| Part | Regular (40 px and up) | Small (32 px and down) |
|---|---|---|
| Arc | centre (50, 50), radius 24, stroke 10, round ends | radius 22, stroke 12.5 |
| Opening | from 16° to -76° (0° is right, clockwise is positive) | from 30° to -90°, wider so it stays open at 16 px |
| Dot | at -30° on the arc, radius 8, Hub Blue | radius 9 |
| Icon | 100 x 100 square, corner radius 22.4, mark scaled 0.9 and centred | same |

Colours: Hub Blue `#0A84FF`, Night `#14161C`, Paper `#F5F5F7`. The wordmark is "overlay" in lowercase Manrope ExtraBold. In the horizontal lockup the mark's "O" spans the wordmark's lowercase letters, like one more letter.

## Files

| Folder | What | Use |
|---|---|---|
| `svg/overlay-mark-*` | The mark alone: `color-on-dark`, `color-on-light`, `white`, `ink` | Docs, slides, anywhere the mark sits on a plain background |
| `svg/overlay-icon-blue*`, `png/overlay-icon-blue-*` | The app icon: white mark on Hub Blue | Window, taskbar, store listings, favicons. Use `-small` at 32 px and below |
| `svg/overlay-icon-dark*`, `png/overlay-icon-dark-*` | Alternate: Paper mark and blue dot on Night | Dark surfaces where a blue tile is too loud |
| `ico/overlay.ico`, `ico/overlay-dark.ico` | Windows icons, 16 to 256 px (blue, and Night) | `overlay.ico` is embedded in `overlay.exe` by `build.rs` |
| `svg/overlay-wordmark-*` | "overlay" alone | Rare; prefer a lockup |
| `svg/overlay-lockup-horizontal-*`, `svg/overlay-lockup-stacked-*` | Mark and wordmark together, `on-dark` and `on-light` | Headers, about screens, splash |
| `svg/overlay-readme-header-*` | Lockup and tagline, transparent | The top of the README (dark and light GitHub themes) |
| `png/overlay-social-banner-1280x640.png` | Lockup and tagline on Night | Repository social preview, posts |
| `svg/favicon.svg` | Small blue icon | Web pages |

All text in the SVGs is outlined (paths), so they look the same without the fonts installed.

## Rules

- Keep one blue dot. Never add a second accent colour to the mark.
- On dark backgrounds use Paper strokes; on light ones use Night. The dot stays Hub Blue, except in single-colour versions (`white`, `ink`).
- Leave clear space around the mark of at least the dot's diameter.
- Don't rotate the mark, close the gap, or move the dot. The open "O" and the dot are the idea.
- Use the small cut at 32 px and below.

## Regenerating

Everything here is generated from code, so changes stay consistent across every size:

```
cargo run --release --manifest-path tools/brand/Cargo.toml
```

Run it from the repository root. It reads the fonts in `assets/fonts/` and writes into `brand/`. The mark geometry is defined at the top of `tools/brand/src/main.rs`, and the app's own drawing of it is in `paint_mark` in `src/theme.rs`.
