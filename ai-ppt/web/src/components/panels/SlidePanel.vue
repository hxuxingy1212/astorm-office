<script setup lang="ts">
// 幻灯片属性面板：画布尺寸 / 背景 / 过渡 / 备注（分组折叠）
import { computed } from 'vue'
import { useSlidesStore } from '@/stores/slides'
import { useSnapshotStore } from '@/stores/snapshot'
import { withHash } from '@/utils/convert'
import SectionGroup from './SectionGroup.vue'
import type { Slide, Transition } from '@/types/presentation'

const slides = useSlidesStore()
const snapshot = useSnapshotStore()

function commit(props: Partial<Slide>) {
  slides.updateSlide(props)
  snapshot.addHistorySnapshot(slides.slides, slides.slideIndex)
}

// === 画布尺寸 ===
const presets = [
  { label: '16:9', w: 13.333, h: 7.5 },
  { label: '16:10', w: 13.333, h: 8.333 },
  { label: '4:3', w: 10, h: 7.5 },
  { label: 'A4 横向', w: 11.693, h: 8.268 },
]
function applyPreset(p: { w: number; h: number }) {
  slides.setCanvasSize(p.w, p.h)
  snapshot.addHistorySnapshot(slides.slides, slides.slideIndex)
}
function setSize(key: 'w' | 'h', value: number) {
  const w = key === 'w' ? value : slides.width
  const h = key === 'h' ? value : slides.height
  slides.setCanvasSize(w, h)
  snapshot.addHistorySnapshot(slides.slides, slides.slideIndex)
}
const sizeLabel = computed(() => {
  const r = slides.width / slides.height
  const map: Record<string, string> = {}
  for (const p of presets) map[`${(p.w / p.h).toFixed(3)}`] = p.label
  return map[r.toFixed(3)] ?? '自定义'
})

// === 背景 ===
type BgKind = 'none' | 'solid' | 'gradient' | 'image'
const bgType = computed<BgKind>(() => {
  const bg = slides.currentSlide?.background
  if (!bg) return 'none'
  if (typeof bg === 'string') return 'solid'
  if (typeof bg === 'object' && bg) {
    if (bg.type === 'gradient') return 'gradient'
    if (bg.type === 'image') return 'image'
  }
  return 'none'
})

const bgColor = computed(() => {
  const bg = slides.currentSlide?.background
  if (typeof bg === 'string') return withHash(bg)
  return '#ffffff'
})

const gradientModel = computed(() => {
  const bg = slides.currentSlide?.background
  if (typeof bg === 'object' && bg && bg.type === 'gradient') {
    return {
      stops: (bg.stops as { color: string; position: number }[]) ?? [],
      angle: Number((bg.angle ?? 90) as number),
    }
  }
  return { stops: [], angle: 90 }
})

const bgImageSrc = computed(() => {
  const bg = slides.currentSlide?.background
  if (typeof bg === 'object' && bg && bg.type === 'image' && bg.src) return String(bg.src)
  return ''
})

const mediaList = computed(() => slides.media ?? [])

function setBg(kind: BgKind) {
  if (kind === 'none') commit({ background: undefined })
  else if (kind === 'solid') commit({ background: bgColor.value.slice(1) })
  else if (kind === 'gradient') {
    const stops = gradientModel.value.stops.length
      ? gradientModel.value.stops
      : [
          { color: '4472C4', position: 0 },
          { color: 'FFFFFF', position: 1 },
        ]
    commit({ background: { type: 'gradient', stops, angle: gradientModel.value.angle } })
  } else if (kind === 'image') {
    commit({ background: { type: 'image', src: bgImageSrc.value } })
  }
}
function setSolid(color: string) {
  commit({ background: color.slice(1) })
}
function setBgImage(src: string) {
  commit({ background: { type: 'image', src } })
}
function setGradient(angle: number, stops: { color: string; position: number }[]) {
  commit({ background: { type: 'gradient', stops, angle } })
}
function setGradientAngle(angle: number) {
  setGradient(angle, gradientModel.value.stops.length ? gradientModel.value.stops : [
    { color: '4472C4', position: 0 },
    { color: 'FFFFFF', position: 1 },
  ])
}
function updateStop(index: number, color: string) {
  const stops = gradientModel.value.stops.map((s, i) => (i === index ? { ...s, color } : s))
  setGradient(gradientModel.value.angle, stops)
}
function addStop() {
  const stops = [
    ...gradientModel.value.stops,
    { color: 'FFFFFF', position: gradientModel.value.stops.length },
  ]
  setGradient(gradientModel.value.angle, stops)
}
function removeStop(index: number) {
  const stops = gradientModel.value.stops.filter((_, i) => i !== index)
  setGradient(gradientModel.value.angle, stops)
}

// === 过渡 ===
const TRANSITIONS = [
  { value: 'none', label: '无' },
  { value: 'fade', label: '淡入淡出' },
  { value: 'push', label: '推入' },
  { value: 'wipe', label: '擦除' },
  { value: 'split', label: '分裂' },
  { value: 'cover', label: '覆盖' },
  { value: 'cut', label: '切页' },
  { value: 'dissolve', label: '溶解' },
  { value: 'random', label: '随机' },
]
const trans = computed<Transition>(() => slides.currentSlide?.transition ?? { type: 'none' })
function setTransition(patch: Partial<Transition>) {
  const next: Transition = { ...trans.value, ...patch }
  commit({ transition: next.type === 'none' ? undefined : next })
}

// === 备注 ===
</script>

<template>
  <div class="panel slide-panel">
    <SectionGroup title="画布尺寸" defaultOpen>
      <div class="preset-row">
        <button
          v-for="p in presets"
          :key="p.label"
          class="preset-chip"
          :class="{ active: sizeLabel === p.label }"
          @click="applyPreset(p)"
        >
          {{ p.label }}
        </button>
      </div>
      <div class="row">
        <label>宽</label>
        <input type="number" :value="slides.width" step="0.1" min="1" @input="(e) => setSize('w', Number((e.target as HTMLInputElement).value))" />
        <span class="unit">in</span>
      </div>
      <div class="row">
        <label>高</label>
        <input type="number" :value="slides.height" step="0.1" min="1" @input="(e) => setSize('h', Number((e.target as HTMLInputElement).value))" />
        <span class="unit">in</span>
      </div>
      <div class="hint-row">
        <span class="dot" />当前 {{ sizeLabel }} · {{ (slides.width / slides.height).toFixed(2) }}:1
      </div>
    </SectionGroup>

    <SectionGroup title="背景">
      <div class="seg">
        <button v-for="t in (['none','solid','gradient','image'] as BgKind[])" :key="t" class="seg-btn" :class="{ active: bgType === t }" @click="setBg(t)">
          {{ t === 'none' ? '无' : t === 'solid' ? '纯色' : t === 'gradient' ? '渐变' : '图片' }}
        </button>
      </div>

      <template v-if="bgType === 'solid'">
        <div class="row">
          <label>颜色</label>
          <input type="color" :value="bgColor" @input="(e) => setSolid((e.target as HTMLInputElement).value)" />
        </div>
      </template>

      <template v-else-if="bgType === 'gradient'">
        <div class="row">
          <label>角度</label>
          <input type="range" min="0" max="360" :value="gradientModel.angle" @input="(e) => setGradientAngle(Number((e.target as HTMLInputElement).value))" />
          <span class="val">{{ Math.round(gradientModel.angle) }}°</span>
        </div>
        <div class="stop-row" v-for="(s, i) in gradientModel.stops" :key="i">
          <input type="color" :value="withHash(s.color)" @input="(e) => updateStop(i, (e.target as HTMLInputElement).value.slice(1))" />
          <span class="stop-pos">{{ Math.round(s.position * 100) }}%</span>
          <button class="mini" @click="removeStop(i)">×</button>
        </div>
        <button class="ghost-btn" @click="addStop">＋ 添加渐变点</button>
      </template>

      <template v-else-if="bgType === 'image'">
        <div class="row" v-if="mediaList.length">
          <label>选择</label>
          <select :value="bgImageSrc" @change="(e) => setBgImage((e.target as HTMLSelectElement).value)">
            <option value="">未选择</option>
            <option v-for="m in mediaList" :key="m" :value="m">{{ m.split('/').pop() }}</option>
          </select>
        </div>
        <div class="hint-row" v-else>无可用媒体，请插入背景图</div>
      </template>
    </SectionGroup>

    <SectionGroup title="切换过渡">
      <div class="row">
        <label>类型</label>
        <select :value="trans.type" @change="(e) => setTransition({ type: (e.target as HTMLSelectElement).value })">
          <option v-for="t in TRANSITIONS" :key="t.value" :value="t.value">{{ t.label }}</option>
        </select>
      </div>
      <template v-if="trans.type !== 'none'">
        <div class="row">
          <label>速度</label>
          <select :value="trans.speed ?? 'med'" @change="(e) => setTransition({ speed: (e.target as HTMLSelectElement).value })">
            <option value="slow">慢</option>
            <option value="med">中</option>
            <option value="fast">快</option>
          </select>
        </div>
        <div class="row">
          <label>自动翻页</label>
          <input type="number" :value="trans.advance_after ?? 0" min="0" step="0.5" @input="(e) => setTransition({ advance_after: Number((e.target as HTMLInputElement).value) })" />
          <span class="unit">s</span>
        </div>
      </template>
    </SectionGroup>
  </div>
</template>

<style scoped>
.slide-panel {
  padding: 2px 0 10px;
}
.preset-row {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(56px, 1fr));
  gap: 6px;
  margin-bottom: 10px;
}
.preset-chip {
  padding: 6px 4px;
  border: 1px solid #e0e0e0;
  background: #fff;
  cursor: pointer;
  font-size: 12px;
  color: #555;
  text-align: center;
}
.preset-chip:hover {
  border-color: #4472c4;
  color: #4472c4;
}
.preset-chip.active {
  background: #4472c4;
  color: #fff;
  border-color: #4472c4;
}
.row {
  display: flex;
  align-items: center;
  gap: 8px;
  margin: 8px 0;
}
.row label {
  min-width: 48px;
}
.row input[type='number'],
.row input[type='text'],
.row select {
  flex: 1;
  min-width: 0;
}
.unit {
  font-size: 12px;
  color: #888;
}
.hint-row {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 11px;
  color: #999;
  margin-top: 6px;
}
.hint-row .dot {
  width: 4px;
  height: 4px;
  background: #4472c4;
}
.seg {
  display: flex;
  background: #f1f3f7;
  padding: 3px;
  gap: 3px;
  margin-bottom: 8px;
}
.seg-btn {
  flex: 1;
  border: none;
  background: transparent;
  padding: 6px 4px;
  font-size: 12px;
  color: #666;
  cursor: pointer;
}
.seg-btn.active {
  background: #fff;
  color: #333;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.1);
}
.stop-row {
  display: flex;
  align-items: center;
  gap: 8px;
  margin: 6px 0;
}
.stop-pos {
  font-size: 12px;
  color: #888;
  flex: 1;
}
button.mini {
  width: 22px;
  height: 22px;
  border: 1px solid #e0e0e0;
  background: #fff;
  cursor: pointer;
  color: #999;
  line-height: 1;
}
button.mini:hover {
  color: #e5484d;
  border-color: #f3c2c4;
}
.ghost-btn {
  width: 100%;
  padding: 6px;
  border: 1px dashed #ccc;
  background: #fff;
  color: #888;
  font-size: 12px;
  cursor: pointer;
}
.ghost-btn:hover {
  border-color: #4472c4;
  color: #4472c4;
}
.val {
  font-size: 12px;
  color: #666;
  min-width: 32px;
  text-align: right;
}
.notes-editor textarea {
  width: 100%;
  box-sizing: border-box;
}
</style>
