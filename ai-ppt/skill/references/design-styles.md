# Design Style System for json2pptx

> **中文版**：完整翻译见 [docs/设计风格指南.md](docs/设计风格指南.md)（英文原文与中文翻译双份维护，改一处请同步另一处）。

A comprehensive design style system merged from 10+ GitHub projects:
- AAAAAAAJ/slides (19 styles)
- oh-my-ppt / arcsin1 (70+ styles)
- ppt-from-anything / Benioh (4 structured style templates)
- auto-ppt-engine / lijunliu-gh (6 themes)
- slides-ai-plugin / proyecto26 (12 presets)
- ppt-master (consulting/MBB)
- NanoBanana-PPT-Skills / op7418
- GordenPPTSkill (17 Chinese templates)
- banana-PPT / zhangzhengrant
- powerpoint-skill / Noi1r (5 academic themes)

---

## Quick Reference: 12 Core Styles

| # | Style | Vibe | Best For |
|---|-------|------|----------|
| 1 | **Business Professional** | Clean, structured, corporate | 汇报、商务、数据展示 |
| 2 | **Consulting (MBB)** | High-density, insight-driven | 战略咨询、行业分析 |
| 3 | **Minimalist Clean** | White space, less is more | 发布会、产品介绍 |
| 4 | **Tech/Neon** | Dark mode, glow accents | 科技、AI、创新产品 |
| 5 | **Academic** | Formal, formula-friendly | 论文答辩、教学、研究 |
| 6 | **Swiss International** | Grid, geometry, bold color | 设计作品集、创意提案 |
| 7 | **Bento Grid** | Modular cards, Apple-like | SaaS dashboards, 产品展示 |
| 8 | **Glassmorphism** | Frosted glass, depth, blur | 现代界面、AI 产品 |
| 9 | **Bauhaus** | Primary colors, geometric | 创意、文化、艺术 |
| 10 | **Japanese Minimal** | Restrained, refined, wabi-sabi | 文化、生活方式、品牌 |
| 11 | **Cinematic Photo** | Full-bleed images, storytelling | 旅行、品牌叙事、案例 |
| 12 | **Dark Executive** | Dark background, gold/white text | 年度晚宴、高端品牌 |

---

## 1. Business Professional (商务专业)

**源自:** auto-ppt-engine (business-clean, corporate-blue), oh-my-ppt (极简白), slides-ai-plugin

```
Professional clean presentation slide, white/light gray background,
structured layout with clear visual hierarchy,
bold title at top left, subtitle below, body content in bottom 2/3 area,
corporate color palette: navy blue #1A365D, steel blue #4472C4,
medium blue #5B9BD5, light blue #BDD7EE, accent orange #ED7D31,
clean sans-serif typography, generous margins, data charts with consistent styling,
footer with page number and company logo,
Professional business presentation design, 16:9
```

**配色:**
| Role | Hex | Usage |
|------|-----|-------|
| Primary | `1A365D` | Titles, headers, key shapes |
| Secondary | `4472C4` | Accent shapes, chart series |
| Light | `BDD7EE` | Table headers, highlights |
| Accent | `ED7D31` | CTAs, emphasis elements |
| Background | `F2F2F2` | Slide background |
| Text | `333333` | Body text |

**字体:** major_font=`Calibri Light`, minor_font=`Calibri`
**字号层级:** Title 40pt, Subtitle 24pt, Body 16pt, Caption 12pt
**版式:** 顶部标题栏 + 下方内容区, 标准卡片布局, 数据图表居中

**图片生成提示词:**
| Type | Prompt |
|------|--------|
| Background | `Abstract geometric pattern, light gray and navy blue, subtle grid lines, professional corporate texture, 16:9 background` |
| Hero | `Modern office building interior, clean lines, blue hour lighting, professional atmosphere, ultra wide angle, high resolution` |
| Illustration | `Flat vector illustration of team collaboration, blue and white color scheme, minimalist style, business people around table` |
| Data Visual | `Abstract data flow visualization, blue gradient, glowing nodes, network graph connections, business analytics concept` |
| Icon Set | `Line art business icons, consistent style, thin 2px stroke, navy blue color, grid of 4 icons on clean white background` |
| Chart BG | `Clean bar chart minimal design, business analytics, soft blue gradient background, isometric view, 3D rendered` |

---

## 2. Consulting (MBB/咨询级)

**源自:** ppt-master (Executor_Consultant_Top), GordenPPTSkill, auto-ppt-engine

```
Premium consulting presentation slide, McKinsey-style aesthetic,
structured framework layout, executive summary at top,
insight-driven headline (one clear takeaway per slide),
high information density with clear hierarchy,
corporate palette: dark navy #0B1D3A, accent teal #008B8B,
gold accent #C4964E, warm gray #F5F0EB, dark gray #2D2D2D,
thin elegant lines, MECE framework diagrams, data tables,
professional serif/sans-serif mix, black and white with single accent color,
Consulting presentation design, 16:9
```

**配色:**
| Role | Hex | Usage |
|------|-----|-------|
| Background | `F5F0EB` | Warm off-white |
| Dark | `0B1D3A` | Heavy titles |
| Accent1 | `008B8B` | Data highlights |
| Accent2 | `C4964E` | Select emphasis |
| Text | `2D2D2D` | Body copy |

**核心理念:** 一页一个核心洞察, MECE 结构, 数据驱动, 结论先行
**布局框架:** 标题(insight) → 副标题 → 主体(图表/框架/表格) → 底部注释

**图片生成提示词:**
| Type | Prompt |
|------|--------|
| Background | `Warm off-white paper texture, subtle grain, minimalist corporate, elegant subtle pattern, consulting report style` |
| Hero | `Executive boardroom top down view, dark navy tones, polished wood table, natural lighting, professional corporate` |
| Illustration | `2x2 matrix framework diagram, teal and navy color scheme, MECE principle, strategic consulting visual, clean lines` |
| Data Visual | `Premium data dashboard, dark navy background, gold and teal accent charts, KPI metrics, executive summary style` |
| Framework | `Strategic framework diagram, pyramid structure, thin elegant lines, dark navy and gold, consulting methodology visual` |
| Portrait | `Professional executive portrait, corporate headshot style, navy suit, warm lighting, clean gray background` |

---

## 3. Minimalist Clean (极简主义)

**源自:** AAAAAAAJ/slides (Minimalist Clean), oh-my-ppt (极简白/日式简约), auto-ppt-engine (minimal)

```
Minimalist clean design slide, White or near-white background,
generous whitespace, centered title text, subtitle below,
limited color palette: off-white #FAFAFA, dark gray #333333,
medium gray #666666, single accent color #4472C4,
thin elegant lines, sans-serif typography (Inter/Helvetica),
one visual element per slide, high contrast, no decoration,
Apple Keynote style, Professional presentation design, 16:9
```

**配色:**
| Role | Hex | Usage |
|------|-----|-------|
| Background | `FFFFFF` or `FAFAFA` | Pure white |
| Primary Text | `1A1A1A` | Titles |
| Secondary | `666666` | Subtitles, metadata |
| Accent | `4472C4` | Minimal highlight |
| Divider | `E0E0E0` | Separation lines |

**黄金法则:** 留白 ≥ 40% 页面面积, 单页不超过 1 个视觉焦点

**图片生成提示词:**
| Type | Prompt |
|------|--------|
| Background | `Pure white minimal surface, subtle shadow gradient corner, clean studio lighting, product photography backdrop` |
| Hero | `Single elegant product on white pedestal, studio lighting, soft gray gradient background, Apple aesthetic, minimalist` |
| Illustration | `Minimal line art, single continuous line drawing, black on white, elegant silhouette, minimalist art style` |
| Icon Set | `Minimalist black glyph icons, 24x24 grid, consistent round stroke, no fill, on transparent background, UI icon set` |
| Photo | `High key photography, white background, soft even lighting, clean composition, minimal elements, editorial style` |

---

## 4. Tech/Neon (科技赛博)

**源自:** AAAAAAAJ/slides (Cyberpunk Neon), oh-my-ppt (赛博霓虹), auto-ppt-engine (tech)

```
Cyberpunk/tech presentation slide, Dark charcoal/black background,
title text with neon glow effects, subtitle below,
neon color palette: magenta #FF00FF, cyan #00FFFF,
neon yellow #FFFF00, electric blue #4DA3FF, violet #8B5CFF,
tech grid patterns, circuit board decorations,
holographic data panels, glow effects, glass panels,
futuristic UI elements, modern sans-serif typography,
Digital tech presentation design, 16:9
```

**配色:**
| Role | Hex | Usage |
|------|-----|-------|
| Background | `0D1117` or `1A1A2E` | Deep dark |
| Primary | `00FFFF` | Cyan glow, key text |
| Secondary | `FF00FF` | Magenta accent |
| Tertiary | `4DA3FF` | Electric blue |
| Highlight | `FFFF00` | Yellow CTA |
| Text | `E0E0E0` | Body text |

**特效:** 霓虹发光 (glow)、网格线条 (grid)、全息面板 (holographic)
**渐变推荐:** 深蓝→紫过渡, cyan→magenta 线性渐变

**图片生成提示词:**
| Type | Prompt |
|------|--------|
| Background | `Dark grid matrix background, cyberpunk city silhouette, neon cyan and magenta lights, rain reflections, digital rain` |
| Hero | `Futuristic AI chip with neon glow, circuit board pattern, dark background, cyan and magenta lighting, macro photography` |
| Illustration | `Holographic UI interface floating in dark space, neon cyan wireframe, futuristic technology concept art, isometric` |
| Data Visual | `Cyberpunk data dashboard, holographic charts floating in dark space, neon cyan grid lines, futuristic analytics` |
| Tech Element | `Abstract 3D wireframe sphere, neon cyan outline, dark background, glowing nodes, futuristic network visualization` |
| Icon Set | `Futuristic neon line icons, glowing cyan outlines, dark background, technology UI elements, consistent grid layout` |

---

## 5. Academic (学术论文)

**源自:** Noi1r/powerpoint-skill (5 Academic themes), auto-ppt-engine

```
Academic presentation slide, clean formal layout, white background,
title at top with horizontal rule divider, body content below,
OMML math formulas typeset natively, theorem-proof blocks,
formal palette: navy #1B3A5C, dark red #8B0000, dark green #2E7D32,
steel gray #546E7A, beige #FFF8E7,
serif font for body (Times New Roman/Cambria), sans-serif for titles,
addBullets, addCard, addTable, addFormula helpers,
bibliography citations, academic presentation design, 16:9
```

**配色:**
| Role | Hex | Usage |
|------|-----|-------|
| Background | `FFFFFF` | White |
| Primary | `1B3A5C` | Titles, headers |
| Accent | `8B0000` | Theorem labels, emphasis |
| Secondary | `2E7D32` | Examples, positive results |
| Code | `546E7A` | Code blocks |
| Formula | `333333` | Math text |

**Slide 类型:** Definition, Theorem-Proof, Comparison, Example, Insight
**特殊能力:** OMML 原生数学公式, 600DPI LaTeX 渲染, Graphviz/Mermaid 图表

**图片生成提示词:**
| Type | Prompt |
|------|--------|
| Background | `Light beige paper texture, subtle mathematical grid overlay, academic journal style, clean and formal background` |
| Diagram | `Scientific diagram, clean vector style, navy and dark red color scheme, labeled axes, formal academic chart` |
| Formula Visual | `Mathematical formula visualization, elegant typography, 3D coordinate system, scientific research concept, clean design` |
| Illustration | `Scientific illustration, biological or technical diagram, precise lines, labeled parts, educational textbook style` |
| Data Chart | `Academic research data chart, bar graph with error bars, clean axes, serif labels, formal scientific publication style` |
| Figure | `Microscope or laboratory equipment photography, shallow depth of field, research lab lighting, scientific backdrop` |

---

## 6. Swiss International (瑞士国际主义)

**源自:** AAAAAAAJ/slides (Swiss International), oh-my-ppt (包豪斯)

```
Swiss international style slide, brutalist graphic design influence,
Light gray or white background, bold title text,
asymmetric layout, diagonal elements,
high saturation palette: blue #007AFF, green #00994D,
yellow #FFF066, purple #9966FF, pink #FF3399, orange #FF8800,
Helvetica/Univers font, geometric blocks, photo-montage,
grid-based composition, asymmetric balance,
Swiss design presentation, 16:9
```

**核心原则:** 网格系统驱动, 非对称平衡, 无装饰, 纯粹排版
**版式特征:** 大号无衬线字体, 色块分区, 图片与文字块分离

**图片生成提示词:**
| Type | Prompt |
|------|--------|
| Background | `White background with subtle grid overlay, Swiss design style, clean and minimal, architectural grid pattern` |
| Photo Montage | `Bold color block photo composition, blue and orange contrast, geometric overlays, Swiss poster style, editorial` |
| Shape Element | `Abstract geometric composition, circles and rectangles in primary colors, Bauhaus influence, bold color blocking` |
| Typography Art | `Large bold typography as visual element, Helvetica font, black and white contrast, Swiss poster design, minimal text` |
| Collage | `Photo collage with geometric cutouts, asymmetric layout, bold colors, Swiss modernist style, dynamic composition` |

---

## 7. Bento Grid (便当盒网格)

**源自:** AAAAAAAJ/slides (Bento Grid), oh-my-ppt

```
Bento Grid style slide, modular rounded-rectangle card layout,
asymmetric but tightly aligned grid, bold title in hero block,
3-5 key statistics in smaller cards, product screenshot module,
premium Apple-style aesthetic, soft neutral background,
crisp card boundaries, subtle shadows,
colors: off-white #F6F4EF, graphite #1F2937,
cobalt blue #4F7CFF, mint #6FD3C0, soft orange #FFB36A,
highly structured information hierarchy,
Modern UI presentation design, 16:9
```

**布局特点:** 自适应网格, 卡片有明确主次, 圆角矩形 (roundRect)
**适用场景:** 产品发布会、SaaS 面板、功能概览

**图片生成提示词:**
| Type | Prompt |
|------|--------|
| Background | `Soft warm neutral gradient background, off-white to light beige, subtle organic shapes, premium Apple keynote style` |
| Product Shot | `Apple-style product photography, single device floating on clean background, soft dramatic lighting, premium minimalist` |
| Module | `Rounded card UI mockup, off-white background, subtle shadow, clean layout, bento grid composition, modern interface` |
| Icon Set | `Premium rounded icon set, soft color fills, consistent rounded square shape, modern iOS style, flat design` |
| Dashboard | `Modern analytics dashboard mockup, card-based layout with charts, bento grid composition, Apple design aesthetic` |

---

## 8. Glassmorphism (毛玻璃拟态)

**源自:** AAAAAAAJ/slides (Light/Dark Glassmorphism, Aurora UI)

### 8a. Light Glassmorphism
```
Light glassmorphism slide, airy translucent interface,
soft frosted glass panels over vibrant blurred gradient background,
thin luminous white borders, subtle refraction, layered depth,
colors: icy white #F7FBFF, sky blue #7CC7FF,
aqua #7FE7DD, lavender #C6B5FF, soft coral #FFB7C5,
clean modern sans-serif, macOS Big Sur inspired, 16:9
```

### 8b. Dark Glassmorphism
```
Dark glassmorphism slide, premium AI control-plane aesthetic,
deep obsidian background with blurred aurora glow,
smoked frosted glass panels, translucent dashboard modules,
thin luminous white borders, cyan and violet highlights,
deep #0A0A1A background, electric blue #4DA3FF,
cyan #66F5FF, violet #8B5CFF, magenta #FF5FD2,
enterprise SaaS design, 16:9
```

**毛玻璃要素:** backdrop-filter blur, 半透明面板, 亮色边框, 层叠深度
**背景:** 渐变色模糊 (mesh gradient aurora) + 微颗粒纹理防条纹

**图片生成提示词:**
| Type | Prompt |
|------|--------|
| Background (Light) | `Soft vibrant pastel gradient background, pink to blue to purple, dreamy atmospheric colors, smooth blur, airy texture` |
| Background (Dark) | `Deep obsidian to violet gradient, aurora borealis glow, dark and moody, premium dark UI background, subtle grain` |
| Glass Panel | `Frosted glass panel on gradient background, translucent effect, blurred backdrop visible through glass, modern UI` |
| UI Element | `Floating translucent card with glassmorphism effect, white borders, soft shadow, layered depth, modern interface design` |
| Gradient Art | `Abstract fluid gradient art, vibrant colors flowing together, smooth transitions, ethereal aesthetic, digital art` |
| Illumination | `Luminous glowing orb, soft light bloom, cyan and purple neon glow, floating in dark space, magical lighting` |

---

## 9. Bauhaus / Constructivism (包豪斯/构成主义)

**源自:** oh-my-ppt (包豪斯), AAAAAAAJ/slides (Neo-Brutalism)

```
Bauhaus/constructivist slide, geometric abstraction,
primary color blocks: red #E02A2A, blue #0057B8, yellow #F9D03F,
black bold geometric typography, white background,
asymmetric composition, circle/square/rectangle shapes,
horizontal and vertical lines as dividers,
photographic elements in black and white,
Bauhaus design aesthetic, 16:9
```

**造型语言:** 基础几何形 (圆/方/三角), 原色, 粗轮廓线, 无装饰
**排版:** 无衬线字体, 全大写标题, 强烈的字体大小对比

**图片生成提示词:**
| Type | Prompt |
|------|--------|
| Background | `White background with bold geometric shapes, red circle and blue square, primary color blocks, Bauhaus poster style` |
| Composition | `Abstract geometric composition, intersecting circles and rectangles, primary colors red yellow blue, constructivist art` |
| Typography | `Bold typographic poster, black text on white, geometric layout, Bauhaus design, asymmetric grid, modern art style` |
| Collage | `Bauhaus photomontage, black and white photography with primary color overlays, geometric cutouts, avant-garde` |
| Pattern | `Geometric pattern repeat, circles squares and triangles, primary color palette, Bauhaus textile design style` |

---

## 10. Japanese Minimal (日式简约)

**源自:** oh-my-ppt (日式简约), AAAAAAAJ/slides

```
Japanese minimal slide, restrained elegant aesthetic,
warm off-white or light washi paper texture background,
subtle earth tones: charcoal #2D2D2D, warm gray #8C8C8C,
vermilion accent #BC3E3E, indigo #264E70, bamboo green #6B8E23,
generous negative space, asymmetric layout,
horizontal rule separators, refined typography (Noto Sans JP/Shippori Mincho),
zen-like simplicity, wabi-sabi influence,
Japanese design aesthetic, 16:9
```

**核心理念:** 间 (Ma) - 留白的价值, 不对称是自然的, 克制即优雅
**字体建议:** Noto Sans JP (无衬线) 或 Shippori Mincho (衬线)

**图片生成提示词:**
| Type | Prompt |
|------|--------|
| Background | `Warm washi paper texture, subtle fibers visible, off-white natural tone, traditional Japanese paper, zen minimal` |
| Nature | `Japanese garden photograph, zen rock garden, raked sand pattern, maple branch, soft natural lighting, peaceful mood` |
| Illustration | `Sumi-e ink wash painting, bamboo branch, black ink on white rice paper, flowing brush strokes, traditional Japanese art` |
| Pattern | `Traditional Japanese pattern, seamless repeat, indigo blue and white, geometric wave pattern, textile design` |
| Calligraphy | `Japanese calligraphy single character, bold black ink stroke on white paper, traditional brush technique, zen aesthetic` |
| Still Life | `Wabi-sabi still life, ceramic vessel on wooden surface, natural lighting, earthy tones, minimalist Japanese arrangement` |

---

## 11. Cinematic Photo Impact (大图叙事)

**源自:** ppt-from-anything (cinematic-photo-impact), oh-my-ppt

```
Cinematic photo slide, full-bleed background image,
large dramatic photography as primary visual,
bold title overlaid on image (bottom-left or center),
semi-transparent gradient overlay for text readability,
subtitle in smaller weight, date/location in caption bar,
colors extracted from photo dominant hue,
dark gradient overlay #00000088 to #00000000,
serif or sans-serif mixed typography,
National Geographic style storytelling, 16:9
```

**布局公式:** 全出血图片 → 渐变蒙层 → 文字叠放
**图片要求:** 高分辨率, 16:9 适配, 主体在安全区内
**文字区:** 底部左对齐或居中对齐, 白字为主

**图片生成提示词:**
| Type | Prompt |
|------|--------|
| Hero (Landscape) | `Dramatic wide-angle landscape photography, golden hour light, misty mountains at sunrise, cinematic composition, National Geographic style` |
| Hero (Urban) | `Aerial cityscape photography, blue hour, city lights reflecting on water, dramatic clouds, ultra wide angle, cinematic mood` |
| Texture | `Dark moody texture overlay, grunge grain, cinematic film grain, subtle vignette, dark atmospheric background` |
| Portrait | `Cinematic environmental portrait, dramatic side lighting, shallow depth of field, emotional storytelling, film still quality` |
| Detail Shot | `Macro detail photography, natural textures, shallow DOF, dramatic lighting, abstract natural patterns, editorial style` |
| Travel | `Epic travel photography, remote landscape, dramatic weather, rich colors, wide aspect ratio, adventure documentary style` |

---

## 12. Dark Executive (暗黑高级)

**源自:** auto-ppt-engine (dark-executive), AAAAAAAJ/slides (Dark Editorial)

```
Dark executive slide, premium luxury aesthetic,
deep charcoal to pure black gradient background,
gold and white accent text, minimal decoration,
dramatic lighting effect, spotlight composition,
colors: background #0D0D0D, gold #C4964E,
pure white #FFFFFF, warm gray #8C8C8C,
trace amber #BF6F00,
thin hairline dividers, generous letter spacing,
elegant serif headings (Garamond/Playfair Display),
sans-serif body text,
Luxury brand presentation design, 16:9
```

**配色:**
| Role | Hex | Usage |
|------|-----|-------|
| Background | `0D0D0D` | Near black |
| Primary | `FFFFFF` | White text |
| Gold | `C4964E` | Headings, accents |
| Warm Gray | `8C8C8C` | Secondary text |
| Amber | `BF6F00` | Highlight |

**适用场景:** 品牌年度晚宴、奢侈品发布、高端客户提案

**图片生成提示词:**
| Type | Prompt |
|------|--------|
| Background | `Black to dark charcoal gradient, subtle golden particles floating, premium dark texture, luxury event backdrop` |
| Hero | `Luxury product on dark pedestal, dramatic spotlight, gold reflections, premium studio lighting, high-end editorial` |
| Atmosphere | `Dark moody abstract, golden light streaks, premium texture, black and gold color scheme, luxury brand aesthetic` |
| Texture | `Premium dark marble texture, subtle gold veining, polished stone surface, luxury material, elegant background` |
| Detail Shot | `Extreme macro of luxury material, gold foil texture, dark background, premium detail, abstract luxury aesthetic` |
| Still Life | `Elegant dark still life, single flower in golden vase, dramatic lighting, dark background, fine art photography` |

---

## Style Preset JSON

Each style can be referenced as a JSON preset:

```json
{
  "style": "business-professional",
  "theme": {
    "colors": {
      "accent1": "1A365D",
      "accent2": "4472C4",
      "accent3": "5B9BD5",
      "accent4": "BDD7EE",
      "accent5": "ED7D31",
      "dk1": "333333",
      "dk2": "1A1A1A",
      "lt1": "FFFFFF",
      "lt2": "F2F2F2"
    },
    "major_font": "Calibri Light",
    "minor_font": "Calibri"
  },
  "design_rules": {
    "title_size": 40,
    "subtitle_size": 24,
    "body_size": 16,
    "caption_size": 12,
    "whitespace_ratio": 0.25,
    "corner_radius": 0.15,
    "max_bullets": 6,
    "max_words_per_slide": 60
  }
}
```

All 12 styles are available as JSON presets in the `templates/styles/` directory.

---

## Design Principles (merged from all projects)

### 1. Content Hierarchy
- **Title (36-54pt):** One clear headline per slide
- **Subtitle (20-28pt):** Supporting context
- **Body (12-18pt):** Main content
- **Caption (10-12pt):** References, sources, footnotes

### 2. Color Strategy
- Use theme colors consistently → `theme.colors` in JSON
- Ensure WCAG AA contrast (4.5:1 for body text)
- One accent color per deck (not per slide)
- Dark slides: white text on dark bg; Light slides: dark text on light bg

### 3. Layout Rules
- **16:9 canvas:** 13.333 × 7.5 inches
- **Margins:** minimum 0.5 inches on all sides
- **Whitespace:** at least 15-25% of slide area
- **Bullet limit:** no more than 6 per slide
- **Font minimum:** 14pt for projected presentations, 12pt for handouts

### 4. Visual Language
- **Shapes:** RoundRect for modern UI; Rect for professional/business
- **Shadows:** Use sparingly, consistent direction (usually 135°, bottom-right)
- **Gradients:** Subtle and purposeful (not decorative)
- **Animations:** Entrance (flyIn/fadeIn) only; no exit/emphasis in content slides
- **Transitions:** fade or push for smooth flow; avoid random

### 5. Typography
| Font Pair | Style | Use Case |
|-----------|-------|----------|
| Calibri Light + Calibri | Sans-serif | Default business |
| Inter + Noto Sans SC | Sans-serif | Modern tech/CJK |
| Helvetica + Noto Sans JP | Sans-serif | Swiss/Japanese |
| Garamond + Calibri | Serif + Sans | Dark Executive |
| Times New Roman + Arial | Serif + Sans | Academic |
| Playfair Display + Inter | Display + Sans | Luxury |

### 6. Component Usage Guidelines
- **kpiCard:** max 3 per slide, top row
- **barChart:** one per slide, center area
- **timeline:** bottom 1/3 of slide, 4-6 items
- **processFlow:** 4-5 steps, middle band
- **progressBar:** top or bottom edge, subtle
- **progressRing:** side panel, supporting data
- **ratingStars:** product review slides only

---

## Style Selection Guide

Ask yourself these 3 questions to pick the right style:

1. **Audience:** Who will see this? (executives → Consulting; investors → Minimalist; peers → Tech)
2. **Topic:** What is it about? (data report → Business; creative → Swiss/Bauhaus; academic → Academic)
3. **Tone:** What feeling? (premium → Dark Executive; modern → Glassmorphism; cultural → Japanese)

| If you want... | Pick this style |
|----------------|-----------------|
| "专业汇报" | Business Professional |
| "顶咨感觉" | Consulting (MBB) |
| "高级感" | Dark Executive |
| "科技感" | Tech/Neon |
| "论文答辩" | Academic |
| "产品发布" | Minimalist Clean 或 Bento Grid |
| "创意提案" | Swiss International 或 Bauhaus |
| "品牌故事" | Cinematic Photo |
| "文化调性" | Japanese Minimal |
| "现代AI" | Glassmorphism |
