//! 内置图标库：图标名称 → SVG path（viewBox 0 0 24 24，风格化的单色图标）
//!
//! 图标元素在前端用 SVG 渲染；打包时后端降级为椭圆形状。

export interface IconDef {
  name: string
  label: string
  paths: string[]
}

const ICONS: IconDef[] = [
  { name: 'star', label: '星形', paths: ['M12 2l3.09 6.26L22 9.27l-5 4.87 1.18 6.88L12 17.77l-6.18 3.25L7 14.14 2 9.27l6.91-1.01L12 2z'] },
  { name: 'heart', label: '爱心', paths: ['M12 21s-7.5-4.9-9.75-9a5.5 5.5 0 1 1 9.75-3.5A5.5 5.5 0 1 1 21.75 12C19.5 16.1 12 21 12 21z'] },
  { name: 'check', label: '对勾', paths: ['M9 16.2 4.8 12l-1.4 1.4L9 19 21 7l-1.4-1.4L9 16.2z'] },
  { name: 'close', label: '叉', paths: ['M19 6.4 17.6 5 12 10.6 6.4 5 5 6.4 10.6 12 5 17.6 6.4 19 12 13.4 17.6 19 19 17.6 13.4 12 19 6.4z'] },
  { name: 'plus', label: '加号', paths: ['M13 5h-2v6H5v2h6v6h2v-6h6v-2h-6V5z'] },
  { name: 'minus', label: '减号', paths: ['M5 11h14v2H5z'] },
  { name: 'arrow', label: '箭头', paths: ['M12 4l-7 7h4v8h6v-8h4l-7-7z'] },
  { name: 'search', label: '搜索', paths: ['M15.5 14h-.8l-.3-.3a6.5 6.5 0 1 0-.7.7l.3.3v.8l5 5 1.5-1.5-5-5zm-6 0a4.5 4.5 0 1 1 0-9 4.5 4.5 0 0 1 0 9z'] },
  { name: 'home', label: '主页', paths: ['M12 3 3 12h3v8h5v-6h2v6h5v-8h3L12 3z'] },
  { name: 'mail', label: '邮件', paths: ['M20 4H4a2 2 0 0 0-2 2v12a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2V6a2 2 0 0 0-2-2zm0 4-8 5-8-5V6l8 5 8-5v2z'] },
  { name: 'user', label: '用户', paths: ['M12 12a4 4 0 1 0 0-8 4 4 0 0 0 0 8zm0 2c-4 0-8 2-8 6v2h16v-2c0-4-4-6-8-6z'] },
  { name: 'chart', label: '图表', paths: ['M3 3h2v18H3V3zm6 6h2v12H9V9zm6 3h2v9h-2v-9zm6-6h2v15h-2V6z'] },
  { name: 'calendar', label: '日历', paths: ['M7 2v2H5a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2V6a2 2 0 0 0-2-2h-2V2h-2v2H9V2H7zm12 8v10H5V10h14z'] },
  { name: 'bell', label: '提醒', paths: ['M12 2a6 6 0 0 0-6 6v4l-2 2v2h16v-2l-2-2V8a6 6 0 0 0-6-6zm-2 16a2 2 0 0 0 4 0h-4z'] },
  { name: 'doc', label: '文档', paths: ['M6 2h8l4 4v16H6V2zm8 1.5V7h3.5L14 3.5zM8 10h8v2H8v-2zm0 4h8v2H8v-2z'] },
  { name: 'camera', label: '相机', paths: ['M9 3 7.2 5H4a2 2 0 0 0-2 2v12a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2V7a2 2 0 0 0-2-2h-3.2L15 3H9zm3 6a5 5 0 1 1 0 10 5 5 0 0 1 0-10z'] },
  { name: 'clock', label: '时钟', paths: ['M12 2a10 10 0 1 0 0 20 10 10 0 0 0 0-20zm1 5h-2v6l5 3 1-1.7-4-2.3V7z'] },
  { name: 'eye', label: '眼睛', paths: ['M12 5C7 5 2.7 8 1 12c1.7 4 6 7 11 7s9.3-3 11-7c-1.7-4-6-7-11-7zm0 11a4 4 0 1 1 0-8 4 4 0 0 1 0 8z'] },
  { name: 'flag', label: '旗帜', paths: ['M5 3v18h2V4h10v8H7v2h10a2 2 0 0 0 2-2V4a2 2 0 0 0-2-2H5z'] },
  { name: 'gear', label: '齿轮', paths: ['M12 8a4 4 0 1 0 0 8 4 4 0 0 0 0-8zm9 4c0-.7-.1-1.3-.2-2l2-1.6-2-3.4-2.4 1a7 7 0 0 0-1.7-1L16.5 2h-4l-.3 2.5c-.6.2-1.2.5-1.7 1l-2.4-1-2 3.4 2 1.6a8 8 0 0 0 .1 2L6.1 13l2 3.4 2.4-1c.5.5 1.1.8 1.7 1l.3 2.6h4l.3-2.5c.6-.2 1.2-.5 1.7-1l2.4 1 2-3.4-2-1.6c.1-.7.2-1.3.2-2z'] },
]

export const ICON_LIST: IconDef[] = ICONS

/** 按名称获取图标定义 */
export function iconDef(name: string): IconDef | undefined {
  return ICONS.find((i) => i.name === name)
}
