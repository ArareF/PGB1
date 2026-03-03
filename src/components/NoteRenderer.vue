<script setup lang="ts">
import { computed } from 'vue'
import { openUrl } from '@tauri-apps/plugin-opener'

const props = defineProps<{
  text: string
}>()

const emit = defineEmits<{
  'toggle-checkbox': [lineIndex: number]
}>()

/** 行内解析：命名链接 | 裸URL → 链接，**..** → 粗体，*..*→ 斜体，其余 → 文本 */
interface InlineSegment {
  type: 'text' | 'link' | 'strong' | 'em'
  content: string
  /** 命名链接的目标 URL；裸链接时为 undefined（href = content） */
  href?: string
}

function parseInline(text: string): InlineSegment[] {
  const segments: InlineSegment[] = []
  // 合并正则：命名链接 [text](url) | 裸URL | 粗体 | 斜体
  const INLINE_RE = /\[([^\]]+)\]\((https?:\/\/[^)]+)\)|(https?:\/\/\S+)|\*\*(.+?)\*\*|(?<!\*)\*(?!\*)(.+?)(?<!\*)\*(?!\*)/g
  let lastIndex = 0
  let match: RegExpExecArray | null

  while ((match = INLINE_RE.exec(text)) !== null) {
    // 前置纯文本
    if (match.index > lastIndex) {
      segments.push({ type: 'text', content: text.slice(lastIndex, match.index) })
    }
    if (match[1] && match[2]) {
      // 命名链接 [text](url)
      segments.push({ type: 'link', content: match[1], href: match[2] })
    } else if (match[3]) {
      // 裸URL
      segments.push({ type: 'link', content: match[3] })
    } else if (match[4]) {
      segments.push({ type: 'strong', content: match[4] })
    } else if (match[5]) {
      segments.push({ type: 'em', content: match[5] })
    }
    lastIndex = match.index + match[0].length
  }
  // 尾部纯文本
  if (lastIndex < text.length) {
    segments.push({ type: 'text', content: text.slice(lastIndex) })
  }
  return segments
}

interface ParsedLine {
  isCheckbox: boolean
  checked: boolean
  segments: InlineSegment[]
}

const parsedLines = computed<ParsedLine[]>(() => {
  if (!props.text) return []
  return props.text.split('\n').map(line => {
    const checkedMatch = line.match(/^- \[([ x])\] (.*)$/i)
    if (checkedMatch) {
      return {
        isCheckbox: true,
        checked: checkedMatch[1].toLowerCase() === 'x',
        segments: parseInline(checkedMatch[2]),
      }
    }
    return {
      isCheckbox: false,
      checked: false,
      segments: parseInline(line),
    }
  })
})

function onLinkClick(url: string) {
  openUrl(url)
}

function onCheckboxToggle(lineIndex: number) {
  emit('toggle-checkbox', lineIndex)
}
</script>

<template>
  <div class="note-rendered">
    <div
      v-for="(line, idx) in parsedLines"
      :key="`${idx}-${line.checked}`"
      :class="line.isCheckbox ? 'checkbox-line' : 'text-line'"
    >
      <!-- Checkbox 行 -->
      <template v-if="line.isCheckbox">
        <label class="checkbox-label" @click.prevent="onCheckboxToggle(idx)">
          <input
            type="checkbox"
            :checked="line.checked"
            class="checkbox-input"
            tabindex="-1"
            @click.prevent
          />
          <span :class="{ 'checkbox-text-done': line.checked }">
            <template v-for="(seg, si) in line.segments" :key="si">
              <a
                v-if="seg.type === 'link'"
                class="note-link"
                href="#"
                @click.prevent="onLinkClick(seg.href ?? seg.content)"
              >{{ seg.content }}</a>
              <strong v-else-if="seg.type === 'strong'">{{ seg.content }}</strong>
              <em v-else-if="seg.type === 'em'">{{ seg.content }}</em>
              <span v-else>{{ seg.content }}</span>
            </template>
          </span>
        </label>
      </template>

      <!-- 普通文本行 -->
      <template v-else>
        <template v-for="(seg, si) in line.segments" :key="si">
          <a
            v-if="seg.type === 'link'"
            class="note-link"
            href="#"
            @click.prevent="onLinkClick(seg.href ?? seg.content)"
          >{{ seg.content }}</a>
          <strong v-else-if="seg.type === 'strong'">{{ seg.content }}</strong>
          <em v-else-if="seg.type === 'em'">{{ seg.content }}</em>
          <span v-else>{{ seg.content }}</span>
        </template>
      </template>
    </div>
  </div>
</template>
