<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="brand/svg/strata-readme-header-dark.svg">
    <img alt="Strata. A whiteboard where every image, sketch and shape is a node." src="brand/svg/strata-readme-header-light.svg" width="720">
  </picture>
</p>

<p align="center">
  <a href="https://github.com/gowthamjeyabalan/strata/releases/latest"><img alt="Latest release" src="https://img.shields.io/github/v/release/gowthamjeyabalan/strata?color=0A84FF"></a>
  <img alt="Windows" src="https://img.shields.io/badge/platform-Windows-14161C">
  <img alt="Rust" src="https://img.shields.io/badge/made%20with-Rust-14161C">
  <a href="LICENSE"><img alt="MIT licence" src="https://img.shields.io/badge/licence-MIT-14161C"></a>
</p>

**Strata** is a fast, lightweight whiteboard for Windows. Drop in images, sketch with a pen, marker or highlighter, and build flowcharts. Every item on the board is a node with ports on all four sides, so an image, a scribble, a text box or a whole group can be connected to anything else. Connections show relationships; they do not carry data.

<p align="center"><img alt="Strata with two frames on the Plan page: a flowchart with yes and no branches and a sketch, and two linked note cards with a sticky note, a curve and a polygon. The Notes list with folders is on the left, the page buttons are below the board, and Properties is on the right." src="docs/screenshot.png" width="900"></p>

## Download

From the [latest release](https://github.com/gowthamjeyabalan/strata/releases/latest), either:

- **Install it:** `Strata-<version>.msix` adds Strata to the Start menu, opens `.strata` boards from Explorer, and uninstalls from Settings > Apps. The package is signed with a self-signed certificate, so each PC must trust it once before the first install: download `Strata.cer`, double-click it, choose **Install Certificate…**, **Local Machine**, then **Place all certificates in the following store** > **Trusted People**, and finish (this needs administrator rights). After that, double-click the `.msix` and choose Install. Updates install the same way.
- **Or run it portable:** `strata.exe` is a single 11 MB file with nothing to install.

Both need Windows 10 (1809) or 11. Try it on [`examples/demo.strata`](examples/demo.strata). Strata was called Overlay until 0.6.0; its settings and `.overlay` boards carry over.

## Features

**Draw and place**
- Pen, pencil, marker and highlighter, each with its own width from hairline (0.25) to very thick (1000). Strokes are smoothed as you draw. As in Photoshop, Shift+click draws a straight stroke from where the last one ended.
- Lines, arrows, rectangles, ellipses and text. While typing, a small bar sets bold, italic, underline, size, colour and alignment (text boxes and the text in shapes).
- Tables (J): drag a box (the arrow keys set the rows and columns while dragging) or click. Double-click a cell to type, Tab moves to the next one (past the last cell it adds a row), rows grow to fit their text, and dragging a border resizes a row or column. The + beside a selected table adds a column or a row; Properties adds and removes them next to the cell typed in last, switches the heading row, and evens out the sizes.
- Grid notes (G): a dotted area like a frame, full of dots; drag from dot to dot to draw straight lines on it. Click a drawing to select it (the lines joined to the one clicked; Alt+click for one line), fill a closed shape, and drag it along the dots, onto another grid, or off the grid, where it becomes a sketch (a closed shape becomes a filled outline). Straight lines dropped onto a grid snap to its dots and join it again.
- Polylines: click to add corners, straight or as a smooth curve. Click the first point again to close the shape and fill it.
- Images by drag and drop from Explorer, Ctrl+V (including screenshots) or Ctrl+I.
- Eleven flowchart shapes: empty node, process, decision, start or end, input or output, document, database, subprocess, manual input, connector and note.

**Connect everything**
- Every item has a port in the middle of each side. Drag from a port to another item to connect them.
- Curved, straight or right-angle (elbow) connections, solid, dashed or dotted, thin to heavy, in any colour, with captions such as "yes" or "no" in four sizes, placed anywhere along the line, with or without a box.
- Ctrl+drag an item to drag a copy of it; the copy is connected to the original where you drop it.
- Press `/` to add an item from a searchable list. With a node selected, the new item appears beside it, already connected and ready to type in, so a flowchart can be built from the keyboard alone.
- Drag a connection into empty space and the same list opens there.

**Organise**
- Named, coloured groups that move together, have their own ports, nest, and collapse into a single node.
- Sketches: group strokes alone and they join into one item with no frame, which moves, resizes and connects as one.
- Layers you make yourself: name them, reorder them to choose what is in front, open them to list their items (a group or sketch is one row that opens to show what is inside) and drag items between them, hide or lock them, and give them a colour that their items without a colour of their own take. Right-click a layer for everything else, such as selecting its items.
- Snapping to other items' edges, centres and equal gaps while you move or resize, with guide lines. Hold Alt to skip it.
- Lock items, change opacity, and add comment threads to any item.
- Rotate any item by the round handle above it (Shift keeps 15° steps), and crop images (double-click one).
- Drag left to right to select what is wholly inside the box, or right to left to select whatever it touches; connections are picked up too. Shift+click adds a connection.
- Several selected items scale together by the handles of the dashed box around them, or by Smaller and Larger in Properties; text grows with them.
- While drawing a box, the arrow keys split it into a grid: right and left for columns, up and down for rows.
- A radial menu: hold the right mouse button on an item, or hold it still on empty board, and move to an action, then let go. Add opens sections (Draw, Shapes, Flowchart, Note, Image) that each open a further ring of tools or items, Tools has select, pan and connect, Width and Colour set the line for the selection and for what you draw next, View has zoom to fit, the Overview and snapping, and Layer and Arrange open a second ring to pick from; Delete, Duplicate, Lock, Group and Properties act straight away. This is where the tools live; there is no toolbar.
- A floating Overview map of the whole board, and zoom to the selection (Shift+2).

**Notes**
- A Notes list on the left shows every note as a card with a preview, in folders you make. Double-click a note to open it in its own tab beside the board, to write, read, or both side by side.
- Markdown notes: headings, bold, italic, strikethrough, code, quotes, lists, tasks you can tick, tables, web links and `[[Name]]` links to other notes or items.
- Place a note on the board as a card: it can be resized, scrolls inside itself, and connects, groups and layers like any item. The same note can have several cards.
- Pasting markdown or dropping `.md` files onto the window makes notes.
- Export a note as a `.md` file from its tab, or every note (or the ones picked) at once with Export… in the Notes list: one file per note, folders as subfolders, and pictures in an `images` folder beside them.
- Pictures in notes: `![name](path)` on a line of its own, Insert image… in a note's tab, or Ctrl+V of a screenshot or of images copied on the board.
- Backlinks: a note's tab lists the notes that link to it and those that mention it. Typing `[[` suggests note names; a link to a note that does not exist yet shows paler, and following it (a click when reading, Ctrl+click when writing) makes that note and opens it. Dotted lines join the cards of linked notes on the board (View > Links between note cards).

**Plots**
- A Plot panel beside the board: type equations on the left and see them graphed on the right. `y = …` (or just an expression in x), `r = …` (polar, in θ), `(x(t), y(t))` or `x = …, y = …` (parametric), `x = 3` for a vertical line, and `a = 2` to set a constant. Any other letter is a constant with a slider. Click a curve's dot to hide it; drag the graph to move it and use the wheel to zoom.
- Place on board turns the plot into a live item that redraws from its equations; select it to change it in the panel, or double-click it to bring the panel forward.

**Pages, frames and presenting**
- Several pages in one board, as a row of buttons under the board that scrolls sideways (mouse wheel or the arrows) when there are many; ☰ lists them all, dragging a page button reorders the pages, and hovering one shows a small picture of that page. Each page has its own items; notes are shared by all pages.
- Frames mark named areas of a page (the Frame tool, F; Shift keeps 16:9) and carry what is inside them when moved. The Frames panel lists them in slide order.
- Present (F5) shows the frames full screen one by one; arrow keys or a click move on, Esc returns.

**Search everything**
- Ctrl+K finds notes, text, comments and connection labels on every page, and frames, layers and pages, and runs any command or tool by name.

**Dates**
- Every item records when it was made and each day it was changed, automatically.
- A month calendar under the notes, like the Windows one, marks the days something was made or changed. Click a day (Ctrl-click for more) to highlight that day's items on the board.

**Style**
- Style presets, solid or gradient fills, solid, dashed or dotted lines, rounded corners, and sketch, glass and shadow effects.
- An eyedropper that takes a colour from any item, or from a pixel of an image.
- Dark, light, or follow Windows.
- Windows 11 translucency: Acrylic, Mica or Mica Alt behind the window, as in Windows 11 apps. Settings > Appearance sets the material and how see-through the panels, the board and the menus are.

**Undo and saving**
- Ctrl+Z and Ctrl+Y undo and redo the last 100 changes while Strata is open.
- Autosave every few minutes (2 by default): to the board's own file, or only to a recovery copy. If Strata closes without saving, the next start offers the unsaved boards back.
- Save increment (Ctrl+Alt+S) saves the next numbered copy beside the board (plan-001, plan-002, …) and goes on in it. Backups when saving keeps the file as it was in a "<name> backups" folder, as many as you choose.

**Share**
- Export the board, the selection or the visible area as PNG or JPG, at up to 4x, with an optional transparent background.

## Keyboard

These are the defaults; every tool and most commands can be given other keys in Settings > Keyboard shortcuts.

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
| Ctrl+0, Shift+1, Shift+2 | Actual size, zoom to fit, zoom to the selection |
| M | Show or hide the Overview |
| Ctrl+E | Export an image |
| Ctrl+K | Command menu: search everything, run a command (Settings > Keyboard shortcuts changes the key) |
| Ctrl+, | Settings |
| F, G, J, F5 | Frame tool, grid note tool, table tool, present |
| Ctrl+B, Ctrl+I, Ctrl+U | Bold, italic, underline (while typing) |
| Ctrl+N, Ctrl+O, Ctrl+S, Ctrl+Shift+S, Ctrl+Alt+S | New, open, save, save as, save increment |

Pan with the middle mouse button, the right mouse button on empty board, or hold Space and drag. Holding the right button on an item opens the radial menu. The mouse wheel zooms at the pointer.

<details>
<summary><b>More detail on how things work</b></summary>

| Topic | How |
|---|---|
| Resize | Drag any of the eight handles around a selected item (text boxes too: their height follows the text). Corner handles keep an image's shape (Shift frees it). |
| Rotate | Drag the round handle above a selected item; Shift keeps 15° steps. Properties > Rotation sets an exact angle. |
| Crop | Double-click an image (or Properties > Crop), drag its handles, then press Enter or click elsewhere. Reset crop shows the whole picture again. |
| Corner radius | Drag the small diamond inside a selected shape's top-left corner, or use Rounded and Corner in Properties. |
| Labels | Double-click an item, or select it and press Enter. Images and sketches get a caption. Double-click empty board for new text. |
| Connections | Drag from a port to another item's port, or anywhere on it (the facing side is used). The Connect tool connects from anywhere on an item. Double-click a connection to label it. |
| Polylines | With the Polyline tool (U), click to add points; double-click or Enter ends the line, Esc drops it, Backspace takes back a point, Shift keeps 45° steps. Clicking the first point closes the shape. Straight or Curved is set in the Shapes panel and per line in Properties. |
| Sketches | Select only strokes and press Ctrl+G: they become one sketch with no frame. Ctrl+Shift+G splits it back into strokes. |
| Layers | New layer adds one above the current layer, which is where new and pasted items go. Click a layer to make it current, double-click to rename, use the arrows to reorder, Move to put the selected items on it. Deleting a layer deletes what is on it. |
| Groups | Ctrl+G groups the selection. Double-click the name to rename it; Properties sets its colour and fill. Click the arrow beside the name to collapse it; connections to what is inside then attach to the group. |
| Locking | Locked items can be selected but not moved, edited, erased or deleted. |
| Comments | Click the speech bubble at an item's top-right corner. Ctrl+Enter posts. |
| Radial menu | Hold the right button on an item (it is selected if it was not), or hold it still for a moment on empty board, and move outward to an action; let go to do it. Moving onto Add, Tools, Width, Layer, Colour, Arrange or View opens an outer ring (Add's sections open a third): move onto a colour, a layer or front/back and let go. Letting go in the middle, or pressing Esc, cancels. A quick right-click leaves the ring open to click in. |
| Notes | New note starts one (in the folder picked in the list) and opens it in a tab; Write, Split and Read switch between the editor and the preview. Double-click a note in the list, or its card on the board, to open it. Click notes in the list to pick them (Ctrl-click for more) and press Delete to delete them. New folder makes a folder; double-click its name to rename it, drag notes onto it to move them in, and click its arrow to fold it. Holding the right button on a note in the list opens its ring: open, place on board, move to a folder, delete. On the board, a selected note card's ring has a Note section with the same. Place on board adds a card; The mouse wheel over a card scrolls it (Ctrl+wheel zooms). Click a checkbox on a card to tick it, or a link to follow it. Deleting a note also deletes its cards. |
| Settings | File > Settings…, Ctrl+, or Ctrl+K "Settings". Sections for appearance (mode, interface size, translucency), the board, drawing, notes, the radial menu, undo, files, and keyboard shortcuts. In Keyboard shortcuts, click a shortcut and press the new keys; a shortcut belongs to one action, so giving it to another moves it. Esc, Delete, Enter, the arrows, copy and paste, and Space stay fixed. |
| Pages | Click a page button under the board to show it, double-click to rename it, + adds one, and the × beside the current page deletes it with what is on it (Ctrl+Z brings it back). Each page keeps its own view. |
| Grid notes | Drag with the grid note tool (G), or Add > Shapes. Drag from one dot to another to draw a line in the current drawing colour and width; the eraser takes lines off, and Properties changes the dot spacing or clears the lines. Like a frame, it is picked by its edge or name and carries what is on it. |
| Frames | Frames sit behind everything and are picked by their edge or name. Double-click the name to rename it. In the Frames panel, click a frame to go to it, use the arrows to change the slide order, and ▶ to present from it. |
| Calendar | Click a day in the calendar under the notes to highlight what was made or changed that day (Ctrl-click for more days, click again or Clear to stop). The arrows or the mouse wheel change the month. |
| Panels | Notes, Calendar, Properties, Layers and Frames are dockable tabs. The Window menu turns them on and off, saves the arrangement as a named workspace to come back to, and restores the default layout. |
| Start view | Shows when Strata opens (unless a board was opened from Explorer): new board, open, the demo, and recent boards. Settings > Files turns it off, and the last board then opens straight away. |
| Autosave | Settings > Files: Off, To the file (the board's file is saved; a board never saved gets a recovery copy), or Recovery copy (your file changes only when you save), and how many minutes between saves. Recovery copies are kept in %APPDATA%\Strata\recovery. Settings > Files also sets how many backups a save keeps. |
| Colours | In the radial menu, Colour offers the ready colours and the ones picked lately; More colours… opens a picker with a colour square, a hue strip, red, green, blue and hex values, and the colour before and after. |

</details>

## Build from source

You need [Rust](https://rustup.rs) (stable) on Windows.

```
git clone https://github.com/gowthamjeyabalan/strata
cd strata
cargo run --release
```

`cargo run --release -- path\to\board.strata` opens a board directly. Use the release build: debug builds are much slower.

The installer is an MSIX package built with the Windows SDK: `installer\make-cert.ps1` makes the signing certificate once, then `installer\build-msix.ps1` packs and signs `target\release\strata.exe` into `target\Strata-<version>.msix`.

## How it is built

Strata is written in Rust with [egui](https://github.com/emilk/egui) and [egui_dock](https://github.com/Adanos020/egui_dock), using the OpenGL renderer to stay light. Boards are `.strata` files: JSON with images embedded. Boards saved as `.overlay`, from when Strata was called Overlay, still open (and save as `.strata`).

| File | What it holds |
|---|---|
| `src/board.rs` | Items, connections, images, groups and the file format |
| `src/editor.rs` | Selection, clipboard, image import, comments, open and save |
| `src/history.rs` | Undo and redo |
| `src/canvas.rs` | Camera, the tools' pointer gestures, drawing |
| `src/shapes.rs` | Shape outlines, fills, dashes and effects |
| `src/snap.rs` | Snapping to edges, centres and equal gaps |
| `src/insert.rs` | The `/` list |
| `src/export.rs` | PNG and JPG export (a small software renderer) |
| `src/panels.rs`, `src/overview.rs` | Notes, Properties, Layers, Frames and the Overview |
| `src/radial.rs` | The radial menu: tools, new items and actions on the selection |
| `src/markdown.rs` | Reading and drawing notes' markdown |
| `src/search.rs` | The command menu: search everything (Ctrl+K) |
| `src/plot.rs` | Equations, the Plot panel and plot items |
| `src/autosave.rs` | Autosave, recovery copies, save increment and backups |
| `src/dates.rs`, `src/calendar.rs` | Dates on items and the calendar |
| `src/app.rs`, `src/theme.rs` | Window, menus, shortcuts, settings, and the theme |

The look follows the **GJ Design System**: a quiet grey interface with one blue accent, Inter type, and round strokes. Its tokens and notes are in [`design/`](design). The logo is one strip folded into three layers, an S for Strata, ended by a blue dot (a node); the brand kit and its generator are in [`brand/`](brand).

## Licence

Strata is released under the [MIT licence](LICENSE). The bundled Inter and Manrope fonts are under the [SIL Open Font License](assets/fonts/OFL.txt).
