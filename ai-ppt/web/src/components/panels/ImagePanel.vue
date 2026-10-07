<script setup lang="ts">
// 图片属性面板：裁剪（a:srcRect，单位 1/1000 百分比，100000 = 100%）
import { computed } from 'vue'
import type { ImageElement } from '@/types/presentation'
import { withHash } from '@/utils/convert'

const props = defineProps<{ el: ImageElement }>()
const emit = defineEmits<{ (e: 'update', props: Partial<ImageElement>): void }>()

const crop = computed(() => props.el.crop ?? { left: 0, top: 0, right: 0, bottom: 0 })

function set(prop: string, value: unknown) {
  emit('update', { [prop]: value } as Partial<ImageElement>)
}

function setCrop(key: 'left' | 'top' | 'right' | 'bottom', value: number) {
  const next = { ...crop.value, [key]: Math.max(0, Math.min(100000, value)) }
  set('crop', next)
}
</script>

<template>
  <div class="panel">
    <h4>图片</h4>
    <div class="row">
      <label>来源</label>
      <input
        type="text"
        :value="el.src"
        @input="(e) => set('src', (e.target as HTMLInputElement).value)"
      />
    </div>
    <label>裁剪（相对源图，0~100%）</label>
    <div class="row">
      <label>上</label>
      <input
        type="number"
        min="0"
        max="100"
        step="1"
        :value="Math.round((crop.top / 100000) * 100)"
        @input="(e) => setCrop('top', Number((e.target as HTMLInputElement).value) * 1000)"
      />
    </div>
    <div class="row">
      <label>下</label>
      <input
        type="number"
        min="0"
        max="100"
        step="1"
        :value="Math.round((crop.bottom / 100000) * 100)"
        @input="(e) => setCrop('bottom', Number((e.target as HTMLInputElement).value) * 1000)"
      />
    </div>
    <div class="row">
      <label>左</label>
      <input
        type="number"
        min="0"
        max="100"
        step="1"
        :value="Math.round((crop.left / 100000) * 100)"
        @input="(e) => setCrop('left', Number((e.target as HTMLInputElement).value) * 1000)"
      />
    </div>
    <div class="row">
      <label>右</label>
      <input
        type="number"
        min="0"
        max="100"
        step="1"
        :value="Math.round((crop.right / 100000) * 100)"
        @input="(e) => setCrop('right', Number((e.target as HTMLInputElement).value) * 1000)"
      />
    </div>
    <button class="small" @click="set('crop', { left: 0, top: 0, right: 0, bottom: 0 })">清除裁剪</button>
    <div class="row">
      <label>边框色</label>
      <input
        type="color"
        :value="withHash(el.line?.color ?? '#000000')"
        @input="(e) => set('line', { color: (e.target as HTMLInputElement).value.slice(1), width: el.line?.width ?? 1 })"
      />
    </div>
    <div class="row">
      <label>边框宽</label>
      <input
        type="number"
        :value="el.line?.width ?? 0"
        min="0"
        max="20"
        step="0.5"
        @input="(e) => set('line', { color: el.line?.color ?? '000000', width: Number((e.target as HTMLInputElement).value) })"
      />
    </div>
  </div>
</template>
