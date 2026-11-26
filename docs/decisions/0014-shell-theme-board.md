# 0014: The shell, theming and boards

Date: 2026-11-24 · Status: accepted · Amended 2026-11-26 (file dialogs, keys, start-up, brand
loading) · General parts of Wordsmith Script decisions 0011, 0016, 0020 (preferences), 0022 (index
cards), 0028 (boards), 0036, 0037, 0038 and 0043

## Decision

- **`jcj-shell` draws the window around an app's document, the same in every app**: a quick-access
  row; a ribbon whose tabs and groups the app supplies, with groups that shrink and fold to fit
  (`ribbon_fit`, derived from WordCraft) and key tips (F10); side panels docked left, right or at
  the bottom, pinned or floating, with the layout saved; the File page (New, Open, Recent, Save,
  Save As, Export, Print, Preferences, Keys, About, and the app's own pages); the command palette;
  "save changes first?"; crash recovery; preferences; the user's own keys; auto-save's schedule;
  updates; generic icons drawn in code; an offscreen renderer for pictures and checks.
- **The app implements `App`** (its document view, commands, ribbon tabs, panels, File pages,
  About) and calls `Shell::ui` each frame from its own `eframe::App`, or lets the core's
  start-up do it (below, "Start-up"). `jcj-shell` re-exports
  `egui`, so apps draw with the version the core pins.
- **Every control runs a command by id** (Script 0043). The shell's own commands start with
  `shell.`; standard ids (`file.save`, `edit.undo`, ...) are the app's to provide, and a control
  for a command the app lacks is shown disabled. Commands queue and run after the frame is drawn.
- **Options default to Script's behavior** (`ShellOptions`): a change that alters what users see
  in other apps goes behind one (decision 0011, plan "Step 3"). Panels may float unpinned; pinned
  is the default.
- **File dialogs** (added 2026-11-26, core issue #28): the system's open, save and folder dialogs
  are asked for through the shell (`Shell::ask_file`) and answered as a command, with the path
  chosen in its parameters, on a later frame. They are craft-dialogs' through `jcj-upstream`,
  which never stops the window's event loop: a blocking dialog can crash an app on macOS. The
  shell draws inside a window it does not own, so the app's window keeps the dialogs
  (`files::FileDialogs`) beside its shell and shows them once a frame, handing itself over;
  that is the one call an app adds. They are not a field of the shell because a dialog on
  screen belongs to the thread that showed it, and `Shell` stays `Send`. `jcj-shell` names
  `raw-window-handle`, the traits a window offers, for that signature and nothing else.
- **Keys** (added 2026-11-26, core issue #29): the shell takes bound keys with Mod, Ctrl or Alt
  and function keys, and leaves every other key to the document. A document view asks the key
  map what a key runs (`KeyMap::command_for`), which holds the one rule for a shifted key with
  no command of its own: with a caret movement's key it extends the selection, and
  Shift+Backspace deletes back.
- **Start-up** (added 2026-11-26, core issue #46): three apps each wrote the code that opens the
  window, and each copy left out a step another had (the close that starts an installed update
  and its helper; a file whose name is not Unicode; the file dialogs; coming back to write a
  preference that waited). `jcj_shell::desktop` is that code once. The app gives its data
  (`Desktop`: name, id, app id, version, brand, where its releases are, a variable naming
  another settings folder) and what only it can do (`DesktopApp`: build itself, apply and
  report its preferences, open its files, say whether it may close, its last work), and
  `Desktop::main` is the whole of its `main`. The steps of a frame and of the end are in
  `desktop::Window`, which needs no window toolkit, so tests drive it. Opening the window needs
  eframe, which the shell does not otherwise use: it is behind the `window` feature, off unless
  an app asks, so an app that keeps its own start-up builds as before. The command line stays
  the app's (decision 0017): the start-up takes the files to open and reads no option, but for
  the one start that is not the user's, the updater's helper on Windows
  (`Desktop::finish_update`, core issue #36).
- **Brand loading** (added 2026-11-26, core issue #46): the brand is the app's, the loading is
  the core's. `jcj_theme::Brand` reads the tokens file (the neutral set when it does not read,
  so a window always opens), the app's own section of it and the window's icon;
  `jcj_theme::Mask` paints a mark from a white mask in the theme's ink; the WCAG contrast
  figures a brand's tests hold its colors to are beside them.
- **The Windows icon** is embedded by the app's build script. It stays a file of the app, the
  same in every app: the library it calls reads the program's name, company and copyright from
  `[package.metadata.winresource]` in the app's manifest, so the script holds no data but the
  icon's path, and the template carries it. A crate of the core for six lines around one call
  would cost every app a dependency and the core a crate.
- **Preferences** are JSON in the app's settings folder (WordCraft's settings location with the
  app's own name), written atomically; a damaged file is set aside and defaults used. The shell's
  preferences and the app's own type live side by side (`Prefs<A>`).
- **Updates** come from the app's own GitHub releases through craft-update (Script 0038); the app
  supplies its release identity (`ReleaseSource`). At our craft-update pin the bundle id cannot be
  overridden, so update paths use craft-update's naming with the app's id.
- **Theming** (`jcj-theme`): a product describes its look in `tokens.json` (`TOKENS.md`): a palette
  of named colors, 14 color roles in light, dark and extra-dark themes, fonts, type sizes, spacing
  and radii. The core ships only neutral tokens; brands live in the app repositories.
- **Fonts**: egui's built-in fonts are not used (Ubuntu Font Licence, decision 0001). Any font
  role the product's tokens leave open takes the core's default: Inter and JetBrains Mono from
  WordCraft's bundled OFL fonts. Apps supplying their own interface fonts must cover the symbols
  they show (on macOS, ⌘⇧⌥⌃ in shortcut labels).
- **Boards** (`jcj-board`): cards with title, body, color, label and links; a corkboard (rows,
  drag to reorder) and a free canvas (positioned cards, links, snapping, arrange). The data model
  is separate from the widgets; views report gestures as events the app applies or turns into its
  own commands.
- **Left in the apps**: document tabs, split views, writing statistics and sprints, start windows
  and template choosers (Script 0036) until a second app wants them; screenplay panels, tabs and
  icons.
