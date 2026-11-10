# Assets

Embedded into `Launcher.exe` at compile time by `slint-build`.

## Art

Copied unchanged from the approved kit at `.coord/pages/review/launcher-kit/`:
`logo-nvplus.png`, `art-wasteland.png`, `panel-small.png`, `plate-gold.png`, `plate-dark.png`,
`wb-plate.png`, `icon-terminal.png`, `slogan.png`, `slogan@2x.png`.

## Generated

Nine-slice does not scale a corner, so each source is pre-scaled to the width the frame is drawn
at. `panel-small.png` is 339 x 185 with a 34 px corner; the three files below resize it so that
corner lands on 22, 24 and 9 px.

```
magick panel-small.png -resize '219x120!' frame-22.png
magick panel-small.png -resize '239x131!' panel-24.png
magick panel-small.png -resize '90x49!'   bar-9.png
```

`stripe.png` is the loading bar's fill, the mockup's
`repeating-linear-gradient(115deg,#e9b545 0 14px,#1a120a 14px 19px)`, as a 21 x 45 tile that
repeats without a seam at that angle:

```
magick -size 21x45 xc:black -fx "mod(i*0.906308+j*0.422618,19) < 14 ? 1 : 0" -threshold 50% mask.png
magick -size 21x45 xc:"#1a120a" mask.png -compose Over -composite stripe.png
magick -size 21x45 xc:"#e9b545" mask.png -alpha off -compose CopyOpacity -composite gold.png
magick -size 21x45 xc:"#1a120a" gold.png -compose Over -composite stripe.png
```

## Fonts

`fonts/` holds Barlow Condensed Regular, SemiBold and Bold, and their licence in `OFL-Barlow.txt`
(SIL Open Font License 1.1). The slogan is a finished picture, so no Yellowtail font is shipped.
