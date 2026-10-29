---
status: accepted
---

# A chart can ask for a named height

ADR 0071 made a chart a box that fills its parent. An application built from the prelude alone cannot give that parent a height: it has `Space` and no pixel number, and the largest `Space` is `s6`, 24 px at the default text size. So every chart Atmos draws is 24 px tall, a sparkline is cramped, and the Wi-Fi page's QR code, some 37 modules across with its quiet zone, gets less than a pixel a module, which no phone reads. The QML window it is replacing draws its charts 96 px tall, its sparklines 28 and its QR code at 5 px a module.

**Decision.** `Chart::height(ChartHeight)` asks for one of three heights, and a chart that asks is that tall and as wide as its parent, whatever its parent's height; one that does not ask fills its parent as before. `Compact` is the foundation's `s8` (32 px at the default text size), for a sparkline; `Regular` is twice `s12` (96 px), for a chart; `Tall` is four times `s12` (192 px), for one that needs room, such as a code to scan. They are the component layer's `chart()` tokens (`heights`), from the foundation's spacing, so they grow with the user's text size as the spacing does and no application names a number.

**Considered options.**

- **A square chart, as tall as it is wide.** No new number, but its width is its parent's, and a page's column is hundreds of pixels wide: a QR code as tall as the page, or, in a parent the prelude can size, 24 px again.
- **A larger `Space`.** `Space` is the gap between things; a chart's height is not one, and every layout would gain a step it does not use.
- **One height.** A sparkline and a chart differ by three times, and a code to scan needs twice a chart.

This amends ADR 0071's "a box that fills its parent": it still does, unless it asks for a height. The rest of ADR 0071 stands.
