import { spawn } from 'node:child_process';
import { existsSync, mkdirSync, readdirSync } from 'node:fs';
import { delimiter, dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const scriptPath = fileURLToPath(import.meta.url);
const projectRoot = dirname(dirname(scriptPath));

export function cmakeEnvironment(environment) {
  if (Object.keys(environment).some(key => key.toLowerCase() === 'cmake')) return environment;
  // 优先沿用用户配置；仅给子进程补充已安装的 VS CMake，不修改全局 PATH。
  for (const prefix of [environment.ProgramFiles, environment['ProgramFiles(x86)']].filter(Boolean)) {
    const root = join(prefix, 'Microsoft Visual Studio');
    if (!existsSync(root)) continue;
    for (const version of readdirSync(root, { withFileTypes: true }).filter(entry => entry.isDirectory()).reverse()) {
      const directory = join(root, version.name);
      for (const edition of readdirSync(directory, { withFileTypes: true }).filter(entry => entry.isDirectory())) {
        const cmake = join(directory, edition.name, 'Common7/IDE/CommonExtensions/Microsoft/CMake/CMake/bin/cmake.exe');
        if (existsSync(cmake)) return { ...environment, CMAKE: cmake };
      }
    }
  }
  return environment;
}

export function describeLaunch(mode = 'dev', projectPath = projectRoot, environment = process.env, extraArgs = []) {
  if (!['dev', 'build', 'test', 'check', 'smoke', 'benchmark', 'audit', 'audit-install'].includes(mode)) {
    throw new Error(`Unknown desktop mode: ${mode}`);
  }
  environment = cmakeEnvironment(environment);

  const cargoHome = join(projectPath, '.tools/cargo');
  const localCargo = join(cargoHome, 'bin/cargo.exe');
  const useLocalRust = existsSync(localCargo);
  const tempPath = join(projectPath, '.tools/tmp');
  const inheritedPath = Object.entries(environment).find(([key]) => key.toLowerCase() === 'path')?.[1];
  const overrides = {
    ...(useLocalRust ? { CARGO_HOME: cargoHome, RUSTUP_HOME: join(projectPath, '.tools/rustup') } : {}),
    npm_config_cache: join(projectPath, '.tools/npm-cache'),
    TEMP: tempPath,
    TMP: tempPath,
    PATH: useLocalRust ? join(cargoHome, 'bin') + (inheritedPath ? delimiter + inheritedPath : '') : inheritedPath ?? '',
  };
  // 只给子进程设置本地工具链，避免 Windows 环境变量出现大小写重复键。
  const overriddenKeys = new Set(Object.keys(overrides).map((key) => key.toLowerCase()));
  const childEnvironment = Object.fromEntries(Object.entries(environment)
    .filter(([key]) => !overriddenKeys.has(key.toLowerCase())));
  Object.assign(childEnvironment, overrides);

  const isTauri = mode === 'dev' || mode === 'build';
  return {
    command: isTauri ? process.execPath : useLocalRust ? localCargo : 'cargo',
    args: isTauri
      ? [join(projectPath, 'node_modules/@tauri-apps/cli/tauri.js'), mode, ...extraArgs]
      : mode === 'smoke' || mode === 'benchmark'
        ? ['run', '--manifest-path', 'src-tauri/Cargo.toml', '--', mode === 'benchmark' ? '--ui-benchmark' : '--smoke-test', ...extraArgs]
        : mode === 'audit'
          ? ['audit', '--file', 'src-tauri/Cargo.lock', ...extraArgs]
          : mode === 'audit-install'
            ? ['install', 'cargo-audit', '--locked', ...extraArgs]
            : [mode, '--manifest-path', 'src-tauri/Cargo.toml', ...extraArgs],
    options: { cwd: projectPath, env: childEnvironment, stdio: 'inherit', windowsHide: true },
  };
}

// 导入时仅提供启动描述；直接执行脚本时才创建目录并启动子进程。
if (process.argv[1] && resolve(process.argv[1]) === scriptPath) {
  const mode = process.argv[2] ?? 'dev';
  try {
    const { command, args, options } = describeLaunch(mode, projectRoot, process.env, process.argv.slice(3));
    mkdirSync(options.env.TEMP, { recursive: true });
    const child = spawn(command, args, options);
    child.on('error', (error) => {
      console.error(`Failed to launch desktop ${mode}: ${error.message}. Install Rust with the MSVC toolchain and restart your terminal; see docs/development.md.`);
      process.exitCode = 1;
    });
    child.on('exit', (code) => { process.exitCode = code ?? 1; });
  } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
  }
}
