<script setup lang="ts">
// 位置与尺寸面板
import type { Element } from '@/types/presentation'
import { round2 } from '@/utils/convert'
import { elementRotation } from '@/utils/elements'

const props = defineProps<{ el: Element }>()
const emit = defineEmits<{ (e: 'update', props: Partial<Element>): void }>()

function setPos(key: 'x' | 'y' | 'w' | 'h', value: number) {
  const pos = { ...props.el.position, [key]: round2(value) }
  emit('update', { position: pos })
}

function setRotation(value: number) {
  emit('update', { rotation: round2(value) } as unknown as Partial<Element>)
}
</script>

<template>
  <div class="panel">
    <h4>位置与尺寸</h4>
    <div class="row">
      <label>X</label>
      <input
        type="number"
        step="0.1"
        :value="el.position.x"
        @input="(e) => setPos('x', Number((e.target as HTMLInputElement).value))"
      />
      <label>Y</label>
      <input
        type="number"
        step="0.1"
        :value="el.position.y"
        @input="(e) => setPos('y', Number((e.target as HTMLInputElement).value))"
      />
    </div>
    <div class="row">
      <label>宽</label>
      <input
        type="number"
        step="0.1"
        min="0.05"
        :value="el.position.w"
        @input="(e) => setPos('w', Number((e.target as HTMLInputElement).value))"
      />
      <label>高</label>
      <input
        type="number"
        step="0.1"
        min="0.05"
        :value="el.position.h"
        @input="(e) => setPos('h', Number((e.target as HTMLInputElement).value))"
      />
    </div>
    <div class="row">
      <label>旋转</label>
      <input
        type="number"
        step="5"
        :value="elementRotation(el) ?? 0"
        @input="(e) => setRotation(Number((e.target as HTMLInputElement).value))"
      />
    </div>
  </div>
</template>
