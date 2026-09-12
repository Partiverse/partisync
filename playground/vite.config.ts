import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'
import { defineConfig, type Plugin } from 'vite'

const SHOWCASE_FILES = ['basics', 'forms', 'overlays', 'navigation', 'data']

/**
 * 本地插件：构建时（buildStart）与 HMR（handleHotUpdate 监听 src/showcase/*.tsx）时
 * 重新提取 demo 源码，保证 src/demo-sources.generated.ts 与 showcase 文件保持一致（永不过期）。
 */
function demoSourcesPlugin(): Plugin {
  let run: (() => Promise<void>) | null = null

  return {
    name: 'gen-demo-sources',
    async buildStart() {
      let r = run
      if (!r) {
        // 动态 import 生成器（直接函数调用，不走 shell 子进程）
        // @ts-expect-error -- 无类型声明的本地 .mjs 构建脚本
        const mod = (await import('./scripts/gen-demo-sources.mjs')) as {
          runGenerator: () => Promise<void>
        }
        r = run = mod.runGenerator
      }
      await r()
    },
    handleHotUpdate(ctx) {
      const file = ctx.file.replaceAll('\\', '/')
      const base = file.slice(file.lastIndexOf('/') + 1).replace(/\.tsx$/, '')
      if (file.includes('/src/showcase/') && SHOWCASE_FILES.includes(base)) {
        void run?.()
      }
    },
  }
}

export default defineConfig({
  plugins: [react(), tailwindcss(), demoSourcesPlugin()],
  resolve: {
    alias: {
      '@': '/src',
    },
  },
  server: {
    proxy: {
      '/api': {
        target: 'http://localhost:8080',
        changeOrigin: true,
      },
    },
  },
})
