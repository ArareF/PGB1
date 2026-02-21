# ConvertPage 实现计划

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 将格式转换功能从模态弹窗改为独立路由页面，进度显示集成在右侧控制区内，不再使用悬浮窗。

**Architecture:** 新增 `/project/:projectId/task/:taskId/convert` 路由 → `ConvertPage.vue`。左侧素材区直接复用 ConversionDialog 的卡片展示逻辑（静帧分区 + 序列帧分区 + FPS 输入），右侧控制区含选中统计、进度展示、操作按钮。转换执行逻辑（invoke + listen）从 TaskPage 迁移过来，后端命令不变。

**Tech Stack:** Vue 3 Composition API, Vue Router, Tauri invoke + listen, 复用 MaterialCard.vue / useMaterials.ts / useSettings.ts

---

## 关键约定

- **过滤规则**：progress 为 `done` 或 `uploaded` 的素材不显示
- **默认全选**：进入页面时所有待转换素材默认选中
- **FPS 校验**：选中的序列帧必须填写 1-120 的整数，否则「开始制作」按钮禁用
- **进度集成**：执行后右侧控制区切换为进度模式，显示进度条 + 提示文字 + 「完成转换」按钮
- **完成后**：点「完成转换」停止会话并 `router.back()`
- **taskPath / settings**：通过路由 query 传 taskPath，settings 在页面内独立加载

---

## Task 1：路由注册

**Files:**
- Modify: `src/router/index.ts`

**Step 1: 在 scale 路由后新增 convert 路由**

```ts
{
  path: '/project/:projectId/task/:taskId/convert',
  name: 'convert',
  component: () => import('../views/ConvertPage.vue'),
},
```

---

## Task 2：TaskPage 转换按钮改为路由跳转 + 清理废弃代码

**Files:**
- Modify: `src/views/TaskPage.vue`

**Step 1: 找到 updateNavigation 中的转换按钮 handler（约第 389 行），改为 router.push**

当前：
```ts
{ id: 'convert', label: '转换', handler: () => { showConversionDialog.value = true } },
```

改为：
```ts
{ id: 'convert', label: '转换', handler: () => router.push({ name: 'convert', params: { projectId, taskId }, query: { taskPath: taskFolderPath } }) },
```

**Step 2: 删除以下废弃变量和 import**

- `import ConversionDialog` → 删除
- `const showConversionDialog = ref(false)` → 删除
- `const isConverting = ref(false)` → 删除
- `const conversionProgress = ref(...)` → 删除
- `let unlistenOrganized` → 删除
- `handleStartConversion` 函数 → 删除
- `stopConversion` 函数 → 删除
- `onUnmounted` 中的 `unlistenOrganized` 清理 → 删除（若 onUnmounted 因此变空则删除整个 onUnmounted）

**Step 3: 删除模板中的 ConversionDialog 标签和转换进度悬浮窗**

搜索并删除：
- `<ConversionDialog` 整段（含属性和事件）
- `<!-- 转换进度悬浮窗 -->` 整段（从 `<Transition name="fade">` 到对应 `</Transition>`）

**Step 4: 验证编译无报错**

```bash
cd D:/work/pgsoft/PGB1
npm run build 2>&1 | head -30
```

---

## Task 3：创建 ConvertPage.vue

**Files:**
- Create: `src/views/ConvertPage.vue`

**完整文件内容：**

```vue
<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { useNavigation } from '../composables/useNavigation'
import { useMaterials } from '../composables/useMaterials'
import { useSettings } from '../composables/useSettings'
import type { MaterialInfo } from '../composables/useMaterials'
import MaterialCard from '../components/MaterialCard.vue'

const route = useRoute()
const router = useRouter()
const { setNavigation } = useNavigation()
const { materials, loading, loadMaterials } = useMaterials()
const { settings, loadSettings } = useSettings()

const taskId = route.params.taskId as string
const taskPath = route.query.taskPath as string

// ─── 素材过滤 ────────────────────────────────────────

const pendingImages = computed(() =>
  materials.value.filter(m =>
    m.material_type === 'image' &&
    m.progress !== 'done' &&
    m.progress !== 'uploaded'
  )
)

const pendingSequences = computed(() =>
  materials.value.filter(m =>
    m.material_type === 'sequence' &&
    m.progress !== 'done' &&
    m.progress !== 'uploaded'
  )
)

const totalPending = computed(() => pendingImages.value.length + pendingSequences.value.length)

// ─── 选中状态 ────────────────────────────────────────

const selectedPaths = ref<Set<string>>(new Set())

function toggleItem(path: string) {
  const newSet = new Set(selectedPaths.value)
  if (newSet.has(path)) {
    newSet.delete(path)
  } else {
    newSet.add(path)
  }
  selectedPaths.value = newSet
}

function toggleSelectAll() {
  if (selectedPaths.value.size === totalPending.value) {
    selectedPaths.value = new Set()
  } else {
    const all = new Set<string>()
    pendingImages.value.forEach(m => all.add(m.path))
    pendingSequences.value.forEach(m => all.add(m.path))
    selectedPaths.value = all
  }
}

const selectedImageCount = computed(() =>
  pendingImages.value.filter(m => selectedPaths.value.has(m.path)).length
)

const selectedSequenceCount = computed(() =>
  pendingSequences.value.filter(m => selectedPaths.value.has(m.path)).length
)

// ─── FPS 映射 ────────────────────────────────────────

const fpsMap = ref<Record<string, string>>({})

// ─── 校验 ────────────────────────────────────────────

const canStart = computed(() => {
  if (selectedPaths.value.size === 0) return false
  for (const seq of pendingSequences.value) {
    if (selectedPaths.value.has(seq.path)) {
      const fps = Number(fpsMap.value[seq.path])
      if (isNaN(fps) || fps < 1 || fps > 120) return false
    }
  }
  return true
})

// ─── 转换执行 ────────────────────────────────────────

const isConverting = ref(false)
const conversionProgress = ref({ current: 0, total: 0 })
let unlistenOrganized: (() => void) | null = null

async function handleStart() {
  if (!canStart.value || !taskPath) return

  const images: Record<string, number> = {}
  const sequences: { name: string; fps: number }[] = []

  for (const img of pendingImages.value) {
    if (selectedPaths.value.has(img.path)) {
      images[img.name] = 0
    }
  }
  for (const seq of pendingSequences.value) {
    if (selectedPaths.value.has(seq.path)) {
      sequences.push({ name: seq.name, fps: Number(fpsMap.value[seq.path]) })
    }
  }

  isConverting.value = true
  conversionProgress.value = { current: 0, total: Object.keys(images).length + sequences.length }

  try {
    if (unlistenOrganized) unlistenOrganized()
    unlistenOrganized = await listen<string>('conversion-organized', () => {
      conversionProgress.value.current++
    })

    if (!settings.value) throw new Error('应用设置未加载')

    await invoke('start_conversion', {
      request: {
        task_path: taskPath,
        images,
        sequences,
        imagine_path: settings.value.workflow.imaginePath,
        texture_packer_cli_path: settings.value.workflow.texturePackerCliPath,
        texture_packer_gui_path: settings.value.workflow.texturePackerGuiPath,
      }
    })

    if (sequences.length > 0) {
      await invoke('execute_sequence_conversion', { sequences })
    }
  } catch (err) {
    console.error('转换流程启动失败:', err)
    isConverting.value = false
  }
}

async function handleFinish() {
  try {
    await invoke('stop_conversion')
  } catch (err) {
    console.error('停止转换失败:', err)
  } finally {
    isConverting.value = false
    if (unlistenOrganized) {
      unlistenOrganized()
      unlistenOrganized = null
    }
    router.back()
  }
}

onUnmounted(() => {
  if (unlistenOrganized) unlistenOrganized()
})

// ─── 初始化 ──────────────────────────────────────────

onMounted(async () => {
  setNavigation({
    title: `转换 · ${taskId}`,
    showBackButton: true,
    onBack: () => router.back(),
    actions: [],
    moreMenuItems: [],
  })
  await Promise.all([
    taskPath ? loadMaterials(taskPath) : Promise.resolve(),
    loadSettings(),
  ])
  // 默认全选
  const all = new Set<string>()
  pendingImages.value.forEach(m => all.add(m.path))
  pendingSequences.value.forEach(m => all.add(m.path))
  selectedPaths.value = all
})
</script>

<template>
  <div class="convert-page">
    <!-- 左侧：素材区 -->
    <div class="card-area custom-scroll">
      <p v-if="loading" class="hint-text">扫描中...</p>
      <div v-else-if="totalPending === 0" class="hint-text">暂无需要转换的素材</div>
      <template v-else>
        <!-- 静帧分区 -->
        <div v-if="pendingImages.length > 0" class="section">
          <p class="section-label">静帧素材 ({{ pendingImages.length }})</p>
          <div class="material-grid">
            <MaterialCard
              v-for="m in pendingImages"
              :key="m.path"
              :material="m"
              :multi-select="true"
              :checked="selectedPaths.has(m.path)"
              class="mini-card"
              @click="toggleItem(m.path)"
            />
          </div>
        </div>

        <!-- 序列帧分区 -->
        <div v-if="pendingSequences.length > 0" class="section">
          <p class="section-label">序列帧素材 ({{ pendingSequences.length }})</p>
          <div class="material-grid">
            <div
              v-for="m in pendingSequences"
              :key="m.path"
              class="seq-item-container"
            >
              <MaterialCard
                :material="m"
                :multi-select="true"
                :checked="selectedPaths.has(m.path)"
                class="mini-card"
                @click="toggleItem(m.path)"
              />
              <div class="fps-control">
                <span class="fps-label">FPS:</span>
                <input
                  v-model="fpsMap[m.path]"
                  type="text"
                  class="fps-input"
                  placeholder="24"
                  maxlength="3"
                  :disabled="!selectedPaths.has(m.path)"
                />
              </div>
            </div>
          </div>
        </div>
      </template>
    </div>

    <!-- 右侧：控制区 -->
    <aside class="control-panel glass-medium">
      <!-- 选择模式 -->
      <div v-if="!isConverting" class="panel-body">
        <p class="panel-title">格式转换</p>

        <div class="stats">
          <div class="stat-row">
            <span class="stat-label">静帧</span>
            <span class="stat-value">{{ selectedImageCount }} / {{ pendingImages.length }}</span>
          </div>
          <div class="stat-row">
            <span class="stat-label">序列帧</span>
            <span class="stat-value">{{ selectedSequenceCount }} / {{ pendingSequences.length }}</span>
          </div>
        </div>

        <button class="ghost-btn" @click="toggleSelectAll">
          {{ selectedPaths.size === totalPending ? '取消全选' : '全选' }}
        </button>

        <p v-if="selectedSequenceCount > 0" class="fps-hint">
          序列帧需填写帧率（1-120）
        </p>
      </div>

      <!-- 进度模式 -->
      <div v-else class="panel-body">
        <p class="panel-title">正在转换...</p>

        <div class="progress-section">
          <div class="progress-count">
            {{ conversionProgress.current }} / {{ conversionProgress.total }}
          </div>
          <div class="progress-track">
            <div
              class="progress-fill"
              :style="{ width: conversionProgress.total > 0 ? (conversionProgress.current / conversionProgress.total * 100) + '%' : '0%' }"
            />
          </div>
          <p class="progress-hint">
            <template v-if="conversionProgress.current < conversionProgress.total">
              请在外部工具中完成导出，程序将自动分类归位
            </template>
            <template v-else>
              转换已全部完成！
            </template>
          </p>
        </div>
      </div>

      <div class="panel-footer">
        <template v-if="!isConverting">
          <button class="cancel-btn" @click="router.back()">取消</button>
          <button
            class="execute-btn"
            :disabled="!canStart"
            @click="handleStart"
          >
            开始制作
          </button>
        </template>
        <template v-else>
          <button
            class="execute-btn"
            :class="{ done: conversionProgress.current >= conversionProgress.total }"
            @click="handleFinish"
          >
            完成转换
          </button>
        </template>
      </div>
    </aside>
  </div>
</template>

<style scoped>
.convert-page {
  display: flex;
  height: 100%;
  gap: var(--spacing-4);
  padding: var(--spacing-4);
  overflow: hidden;
}

/* 左侧素材区 */
.card-area {
  flex: 1;
  overflow-y: auto;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: var(--spacing-6);
}

.hint-text {
  color: var(--text-tertiary);
  font-size: var(--text-sm);
  padding: var(--spacing-8);
  text-align: center;
}

.section-label {
  font-size: var(--text-base);
  font-weight: var(--font-bold);
  color: var(--text-secondary);
  margin-bottom: var(--spacing-3);
  padding-bottom: var(--spacing-2);
  border-bottom: 1px solid var(--border-medium);
}

.material-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(150px, 1fr));
  gap: var(--spacing-3);
}

.seq-item-container {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-2);
}

.fps-control {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: var(--spacing-2);
}

.fps-label {
  font-size: var(--text-xs);
  color: var(--text-secondary);
  font-weight: var(--font-bold);
}

.fps-input {
  width: 40px;
  height: 24px;
  text-align: center;
  border-radius: var(--radius-sm);
  border: 1px solid var(--border-heavy);
  background: var(--bg-tertiary);
  color: var(--text-primary);
  font-size: var(--text-xs);
  font-weight: var(--font-bold);
  transition: all var(--duration-fast);
}

.fps-input:focus {
  outline: none;
  border-color: var(--color-primary-500);
  background: var(--bg-primary);
}

.fps-input:disabled {
  opacity: 0.5;
  background: var(--bg-app);
  color: var(--text-tertiary);
}

.mini-card {
  --card-material-width: 100% !important;
  --card-material-padding: var(--spacing-2) !important;
  --card-material-gap: var(--spacing-2) !important;
}

.mini-card :deep(.card-name) { font-size: var(--text-xs) !important; }
.mini-card :deep(.progress-tag) { height: 18px !important; font-size: 10px !important; padding: 0 4px !important; }
.mini-card :deep(.size-tag) { font-size: 10px !important; }
.mini-card :deep(.checkbox) { width: 16px !important; height: 16px !important; }

/* 右侧控制区 */
.control-panel {
  width: 220px;
  flex-shrink: 0;
  border-radius: var(--radius-2xl);
  display: flex;
  flex-direction: column;
  justify-content: space-between;
  overflow: hidden;
}

.panel-body {
  padding: var(--spacing-5);
  display: flex;
  flex-direction: column;
  gap: var(--spacing-4);
  flex: 1;
}

.panel-title {
  font-size: var(--text-base);
  font-weight: var(--font-bold);
  color: var(--text-primary);
}

.stats {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-2);
}

.stat-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.stat-label {
  font-size: var(--text-sm);
  color: var(--text-secondary);
}

.stat-value {
  font-size: var(--text-sm);
  font-weight: var(--font-bold);
  color: var(--text-primary);
}

.ghost-btn {
  background: transparent;
  border: 1px solid var(--border-medium);
  color: var(--color-primary-500);
  font-size: var(--text-sm);
  font-weight: var(--font-bold);
  cursor: pointer;
  padding: var(--spacing-2) var(--spacing-3);
  border-radius: var(--radius-md);
  transition: all var(--duration-fast);
  width: 100%;
}

.ghost-btn:hover {
  background: var(--bg-hover);
}

.fps-hint {
  font-size: var(--text-xs);
  color: var(--text-tertiary);
  line-height: 1.4;
}

/* 进度模式 */
.progress-section {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-3);
}

.progress-count {
  font-size: var(--text-2xl);
  font-weight: var(--font-bold);
  color: var(--text-primary);
  text-align: center;
}

.progress-track {
  height: 8px;
  background: var(--border-heavy);
  border-radius: var(--radius-full);
  overflow: hidden;
}

.progress-fill {
  height: 100%;
  background: var(--color-primary-500);
  border-radius: var(--radius-full);
  transition: width var(--duration-normal);
}

.progress-hint {
  font-size: var(--text-xs);
  color: var(--text-tertiary);
  line-height: 1.4;
}

/* 底部操作区 */
.panel-footer {
  padding: var(--spacing-4) var(--spacing-5);
  border-top: 1px solid var(--border-light);
  display: flex;
  gap: var(--spacing-2);
}

.cancel-btn {
  flex: 1;
  height: 36px;
  border-radius: var(--radius-md);
  border: 1px solid var(--border-medium);
  background: transparent;
  color: var(--text-secondary);
  font-size: var(--text-sm);
  cursor: pointer;
  transition: all var(--duration-fast);
}

.cancel-btn:hover {
  background: var(--bg-hover);
}

.execute-btn {
  flex: 2;
  height: 36px;
  border-radius: var(--radius-md);
  border: none;
  background: var(--color-primary-500);
  color: white;
  font-weight: var(--font-bold);
  font-size: var(--text-sm);
  cursor: pointer;
  transition: all var(--duration-fast);
}

.execute-btn:hover:not(:disabled) {
  background: var(--color-primary-600);
}

.execute-btn:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

.execute-btn.done {
  background: var(--color-success-500, #22c55e);
}

.execute-btn.done:hover {
  background: var(--color-success-600, #16a34a);
}
</style>
```

---

## Task 4：整体验收

**手动测试流程：**

1. 启动 `npm run tauri dev`
2. 打开任意任务页，点「转换」→ 跳转到 ConvertPage，标题显示「转换 · 任务名」
3. 确认静帧/序列帧分区正确显示，默认全选
4. 取消部分选中，确认「静帧 X/Y」统计正确更新
5. 选中序列帧但不填 FPS →「开始制作」按钮灰色
6. 填入有效 FPS → 按钮可点击
7. 点「开始制作」→ 右侧切换为进度模式，进度条随转换推进
8. 点「完成转换」→ 停止会话，返回任务页
9. 确认 TaskPage 的「转换」按钮已正确跳转，不再弹窗
