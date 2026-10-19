<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="brand/svg/overlay-readme-header-dark.svg">
    <img alt="Overlay. A whiteboard where every image, sketch and shape is a node." src="brand/svg/overlay-readme-header-light.svg" width="720">
  </picture>
</p>

<p align="center">
  <a href="https://github.com/gowthamjeyabalan/overlay/releases/latest"><img alt="Latest release" src="https://img.shields.io/github/v/release/gowthamjeyabalan/overlay?color=0A84FF"></a>
  <img alt="Windows" src="https://img.shields.io/badge/platform-Windows-14161C">
  <img alt="Rust" src="https://img.shields.io/badge/made%20with-Rust-14161C">
  <a href="LICENSE"><img alt="MIT licence" src="https://img.shields.io/badge/licence-MIT-14161C"></a>
</p>

**Overlay** is a fast, lightweight whiteboard for Windows. Drop in images, sketch with a pen, marker or highlighter, and build flowcharts. Every item on the board is a node with ports on all four sides, so an image, a scribble, a text box or a whole group can be connected to anything else. Connections show relationships; they do not carry data.

<p align="center"><img alt="Overlay with a grouped flowchart, a group of ideas, pen, pencil and marker sketches, and the Overview map" src="docs/screenshot.png" width="900"></p>

## Download

Get `overlay.exe` from the [latest release](https://github.com/gowthamjeyabalan/overlay/releases/latest) and run it. There is nothing to install: it is a single 11 MB file and needs Windows 10 or 11. Try it on [`examples/demo.overlay`](examples/demo.overlay).

## Features

**Draw and place**
- Pen, pencil, marker and highlighter, each with its own width from hairline (0.25) to very thick (200). Strokes are smoothed as you draw.
- Lines, arrows, rectangles, ellipses and text.
- Polylines: click to add corners, straight or as a smooth curve. Click the first point again to close the shape and fill it.
- Images by drag and drop from Explorer, Ctrl+V (including screenshots) or Ctrl+I.
- Eleven flowchart shapes: empty node, process, decision, start or end, input or output, document, database, subprocess, manual input, connector and note.

**Connect everything**
- Every item has a port in the middle of each side. Drag from a port to another item to connect them.
- Curved, straight or right-angle (elbow) connections, with labels such as "yes" or "no".
- Press `/` to add an item from a searchable list. With a node selected, the new item appears beside it, already connected and ready to type in, so a flowchart can be built from the keyboard alone.
- Drag a connection into empty space and the same list opens there.

**Organise**
- Named, coloured groups that move together, have their own ports, nest, and collapse into a single node.
- Sketches: group strokes alone and they join into one item with no frame, which moves, resizes and connects as one.
- Layers you make yourself: name them, reorder them to choose what is in front, and move items between them.
- Snapping to other items' edges, centres and equal gaps while you move or resize, with guide lines. Hold Alt to skip it.
- Lock items, change opacity, and add comment threads to any item.
- A Graph panel listing connections, and a floating Overview map of the whole board.

**Style**
- Style presets, solid or gradient fills, solid, dashed or dotted lines, rounded corners, and sketch, glass and shadow effects.
- An eyedropper that takes a colour from any item, or from a pixel of an image.
- Dark, light, or follow Windows.

**History with branches**
- Every change is kept automatically. Deleting something starts a new branch, so the version before it is never lost.
- The History tab draws states and branches as a graph. Hover to see what changed, double-click to go back, rename branches, and select states or branches to delete them or keep only them.
- The whole history is saved inside the `.overlay` file.

**Share**
- Export the board, the selection or the visible area as PNG or JPG, at up to 4x, with an optional transparent background.

## Keyboard

| Keys | Action |
|---|---|
| V, H | Select, pan |
| P, B, K, Y, E | Pen, pencil, marker, highlighter, eraser |
| L, A, U, R, O, T | Line, arrow, polyline, rectangle, ellipse, text |
| N, C | Flowchart shape, connect |
| `/` | Add an item (connected to the selection) |
| Enter | Edit the selected item's label |
| Ctrl+G, Ctrl+Shift+G | Group, ungroup (strokes alone join into one sketch) |
| Ctrl+L | Lock or unlock |
| Ctrl+D, Delete | Duplicate, delete |
| Ctrl+Z, Ctrl+Y | Undo, redo |
| Ctrl+C, Ctrl+X, Ctrl+V | Copy, cut, paste (images and text too) |
| Ctrl+[, Ctrl+] | Send to back, bring to front |
| Arrow keys | Nudge (Shift for 10) |
| Ctrl+0, Shift+1 | Actual size, zoom to fit |
| M | Show or hide the Overview |
| Ctrl+E | Export an image |
| Ctrl+N, Ctrl+O, Ctrl+S, Ctrl+Shift+S | New, open, save, save as |

Pan with the right or middle mouse button, or hold Space and drag. The mouse wheel zooms at the pointer.

<details>
<summary><b>More detail on how things work</b></summary>

| Topic | How |
|---|---|
| Resize | Drag any of the eight handles around a selected item. Corner handles keep an image's shape (Shift frees it). |
| Corner radius | Drag the small diamond inside a selected shape's top-left corner, or use Rounded and Corner in Properties. |
| Labels | Double-click an item, or select it and press Enter. Images and sketches get a caption. Double-click empty board for new text. |
| Connections | Drag from a port to another item's port, or anywhere on it (the facing side is used). The Connect tool connects from anywhere on an item. Double-click a connection to label it. |
| Polylines | With the Polyline tool (U), click to add points; double-click or Enter ends the line, Esc drops it, Backspace takes back a point, Shift keeps 45° steps. Clicking the first point closes the shape. Straight or Curved is set in the Shapes panel and per line in Properties. |
| Sketches | Select only strokes and press Ctrl+G: they become one sketch with no frame. Ctrl+Shift+G splits it back into strokes. |
| Layers | New layer adds one above the current layer, which is where new and pasted items go. Click a layer to make it current, double-click to rename, use the arrows to reorder, Move to put the selected items on it. Deleting a layer deletes what is on it. |
| Groups | Ctrl+G groups the selection. Double-click the name to rename it; Properties sets its colour and fill. Click the arrow beside the name to collapse it; connections to what is inside then attach to the group. |
| Locking | Locked items can be selected but not moved, edited, erased or deleted. |
| Comments | Click the speech bubble at an item's top-right corner. Ctrl+Enter posts. |
| History | Click a dot or a branch name to select it, Ctrl-click to select more, double-click a dot to go back to it. Deleting a state joins the states before and after it. Deleting a branch removes its states, and branches that grew from it are kept. "Keep only" deletes everything except the selection and the states needed to reach it. These cannot be undone. |
| Panels | Select, pan, connect, text and images are in the strip on the left. Draw, Shapes, Flowchart, Properties, Layers, Graph and History are dockable tabs. View > Reset panel layout restores the default. |

</details>

## Build from source

You need [Rust](https://rustup.rs) (stable) on Windows.

```
git clone https://github.com/gowthamjeyabalan/overlay
cd overlay
cargo run --release
```

`cargo run --release -- path\to\board.overlay` opens a board directly. Use the release build: debug builds are much slower.

## How it is built

Overlay is written in Rust with [egui](https://github.com/emilk/egui) and [egui_dock](https://github.com/Adanos020/egui_dock), using the OpenGL renderer to stay light. Boards are `.overlay` files: JSON with images embedded, and history stored so unchanged items are written once.

| File | What it holds |
|---|---|
| `src/board.rs` | Items, connections, images, groups and the file format |
| `src/editor.rs` | Selection, clipboard, image import, comments, open and save |
| `src/history.rs` | States and branches, undo and redo |
| `src/canvas.rs` | Camera, the tools' pointer gestures, drawing |
| `src/shapes.rs` | Shape outlines, fills, dashes and effects |
| `src/snap.rs` | Snapping to edges, centres and equal gaps |
| `src/insert.rs` | The `/` list |
| `src/export.rs` | PNG and JPG export (a small software renderer) |
| `src/panels.rs`, `src/overview.rs` | Tool strip, Draw, Shapes, Flowchart, Properties, Layers, Graph, History and the Overview |
| `src/app.rs`, `src/theme.rs` | Window, menus, shortcuts, settings, and the theme |

The look follows the **Grapht** design system: a quiet grey interface with one blue accent, Inter type, and round strokes. Its tokens and notes are in [`design/`](design). The logo is an "O" left open and closed by a blue dot; the brand kit and its generator are in [`brand/`](brand).

## Licence

Overlay is released under the [MIT licence](LICENSE). The bundled Inter and Manrope fonts are under the [SIL Open Font License](assets/fonts/OFL.txt).
