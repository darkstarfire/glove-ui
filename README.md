# Glove UI

The toolkit, drawing library, theme and shared settings that [Glove](https://github.com/fabioaacarneiro/glove)
apps are made with — a Mac OS X Tiger–style look (gel buttons, pinstripes, brushed metal,
soft shadows), drawn entirely in code, for Wayland apps written in Rust.

*Português: a biblioteca gráfica dos apps do Glove — veja o [guia](docs/GUIA.md).*

**Guide:** [docs/GUIDE.md](docs/GUIDE.md) · **Examples:** `examples/` in `crates/ui` · **App template:** [`modelo/`](modelo)

```sh
cargo run --release -p glove-ui --example ola
```

| Crate | What it is |
|---|---|
| `glove-ui` | Immediate-mode widgets, Wayland windows (layer of smithay-client-toolkit), menus for Glove's global menu bar, Open/Save sheets, printing, drag and drop |
| `glove-paint` | Drawing: scaled canvas, gel, metal, shadows, text, themes, and the Tiger-style icons |
| `glove-config` | The user's Glove settings (theme, scale, keyboard, sounds…), so every app follows them |
| `glove-i18n` | Translations: `tr!("Abrir")` with gettext `.po` catalogs (Portuguese source, English included) |

All artwork is original. Mac OS X is a style reference only: no Apple images, icons,
trademarks or product names.

## Translations

The library's own texts are in `locales/`. An app adds its catalogs by pointing the
`GLOVE_LOCALES` environment variable at its `locales/` folder at build time (for example in
`.cargo/config.toml`: `[env] GLOVE_LOCALES = { value = "locales", relative = true }`).

## License

GPL-3.0-or-later.
