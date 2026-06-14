# Design Assets

Reusable bitmap backgrounds for DesignCLI examples, templates, and smoke tests.

## Backgrounds

- `paper-warm-bg.png` - warm off-white paper texture for editorial posters and cards.
- `dark-grid-bg.png` - dark technical backdrop for UI mockups and product screenshots.
- `gradient-launch-bg.png` - colorful abstract launch/banner background.
- `studio-pedestal-bg.png` - neutral studio pedestal backdrop for product-style compositions.

All images are PNG files generated for reuse as base layers. Current dimensions are `1672x941`.

## Example

The workspace is infinite — `doc create` takes no size. Add a `frame` to set export bounds (e.g. matching the background image).

```bash
dx --doc demo.dxdoc doc create
dx --doc demo.dxdoc layer add --name bg --image design-assets/paper-warm-bg.png
dx --doc demo.dxdoc draw text 96 96 "DesignCLI" --size 72 --color 20,24,28,255
dx --doc demo.dxdoc frame add page 0 0 1672 941
dx --doc demo.dxdoc export png demo.png --frame page
```
