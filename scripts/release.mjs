import { createHash, randomUUID } from 'node:crypto';
import { existsSync, lstatSync, mkdirSync, readFileSync, readdirSync, realpathSync, renameSync, unlinkSync, writeFileSync } from 'node:fs';
import { dirname, isAbsolute, join, relative, resolve, sep } from 'node:path';
import { fileURLToPath } from 'node:url';
import { verificationSteps } from './verify.mjs';

const scriptPath = fileURLToPath(import.meta.url);
const defaultOutput = 'artifacts/releases/Priority-Optimization-2026-10-15';
const evidenceDefaults = {
  verification: 'test-results/verification.json',
  uiBenchmark: 'test-results/ui-native-benchmark.json',
  resources: 'test-results/resource-usage.json',
};
const maximumEvidenceAgeMs = 24 * 60 * 60 * 1000;
const executableInputs = ['src', 'public', 'index.html', 'svelte.config.js', 'src-tauri/src',
  'src-tauri/vendor', 'src-tauri/resources', 'src-tauri/icons', 'src-tauri/capabilities',
  'package.json', 'package-lock.json', 'src-tauri/Cargo.toml', 'src-tauri/Cargo.lock',
  'src-tauri/tauri.conf.json', 'src-tauri/build.rs', 'vite.config.ts', 'tsconfig.json',
  'LICENSE', 'THIRD-PARTY-NOTICES.md', 'docs/licenses'];
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
const jsonBytes = value => Buffer.from(JSON.stringify(value, null, 2) + '\n');

export function parseReleaseArgs(args) {
  const options = { output: defaultOutput };
  let outputSeen = false;
  for (let index = 0; index < args.length; index++) {
    if (args[index] === '--with-diagnostics' && !options.diagnostics) options.diagnostics = true;
    else if (args[index] === '--output' && !outputSeen && args[index + 1] && !args[index + 1].startsWith('--')) {
      outputSeen = true;
      options.output = args[++index];
    } else throw new Error('Unsupported release option. Usage: node scripts/release.mjs [--output artifacts/releases/<new-directory>] [--with-diagnostics]');
  }
  return options;
}

function checkedPath(root, path) {
  if (typeof path !== 'string' || isAbsolute(path) || path.includes(':') || path.split(/[\\/]/).includes('..')) {
    throw new Error(`Unsafe project path: ${path}`);
  }
  const target = resolve(root, path);
  const within = relative(root, target);
  if (!within || within.startsWith('..') || isAbsolute(within)) throw new Error(`Unsafe project path: ${path}`);
  // 拒绝目录联接和符号链接，避免通过现有路径写到项目外。
  let current = root;
  for (const part of within.split(sep)) {
    current = join(current, part);
    if (existsSync(current) && lstatSync(current).isSymbolicLink()) throw new Error(`Symbolic link or junction is not allowed: ${path}`);
  }
  return target;
}

function loadFile(root, path) {
  const target = checkedPath(root, path);
  const stat = lstatSync(target);
  if (!stat.isFile() || !stat.size) throw new Error(`Missing or empty release input: ${path}`);
  const bytes = readFileSync(target);
  return { bytes, mtimeMs: stat.mtimeMs, sha256: hash(bytes) };
}

function parseJson(file) {
  // PowerShell 5 的 UTF-8 报告可能带 BOM。
  return JSON.parse(file.bytes.toString('utf8').replace(/^\uFEFF/, ''));
}

function requireFresh(file, after, now, label) {
  if (file.mtimeMs < after || file.mtimeMs > now || now - file.mtimeMs > maximumEvidenceAgeMs) {
    throw new Error(`${label} must be fresh and generated after the final build.`);
  }
}

function requirePairedReport(resources, nativeFile) {
  if (typeof resources.nativeReportSha256 !== 'string' || resources.nativeReportSha256.toLowerCase() !== nativeFile.sha256) {
    throw new Error('Native diagnostic report digest does not match its paired resource evidence.');
  }
}

function newestSourceTime(root, paths = [...executableInputs, 'src-tauri/windows', 'scripts', 'tests', 'vitest.config.ts']) {
  let newest = 0;
  const visit = path => {
    const target = checkedPath(root, path);
    if (!existsSync(target)) return;
    const stat = lstatSync(target);
    if (stat.isDirectory()) {
      for (const name of readdirSync(target)) visit(join(path, name));
    } else if (stat.isFile()) newest = Math.max(newest, stat.mtimeMs);
  };
  for (const path of paths) visit(path);
  return newest;
}

export function publishRelease({ projectRoot = dirname(dirname(scriptPath)), output = defaultOutput,
  evidence = evidenceDefaults, now = new Date(), diagnostics = false } = {}) {
  const root = realpathSync(projectRoot);
  const currentTime = new Date(now).getTime();
  if (!Number.isFinite(currentTime)) throw new Error('Invalid publication time.');
  // 发布目录仅允许 artifacts/releases 下一个普通子目录，禁止路径遍历、设备名和备用数据流。
  const match = typeof output === 'string' && output.match(/^artifacts[\\/]releases[\\/]([A-Za-z0-9][A-Za-z0-9._-]*)$/);
  if (!match || match[1].endsWith('.') || /^(?:con|prn|aux|nul|com[1-9]|lpt[1-9])(?:\.|$)/i.test(match[1])) {
    throw new Error('Output must be a new directory directly under artifacts/releases.');
  }
  const outputDirectory = `artifacts/releases/${match[1]}`;
  const destination = checkedPath(root, outputDirectory);
  const latestPath = checkedPath(root, 'artifacts/releases/Latest-Release.json');
  if (existsSync(destination)) throw new Error(`Release output already exists: ${outputDirectory}`);
  if (existsSync(latestPath) && !lstatSync(latestPath).isFile()) throw new Error('Latest release pointer must be a regular file.');

  const packageVersion = parseJson(loadFile(root, 'package.json')).version;
  const tauriVersion = parseJson(loadFile(root, 'src-tauri/tauri.conf.json')).version;
  const cargoPackage = loadFile(root, 'src-tauri/Cargo.toml').bytes.toString('utf8')
    .split(/^\[package\]\s*$/m)[1]?.split(/^\[/m)[0];
  const cargoVersion = cargoPackage?.match(/^\s*version\s*=\s*["']([^"']+)["']\s*(?:#.*)?$/m)?.[1];
  if (!/^\d+\.\d+\.\d+(?:-[A-Za-z0-9.-]+)?$/.test(packageVersion) || packageVersion !== tauriVersion || packageVersion !== cargoVersion) {
    throw new Error('Release versions must match in package.json, Cargo.toml and tauri.conf.json.');
  }
  const installerName = `SuperSearch_${packageVersion}_x64-setup.exe`;
  const executable = loadFile(root, 'src-tauri/target/release/supersearch.exe');
  const installer = loadFile(root, `src-tauri/target/release/bundle/nsis/${installerName}`);
  const finalBuildTime = Math.max(executable.mtimeMs, installer.mtimeMs);
  const paths = { ...evidenceDefaults, ...evidence };
  const verificationFile = loadFile(root, paths.verification);
  const uiFile = loadFile(root, paths.uiBenchmark);
  const resourceFile = loadFile(root, paths.resources);
  const verification = parseJson(verificationFile);
  const ui = parseJson(uiFile);
  const resources = parseJson(resourceFile);
  const requiredSteps = verificationSteps({ smoke: true, auditRust: true }).map(step => step.name);
  if (verification.outcome !== 'passed' || verification.exitCode !== 0 || verification.version !== packageVersion ||
      !Array.isArray(verification.steps) || verification.steps.length !== requiredSteps.length ||
      requiredSteps.some((name, index) => verification.steps[index]?.name !== name) ||
      verification.steps.some(step => step.outcome !== 'passed' || step.exitCode !== 0 || step.error)) {
    throw new Error('Verification must pass every required check, including bundled smoke and Rust audit, for this version.');
  }
  requireFresh(verificationFile, finalBuildTime, currentTime, 'Verification report');
  const verifiedAt = Date.parse(verification.timestamp);
  if (!Number.isFinite(verifiedAt) || verifiedAt < finalBuildTime || verifiedAt > currentTime || currentTime - verifiedAt > maximumEvidenceAgeMs) {
    throw new Error('Verification timestamp must be fresh and follow the final build.');
  }
  if (verifiedAt < newestSourceTime(root)) throw new Error('Verification predates a changed source or configuration file.');
  // 检查与构建并不等价；旧 EXE 即使重新检查通过，也不能覆盖后来修改的生产输入。
  const productionInputTime = newestSourceTime(root, executableInputs);
  if (executable.mtimeMs < productionInputTime) {
    throw new Error('Executable predates a production input; rebuild the desktop application.');
  }
  // Tauri 完成 NSIS 后恢复便携 EXE，允许最多一秒的收尾写入；生产输入及钩子仍严格比较。
  if (installer.mtimeMs < Math.max(executable.mtimeMs - 1000, productionInputTime, newestSourceTime(root, ['src-tauri/windows']))) {
    throw new Error('Installer predates its executable or installation hooks; rebuild the desktop installer.');
  }
  requireFresh(uiFile, executable.mtimeMs, currentTime, 'Native UI evidence');
  requireFresh(resourceFile, executable.mtimeMs, currentTime, 'Resource evidence');
  if (ui.outcome !== 'passed' || ui.native !== true || ui.visibleWindow !== true ||
      ['unique', 'broad'].some(name => {
        const firstResult = ui.scenarios?.[name]?.firstResult;
        return !firstResult || !(firstResult.samples > 0) || !Number.isFinite(firstResult.p95Ms) || firstResult.p95Ms < 0;
      })) throw new Error('Native UI evidence must pass with a visible window and measured search scenarios.');
  if (resources.outcome !== 'passed' || resources.webViewObserved !== true || !Number.isInteger(resources.sampleCount) ||
      resources.sampleCount <= 0 || resources.samples?.length !== resources.sampleCount) {
    throw new Error('Resource evidence must pass with observed WebView and nonempty native process samples.');
  }
  if (typeof resources.executableSha256 !== 'string' || resources.executableSha256.toLowerCase() !== executable.sha256 ||
      (ui.executableSha256 !== undefined && ui.executableSha256.toLowerCase() !== executable.sha256)) {
    throw new Error('Native evidence executable hash does not match the final executable.');
  }
  requirePairedReport(resources, uiFile);

  const entries = [
    ['SuperSearch.exe', executable], [installerName, installer],
    ['Verification-Report.json', verificationFile], ['UI-Native-Benchmark-Report.json', uiFile],
    ['Resource-Usage-Report.json', resourceFile],
    ['LICENSE', loadFile(root, 'LICENSE')],
    ['THIRD-PARTY-NOTICES.md', loadFile(root, 'THIRD-PARTY-NOTICES.md')],
    ['THIRD-PARTY-LICENSES.txt', loadFile(root, 'docs/licenses/THIRD-PARTY-LICENSES.txt')],
  ];
  if (diagnostics) {
    // 浸泡和大目录证据只有时长、采样和最终 EXE 哈希全部符合要求才可发布。
    const soakFile = loadFile(root, 'test-results/ui-native-soak.json');
    const soakResourcesFile = loadFile(root, 'test-results/resource-soak.json');
    requireFresh(soakFile, executable.mtimeMs, currentTime, 'Native soak evidence');
    requireFresh(soakResourcesFile, executable.mtimeMs, currentTime, 'Resource soak evidence');
    const soak = parseJson(soakFile);
    const soakResources = parseJson(soakResourcesFile);
    if (soak.outcome !== 'passed' || soak.native !== true || !(soak.requestedDurationMs >= 1800000) ||
        !(soak.soakElapsedMs >= soak.requestedDurationMs) || !(soak.cycles > 0) ||
        soakResources.outcome !== 'passed' || !(soakResources.sampleCount >= 100) ||
        !(soakResources.elapsedMs >= soak.requestedDurationMs * .95) || !(soakResources.roles?.webview?.samples >= 100) ||
        soakResources.executableSha256?.toLowerCase() !== executable.sha256) {
      throw new Error('Soak evidence must cover at least half an hour with native cycles and matching resource samples.');
    }
    requirePairedReport(soakResources, soakFile);
    const scaleFile = loadFile(root, 'test-results/scale-native-benchmark.json');
    const scaleResourcesFile = loadFile(root, 'test-results/scale-resource-usage.json');
    requireFresh(scaleFile, executable.mtimeMs, currentTime, 'Scale native evidence');
    requireFresh(scaleResourcesFile, executable.mtimeMs, currentTime, 'Scale resource evidence');
    const scale = parseJson(scaleFile);
    const scaleResources = parseJson(scaleResourcesFile);
    if (scale.outcome !== 'passed' || scale.native !== true || !scale.visibleWindow || !(scale.fixtureFiles >= 100000) ||
        !scale.exactBeyondFirstPageVerified || !scale.defaultSelectionVerified || !scale.finalQueryVerified ||
        scaleResources.outcome !== 'passed' || scaleResources.executableSha256?.toLowerCase() !== executable.sha256) {
      throw new Error('Scale evidence must pass native search assertions over at least 100000 fixtures for the final executable.');
    }
    requirePairedReport(scaleResources, scaleFile);
    const samplesFile = loadFile(root, 'test-results/resource-soak-samples.ndjson');
    requireFresh(samplesFile, executable.mtimeMs, currentTime, 'Resource soak samples');
    if (typeof soakResources.samplesSha256 !== 'string' || soakResources.samplesSha256.toLowerCase() !== samplesFile.sha256) {
      throw new Error('Raw soak samples digest does not match the paired resource summary.');
    }
    entries.push(['UI-Native-Soak-Report.json', soakFile], ['Resource-Soak-Report.json', soakResourcesFile],
      ['Resource-Soak-Samples.ndjson', samplesFile], ['Scale-Native-Benchmark-Report.json', scaleFile],
      ['Scale-Resource-Usage-Report.json', scaleResourcesFile]);
  }
  const manifest = {
    schemaVersion: 1, version: packageVersion, createdAt: new Date(currentTime).toISOString(),
    measurementScope: diagnostics
      ? 'Isolated native UI, half-hour mixed idle/search soak and 100000-file scale test; machine service is a separate reference. Synthetic input, single active monitor.'
      : 'Short isolated native UI process tree; excludes the machine service. Not an idle or long-duration resource test.',
    files: entries.map(([name, file]) => ({ name, bytes: file.bytes.length, sha256: file.sha256 })),
  };
  const manifestBytes = jsonBytes(manifest);
  const checksumLines = [...manifest.files, { name: 'Release-Manifest.json', sha256: hash(manifestBytes) }]
    .map(file => `${file.sha256}  ${file.name}`).join('\n') + '\n';
  // 独占创建发布目录和每个文件；保留已有证据，失败时不更新 latest 指针。
  mkdirSync(checkedPath(root, 'artifacts/releases'), { recursive: true });
  mkdirSync(destination);
  for (const [name, file] of entries) {
    const target = join(destination, name);
    writeFileSync(target, file.bytes, { flag: 'wx' });
    if (hash(readFileSync(target)) !== file.sha256) throw new Error(`Published artifact checksum mismatch: ${name}`);
  }
  writeFileSync(join(destination, 'Release-Manifest.json'), manifestBytes, { flag: 'wx' });
  writeFileSync(join(destination, 'SHA256SUMS.txt'), checksumLines, { flag: 'wx' });
  const latest = {
    schemaVersion: 1, version: packageVersion, directory: outputDirectory,
    manifest: `${outputDirectory}/Release-Manifest.json`, manifestSha256: hash(manifestBytes),
    executableSha256: executable.sha256, installerSha256: installer.sha256, publishedAt: manifest.createdAt,
  };
  const temporaryPointer = checkedPath(root, `artifacts/releases/.Latest-Release-${randomUUID()}.json`);
  try {
    writeFileSync(temporaryPointer, jsonBytes(latest), { flag: 'wx' });
    renameSync(temporaryPointer, latestPath);
  } finally {
    if (existsSync(temporaryPointer)) unlinkSync(temporaryPointer);
  }
  return manifest;
}

if (process.argv[1] && resolve(process.argv[1]) === scriptPath) {
  try {
    const options = parseReleaseArgs(process.argv.slice(2));
    const manifest = publishRelease(options);
    console.log(`Published SuperSearch ${manifest.version}: ${options.output}`);
  } catch (error) {
    console.error(`Release publication failed: ${error.message}`);
    process.exitCode = 1;
  }
}
