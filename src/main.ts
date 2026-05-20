import './assets/main.css'
import 'virtual:uno.css'
import 'virtual:unocss-devtools'

import { createApp } from 'vue'
import { createPinia } from 'pinia'

import App from './App.vue'

const app = createApp(App)

app.use(createPinia())

app.mount('#app')
