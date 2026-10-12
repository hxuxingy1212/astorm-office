//! 对比脚手架：左 = json2docx render（LibreOffice 物理渲染）页图，右 = DocxViewer 同步翻页
import { createApp, defineComponent, h, onMounted, ref } from 'vue'
import { OfficeViewer } from '@astorm/office-viewer'
import type { DocxDocument } from '@astorm/office-viewer'
import '@astorm/office-viewer/style.css'

/** 样张目录名（`?sample=docx-spec`）；页图只对 docx-real 存在 */
const SAMPLE = new URLSearchParams(location.search).get('sample') ?? 'docx-real'
const PHYS_PAGES = SAMPLE === 'docx-real' ? 3 : 0

const App = defineComponent({
  setup() {
    const data = ref<DocxDocument | null>(null)
    const webPage = ref(1)
    const pageCount = ref(0)
    const pane = ref<'both' | 'web' | 'physical'>('both')
    onMounted(async () => {
      data.value = (await (await fetch(`/samples/${SAMPLE}/document.json`)).json()) as DocxDocument
      pane.value = PHYS_PAGES ? 'both' : 'web'
      readCount()
      window.setTimeout(readCount, 800)
    })
    const flip = (d: number) => {
      const btns = document.querySelectorAll<HTMLButtonElement>('.ov-pager .ov-btn')
      const b = d > 0 ? btns[1] : btns[0]
      if (b && !b.disabled) b.click()
    }
    const readCount = () => {
      window.setTimeout(() => {
        const txt = document.querySelector('.ov-pager .ov-page-no')?.textContent ?? ''
        const m = Number(txt.split('/')[1])
        if (m) pageCount.value = m
      }, 150)
    }
    return () =>
      h('div', { style: 'height:100vh;display:flex;flex-direction:column;background:#1c1c1e;color:#eee' }, [
        h('div', { style: 'display:flex;gap:16px;align-items:center;padding:8px 16px;background:#2c2c2e' }, [
          h('strong', null, `对比 ${SAMPLE}：左 = json2docx render（LibreOffice 物理视图） · 右 = web 组件`),
          h('span', null, `web 第 ${webPage.value} / ${pageCount.value || '?'} 页${PHYS_PAGES ? `（物理渲染共 ${PHYS_PAGES} 页）` : ''}`),
          h('button', { onclick: () => flip(-1), style: 'padding:2px 10px' }, '◀ 上一页'),
          h('button', { onclick: () => flip(1), style: 'padding:2px 10px' }, '下一页 ▶'),
          h('select', { onchange: (e: Event) => (pane.value = (e.target as HTMLSelectElement).value as 'both'), value: pane.value }, [
            h('option', { value: 'both' }, '双栏'),
            h('option', { value: 'web' }, '仅 web'),
            h('option', { value: 'physical' }, '仅物理'),
          ]),
        ]),
        data.value
          ? h('div', { style: 'flex:1;min-height:0;display:flex;gap:2px' }, [
              pane.value !== 'web' && PHYS_PAGES
                ? h('div', { style: 'flex:1;min-width:0;overflow:auto;display:flex;justify-content:center' }, [
                    h('img', {
                      src: `/samples/${SAMPLE}/page-${Math.min(webPage.value, PHYS_PAGES)}.png`,
                      style: 'height:100%;object-fit:contain',
                    }),
                  ])
                : null,
              pane.value !== 'physical'
                ? h(
                    'div',
                    { style: 'flex:1;min-width:0;display:flex;flex-direction:column', 'data-web': '1' },
                    [
                      h(
                        'div',
                        {
                          style: 'flex:1;min-height:0;display:flex;flex-direction:column',
                          onPageChange: (i: number) => {
                            webPage.value = i + 1
                            readCount()
                          },
                        },
                        [h(OfficeViewer, { kind: 'docx', data: data.value, zoom: 1 })],
                      ),
                    ],
                  )
                : null,
            ])
          : h('div', { style: 'padding:24px' }, 'loading…'),
      ])
  },
})
createApp(App).mount('#app')
