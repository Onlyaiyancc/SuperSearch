import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

// 统一调用素材转换脚本，避免重新生成旧版图标。
const result = spawnSync('python', [fileURLToPath(new URL('./build-icons.py', import.meta.url))], {
  stdio: 'inherit', windowsHide: true,
});
if (result.error) console.error(result.error.message);
process.exitCode = result.status ?? 1;
