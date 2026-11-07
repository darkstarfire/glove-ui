# Glove UI guide

*Em português: [GUIA.md](GUIA.md)*

Glove UI is the library Glove apps are made with: Wayland windows, controls in the Mac OS X
Tiger look (gel, pinstripes, brushed metal) drawn in code, menus in Glove's global menu bar,
Open/Save sheets, printing, drag and drop, and translations.

## Getting started

The quickest way is to copy the [`modelo/`](../modelo) folder (the app template) and rename it:

```sh
cp -r modelo ~/my-app && cd ~/my-app
cargo run --release
```

Or add the dependency to your own project:

```toml
[dependencies]
glove-ui = { git = "https://github.com/fabioaacarneiro/glove-ui", tag = "v0.2.1" }
```

You need Rust 1.85 or newer and the `xkbcommon` development files (`libxkbcommon-dev` on
Debian/Ubuntu). Apps run on Glove and on other Wayland desktops.

## How it works

The interface is **immediate mode**: the app keeps its state and, every frame, the `ui`
function describes the whole window from it. There are no button objects to create or delete: a
button is a call that draws and returns whether it was clicked.

```rust
use glove_ui::{run, tr, App, Ui, WindowOptions};

struct Hello { clicks: u32 }

impl App for Hello {
    fn ui(&mut self, ui: &mut Ui) {
        ui.heading("Hello, Glove!");
        if ui.button("Click here") {
            self.clicks += 1;
        }
        ui.label(&format!("You clicked {} times.", self.clicks));
    }
}

fn main() {
    let opts = WindowOptions { title: "Hello".into(), app_id: "org.example.Hello".into(), size: (320, 150), fixed: true, min_size: None };
    run(opts, Hello { clicks: 0 }).unwrap();
}
```

A frame is drawn only when something happens (a click, a key, a settings change): idle apps use
no CPU. To animate or refresh by itself (a clock), return an interval from `App::tick`.

## The `App` trait

| Method | What for |
|---|---|
| `ui(&mut self, ui)` | **Required.** Draws the window. |
| `menus(&self) -> Vec<Menu>` | The app's menus in Glove's menu bar. |
| `tick(&self) -> Option<Duration>` | Redraw at this rate even without events. |
| `on_tick(&mut self)` | Called on every `tick`. |
| `close_requested(&mut self) -> bool` | Return `false` to ask before closing (e.g. to save). |
| `config_changed(&mut self)` | The user changed the theme or another Glove preference. |

To close the window from code: `ui.close = true`.

## Layout

- Controls flow **top to bottom**; `ui.space(px)` adds room.
- `ui.horizontal(|ui| { … })` puts them side by side.
- `ui.form_row("Name:", label_width, |ui| { … })`: a right-aligned label with the control next
  to it, like the Preferences panes.
- `ui.area(Rect::new(x, y, w, h), |ui| { … })`: an exact position.
- `ui.available_width()`: the width that's left.
- `ui.tab_box(&mut tab, &["One", "Two"], height, |ui| { … })` and `ui.tabs(…)`: tabs.

## Controls

| Call | What it does |
|---|---|
| `label`, `heading`, `small`, `paragraph` | Text (normal, title, small, wrapped) |
| `button(t) -> bool`, `default_button(t)` | Buttons; the default one pulses blue and answers Enter |
| `checkbox(&mut bool, t)`, `radio_group(&mut usize, &[t])` | Check box and radio buttons |
| `popup(&mut usize, &[String], width)` | Pop-up menu |
| `slider(&mut f32, min, max, width)` | Slider |
| `text_field(&mut String, width)`, `search_field(…)`, `text_area(&mut TextBuffer, r)` | Text fields |
| `list(&mut Option<usize>, &[String], w, h)`, `table(…)` | List and multi-column table |
| `segmented_text(&[t], Some(i))` | Segmented control |
| `progress(Some(0.5), width)`, `spinner(size)` | Progress |
| `icon(…)`, `icon_tile(…)` | Glove-style icons (`glove_ui::paint::icons::Icon`) |
| `alert(kind, title, text, &[buttons]) -> Option<usize>` | Sheet alert; the last button is the default and "Cancel" answers Esc |

## Menus in Glove's menu bar

```rust
fn menus(&self) -> Vec<Menu> {
    vec![Menu::new("File", vec![
        MenuItem::new("new", "New", "Ctrl+N"),
        MenuItem::Separator,
        MenuItem::new("delete", "Delete", "Delete").enabled(self.something_selected),
    ])]
}

fn ui(&mut self, ui: &mut Ui) {
    for action in ui.menu_actions().to_vec() {
        match action.as_str() { "new" => { /* … */ } _ => {} }
    }
    // …
}
```

An item's shortcut does the same as choosing it. Menus show up when the app is opened by Glove
(the Dock, Quick Search, the Terminal…), which connects the app to the bar; outside Glove the
shortcuts still work.

## Open, save, print

`FileBrowser` shows Glove's Open/Save sheet (`ui.file_sheet(&mut fb)` returns the choice).
Printing (CUPS, Save PDF, Preview) is in the `print` module.

## Translations

Write texts in your source language inside `tr!` (`tr!("Open")`, `tr!("{} items", n)`) and put
translations in `locales/<language>.po`, in gettext format:

```po
msgid "Abrir"
msgstr "Open"
```

Point the `GLOVE_LOCALES` variable at that folder at build time (the template already does it
in `.cargo/config.toml`). The language follows the system `LANG`; `GLOVE_LANG=en` forces one.
Glove UI's own texts (Open, Save, Cancel…) come translated.

Glove's apps are written in Portuguese and ship `en.po`; an app written in English can ship a
`pt.po` the same way.

## Custom drawing

`ui.c` is the window canvas (`glove_ui::paint`): rounded rectangles, gradients, gel, metal,
shadows and text (`ui.fonts`). Theme colors are in `ui.theme`.

## Examples

```sh
cargo run --release -p glove-ui --example ola         # the minimum ("hello")
cargo run --release -p glove-ui --example formulario  # fields, options, alert ("form")
cargo run --release -p glove-ui --example lista       # list with search and menus ("list")
```

The 32 apps of [Glove](https://github.com/fabioaacarneiro/glove) (the `crates/` folder) are
bigger examples: Files, Preferences, Text Editor, Terminal, Mail…
