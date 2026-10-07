//! 后端 API 封装

import type { Overview, Slide } from '@/types/presentation'

async function request<T>(url: string, init?: RequestInit): Promise<T> {
  const res = await fetch(url, init)
  if (!res.ok) {
    let message = `HTTP ${res.status}`
    try {
      const body = await res.json()
      if (body && body.error) message = body.error
    } catch {
      /* 非 JSON 响应 */
    }
    throw new Error(message)
  }
  return res.json() as Promise<T>
}

/** 上传 PPTX 并建立会话，返回 overview */
export async function uploadPptx(file: File): Promise<Overview> {
  const form = new FormData()
  form.append('file', file)
  return request<Overview>('/api/upload', { method: 'POST', body: form })
}

/** 上传媒体（图片）到当前会话媒体目录，返回产物内相对路径 */
export async function uploadMedia(file: File): Promise<string> {
  const form = new FormData()
  form.append('file', file)
  const res = await request<{ success: boolean; src: string }>('/api/media', {
    method: 'POST',
    body: form,
  })
  return res.src
}

/** 获取当前会话 overview */
export function fetchOverview(): Promise<Overview> {
  return request<Overview>('/api/overview')
}

/** 写回单张幻灯片 JSON */
export async function saveSlide(index: number, slide: Slide): Promise<void> {
  await request(`/api/slide/${index}`, {
    method: 'PUT',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(slide),
  })
}

/** 全部幻灯片保存 */
export async function saveAllSlides(slides: Slide[]): Promise<void> {
  for (let i = 0; i < slides.length; i += 1) {
    await saveSlide(i + 1, slides[i])
  }
}

/** repack 产物为 PPTX 并触发下载 */
export async function repackAndDownload(filename: string): Promise<void> {
  const res = await fetch('/api/repack', { method: 'POST' })
  if (!res.ok) {
    let message = `HTTP ${res.status}`
    try {
      const body = await res.json()
      if (body && body.error) message = body.error
    } catch {
      /* ignore */
    }
    throw new Error(message)
  }
  const blob = await res.blob()
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = filename
  a.click()
  URL.revokeObjectURL(url)
}
