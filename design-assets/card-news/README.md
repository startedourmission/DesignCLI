# Card News Backgrounds

Square PNG backgrounds for Korean card-news posts and social explainers.

All files are `1254x1254` PNG images with no embedded text or logo.

## Files

- `01-warm-paper.png` - warm paper, editorial and general purpose.
- `02-soft-gradient.png` - colorful gradient, launch and announcement cards.
- `03-dark-tech.png` - dark tech backdrop for SaaS/startup topics.
- `04-business-sage.png` - calm business/report tone.
- `05-warm-studio.png` - warm lifestyle or local business tone.
- `06-bold-editorial.png` - structured editorial blocks.
- `07-minimal-gray.png` - minimal gray for finance, legal, or education.
- `08-organic-green.png` - calm organic wash for health or lifestyle.
- `09-dark-editorial.png` - premium dark quote/report cards.
- `10-bright-explainer.png` - bright friendly explainer background.

## Example

```bash
dx --doc card.dxdoc doc create --w 1254 --h 1254
dx --doc card.dxdoc layer add --name bg --image design-assets/card-news/01-warm-paper.png
dx --doc card.dxdoc draw text 96 120 "카드뉴스 제목" --size 72 --color 24,28,32,255
dx --doc card.dxdoc draw text 100 240 "본문을 여기에 배치하세요" --size 34 --color 64,70,78,255
dx --doc card.dxdoc export png card.png
```
