//! 媒体解析：组件库通过 provide/inject 注入，默认回退到 `/api/media/`（与 ai-ppt/web 后端一致）
import { inject, provide, type InjectionKey } from 'vue'

export type MediaResolver = (src: string) => string

export const MEDIA_RESOLVER: InjectionKey<MediaResolver> = Symbol('office-media-resolver')

/** 在 Viewer 内提供解析器 */
export function provideMediaResolver(resolver: MediaResolver): void {
  provide(MEDIA_RESOLVER, resolver)
}

/** 元素组件内取用（未注入时用默认 /api/media/ 前缀） */
export function useMediaResolver(): MediaResolver {
  return inject(MEDIA_RESOLVER, (src: string) => (src.startsWith('http') || src.startsWith('data:') ? src : `/api/media/${src}`))
}
