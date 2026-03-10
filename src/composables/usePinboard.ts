import { ref, computed, unref, type Ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { convertFileSrc } from '@tauri-apps/api/core'
import { readImage } from '@tauri-apps/plugin-clipboard-manager'

// ─── 类型 ─────────────────────────────────────────────────

export interface PinAnnotation {
  type: 'pen' | 'arrow' | 'rect' | 'ellipse' | 'text' | 'eraser'
  color: string
  strokeWidth: number
  points?: [number, number][]
  start?: [number, number]
  end?: [number, number]
  text?: string
  position?: [number, number]
  fontSize?: number
}

export interface PinInfo {
  id: string
  image: string
  x: number
  y: number
  width: number
  height: number
  annotations: PinAnnotation[]
  zIndex: number
  created_at: string
}

export interface PinboardViewport {
  panX: number
  panY: number
  zoom: number
}

export interface PinboardCanvasData {
  pins: PinInfo[]
  viewport: PinboardViewport | null
  annotations: PinAnnotation[]
}

// ─── Composable ────────────────────────────────────────────

export function usePinboard(dirPath: Ref<string> | string, canvasKey: Ref<string> | string) {
  const pins = ref<PinInfo[]>([])
  const canvasAnnotations = ref<PinAnnotation[]>([])
  const viewport = ref<PinboardViewport>({ panX: 0, panY: 0, zoom: 1 })
  const loading = ref(false)

  async function loadPinboard() {
    const dir = unref(dirPath)
    const key = unref(canvasKey)
    if (!dir || !key) return
    loading.value = true
    try {
      const canvas = await invoke<PinboardCanvasData>('get_pinboard', { dirPath: dir, key })
      pins.value = canvas.pins ?? []
      canvasAnnotations.value = canvas.annotations ?? []
      if (canvas.viewport) {
        viewport.value = canvas.viewport
      } else {
        viewport.value = { panX: 0, panY: 0, zoom: 1 }
      }
    } catch (e) {
      console.error('加载贴图板失败:', e)
      pins.value = []
    } finally {
      loading.value = false
    }
  }

  async function savePinboard() {
    const dir = unref(dirPath)
    const key = unref(canvasKey)
    if (!dir || !key) return
    try {
      await invoke('save_pinboard', {
        dirPath: dir,
        key,
        canvas: {
          pins: pins.value,
          viewport: viewport.value,
          annotations: canvasAnnotations.value,
        },
      })
    } catch (e) {
      console.error('保存贴图板失败:', e)
    }
  }

  async function pasteImage(): Promise<PinInfo | null> {
    const dir = unref(dirPath)
    if (!dir) return null

    try {
      // 使用 Tauri clipboard 插件读取图片（RGBA 格式）
      const clipImage = await readImage()
      if (!clipImage) return null

      const [rgba, imgSize] = await Promise.all([clipImage.rgba(), clipImage.size()])
      const imgWidth = imgSize.width
      const imgHeight = imgSize.height

      // 发送 RGBA + 尺寸给 Rust，由 Rust 编码为 PNG
      const [filename, savedW, savedH] = await invoke<[string, number, number]>('save_pin_image', {
        dirPath: dir,
        imageData: Array.from(rgba),
        width: imgWidth,
        height: imgHeight,
      })

      // 初始显示尺寸：最大 600px 宽，等比缩放
      const MAX_INITIAL_WIDTH = 600
      let w = savedW
      let h = savedH
      if (w > MAX_INITIAL_WIDTH) {
        h = Math.round(h * (MAX_INITIAL_WIDTH / w))
        w = MAX_INITIAL_WIDTH
      }

      const pin: PinInfo = {
        id: filename.replace('.png', ''),
        image: filename,
        x: 50 + Math.random() * 100,
        y: 50 + Math.random() * 100,
        width: w,
        height: h,
        annotations: [],
        zIndex: pins.value.length > 0
          ? Math.max(...pins.value.map(p => p.zIndex)) + 1
          : 1,
        created_at: new Date().toISOString(),
      }

      pins.value.push(pin)
      await savePinboard()
      return pin
    } catch (e) {
      console.error('粘贴图片失败:', e)
      return null
    }
  }

  async function deletePin(pinId: string) {
    const dir = unref(dirPath)
    if (!dir) return
    const pin = pins.value.find(p => p.id === pinId)
    if (!pin) return

    pins.value = pins.value.filter(p => p.id !== pinId)

    try {
      await invoke('delete_pin_image', { dirPath: dir, filename: pin.image })
    } catch (e) {
      console.error('删除贴图文件失败:', e)
    }

    await savePinboard()
  }

  function updatePin(pinId: string, updates: Partial<PinInfo>) {
    const idx = pins.value.findIndex(p => p.id === pinId)
    if (idx === -1) return
    pins.value[idx] = { ...pins.value[idx], ...updates }
  }

  function bringToFront(pinId: string) {
    const maxZ = pins.value.reduce((max, p) => Math.max(max, p.zIndex), 0)
    updatePin(pinId, { zIndex: maxZ + 1 })
  }

  function getPinImageUrl(pin: PinInfo): string {
    const dir = unref(dirPath)
    if (!dir) return ''
    const filePath = `${dir}\\.pgb1_pins\\${pin.image}`
    return convertFileSrc(filePath)
  }

  const hasPins = computed(() => pins.value.length > 0)

  return {
    pins,
    canvasAnnotations,
    viewport,
    loading,
    hasPins,
    loadPinboard,
    savePinboard,
    pasteImage,
    deletePin,
    updatePin,
    bringToFront,
    getPinImageUrl,
  }
}
