import { defineConfig, presetAttributify, presetUno } from 'unocss'

export default defineConfig({
  // ...UnoCSS options
  presets: [
    presetUno(),
    presetAttributify(),
  ],
  // 可以添加自定义规则
  rules: [
    // ...
  ],
  // 可以添加快捷方式
  shortcuts: {
    // ...
  }
})