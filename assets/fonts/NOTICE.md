# Bundled font

This application bundles **Noto Sans SC Regular** (a static instance of Noto Sans SC Variable) from the Google Fonts repository:

- Source: https://github.com/google/fonts/tree/main/ofl/notosanssc
- License: SIL Open Font License 1.1, included as `OFL-NotoSansSC.txt`
- Font file: `NotoSansSC-Regular.ttf`

The font is embedded in the executable and used on all supported platforms. No system font is required.

The bundled font is instantiated at `wght=400` with FontTools. The upstream
variable font defaults to `wght=100`, which the renderer otherwise displays as
Thin at every text size. A static Regular instance keeps text weight consistent
on every platform without requiring variable-font support from the renderer.
All glyphs are retained; the SIL Open Font License and copyright notices remain
unchanged.

To reproduce from the upstream variable font:

```sh
fonttools varLib.instancer NotoSansSC-VF.ttf wght=400 --update-name-table --output NotoSansSC-Regular.ttf
```
