import { createApp } from 'vue'
import { createPinia } from 'pinia'
import App from './App.vue'
import router from './router'
import './styles/main.css'
import './styles/markdown.css'
import './styles/renderers.css'
import { setupRenderers } from './renderers'
import { useSettingsStore } from './stores/settings'

const app = createApp(App)
app.use(createPinia())
app.use(router)

// 注册渲染器（组件懒加载，不阻塞首屏）
setupRenderers()

// 应用已持久化的主题
useSettingsStore().applyTheme()

app.mount('#app')
