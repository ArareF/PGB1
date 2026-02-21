# ScalePage 实现计划

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 将缩放功能从模态弹窗改为独立路由页面，支持对不同素材分批标注不同缩放比例，一次执行。

**Architecture:** 新增 `/project/:projectId/task/:taskId/scale` 路由 → `ScalePage.vue`。页面左侧为素材卡片区（复用 MaterialCard），右侧为控制区（比例选择 + 应用）。MaterialCard 通过新增 `scaleLabel` prop 在右下角 size-tag 位置显示已标注比例，替代原来的文件大小。执行时按标注批量调用已有 Rust 命令 `execute_scaling`。

**Tech Stack:** Vue 3 Composition API, Vue Router, Tauri invoke, 复用 MaterialCard.vue / useMaterials.ts

---

## 关键约定

- **标注覆盖**：同一素材再次应用新比例，覆盖旧标注（不累加）
- **标注清除**：选中已有标注的卡片，应用相同比例 → 清除标注（toggle）
- **卡片 tag 复用**：MaterialCard 新增可选 prop `scaleLabel?: string`，有值时右下角显示 `70%` 样式标签替代 size-tag，样式复用 `.size-tag`
- **执行范围**：只执行有标注的素材，没标注的跳过
- **执行完成**：成功后自动 `router.back()` 返回任务页

---

## Task 1：路由注册

**Files:**
- Modify: `src/router/index.ts`

**Step 1: 在现有 task 路由后新增 scale 路由**

```ts
{
  path: '/project/:projectId/task/:taskId/scale',
  name: 'scale',
  component: () => import('../views/ScalePage.vue'),
},
```

位置：插在 `task` 路由（第 17 行）之后。

**Step 2: 验证**

启动开发服务器后手动访问 `/project/test/task/test/scale`，应显示空白页（组件文件还不存在会报错，但路由已注册）。

---

## Task 2：MaterialCard 新增 scaleLabel prop

**Files:**
- Modify: `src/components/MaterialCard.vue`

**Step 1: 在 props 定义中新增 scaleLabel**

当前 props（约第 7 行）：
```ts
const props = defineProps<{
  material: MaterialInfo
  multiSelect?: boolean
  checked?: boolean
}>()
```

改为：
```ts
const props = defineProps<{
  material: MaterialInfo
  multiSelect?: boolean
  checked?: boolean
  scaleLabel?: string   // 新增：缩放标注，有值时替换右下角 size-tag
}>()
```

**Step 2: 在模板 card-tags 区域修改 size-tag**

当前（约第 84-89 行）：
```html
<div class="card-tags">
  <span class="progress-tag" :class="`progress-${material.progress}`">
    {{ progressLabel(material.progress) }}
  </span>
  <span class="size-tag">{{ formatSize(material.size_bytes) }}</span>
</div>
```

改为：
```html
<div class="card-tags">
  <span class="progress-tag" :class="`progress-${material.progress}`">
    {{ progressLabel(material.progress) }}
  </span>
  <span v-if="scaleLabel" class="size-tag scale-label-tag">{{ scaleLabel }}</span>
  <span v-else class="size-tag">{{ formatSize(material.size_bytes) }}</span>
</div>
```

**Step 3: 新增 scale-label-tag 样式**

在 `<style scoped>` 末尾追加：
```css
.scale-label-tag {
  color: var(--color-primary-500);
  font-weight: var(--font-bold);
}
```

**Step 4: 验证**

在任务页随便给一张卡片传 `scale-label="70%"` 临时测试，确认右下角显示蓝色 `70%`，然后撤销测试代码。

---

## Task 3：创建 ScalePage.vue

**Files:**
- Create: `src/views/ScalePage.vue`

**完整文件内容：**

```vue
<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { invoke } from '@tauri-apps/api/core'
import { useNavigation } from '../composables/useNavigation'
import { useMaterials } from '../composables/useMaterials'
import type { MaterialInfo } from '../composables/useMaterials'
import MaterialCard from '../components/MaterialCard.vue'

const route = useRoute()
const router = useRouter()
const { setNavigation } = useNavigation()
const { materials, loading, loadMaterials } = useMaterials()

const projectId = route.params.projectId as string
const taskId = route.params.taskId as string

// taskPath 从路由 query 获取（TaskPage 跳转时传入）
const taskPath = route.query.taskPath as string

// 比例选择
const PRESET_SCALES = [100, 70, 50, 40]
const selectedScale = ref(70)
const customScale = ref('')

const finalScale = computed(() => {
  if (customScale.value && !isNaN(Number(customScale.value))) {
    return Math.min(100, Math.max(1, Number(customScale.value)))
  }
  return selectedScale.value
})

// 标注 Map：material.path → scale number
const scaleMap = ref<Map<string, number>>(new Map())

// 当前选中的卡片路径集合
const selectedPaths = ref<Set<string>>(new Set())

// 执行状态
const executing = ref(false)
const error = ref<string | null>(null)

// 只展示静帧
const imageMaterials = computed(() =>
  materials.value.filter(m => m.material_type === 'image')
)

// 有标注的素材数量（用于按钮 disabled 判断）
const annotatedCount = computed(() => scaleMap.value.size)

onMounted(async () => {
  setNavigation({
    title: `缩放 · ${taskId}`,
    showBackButton: true,
    onBack: () => router.back(),
    actions: [],
    moreMenuItems: [],
  })
  if (taskPath) {
    await loadMaterials(taskPath)
  }
})

function selectPreset(scale: number) {
  selectedScale.value = scale
  customScale.value = ''
}

function handleCustomInput(e: Event) {
  const val = (e.target as HTMLInputElement).value
  customScale.value = val.replace(/[^\d]/g, '')
  if (customScale.value) {
    selectedScale.value = 0
  }
}

function toggleCard(m: MaterialInfo) {
  const newSet = new Set(selectedPaths.value)
  if (newSet.has(m.path)) {
    newSet.delete(m.path)
  } else {
    newSet.add(m.path)
  }
  selectedPaths.value = newSet
}

// 应用比例到选中卡片（覆盖旧标注；若已有相同标注则清除）
function applyScale() {
  if (selectedPaths.value.size === 0) return
  const scale = finalScale.value
  if (scale <= 0) return

  const newMap = new Map(scaleMap.value)
  selectedPaths.value.forEach(path => {
    if (newMap.get(path) === scale) {
      // 同比例再次应用 → 清除
      newMap.delete(path)
    } else {
      // 覆盖标注
      newMap.set(path, scale)
    }
  })
  scaleMap.value = newMap
  // 应用后清空选中
  selectedPaths.value = new Set()
}

// 返回某素材的标注文字（用于 scaleLabel prop）
function scaleLabelFor(m: MaterialInfo): string | undefined {
  const s = scaleMap.value.get(m.path)
  return s !== undefined ? `${s}%` : undefined
}

async function handleExecute() {
  if (annotatedCount.value === 0 || !taskPath) return
  executing.value = true
  error.value = null

  try {
    const requests: { original_path: string; target_dir: string; scale_percent: number; base_name: string }[] = []

    scaleMap.value.forEach((scale, path) => {
      const m = imageMaterials.value.find(m => m.path === path)
      if (!m) return
      requests.push({
        original_path: m.path,
        target_dir: `${taskPath}\\01_scale\\[${scale}]`,
        scale_percent: scale,
        base_name: m.name,
      })
    })

    await invoke('execute_scaling', { requests })
    router.back()
  } catch (e) {
    error.value = String(e)
    console.error('执行缩放失败:', e)
  } finally {
    executing.value = false
  }
}
</script>

<template>
  <div class="scale-page">
    <!-- 左侧：素材卡片区 -->
    <div class="card-area">
      <p v-if="loading" class="hint-text">扫描中...</p>
      <p v-else-if="imageMaterials.length === 0" class="hint-text">无静帧素材</p>
      <div v-else class="card-grid">
        <MaterialCard
          v-for="m in imageMaterials"
          :key="m.path"
          :material="m"
          :multi-select="true"
          :checked="selectedPaths.has(m.path)"
          :scale-label="scaleLabelFor(m)"
          @click="toggleCard(m)"
        />
      </div>
    </div>

    <!-- 右侧：控制区 -->
    <aside class="control-panel glass-medium">
      <div class="panel-body">
        <p class="panel-title">缩放比例</p>

        <div class="scale-options">
          <button
            v-for="s in PRESET_SCALES"
            :key="s"
            class="scale-btn"
            :class="{ active: selectedScale === s && !customScale }"
            @click="selectPreset(s)"
          >
            {{ s }}%
          </button>
        </div>

        <div class="custom-row">
          <div class="custom-input-wrapper">
            <input
              type="text"
              class="custom-input"
              placeholder="自定义"
              :value="customScale"
              @input="handleCustomInput"
            />
            <span class="input-suffix">%</span>
          </div>
        </div>

        <button
          class="apply-btn"
          :disabled="selectedPaths.size === 0 || finalScale <= 0"
          @click="applyScale"
        >
          应用到选中 ({{ selectedPaths.size }})
        </button>
      </div>

      <div class="panel-footer">
        <div v-if="error" class="error-msg">{{ error }}</div>
        <div v-if="executing" class="executing-hint">执行中...</div>
        <div class="footer-actions">
          <button class="cancel-btn" :disabled="executing" @click="router.back()">取消</button>
          <button
            class="execute-btn"
            :disabled="annotatedCount === 0 || executing"
            @click="handleExecute"
          >
            {{ executing ? '执行中...' : `开始缩放 (${annotatedCount})` }}
          </button>
        </div>
      </div>
    </aside>
  </div>
</template>

<style scoped>
.scale-page {
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
}

.card-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(var(--card-material-width, 160px), 1fr));
  gap: var(--spacing-4);
  padding: 2px;
}

.hint-text {
  color: var(--text-tertiary);
  font-size: var(--text-sm);
  padding: var(--spacing-8);
  text-align: center;
}

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
}

.panel-title {
  font-size: var(--text-base);
  font-weight: var(--font-bold);
  color: var(--text-primary);
}

.scale-options {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-2);
}

.scale-btn {
  width: 100%;
  height: 36px;
  border-radius: var(--radius-md);
  border: 1px solid var(--border-medium);
  background: var(--bg-tertiary);
  color: var(--text-primary);
  font-weight: var(--font-medium);
  cursor: pointer;
  transition: all var(--duration-fast);
  text-align: left;
  padding: 0 var(--spacing-3);
}

.scale-btn:hover {
  background: var(--bg-hover);
}

.scale-btn.active {
  background: var(--color-primary-500);
  border-color: var(--color-primary-500);
  color: white;
}

.custom-row {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-2);
}

.custom-input-wrapper {
  position: relative;
  display: flex;
  align-items: center;
}

.custom-input {
  width: 100%;
  height: 36px;
  padding: 0 28px 0 var(--spacing-3);
  border-radius: var(--radius-md);
  border: 1px solid var(--border-medium);
  background: var(--bg-tertiary);
  color: var(--text-primary);
  font-size: var(--text-sm);
}

.custom-input:focus {
  outline: none;
  border-color: var(--color-primary-500);
}

.input-suffix {
  position: absolute;
  right: var(--spacing-2);
  color: var(--text-tertiary);
  font-size: var(--text-xs);
  pointer-events: none;
}

.apply-btn {
  width: 100%;
  height: 36px;
  border-radius: var(--radius-md);
  border: none;
  background: var(--color-primary-100);
  color: var(--color-primary-600);
  font-weight: var(--font-bold);
  font-size: var(--text-sm);
  cursor: pointer;
  transition: all var(--duration-fast);
}

.apply-btn:hover:not(:disabled) {
  background: var(--color-primary-200);
}

.apply-btn:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

/* 底部操作区 */
.panel-footer {
  padding: var(--spacing-4) var(--spacing-5);
  border-top: 1px solid var(--border-light);
  display: flex;
  flex-direction: column;
  gap: var(--spacing-3);
}

.footer-actions {
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

.cancel-btn:hover:not(:disabled) {
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

.error-msg {
  padding: var(--spacing-2) var(--spacing-3);
  background: var(--color-danger-light);
  color: var(--color-danger-dark);
  border-radius: var(--radius-md);
  font-size: var(--text-xs);
}

.executing-hint {
  font-size: var(--text-xs);
  color: var(--text-tertiary);
  text-align: center;
}
</style>
```

---

## Task 4：TaskPage 缩放按钮改为路由跳转

**Files:**
- Modify: `src/views/TaskPage.vue`

**Step 1: 找到 startScaling 函数（约第 283 行），替换整个函数**

当前：
```ts
function startScaling() {
  if (isMultiSelect.value && selectedPaths.value.size > 0) {
    materialsToScale.value = selectedMaterials.value.filter(m => m.material_type === 'image')
  } else {
    materialsToScale.value = materials.value.filter(m =>
      m.material_type === 'image' &&
      m.progress !== 'scaled' &&
      m.progress !== 'done' &&
      m.progress !== 'uploaded'
    )
  }
  showScalingDialog.value = true
}
```

改为：
```ts
function startScaling() {
  router.push({
    name: 'scale',
    params: { projectId, taskId },
    query: { taskPath: taskFolderPath },
  })
}
```

**Step 2: 验证**

点任务页「缩放」按钮，应跳转到 ScalePage，TitleBar 显示「缩放 · [任务名]」，返回按钮正常回到任务页。

---

## Task 5：清理废弃代码

**Files:**
- Modify: `src/views/TaskPage.vue`

**Step 1: 删除以下已无用的变量和 import（按行号搜索）**

- `import ScalingDialog` → 删除
- `const showScalingDialog = ref(false)` → 删除
- `const materialsToScale = ref<MaterialInfo[]>([])` → 删除

**Step 2: 删除模板中的 ScalingDialog 组件标签**

搜索 `<ScalingDialog`，删除整段（含属性和事件绑定）。

**Step 3: 验证编译无报错**

```bash
cd D:/work/pgsoft/PGB1
npm run build 2>&1 | head -30
```

期望：无 TypeScript 或 Vue 编译错误。

---

## Task 6：整体验收

**手动测试流程：**

1. 启动开发服务器：`npm run tauri dev`
2. 打开任意任务页
3. 点「缩放」→ 跳转到 ScalePage，顶部显示「缩放 · 任务名」
4. 点几张卡片（高亮），右侧选「70%」→ 点「应用到选中」→ 卡片右下角显示蓝色 `70%`，选中清空
5. 点另外几张卡片，选「50%」→ 应用 → 那些卡片显示 `50%`
6. 点一张已标 70% 的卡片 → 再选 70% → 应用 → 标注清除
7. 点「开始缩放 (N)」→ 执行 → 自动返回任务页
8. 刷新任务页，确认卡片进度更新

**边界情况验证：**
- 没有选中任何卡片时，「应用到选中」按钮为灰色
- 没有标注任何卡片时，「开始缩放」按钮为灰色
- 自定义比例输入非数字时被过滤掉
