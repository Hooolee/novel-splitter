# Research: Chart Library for Vue 3 + Tauri Desktop App (Radar Chart)

- **Query**: Chart library options for Vue 3 + TypeScript + Vite + Tailwind CSS desktop app (Tauri) needing radar chart support
- **Scope**: external (npm data, bundlephobia, docs)
- **Date**: 2026-05-11

## Findings

### Candidate Overview

| Library | Vue 3 | Radar Chart | Bundle (gzip) | Weekly DL | Maintenance |
|---|---|---|---|---|---|
| **ECharts** (vue-echarts) | Yes (vue-echarts 8.x) | Excellent | ~362KB + 3.5KB wrapper | 2.79M + 268K | Apache, very active (v6.0.0) |
| **Chart.js** (vue-chartjs) | Yes (vue-chartjs 5.x) | Built-in | ~68KB + <1KB wrapper | 11.3M + 835K | Very active (v4.5.1) |
| **ApexCharts** (vue-apexcharts) | **No** (wrapper requires Vue 2) | Yes (native) | ~140KB + 1.3KB wrapper | 1.73M + 80K | Active but no Vue 3 wrapper |

---

### 1. ECharts (via vue-echarts)

**Package Info**
- `echarts@6.0.0` (core) + `vue-echarts@8.0.1` (Vue wrapper)
- Weekly downloads: 2,788,862 (echarts) + 267,948 (vue-echarts)
- Bundle size: echarts ~361KB gzip, vue-echarts ~3.5KB gzip (very thin wrapper)
- Peer deps: `echarts ^6.0.0`, `vue ^3.3.0`

**Vue 3 Integration**
- Fully compatible. vue-echarts exports a `<v-chart>` component usable in `<script setup>`.
- Also provides `use` composable for programmatic control.
- TypeScript: Full type definitions for both echarts and vue-echarts.

**Radar Chart Quality**
- Most feature-rich radar chart implementation among candidates.
- Supports: multiple radar indicators, fill color, axis labels, multiple series overlay, tooltip, radar coordinate system with configurable center/radius.
- Extensive customization: indicator names, axis tick marks, split areas, legend, dark/light theme.
- Radar chart options are documented in depth at `echarts.apache.org/en/option.html#radar`.

**Known Tauri Gotchas**
- None. ECharts renders on Canvas which is fully supported in all Tauri WebViews (WKWebView on macOS, WebView2 on Windows, WebKitGTK on Linux).
- Resize handling works via `window.addEventListener('resize', ...)` or ResizeObserver, both available in Tauri.

**Pros**
- Most powerful radar chart API
- Excellent documentation and examples
- Built-in dark/light theme support (easy pairing with app's theme system)
- Responsive out of the box
- Large ecosystem of extensions and themes

**Cons**
- Largest bundle (362KB gzip). For desktop apps this is less of a concern (no network download), but still the heaviest.
- API is more complex; steeper learning curve.
- ECharts 6 may introduce breaking changes from ECharts 5 (released 2026, relatively new).

---

### 2. Chart.js (via vue-chartjs)

**Package Info**
- `chart.js@4.5.1` (core) + `vue-chartjs@5.3.3` (Vue wrapper)
- Weekly downloads: 11,265,096 (chart.js) + 834,683 (vue-chartjs)
- Bundle size: chart.js ~68KB gzip, vue-chartjs <1KB gzip (thin wrapper)
- Peer deps: `chart.js ^4.1.1`, `vue ^3.0.0-0 || ^2.7.0`

**Vue 3 Integration**
- Fully compatible. vue-chartjs exports typed components for each chart type: `<Radar>`, `<Line>`, `<Bar>`, etc.
- Works directly in `<script setup>` with reactive props.
- TypeScript: Good type definitions, though slightly less extensive than ECharts.

**Radar Chart Quality**
- Radar chart is built-in via `RadarController` -- no extra registration or plugins needed.
- Supports: multi-dataset overlay, fill styling, point styling, tooltips, legend.
- Customization options are adequate but more limited than ECharts:
  - Can set background color, border color, point styling per dataset.
  - No built-in radar-specific features like indicator names (uses linear axis labels instead).
  - No split-area shading customization (the polygon fill is per-dataset).
- Tree-shakeable: you can register only `RadarController`, `RadialLinearScale`, `PointElement`, `LineElement` to reduce bundle.

**Known Tauri Gotchas**
- None. Canvas-based rendering works in all Tauri WebViews.
- chart.js uses the canvas 2D API, fully supported.
- One caveat: chart.js does NOT auto-resize by default. You need to call `.resize()` or use ResizeObserver manually. In Tauri, the window can be resized by the user, so this needs to be handled.

**Pros**
- Smallest bundle (68KB gzip) among full-featured options
- Most popular charting library (11M weekly downloads)
- Simple, clean API -- easy to get started quickly
- Tree-shakeable for further size reduction
- Strong community support

**Cons**
- Radar chart customization is limited compared to ECharts
- No built-in themes (must manually configure colors)
- No auto-resize by default
- Radar axis configuration is less intuitive (uses radial linear scale)

---

### 3. ApexCharts (vue-apexcharts)

**Package Info**
- `apexcharts@4.x` (core) + `vue-apexcharts@1.7.0` (Vue wrapper)
- Weekly downloads: 1,725,387 (apexcharts) + 80,344 (vue-apexcharts)
- Bundle size: apexcharts ~140KB gzip, vue-apexcharts ~1.3KB gzip
- Peer deps: `vue ^2.5.17` (Vue 2 only!)

**Vue 3 Integration**
- **Not compatible**. vue-apexcharts v1.7.0 (latest) peer-requires `vue ^2.5.17` -- no Vue 3 support.
- Workaround: use apexcharts directly in Vue 3 without the wrapper. This means manual lifecycle management (init chart on mount, destroy on unmount, watch for prop changes).
- No official Vue 3 wrapper available as of 2026-05-11.

**Radar Chart Quality**
- Native radar/spider chart support with good customization.
- Supports: fill, stroke, markers, multiple series, data labels, y-axis labels on radial scale.
- Features like polygon/spider web styling are available.
- Documentation is good with examples.

**Known Tauri Gotchas**
- None known.

**Verdict: Not recommended** for this project due to lack of Vue 3 wrapper. Using apexcharts directly without Vue integration adds complexity for no benefit over Chart.js or ECharts which have first-class Vue 3 support.

---

### 4. D3.js (Custom Radar Implementation)

**Package Info**
- `d3@7.9.0`
- Weekly downloads: 12,221,641
- Bundle size: d3 ~92KB gzip (full library) - but can use only needed modules (d3-scale, d3-shape, d3-axis, etc.) for ~30-40KB gzip

**Vue 3 Integration**
- Vue-agnostic. Works with any framework.
- Integration pattern: bind an SVG ref to a `ref<SVGSVGElement>`, use D3 in `onMounted`.
- TypeScript: Good type definitions available.

**Radar Chart Quality**
- No built-in radar chart. Requires custom implementation using:
  - `d3-scale` for radial scales
  - `d3-shape` for polygon path generation
  - `d3-axis` for radial axes
- Implementation effort: medium-high (100-200 lines of D3 code for a basic radar chart)
- Full control over every visual aspect.

**Known Tauri Gotchas**
- None. D3 uses SVG, which is fully supported in Tauri WebViews.

**Verdict**: Feasible but overkill unless the app needs many other custom visualizations. Not recommended for a single radar chart.

---

## Comparison Matrix

| Criterion | ECharts (vue-echarts) | Chart.js (vue-chartjs) | ApexCharts |
|---|---|---|---|
| Vue 3 `<script setup>` | Excellent | Excellent | Not supported |
| Radar chart quality | Best-in-class | Good | Good |
| Bundle size (gzip) | 362KB | 68KB | 140KB |
| TS support | Excellent | Good | Good |
| Tree-shakeable | No (all-or-nothing) | Yes | No |
| Theming | Built-in (light/dark) | Manual | Manual |
| Docs quality | Excellent | Very good | Good |
| Tauri compatibility | No issues | No issues | No issues |

---

## Recommendation

**Use Chart.js (vue-chartjs).**

Rationale:

1. **Sufficient for the task.** The radar chart in this app will display novel tag/genre analysis scores (multi-dimensional categorical data with numerical values). Chart.js's radar chart handles this perfectly with multi-dataset support, fill styling, and tooltips.

2. **Smallest bundle (68KB gzip).** While desktop apps are less sensitive to bundle size, keeping the JS bundle lean improves cold-start time. 68KB vs 362KB is a meaningful difference even in Tauri.

3. **Simplest API.** The vue-chartjs API is intuitive: pass reactive `data` and `options` props to `<Radar>` component. Less boilerplate than ECharts.

4. **Most popular.** 11M weekly downloads vs 2.8M for ECharts means more community resources, more StackOverflow answers, and better long-term maintenance visibility.

5. **Tree-shakeable.** If only radar chart is needed, tree-shaking can reduce chart.js well below 68KB by registering only the necessary components. ECharts is all-or-nothing.

6. **Follow the KISS principle.** The app does not need ECharts's advanced features (3D, geo maps, complex interactions). Introducing a 362KB library for a single radar chart is over-engineering.

### Why not ECharts?

ECharts has a superior radar chart API with richer customization. However, the added complexity (362KB, more complex API) is not justified by the app's requirements. The radar chart will display ~5-10 categorical dimensions with numeric scores -- within Chart.js's radar capability.

### Why not ApexCharts?

No official Vue 3 wrapper. Using apexcharts directly in Vue 3 requires manual lifecycle management, increasing code complexity and maintenance burden.

### Why not D3.js?

Custom radar chart implementation requires significant development effort (100-200+ lines of D3 code) for what is a standard chart type. Not worth the investment for a single use case.

### Implementation Notes for Tauri

- chart.js does NOT auto-resize. Use a `ResizeObserver` on the chart container element to call `chart.resize()` when the Tauri window is resized.
- Ensure the chart canvas is not affected by Tauri's `window.setSize()` or maximize/restore events -- a ResizeObserver handles these transparently.
- No special CSP or security configuration is needed in Tauri for chart.js (it uses canvas 2D, no external resources).

### npm Installation

```bash
npm install chart.js vue-chartjs
```

### Basic Usage

```vue
<script setup lang="ts">
import { Radar } from 'vue-chartjs'
import {
  Chart as ChartJS,
  RadialLinearScale,
  PointElement,
  LineElement,
  Filler,
  Tooltip,
  Legend
} from 'chart.js'

ChartJS.register(
  RadialLinearScale,
  PointElement,
  LineElement,
  Filler,
  Tooltip,
  Legend
)

const chartData = {
  labels: ['世界观', '人物塑造', '剧情', '文笔', '创新性'],
  datasets: [
    {
      label: '小说 A',
      data: [80, 90, 70, 85, 75],
      backgroundColor: 'rgba(255, 99, 132, 0.2)',
      borderColor: 'rgba(255, 99, 132, 1)',
    }
  ]
}
</script>

<template>
  <Radar :data="chartData" />
</template>
```

---

## Related Specs

- `.trellis/spec/guides/stack.md` (if exists) -- tech stack decisions
- `package.json` -- Vue 3.5, Tauri 2

## Caveats

- ECharts 6.0.0 was released recently (2026). If the project later needs advanced visualization features that Chart.js cannot provide, migration to ECharts would be the fallback path.
- vue-chartjs has not had major releases in a while (v5.3.3 is stable, latest). Verify maintenance status before committing.
- If the radar chart requires very specific visual customization (custom grid shapes, complex multi-axis), Chart.js may not suffice and ECharts would be the better choice despite the larger bundle.
