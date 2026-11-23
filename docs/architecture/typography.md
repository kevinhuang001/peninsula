# Typography contract (dark-only)

All extension text is specified semantically. The public declarative API exposes
`FontSize::Small`, `Body` (default), `Title`, `Heading`, or
`Relative(f32)`. A relative factor of `1.0` is exactly the normal Body
size; `0.8` is 80% of Body and `1.25` is 125%. Extensions must not
hard-code display-point sizes, query a monitor, or account for DPI.

## User setting

The Core-owned **通用 → 整体文字大小** selector has four choices:

| Choice | User factor |
|---|---:|
| 较小 | 0.90 |
| 标准 | 1.00 |
| 较大 | 1.12 |
| 特大 | 1.25 |

The setting is persisted as `text_scale` in `preferences.conf`. The
separate Capsule text-size override remains available. Neither setting
affects the accent or text color tokens.

## Resolving a size

```text
resolved logical points
  = semantic baseline
  × user text-scale factor
  × bounded monitor-width factor
```

The monitor-width factor uses the **monitor size in egui logical points**
instead of physical pixel resolution; it is proportional to the square
root of monitor width / 1440, clamped to `[0.95, 1.12]`. Missing or
invalid monitor dimensions fall back to `1.0`. The OS/egui already
handles physical DPI and Retina scaling.

`Typography::install` updates native egui text styles and stores the
same factor for declarative text measurement and rendering. It runs on
startup, on preference changes, and when the monitor changes. Do not
multiply by `pixels_per_point` again.

## Native widgets

Use `egui::TextStyle::Small/Body/Heading` in `RichText`, or
`Typography::font_id(ui.ctx(), Typography::BODY)` when an explicit
`FontId` is required for a painter. Native and declarative fonts
must use the same resolver.

The default semantic appearance is `Body + PrimaryText`. Colors are
independent semantic tokens resolved by the dark-only theme. Font size
and font color must not be derived from the accent seed.

The UI must remain usable at the minimum and maximum preset on
1024, 1440, 1920 and HiDPI monitor configurations, including input
fields, notifications, popups, labels and Capsule navigation. Size
measurements must use the same resolved font metrics as painting.
