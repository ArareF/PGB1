# 规范化完成项折叠 Implementation Plan

> **For Codex:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 规范化页面将已完成命名规范化的素材稳定下沉到底部并默认折叠，同时保留其内容操作与恢复能力。

**Architecture:** 后端扫描结果与 `selections[]` 平行数组保持不变；前端通过纯函数按 `needs_rename` 稳定分组，并携带原始索引供行内操作读写。页面只改变展示顺序和可见性，不改变执行请求、备份或恢复逻辑。

**Tech Stack:** Vue 3、TypeScript、vue-i18n、Node.js 内置测试运行器

---

### Task 1: 锁定稳定分组行为

**Files:**
- Create: `tests/normalizeItems.test.mjs`
- Create: `src/utils/normalizeItems.ts`

**Step 1: Write the failing test**

测试混排输入被拆为待规范化与已规范化两组，并验证每项保留原始索引。

**Step 2: Run test to verify it fails**

Run: `node --experimental-strip-types tests/normalizeItems.test.mjs`
Expected: FAIL，原因是 `normalizeItems.ts` 尚不存在。

**Step 3: Write minimal implementation**

实现泛型纯函数 `partitionNormalizeItems()`，单次遍历完成稳定分组。

**Step 4: Run test to verify it passes**

Run: `node --experimental-strip-types tests/normalizeItems.test.mjs`
Expected: PASS。

### Task 2: 接入规范化页面折叠区

**Files:**
- Modify: `src/views/NormalizePage.vue`
- Modify: `src/locales/zh-CN.ts`
- Modify: `src/locales/en.ts`

**Step 1: Add view state and grouped rows**

增加默认关闭的展开状态；待规范化组始终渲染，已规范化组仅在展开时渲染。

**Step 2: Add the bottom disclosure control**

折叠栏显示组名、数量和展开状态，使用现有 Design System token，并设置 `aria-expanded`。

**Step 3: Preserve row operations**

所有 checkbox、预览和恢复操作继续使用分组项携带的原始索引访问 `selections[]`。

### Task 3: 同步设计与代码索引

**Files:**
- Modify: `design/文件命名与组织规则.md`
- Modify: `docs/code/views.md`
- Modify: `CODE_INDEX.md`

记录“待规范化置顶、已规范化置底且默认折叠”的交互规则，并更新页面行数口径。

### Task 4: 验证

**Step 1:** 运行纯函数测试。

**Step 2:** 运行 `npm run check:i18n-parity`。

**Step 3:** 运行 `npm run build`，覆盖 TypeScript、项目检查与生产构建。

**Step 4:** 检查 `git diff --check` 与目标文件 diff，确认无无关格式化。
