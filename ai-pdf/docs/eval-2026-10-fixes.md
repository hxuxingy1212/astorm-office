# ai-pdf 缺陷修复记录（2026-10）

针对 `docs/eval-2026-10.md` 的 18 条缺陷逐条修复，并在两批语料（原 1463 份 + 新 1578 份）上做回归与抽样验证。
修复期间又发现并修掉了一批新的缺陷类。

## 1. 已修复缺陷

### 报告内 18 条

| # | 缺陷 | 修复要点 | 文件 |
| --- | --- | --- | --- |
| 1 | 裁剪路径 `re W n` 误记为填充矩形 | 路径改为「先挂起、`f/S/B` 才落元素」，`W n` 仅记录裁剪 | `unpack.rs` |
| 2 | CID 字体 W 数组未换算 1/1000 em | `glyph_width_1000`/`scale_1000`，W、DW、FontDescriptor 全部按 1/1000 em | `fonts.rs`、`repack.rs` |
| 3 | 间接 `/Resources` 与 XObject 字典未解引用 | 遍历 `get_page_resources` 的 `resource_ids` 逐层合并并解引用 | `unpack.rs` |
| 4 | 图像滤镜链不完整 / 伪 JPEG | 自实现滤镜链（Flate/LZW/ASCIIHex/ASCII85/RunLength + Predictor 2/10-15）；无法解码的按原滤镜直通 | `images.rs`、`unpack.rs` |
| 5 | JP2 不可嵌入 | `/JPXDecode` 原码流直通（含 ihdr 尺寸解析） | `repack.rs` |
| 6 | `cs/sc/scn` 颜色算子缺失 | 设备色空间直解 + 资源色空间解析（ICCBased/CalRGB/CalGray/Indexed/DeviceN/Separation），分量数兜底 | `unpack.rs` |
| 7 | Form XObject 不递归 | `Do` 命中 Form 时按 Matrix 递归解释，资源分层覆盖 | `unpack.rs` |
| 8 | 透明度不支持 | ExtGState `/ca /CA /BM /LW /D`、图像 `/SMask`、软掩膜组按绘制区域裁剪 | `unpack.rs`、`repack.rs` |
| 9 | shading / 平铺图案丢失 | 轴向/径向渐变解析为色标元素；PatternType 1 递归为图案元素；PatternType 2 用被填充路径裁剪 | `unpack.rs`、`repack.rs` |
| 10 | 文本旋转丢失 | 文本矩阵旋转角写入元素（含 180°：`a<0 && d<0` 判定），重建按矩阵旋转 | `text.rs`、`repack.rs` |
| 11 | 空 pages / 旋转告警 / 内联图像 / 贝塞尔 / 注释 | 0 页报错；路径保留贝塞尔控制点；`BI/ID/EI` 内联图像解析；注释 `/AP /N` 外观流按 Rect 映射提取 | `unpack.rs`、`text.rs` |
| 12 | 字体内嵌与文本宽度 | 抽取 `FontFile2/FontFile3` + ToUnicode/CIDToGIDMap 推导 unicode→gid 与 1000 单位字宽，重建用原字体 | `unpack.rs`、`fonts.rs` |
| 13 | 图形状态线宽/虚线不完整 | `d` 操作符 + ExtGState `/D`，虚线长度随 CTM 缩放 | `unpack.rs` |
| 14 | CMYK/Decode 反相 | 直通保留 `/Decode`；解码路径按 Decode 反相 | `images.rs` |

### 修复期间新发现并修复

| 缺陷 | 说明 |
| --- | --- |
| 媒体/字体编号按页重置 | 每页从 `img-001` 重新计数导致跨页覆盖 → 改为文档级共享计数与字体缓存 |
| 嵌入流用裸 deflate | PDF FlateDecode 需要 zlib 流；改用 `ZlibEncoder`（此前渲染器报 "incorrect header check" 整页变黑） |
| 字节被再次 UTF-8 编码 | 标准字体写字节串时把 0x80-0xFF 当 char 写入 → 改八进制转义 |
| 滤镜名大小写 | 直通写出 `/jbig2decode`（小写）不被识别 → 规范名映射 |
| 标准 14 字体误走 CID | 非 ASCII 即走系统字体替换 → 补 WinAnsi 映射表（含引号/破折号/省略号），仅在不可编码时回退 |
| 页面框原点丢失 | MediaBox/CropBox 原点非零时内容整体偏移 → 保留 MediaBox 与 CropBox |
| 隐式页面裁剪 | 用 `[0,0,w,h]` 当裁剪会把页外内容误删 → 仅显式 `W n` 才设裁剪 |
| 无 `/Length` 的流 | lopdf 解析为空 → 从原始文件字节回退提取（页面内容、表单、SMask） |
| 180° 旋转 | 线性部分 (-1,0,0,-1) 的 b/c 为 0 被误判为无旋转 → 单独判定 |
| ExtGState LW/D | 线宽与虚线可来自 ExtGState，此前只读 ca/CA |
| 图片 SMask 在 PNG 路径丢失 | 解码为 PNG 时丢弃 SMask → 元素级携带并重建 `/SMask` |

## 2. 回归与抽样

### 原语料（1463 份：pdf.js/pdfbox/pdfplumber/pypdf/pikepdf/pdfminer/ocrmypdf/pdf20examples）

| 指标 | 修复前 | 修复后 |
| --- | --- | --- |
| unpack 成功 | 1321 (90.3%) | 1312 (89.7%，含 9 份空页面树改为显式报错) |
| round-trip 成功 | 1301 (88.9%) | 1312 (89.7%) |
| repack 失败 | 20 | **0** |
| 嵌入图片总数 | ~0 | 4536 |
| 100 份抽样中位差 | 3.11 | **0.21** |
| 100 份抽样均差 | 15.86 | **3.20** |
| 抽样 ≤1.0 份数 | 40 | **63** |

### 新语料（1578 份：fpdf2/pdfcpu/pdfium/tika/qpdf/itext，与首批无重名）

- round-trip 成功 **1480/1578 = 93.8%**，repack 失败 **0**
- 失败均为畸形/加密测试样本（pdfium `bug_*` 模糊样本、tika 坏文件）
- 抽样轮次（100 份/轮，阈值 2.0）：

| 轮次 | 问题数 | 说明 |
| --- | --- | --- |
| round1 | 15 | 修复前基线 |
| round2（另一随机抽样） | 18 | 修复前基线 |
| round3 | 8 | 第一批修复后 |
| round4 | **5** | 追加修复（着色图案裁剪、Ts、180° 旋转等）后 |

### 已知样例复验（修复前 → 修复后）

| 样例 | 前 | 后 |
| --- | --- | --- |
| pikepdf/multipage（混合滤镜/伪 JPEG/间接资源） | 65.3 | **0.00** |
| pdf.js/issue15053（CID 字宽） | 99.6 | **1.18** |
| pdfplumber/background-checks（ICCBased 颜色） | 94.1 | **2.33** |
| pdf.js/annotation-line-without-appearance（裁剪） | 99.8 | **0.18** |
| pdfminer/issue-449-vertical（竖排） | 99.6 | **0.03** |
| pdf.js/transparency_group（透明组/混合） | 87.7 | **3.21** |
| pdf.js/tiling-pattern-large-steps（平铺图案） | 49.2 | **0.00** |
| pdf.js/smask_alpha_bc（软掩膜） | 26.5 | **3.46** |
| ocrmypdf/cardinal（JBIG2） | 62.0 | **2.56** |
| ocrmypdf/lichtenstein（JP2） | 16.3 | **0.00** |
| pdf.js/S2（JP2 多图） | 72.4 | **0.05** |
| pdf.js/attachment（注释） | 99.9 | **0.14** |
| pdfium/bug_750568（无 /Length 流） | 100 | **0.01** |
| fpdf2/gradient_linear_stops_extend_before（着色图案裁剪） | 23.2 | **1.84** |
| pdfcpu/cutHor_page_1（虚线） | 7.9 | **0.06** |
| pdfcpu/italian（CropBox 原点） | 3.4 | **0.05** |
| pdfcpu/bookletResized（180° 旋转） | 3.9 | **0.18** |
| pdfcpu/posterScaled_page_1（MediaBox 原点） | 12.0 | **0.16** |
| fpdf2/rect02、line01（ExtGState LW/D） | 5.2 / 4.2 | **0.01 / 0.01** |
| fpdf2/svg_bungee（Type3 字形过程） | 22.4 | **2.36** |
| pdfium/form_object_with_path（表单流回退） | 8.8 | **0.01** |

## 3. 第二轮修复（批次 2，全部定位到字节级根因）

第二轮把 round4 剩余项与后续抽样新暴露的类别逐一定位修复，每项都有最小样例实证：

| # | 类别 | 根因（实证） | 修复 | 样例前后 |
| --- | --- | --- | --- | --- |
| 1 | 嵌入字体被静默替换 | 字体流 zlib 压缩但未声明 `/Filter /FlateDecode`，阅读器把压缩字节当字体（FT "unknown file format"）；全库 12%（189/1578）样本受影响 | `repack.rs` 补 Filter（+Length1） | fonts_otf 2.90→**0.00**、symbol_shaping 2.02→**0.00** |
| 2 | 渐变整体消失 | 图案 Matrix 映射到所在内容流的**默认用户空间**，不应再叠当前 CTM（MuPDF 最小样例实证：翻转 cm 内图案位置不变）；此前 `ctm ∘ pm` 双重翻转把渐变轴移出页外 | 引入 GState.base（内容流入口 CTM），图案 placement = base∘Matrix；`sh` 仍随当前 CTM | gradient_shared 2.16→**0.03**、spread_methods 5.48→**0.11** |
| 3 | 裁剪路径错位 | `re` 段在本地空间存 [x,y],[w,h] 两点，按点做 CTM 变换在翻转/旋转下得到错误矩形；孤立 `m` 撑大包围盒 | `normalized_path_segs`：re 展开为闭合多边形、剔除孤立 moveto，bbox 与段一致 | 同上（渐变裁剪） |
| 4 | 镜像文本 | det<0 的文本基（如转置 0 1 1 0）被当作 90° 旋转 | 模型加 `mirror`，按 R(θ)·diag(1,-1) 分解/还原 | mirror_multi_cell 3.21→1.86 |
| 5 | 逐属性透明度 | ExtGState `ca≠CA` 被塌缩为 min | Rect/Path 加 `stroke_alpha`，GS 键改为 (ca,CA,BM) 三元组 | half_fill_opacity 4.52→**0.31** |
| 6 | 虚线外观 | `/LC`/`/LJ`（线端/连接）未建模，圆帽虚线渲染成方点 | GState + Path 加 line_cap/line_join，回写 `J`/`j` | 50-50 dash 2.15→**0.13** |
| 7 | 颜色整体偏移 | SMask 流的 `/Matte`（解预乘底色）丢失 | 模型加 `matte`，回写到 SMask 流字典 | pdfium/matte 3.34→**0.00** |
| 8 | itext 私有滤镜 | `/BrotliDecode` 非标准滤镜（lopdf 不支持，MuPDF 支持） | 入口 `normalize_brotli_streams`：单滤镜 Brotli 流解码后去滤镜 | cmp_pushButton 4.48→**0.02**、cmp_simpleLowCompression 3.41→**0.00** |
| 9 | 衬线文档变无衬线 | 未知字体名兜底到 CJK 无衬线字体 | 按 Times/Nimbus/Helvetica/Courier 等别名分类选系统字体（显式关键字优先，避免 "Arial Unicode" 命中 Arial） | qpdf/source2 3.79→**0.46** |
| 10 | 行内逐词漂移 | 简单字体未读 `/Widths`、标准 14 用通用估算表（含行内多段 Tj 与 TJ 调距） | 简单字体宽度取自 /Widths（经解码器折算 unicode）；标准 14 用 MuPDF 基准 AFM 度量 | readAndUpdatePage p2 9.48→**0.07**、TestTextAlignJustifyColumnDemo 4.34→**0.18**、TextBorderNoMargin 2.08→**0.14**、varfrags_stretch_spacing 2.85→**0.01** |
| 11 | 亮度软掩膜渐变 | ExtGState `/SMask /S /Luminosity /G` 表单内的渐变被当作实心 alpha | ShadingDef 加 `mask`；重建 DeviceGray 表单 + `/SMask`（元素专属 ExtGState） | gradient_opacity 27.32→**0.10** |
| 12 | 整页文本下移一个字高 | `(text)'` 的 T\* 使用 leading，而 PDF 规范 leading 默认 0（本文档未设 TL），此前默认 12 | GState.leading 默认 0 | TIKA-4444 10.81→**0.04** |
| 13 | 页面尺寸退回 A4 | `/MediaBox 7 0 R`（间接引用）未解引用 | page_mediabox/CropBox/Rotate 解引用 | pdfium/bug_1287409 8.78→**0.00** |
| 14 | 文本标记注释丢失 | Highlight/Underline/StrikeOut/Squiggly 多数无 /AP，需按 `/QuadPoints` + `/C` 绘制（Highlight 用 Multiply 混合） | run_annotations 增加标记注释分支 | highlighted_over_page_break 12.01→**0.18** |
| 15 | 符号字体码位错位 | ZapfDingbats/Symbol 使用内建编码，被按 WinAnsi 解码并声明 WinAnsiEncoding | 解码走 Latin1 原码；repack 不写 Encoding 且字节原样透传 | zapfdingbats 2.22→**0.48** |
| 16 | 网格渐变（ShadingType 4-7） | 仅支持 2/3，其余直接跳过 | 码流直通：解码后写 `shadings/*.bin`，重建用 `cm`（在 `W n` 之后）放回页面 | gradient_sweep_types 3.75→**0.00** |
| 17 | 简单字体 CID 映射 | 无 cmap 的 Identity-H 子集字体（Word/Acrobat），CID≠Unicode 只存在于 ToUnicode；Word 生成的 CMap 把 `<lo><hi><dst>` 连写无空格 | ToUnicode 解析改为提取行内所有 `<...>` 组 | TIKA-4444 字形全空 → 正常 |
| 18 | 裸 CFF（Type1C） | FontFile3 `/Subtype /Type1C` 未提取，回退系统字体导致空框 | 新增 Type1C 直通：简单 Type1 + FontFile3 /Type1C + 原 /Widths/Encoding/度量 | XFA govdocs 5.58→**1.18** |
| 19 | 简单字体 ToUnicode 码宽 | ToUnicode 声明 `<0000><FFFF>` 但条目为 1 字节码，按 2 字节解码整页 U+FFFD | 简单字体恒按单字节码解码 | 同上 |
| 20 | 文本标记注释丢失 | Highlight/Underline/StrikeOut/Squiggly 多数无 /AP，需按 `/QuadPoints` + `/C` 绘制（Highlight 用 Multiply 混合） | run_annotations 增加标记注释分支 | highlighted_over_page_break 12.01→**0.18** |
| 21 | 符号字体码位错位 | Symbol/ZapfDingbats 使用内建编码，被按 WinAnsi 解码/声明 | 解码走 Latin1 原码；repack 不写 Encoding 且字节原样透传 | zapfdingbats 2.22→**0.48** |
| 22 | 网格渐变（ShadingType 4-7） | 仅支持 2/3，其余跳过 | 码流直通 + `cm` 放回页面（在 `W n` 之后） | gradient_sweep_types 3.75→**0.00** |
| 23 | 退化路径铺满整页 | `0 0 m h B`（零面积）时图案填充无裁剪 → 铺满 | 规范化后无段的路径不绘制 | text_fill_gradient 20.95→**0.11** |
| 24 | 颜色键遮罩丢失 | `/Mask [0 0 0 0 0 0]`（含 Indexed 图像）未应用 → 黑色区域不透明 | 解码出口按颜色键生成 alpha（Indexed 分支同样处理） | pdfium/bug_343075986 12.5→**0.00** |
| 25 | FreeText 注释丢失 | 无 /AP 时需按 /Rect + /C + /Contents 绘制，/DA 缺失用默认 Helv 12 | run_annotations 增加 FreeText 分支（含 /DA 解析） | freetext_annotation_without_da 2.31→**0.22** |
| 26 | Type3 位图字形相位差 | 扁平化为页面图片后，阅读器字形缓存栅格化与直接画图有采样相位差 | Type3 直通：CharProcs 原样保留 + 字形图片（含 SMask）+ 原编码/宽度/名称 | sbix_bungee 5.01→**0.01**、sbix_compyx 4.33→**0.02** |
| 27 | 复杂文种整形 | 阿拉伯/希伯来等连写由 shaper 决定，按 unicode 回写退化为孤立形 | 文本记录原始码位（codes），重建按原码写出 | bidi_arabic_lorem_ipsum 9.68→**1.55** |
| 28 | 非 Identity CIDToGIDMap | GID≠CID 时按 GID 回写码位/宽度数组 → 字形与步进皆错 | 保留 CIDToGIDMap 码流 + 原 W/DW 数组，原码配套回写 | 同上 |

## 4. 验证

单元/集成测试：**84 项全绿**（新增字体 Filter、镜像往返、掩膜渐变、Type3 直通、ToUnicode 解析等回归测试）。

抽样（每次随机 100 份，阈值 2.0，中位差）：

| 轮次 | 问题数 | 中位差 | 均值 |
| --- | --- | --- | --- |
| round6 | 1 | 0.0 | 0.22 |
| round7 | 5 | 0.01 | 0.31 |
| round8 | 2 | 0.0 | 0.08 |
| round9 | **0** | 0.0 | — |
| round10 | 2 | 0.0 | — |
| round11 | 1 | — | — |
| round12 | 3 | — | — |
| round13 | **0** | 0.0 | 0.07 |
| round14 | 2 | 0.0 | 1.11 |
| round15 | **0** | 0.0 | — |
| round16 | **0** | 0.0 | 0.07 |

**结论：round15 与 round16 连续两轮抽样 0 问题，「连续 2 次抽样 0 问题」达成**（阈值 2.0，每轮 100 份）。

历史问题集（round1-16 全部问题文件）全量复测：除两个 0 页畸形 PDF
（`pdfium/no_page_count`、`pdfium/page_tree_empty_node`，两版均无内容可渲染，抽样器已跳过）
外**全部达标**。

本轮修复的问题样例（9.68 → 1.55 等）与 round14 新发现：
`bug_2034`（4bit Indexed 索引被缩放，99.2→0.01）、`varfrags_text_mode`（Tr 渲染模式，3.05→0.01）。

## 5. 批次 3（2026-10-03）：语料 1 长尾 + 全语料扫描

在批次 2 之后继续对两批语料抽样，逐项修复语料 1（pdf.js / pdfbox /
pdfminer / pdfplumber 测试集）的长尾缺陷。全部有视觉前后对比与测试覆盖。

| # | 缺陷 | 现象与根因 | 修复 | 验证（前后像素差） |
| --- | --- | --- | --- | --- |
| 29 | CalRGB 未做色度转换 | CalRGB/Lab 是线性空间，直接当设备 RGB 用 → 整体偏暗 | 按 /Gamma + /Matrix + /WhitePoint 转 XYZ→sRGB | issue17065 7.48→0.56 |
| 30 | PostScript `atan` 值域 | 返回 (-180,180]，规范要求 [0,360) | 负角 +360 | colorspace_atan 5.83→0.03 |
| 31 | 位解包未按行对齐 | 1bpc 非 8 倍宽图像连续位流读 → 整体错位 | 按行字节对齐展开 | images_1bit_grayscale 7.85→0.47 |
| 32 | 行内图像过滤名缩写 | `/F /fl`、`/ccf` 直接写进 XObject（缩写只在行内合法） | 规范化成完整滤镜名 | 同上 |
| 33 | 行内图像 /D 未传解码器 | ImageMask 极性错误 | 传入 /D（= /Decode） | bug1799927 12.9→0.06 |
| 34 | 内联字体字典被跳过 | /Font 里直接内联的字体（非引用）整页回落 Helvetica | 内联字典克隆后泄漏取 'a 借用 | bug946506 11.57→0.67 |
| 35 | 文本非正交基 | (size, rotation) 只能表达正交基，剪切/两轴缩放不等时字形拉高 | 新增 `tm` 归一化线性部分精确还原 | 同上 |
| 36 | 组表单 /Matrix 重复施加 | 内容 `cm` 与表单 /Matrix 各一次 → 组偏移 | 表单 Matrix 置单位阵 | knockout_nested_group_alpha 13.23→0.00 |
| 37 | 透明组 /K 布尔写法 | `/K true` 按整数读失败 → knockout 丢失 | 布尔/整数皆可 | 同上 |
| 38 | 组内元素继承 Do 时 ca/CA/BM | 组与子元素各应用一次 alpha | 子内容 alpha 复位，组 alpha 走 ExtGState | 同上 |
| 39 | 嵌套组未构建 | 组内再嵌套组时子表单名缺失，组内容整块丢弃 | 递归构建表单（先子后父） | transfer_maps 11.14→4.32 |
| 40 | 页面 /XObject 被表单覆盖 | 注册表单时直接 set，丢掉 Im* 图片引用 | 合并而非覆盖 | 同上 |
| 41 | 对象流「只增不替换」 | lopdf 增量更新文件读到旧版本（/Annots 26→18） | 按参考表重解容器对象流 | AcroFormsBasicFields 6.50→2.19 |
| 42 | 域外观状态未按 /AS 选 | 复选框/单选按钮总是画 Off | 按 /AS 选 /AP /N 状态 | 同上 |
| 43 | 裁剪外文本未丢弃 | 域外观含 2 万行超长文本（y 到 -32000） | 基线在裁剪框外加字号余量即丢弃 | 同上 |
| 44 | CID-keyed CFF 不支持 | FontFile3 /CIDFontType0C 整页回落替代字体 | 新增 CidCff：原字体程序直通 + 原 W/DW + 原 CMap | issue9534_reduced 10.42→1.17 |
| 45 | CMap CID 为裸整数 | `begincidrange` 的 CID 不含尖括号 → 解析为空 | 兼容裸十进制 | 同上 |
| 46 | Type1 /FontFile 未支持 | Ghostscript/LaTeX 常见 Type1 内嵌字体整体回落 | 新增 Type1 直通（原码 + 原 /Encoding(Differences) + 原 /Widths） | highlights 7.12→1.79、tracemonkey_with_annotations 7.05→0.25、issue8960 6.2→0.00 |
| 47 | JBIG2Globals 丢失 | /DecodeParms 的间接引用 JSON 化变 null → 解码器解不出 | 存 media/*.jb2g 并在重建时挂回 | pdf-with-jbig2 99.26→0.00 |
| 48 | FunctionType 0 采样函数按色标抽样 | 数千采样点的条纹渐变被抽成 17 点 | shadings/fn-NNN.bin 直通 | issue14165 24.28→0.00 |
| 49 | CID 字体 /W /DW 未保留 | 只按 gid 重建 → ToUnicode 未映射的 CID 宽度落 DW | 原 /W /DW 一律原样回写 | issue13193 7.8→0.00 |
| 50 | 简单字体原 /Widths 未回写 | 零宽空格等声明宽度被字体自带 advance 覆盖 | code_map/code_widths 保留并回写 | issue6894 5.9→0.00 |
| 51 | 未映射码回落基本编码 | CMap 未列出的单字节码输出 U+FFFD 豆腐块 | 回落码位本身（≈Latin-1） | 同上 |
| 52 | PS `roll` 负 j | `5 -1 roll` 被 max(0) 归零 → DeviceN tint 全错 | 支持负 j（rotate_right 归一化） | issue13520 14.53→9.11 |
| 53 | 解码为控制符仍按 unicode 回写 | ToUnicode 只给空 codespacerange 的 CJK 子集 → 豆腐块 | 控制符/私用区/U+0000 一律原码回写 | issue7696 11.71→0.00 |
| 54 | CalGray 未做色度转换 | 等同 DeviceGray，忽略 WhitePoint/Gamma | 同 CalRGB 通路 | calgray 16.22→0.32 |

### 抽样现状（批次 3 结束时）

| 语料 | 规模 | 抽样问题率 | 说明 |
| --- | --- | --- | --- |
| 语料 2（真实文档：pdfium/qpdf/tika/itext/fpdf2/…） | 1478 | ~2%（1-2/100） | 曾有 8 轮连续 0 问题；本轮长尾为少量图形/字体边界 |
| 语料 1（测试套件：pdf.js/pdfbox/pdfminer/pdfplumber/pypdf） | 1463 | ~12-15%（12-16/100） | 对抗性特性测试，长尾为网格渐变/软掩膜/非内嵌字体等 |

语料 1 剩余问题以测试套件的极端特性为主（如 SMask 作用于任意填充、
非内嵌字体的替代字体差异、viewer 合成部件外观），单文件单特性。

## 6. 批次 4（2026-10-03 续）：注释外观、CJK/非内嵌字体、渐变矩阵

| # | 缺陷 | 现象与根因 | 修复 | 验证 |
| --- | --- | --- | --- | --- |
| 55 | 径向渐变非相似变换 | 圆被压成椭圆时半径无法表达 | `ShadingDef.matrix`：坐标保持着色空间，重建 `W n` → `cm` → `sh` | issue7847_radial 15.73→0.17 |
| 56 | 网格着色缺 /Function | 渲染器按分量数误解析颜色流，只画首个面片 | 网格 /Function（字典/流）直通 | coons-allflags 8.59→0.0000006 |
| 57 | 注释外观 /Matrix 顺序 | 规范 12.5.5 要求先应用表单矩阵再映射到 /Rect；顺序反了整体偏移 | 先矩阵变换 BBox 再算 placement | issue7821 21.64→0.26、AcroFormsRotation 3.52→0.05、itext/AnnotationSampleStandard 4.77→1.21 |
| 58 | 非内嵌 CID 字体 | 无字体程序的 CID 字体退化成「系统字体 + 2 字节码」 | 保留 Type0/CIDFontType2 + /W + /Encoding，交阅读器替换 | issue15977_reduced 9.95→8.3（结构正确化） |
| 59 | 非内嵌中文「宋体」 | `/BaseFont /#CB#CE#CC#E5` + WinAnsi，实际码位是 GBK | 反转义 + GB18030 解码 + macOS 中文名映射 | XiaoBiaoSong 4.05→1.80 |
| 60 | 子集字体码位/宽度错配 | 按 WinAnsi 重编码取到宽度 0 的码位，字距全乱 | Type1/Type1C 一律原码回写；同 unicode 多码优先非零宽度 | issue7101 10.53→0.00 |
| 61 | U+FFFD 文本 | 无法映射的字形按 unicode 重编码成 .notdef | 与私用区/控制符一并改按原码回写 | issue5994 3.21→0.00 |
| 62 | CID 字体 /W /DW | 只按 gid 重建会丢 ToUnicode 未映射 CID 的宽度 | 原 /W /DW 一律原样回写 | issue13193 7.8→0.00 |
| 63 | 简单字体 /Widths | 声明宽度被字体自带 advance 覆盖（零宽空格） | code_map/code_widths 保留并回写 | issue6894 5.9→0.00 |
| 64 | 空 codespacerange 的 ToUnicode | 解码得到 C0 控制符 → 豆腐块 | 控制符/U+0000/私用区一律原码回写 | issue7696 11.71→0.00 |
| 65 | FreeText 不折行 | 单行溢出整页 | 按 /Rect 宽度逐词折行；多行行距 1.2 | pdfbox/Annotations 文字布局对齐 |
| 66 | 空口令加密 PDF | 直接拒绝 | `doc.decrypt("")` 后走既有流程 | rc4-40 等 |

### 抽样现状（批次 4 结束时）

| 语料 | 抽样问题数（每轮 100 份，阈值 2.0） |
| --- | --- |
| 语料 1（测试套件） | 17 → 12 → 1（round6）→ 3（round7） |
| 语料 2（真实文档） | 2 → 0（round7）→ 1（round8） |

剩余长尾：网格渐变的 ICC/DeviceN 色度（需 ICC 引擎）、任意填充上的软掩膜合成、
Line/Square/Circle/Scribble 注释的外观合成（阅读器行为，PDF 内无该数据）、
非内嵌字体的替代字体差异（与参考渲染器的选择绑定）。

## 7. 批次 5（2026-10-03 收尾）：软掩膜直通、网格着色空间

按「性价比」排序后重估了此前判定为"少见"的两类：ICC/DeviceN 网格（1/1463）
确实可以放弃，但**填充/表单上的软掩膜被上一轮扫描低估了**（扫描把
`/GS2 gs /Fm1 Do` 这类"掩膜作用在透明组上"误判成图片掩膜），
而它正是语料 2 最大残差 tika/testPDF_angles 的根因。

| # | 缺陷 | 现象与根因 | 修复 | 验证 |
| --- | --- | --- | --- | --- |
| 67 | ExtGState `/SMask` 未实现 | 掩膜区域被画成不透明（软阴影变实心黑条、smask_* 整块实心） | 新增 `SoftMask` 模型 + 掩膜表单直通：内容/BBox/Matrix/Group/内部图片/ExtGState/Shading/BC/TR 原样重建，repack 挂回 `/SMask` | smask_luminosity_oob_transfer 41.5→**0.13**、smask_alpha_bc 3.46→0.36、knockout_smask 4.23→**0.00**、nonisolated_blend_smask 9.18→**0.13**、issue14297 1.76 |
| 68 | `/SMask` 字典在流上 | `/SMask 57 0 R` 是「字典+流」的间接对象，只认纯字典会漏 | 兼容两种形态 | tika/testPDF_angles 10.26→**2.32** |
| 69 | `/TR` 传递函数丢 | 掩膜亮度到 alpha 的映射丢失（0.25→0.75） | 原样回写（注意序列化解引用后的对象） | smask_luminosity_oob_transfer 14.06→0.13 |
| 70 | 透明组判定未解引用 `/Group` | `/Group 33 0 R` 时整族表单退化成普通 Form，组 alpha/BM/软掩膜全丢 | 判定时解引用 | tika/testPDF_angles 10.26→2.32 |
| 71 | 旧的「按掩膜包围盒近似裁剪」 | 近似裁剪把内容裁掉（红块只剩一条） | 删除（掩膜已按真身直通） | 同 67 |
| 72 | 网格着色固定 DeviceRGB | CMYK/灰度网格分量数不符 → 顶点流错位成彩虹碎片 | 保留源 ColorSpace（复杂空间按分量数折算设备空间） | personwithdog 5.50→**0.95** |
| 73 | 掩膜表单内的 /Shading、图片 | 渐变掩膜、位图掩膜缺资源 | 表单 /Resources 按结构重建 | issue13520 9.11→6.94 |

### 抽样现状（批次 5 结束时）

| 语料 | 最新 100 份抽样 | 说明 |
| --- | --- | --- |
| 语料 2（真实文档） | **0 问题** | 本轮链路上连续 0/1/0/0 |
| 语料 1（测试套件） | **1 问题**（pdfplumber 2.22，临界） | 从批次 3 的 12-17 降下来 |

### 已知未修（有诊断、已记录）

- **性能**：每个 Form XObject 都要深拷贝一次页面级资源作用域
  （`Scope::clone()` + `merge_resources`），Illustrator 类文件 267 个表单 × 600ms
  ≈ 160s（pdf.js/issue6961，输出正确但超时）。修法是把 Scope 的资源表改为
  Rc 链式覆盖层，属架构改动。
- 任意填充上的 SMask 已支持；ICC/DeviceN 网格色度（1/1463）、
  Line/Square/Circle/Scribble 注释的阅读器合成外观未做。

## 8. 批次 6（2026-10-06）：性能、tint 双重转换、子集字体

| # | 缺陷 | 现象与根因 | 修复 | 验证 |
| --- | --- | --- | --- | --- |
| 74 | 资源作用域深拷贝 | 每个 Form 的 `Scope::clone()` 深拷贝整张资源表（XObject/ColorSpace/Pattern/Shading/ExtGState） | 改为与字体层一致的 `Rc` 分层覆盖，clone 只克隆若干 Rc | 合成样本（5000 资源 × 200 表单）0.1s |
| 75 | 字体程序重复解压 | 字体缓存命中检查排在 `get_plain_content()` 之后 → 每个 Form 重新解压一次字体程序 | 缓存检查前移到 desc 解析之后 | pdf.js/issue6961 解包 **234s → 4.9s**（39×），21 份样本产物逐字节等价 |
| 76 | Separation/DeviceN tint 双重转换 | `eval_tint` 兜底先把函数输出转成 RGB，再交给备选空间（DeviceCMYK）转一次 | 改为返回原始输出分量（FunctionType 0/2/3/4 各自直出） | pdfplumber/issue-316 4.26→**0.79** |
| 77 | 采样函数分量数按 3 推断 | 无 `/Decode` 时用 RGB 推断，4 输出的 CMYK tint 采样跨距错位 | 依次按 `/Decode` → `/Range` 推断分量数 | 同 76 |
| 78 | 子集字体 cmap 不可用 | 字体程序有 cmap 表但查不到字符（或只有 symbol 子表），按 CID 重建整页 .notdef | 新增 `simple_rebuild`：命中不足一半时按「简单字体 + 原 /Encoding + 原码位」重建 | pdf.js/pdkids 5.69→**1.77** |
| 79 | /Differences 只对 Type1 记录 | 简单 TrueType 的自定义编码丢失，原码配 WinAnsi 会错位 | `/Encoding` 与 `/Differences` 对所有简单字体都记录 | 回归修复（issue6894 曾 0.00→14.01→0.00） |

### 抽样现状（批次 6 结束时）

| 语料 | 最新 100 份抽样 | 说明 |
| --- | --- | --- |
| 语料 2（真实文档，1578 份） | **0 问题** | 本轮连续 0/0 |
| 语料 1（测试套件，1324 份） | **1 问题**（pdfminer/i1040nr 2.19，临界） | 批次 3 时为 12-17 |

> 注：语料 1 原目录曾被 macOS 的 `/tmp` 定期清理删除（3 天未访问），
> 已用稀疏克隆重新拉取（pdf.js 988 / pdfbox 64 / pdfminer 56 /
> pdfplumber 85 / pikepdf 60 / pypdf 71），脚本在 `/tmp/fetch2.sh`。

### 剩余已知项

- 字距/尺寸类临界差异（issue-604 4.14、eu-001 2.5、pypdf/pdflatex-outline 2.08、
  pdfminer/i1040nr 2.19）：run 内 advance 比参考渲染器窄约 5%，成因是
  子集/非内嵌字体的 advance 与阅读器替代字体不一致，非结构性错误。
- ICC/DeviceN 网格色度（1/1463）、Line/Square/Circle/Scribble 注释的
  阅读器合成外观未做。
