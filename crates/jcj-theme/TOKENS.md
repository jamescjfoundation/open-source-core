# `tokens.json`: a product's design tokens

Each app keeps its look in its own repository as a `tokens.json` file and hands it to
`jcj_theme::Tokens::from_json`. The core holds the code that turns tokens into a theme, never a
product's colors, fonts or logo. `tokens/neutral.json` in this crate is the neutral set
(`Tokens::default()`); copy it to start a product's file.

## Shape

```json
{
  "name": "my-product",
  "palette": { "ink": "#1C2340", "paper": "#FAFAF7" },
  "light":      { "<role>": "<color>", ... },
  "dark":       { "<role>": "<color>", ... },
  "extra_dark": { "<role>": "<color>", ... },
  "fonts":   { "regular": "<face>", "medium": "<face>", "semibold": "<face>", "mono": "<face>" },
  "type":    { "body": 12.5, "caption": 11.0, "title": 13.5, "heading": 26.0 },
  "spacing": { "item_x": 8, "item_y": 3, "button_x": 6, "button_y": 3, "panel_margin": 8, "indent": 18 },
  "radius":  { "control": 4, "window": 8, "card": 6, "chip": 9 }
}
```

Every key is optional. Keys the core does not know are ignored, so a newer file still reads in
an older core.

### `palette`

Named brand colors (`"ink"`, `"paper"`, ...). A theme's role may name one instead of writing hex,
so a brand's few colors are written once and the themes say where each is used.

### Colors: `light`, `dark`, `extra_dark`

A color is `#RRGGBB`, `#RRGGBBAA` (straight alpha) or a `palette` name. The roles:

| Role | Used for |
| --- | --- |
| `canvas` | Behind the document's pages |
| `surface` | Ribbon, panels, status bar |
| `surface_high` | Quick-access row, tab strip, windows, cards on a surface |
| `input` | Fields and list rows |
| `border` | Lines between parts of the window; around fields and cards |
| `text` | Text |
| `text_dim` | Secondary text: captions, hints, group titles |
| `accent` | Current tab, controls that are on, links, focus |
| `accent_soft` | Background of controls that are on, and of the selection |
| `hover` | Hovered controls |
| `danger` | Problems and destructive actions |
| `brand` | The File page's column and the File button: the product's own color |
| `on_brand` | Text and icons on `brand` |
| `highlight` | The small mark on the current item (the current File page entry) |

Fallbacks: a role left out of `light` takes the neutral set's light color; `dark` left out
entirely is the neutral dark theme, and a role left out of it is the neutral dark color;
`extra_dark` (or any role of it) left out is the product's own `dark`.

### `fonts`

Names of font faces. The app supplies each face's bytes under the same name
(`jcj_theme::FontFace`), and the theme installs them. `medium` and `semibold` are the heavier
weights used for tab names, button labels and titles; a weight the app does not supply falls back
to `regular`, then to egui's built-in fonts. Ship only fonts whose licenses allow it (OFL-1.1 is
common) and credit them in the app.

### `type`, `spacing`, `radius`

Points. Values are kept within reason when read, so a damaged or hand-edited file cannot ask for
something absurd: `body` and `caption` 6 to 48, `title` 6 to 72, `heading` 6 to 96; spacing 0 to
64 (`indent` 0 to 96); radii 0 to 32.

## What the theme does with it

- `Tokens::theme(mode)` picks one `Theme` (`Mode::Light`, `Dark` or `ExtraDark`).
- `jcj_theme::apply(ctx, &theme)` sets egui's visuals and spacing for it and keeps it for
  `Theme::get(ctx)`, which the shell's and board's widgets read.
- `jcj_theme::install_fonts(ctx, &tokens.fonts, &faces)` installs the fonts.

A product's logo and app icon are not tokens: the app paints its own mark where the shell asks for
it.
