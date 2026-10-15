import { defineConfig, mergeConfig } from 'vitest/config';
import viteConfig from './vite.config';

// 限定项目测试目录，覆盖前端和工具链测试，排除历史工作副本。
export default mergeConfig(viteConfig, defineConfig({
  test: { include: ['tests/**/*.test.ts', 'tests/**/*.test.mjs'] },
}));
