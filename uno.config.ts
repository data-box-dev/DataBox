import { defineConfig, presetAttributify, presetUno } from 'unocss'

export default defineConfig({
  presets: [
    presetAttributify(),
    presetUno(),
  ],
  shortcuts: {
    'h-100%': 'height: 100%',
    'min-h-0': 'min-height: 0',
    'flex-1': 'flex: 1',
  },
})
