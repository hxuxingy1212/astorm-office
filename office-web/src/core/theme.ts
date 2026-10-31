//! 主题开关：theme prop（light / dark / auto）→ `.ov-dark` 类。
//!
//! auto（默认）跟随系统 `prefers-color-scheme` 并监听变化实时切换；
//! 类挂在各 viewer 根元素上，CSS 变量随 `.ov-dark` 一组生效。
//! 宿主也可自行在外层容器加 `ov-dark`（chrome 暗）与 `ov-dark-pages`（页面画布一起暗），
//! 两条路径可并存——组件只管理自己根元素上的类，不动外层。
import { onBeforeUnmount, onMounted, ref, watch, type Ref } from 'vue'

export type OvTheme = 'light' | 'dark' | 'auto'

const darkMql =
  typeof matchMedia === 'function' ? matchMedia('(prefers-color-scheme: dark)') : null

export function useOvTheme(theme: () => OvTheme | undefined): Ref<boolean> {
  const dark = ref(false)
  const apply = () => {
    const t = theme()
    dark.value = t === 'dark' || (t !== 'light' && !!darkMql?.matches)
  }
  apply()
  watch(theme, apply)
  const onChange = () => apply()
  onMounted(() => darkMql?.addEventListener('change', onChange))
  onBeforeUnmount(() => darkMql?.removeEventListener('change', onChange))
  return dark
}
