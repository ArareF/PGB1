import { check, type Update } from '@tauri-apps/plugin-updater'
import { relaunch } from '@tauri-apps/plugin-process'
import { ref } from 'vue'

const SKIPPED_VERSION_KEY = 'pgb1-skipped-update-version'
/** 启动后延迟检查（毫秒） */
const CHECK_DELAY_MS = 3000

const updateAvailable = ref(false)
const updateInfo = ref<{ version: string; body: string } | null>(null)
const downloading = ref(false)
const progress = ref(0)         // 0–100
const totalBytes = ref(0)
const downloadedBytes = ref(0)

let pendingUpdate: Update | null = null

export function useUpdater() {
  /** 启动时调用：延迟后静默检查更新 */
  function scheduleCheck() {
    setTimeout(async () => {
      try {
        const update = await check()
        if (!update) return

        const skipped = localStorage.getItem(SKIPPED_VERSION_KEY)
        if (skipped === update.version) return

        pendingUpdate = update
        updateInfo.value = {
          version: update.version,
          body: update.body ?? '',
        }
        updateAvailable.value = true
      } catch (e) {
        console.warn('检查更新失败:', e)
      }
    }, CHECK_DELAY_MS)
  }

  /** 下载并安装更新 */
  async function installUpdate() {
    if (!pendingUpdate) return
    downloading.value = true
    downloadedBytes.value = 0
    totalBytes.value = 0
    progress.value = 0

    try {
      await pendingUpdate.downloadAndInstall((event) => {
        if (event.event === 'Started' && event.data.contentLength) {
          totalBytes.value = event.data.contentLength
        } else if (event.event === 'Progress') {
          downloadedBytes.value += event.data.chunkLength
          if (totalBytes.value > 0) {
            progress.value = Math.min(
              Math.round((downloadedBytes.value / totalBytes.value) * 100),
              100,
            )
          }
        } else if (event.event === 'Finished') {
          progress.value = 100
        }
      })
      await relaunch()
    } catch (e) {
      console.error('更新安装失败:', e)
      downloading.value = false
    }
  }

  /** 跳过此版本（不再提醒） */
  function skipVersion() {
    if (updateInfo.value) {
      localStorage.setItem(SKIPPED_VERSION_KEY, updateInfo.value.version)
    }
    dismiss()
  }

  /** 关闭弹窗（下次启动仍会提醒） */
  function dismiss() {
    updateAvailable.value = false
  }

  return {
    updateAvailable,
    updateInfo,
    downloading,
    progress,
    scheduleCheck,
    installUpdate,
    skipVersion,
    dismiss,
  }
}
