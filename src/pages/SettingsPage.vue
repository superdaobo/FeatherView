<script setup lang="ts">
/**
 * 设置页面：主题、字号、行高、内容宽度、字体、阅读选项。
 * 修改即时生效并持久化。
 */
import { useRouter } from 'vue-router'
import { ArrowLeft, Sun, Moon, Monitor } from 'lucide-vue-next'
import { useSettingsStore } from '../stores/settings'
import { useToast } from '../composables/useToast'
import { clearPositions } from '../services/readingPositionService'
import type { ReaderSettings } from '../types'

const router = useRouter()
const settingsStore = useSettingsStore()
const { show } = useToast()

const themeOptions: Array<{ value: ReaderSettings['theme']; label: string; icon: typeof Sun }> = [
  { value: 'system', label: '跟随系统', icon: Monitor },
  { value: 'light', label: '浅色', icon: Sun },
  { value: 'dark', label: '深色', icon: Moon },
]

const widthOptions: Array<{ value: ReaderSettings['contentWidth']; label: string }> = [
  { value: 'narrow', label: '窄' },
  { value: 'medium', label: '适中' },
  { value: 'wide', label: '宽' },
  { value: 'full', label: '全宽' },
]

const fontOptions: Array<{ value: ReaderSettings['fontFamily']; label: string }> = [
  { value: 'system', label: '系统字体' },
  { value: 'serif', label: '衬线字体' },
  { value: 'monospace', label: '等宽字体' },
]

function adjustLineHeight(delta: number): void {
  settingsStore.settings.lineHeight = Math.min(
    2.6,
    Math.max(1.2, Math.round((settingsStore.settings.lineHeight + delta) * 10) / 10),
  )
}

function showSaved(): void {
  show('设置已保存', { kind: 'success', duration: 1500 })
}

function clearReadingPositions(): void {
  if (window.confirm('确定清除所有阅读位置记录吗？此操作不影响最近文件与其他设置。')) {
    clearPositions()
    show('已清除阅读位置记录', { kind: 'success', duration: 2000 })
  }
}
</script>

<template>
  <div class="page settings-page">
    <header class="toolbar">
      <button
        class="btn-icon"
        type="button"
        title="返回"
        @click="router.back()"
      >
        <ArrowLeft :size="18" />
      </button>
      <span class="toolbar-title">设置</span>
    </header>

    <main class="settings-main">
      <section class="settings-section">
        <h2 class="settings-heading">
          外观
        </h2>

        <div class="setting-row">
          <div class="setting-label">
            <span>主题</span>
            <small>深色与浅色模式，Ctrl+Shift+T 快速切换</small>
          </div>
          <div class="seg">
            <button
              v-for="opt in themeOptions"
              :key="opt.value"
              type="button"
              :class="{ active: settingsStore.settings.theme === opt.value }"
              @click="settingsStore.setTheme(opt.value)"
            >
              <component
                :is="opt.icon"
                :size="14"
                style="vertical-align: -2px; margin-right: 4px"
              />
              {{ opt.label }}
            </button>
          </div>
        </div>

        <div class="setting-row">
          <div class="setting-label">
            <span>字体</span>
            <small>阅读正文字体族</small>
          </div>
          <div class="seg">
            <button
              v-for="opt in fontOptions"
              :key="opt.value"
              type="button"
              :class="{ active: settingsStore.settings.fontFamily === opt.value }"
              @click="settingsStore.settings.fontFamily = opt.value"
            >
              {{ opt.label }}
            </button>
          </div>
        </div>

        <div class="setting-row">
          <div class="setting-label">
            <span>内容宽度</span>
            <small>Markdown 阅读区最大宽度</small>
          </div>
          <div class="seg">
            <button
              v-for="opt in widthOptions"
              :key="opt.value"
              type="button"
              :class="{ active: settingsStore.settings.contentWidth === opt.value }"
              @click="settingsStore.settings.contentWidth = opt.value"
            >
              {{ opt.label }}
            </button>
          </div>
        </div>
      </section>

      <section class="settings-section">
        <h2 class="settings-heading">
          阅读
        </h2>

        <div class="setting-row">
          <div class="setting-label">
            <span>正文字号</span>
            <small>当前 {{ settingsStore.settings.fontSize }}px，Ctrl++ / Ctrl+- 调整</small>
          </div>
          <div class="inline-controls">
            <button
              class="btn btn-small"
              type="button"
              @click="settingsStore.adjustFontSize(-1)"
            >
              A−
            </button>
            <input
              class="range-input"
              type="range"
              min="11"
              max="32"
              step="1"
              :value="settingsStore.settings.fontSize"
              @input="settingsStore.settings.fontSize = Number(($event.target as HTMLInputElement).value)"
            >
            <button
              class="btn btn-small"
              type="button"
              @click="settingsStore.adjustFontSize(1)"
            >
              A+
            </button>
          </div>
        </div>

        <div class="setting-row">
          <div class="setting-label">
            <span>行高</span>
            <small>{{ settingsStore.settings.lineHeight.toFixed(1) }}</small>
          </div>
          <div class="inline-controls">
            <button
              class="btn btn-small"
              type="button"
              @click="adjustLineHeight(-0.1)"
            >
              −
            </button>
            <input
              class="range-input"
              type="range"
              min="1.2"
              max="2.6"
              step="0.1"
              :value="settingsStore.settings.lineHeight"
              @input="settingsStore.settings.lineHeight = Number(($event.target as HTMLInputElement).value)"
            >
            <button
              class="btn btn-small"
              type="button"
              @click="adjustLineHeight(0.1)"
            >
              +
            </button>
          </div>
        </div>

        <div class="setting-row">
          <div class="setting-label">
            <span>自动换行</span>
            <small>文本与代码超出宽度时换行</small>
          </div>
          <label class="switch">
            <input
              v-model="settingsStore.settings.wordWrap"
              type="checkbox"
              @change="showSaved"
            >
            <span class="switch-track" />
          </label>
        </div>

        <div class="setting-row">
          <div class="setting-label">
            <span>显示行号</span>
            <small>文本与代码阅读时显示行号</small>
          </div>
          <label class="switch">
            <input
              v-model="settingsStore.settings.showLineNumbers"
              type="checkbox"
              @change="showSaved"
            >
            <span class="switch-track" />
          </label>
        </div>

        <div class="setting-row">
          <div class="setting-label">
            <span>记住阅读位置</span>
            <small>重新打开文档时回到上次阅读位置</small>
          </div>
          <label class="switch">
            <input
              v-model="settingsStore.settings.rememberReadingPosition"
              type="checkbox"
              @change="showSaved"
            >
            <span class="switch-track" />
          </label>
        </div>

        <div class="setting-row">
          <div class="setting-label">
            <span>清除阅读位置记录</span>
            <small>删除所有已保存的阅读位置，不影响最近文件与其他设置</small>
          </div>
          <button
            class="btn btn-small"
            type="button"
            @click="clearReadingPositions"
          >
            清除记录
          </button>
        </div>
      </section>

      <section class="settings-section">
        <h2 class="settings-heading">
          关于
        </h2>
        <div class="about-box">
          <p><strong>览匣 FeatherView</strong> v0.1.1</p>
          <p class="about-text">
            轻量、本地优先的 Markdown 与文件阅读器。<br>
            应用默认在本地处理文件，不会上传、分析或收集你打开的文件内容。
          </p>
        </div>
      </section>
    </main>
  </div>
</template>

<style scoped>
.settings-main {
  flex: 1;
  overflow-y: auto;
  max-width: 720px;
  width: 100%;
  margin: 0 auto;
  padding: 20px 24px 60px;
}

.settings-section {
  margin-bottom: 26px;
}

.settings-heading {
  font-size: 13px;
  font-weight: 600;
  color: var(--text-secondary);
  text-transform: uppercase;
  letter-spacing: 0.5px;
  border-bottom: 1px solid var(--border);
  padding-bottom: 8px;
  margin: 0 0 6px;
}

.setting-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  padding: 12px 4px;
  border-bottom: 1px solid var(--border);
}

.setting-row:last-child {
  border-bottom: none;
}

.setting-label {
  display: flex;
  flex-direction: column;
  gap: 2px;
  font-size: 14px;
}

.setting-label small {
  font-size: 11.5px;
  color: var(--text-faint);
}

.inline-controls {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-shrink: 0;
}

.range-input {
  width: 130px;
  accent-color: var(--accent);
}

/* 开关 */
.switch {
  position: relative;
  display: inline-block;
  flex-shrink: 0;
}

.switch input {
  opacity: 0;
  width: 0;
  height: 0;
  position: absolute;
}

.switch-track {
  display: block;
  width: 40px;
  height: 22px;
  border-radius: 11px;
  background: var(--border-strong);
  transition: background var(--transition);
  cursor: pointer;
  position: relative;
}

.switch-track::after {
  content: '';
  position: absolute;
  top: 3px;
  left: 3px;
  width: 16px;
  height: 16px;
  border-radius: 50%;
  background: #fff;
  transition: transform var(--transition);
}

.switch input:checked + .switch-track {
  background: var(--accent);
}

.switch input:checked + .switch-track::after {
  transform: translateX(18px);
}

.about-box {
  font-size: 13px;
  color: var(--text-secondary);
  line-height: 1.7;
}

.about-box p {
  margin: 4px 0;
}

.about-text {
  font-size: 12px;
  color: var(--text-faint);
}
</style>
