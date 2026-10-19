import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, readFileSync, readdirSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describeLaunch } from './desktop.mjs';

const root = dirname(dirname(fileURLToPath(import.meta.url)));
const read = path => readFileSync(path, 'utf8').replace(/\r\n/g, '\n').trim();
const sections = ['THIRD-PARTY LICENSE NOTICES', 'Generated from package-lock.json and the Windows MSVC Cargo dependency graph.\nBuild dependencies may be included; this list is not a statement that every tool is distributed.'];
const missing = [];
const supplemental = JSON.parse(read(join(root, 'docs/licenses/upstream/sources.json')));
let packages = 0;

function addPackage(label, license, source, directory, explicitFile) {
  let files = readdirSync(directory, { withFileTypes: true }).filter(entry => entry.isFile() && /^(licen[sc]e|copying|notice|unlicense)([._-]|$)/i.test(entry.name)).map(entry => entry.name).sort();
  if (explicitFile && existsSync(join(directory, explicitFile)) && !files.includes(explicitFile)) files.push(explicitFile);
  const texts = files.map(file => `--- ${file} ---\n${read(join(directory, file))}`);
  const supplement = supplemental[label];
  if (!texts.length && supplement) texts.push(`Upstream source: ${supplement.source}\n${supplement.note ?? ''}\n\n${read(join(root, 'docs/licenses/upstream', supplement.file))}`);
  if (!texts.length && ['npm is-reference@3.0.3', 'npm locate-character@3.0.0'].includes(label)) {
    // 这两个上游仅在元数据和 README 中声明 MIT，不伪造其版权年份或原始声明。
    const pkg = JSON.parse(read(join(directory, 'package.json')));
    texts.push(`Upstream package.json and README declare MIT. Author metadata: ${typeof pkg.author === 'string' ? pkg.author : pkg.author?.name ?? 'Rich Harris'}.\nNo standalone copyright/license file is shipped by upstream; standard MIT permission and disclaimer follow.\n\nPermission${read(join(root, 'LICENSE')).split('Permission')[1]}`);
  }
  if (!texts.length) missing.push(label);
  sections.push(`=== ${label} ===\nLicense: ${license || 'NOT DECLARED'}\nSource: ${source || 'Package registry'}\n\n${texts.length ? texts.join('\n\n') : 'No standalone license text was included in this package; see its upstream license.'}`);
  packages++;
}

const lock = JSON.parse(read(join(root, 'package-lock.json')));
for (const [location, entry] of Object.entries(lock.packages).sort(([a], [b]) => a.localeCompare(b))) {
  if (!location || entry.dev && location !== 'node_modules/svelte' || !existsSync(join(root, location, 'package.json'))) continue;
  const pkg = JSON.parse(read(join(root, location, 'package.json')));
  addPackage(`npm ${pkg.name}@${pkg.version}`, pkg.license ?? entry.license, typeof pkg.repository === 'string' ? pkg.repository : pkg.repository?.url, join(root, location));
}

// 复用项目工具链选择；metadata 只读取依赖图，不执行用户程序。
const launch = describeLaunch('check', root, process.env);
const result = spawnSync(launch.command, ['metadata', '--manifest-path', 'src-tauri/Cargo.toml', '--locked', '--format-version', '1', '--filter-platform', 'x86_64-pc-windows-msvc'], { ...launch.options, stdio: ['ignore', 'pipe', 'inherit'], encoding: 'utf8', maxBuffer: 32 * 1024 * 1024 });
if (result.status !== 0) throw new Error('Cannot inspect locked Rust dependencies. Install Rust and retry.');
const metadata = JSON.parse(result.stdout);
const nodes = new Map(metadata.resolve.nodes.map(node => [node.id, node]));
const selected = new Set();
function visit(id) { if (selected.has(id)) return; selected.add(id); for (const dependency of nodes.get(id)?.deps ?? []) visit(dependency.pkg); }
visit(metadata.resolve.root);
for (const pkg of metadata.packages.filter(pkg => pkg.id !== metadata.resolve.root && selected.has(pkg.id)).sort((a, b) => `${a.name}@${a.version}`.localeCompare(`${b.name}@${b.version}`))) {
  addPackage(`Rust ${pkg.name}@${pkg.version}`, pkg.license, pkg.repository ?? pkg.source, dirname(pkg.manifest_path), pkg.license_file);
}
sections.push(`=== Bundled Everything and PCRE ===\n\n${read(join(root, 'src-tauri/resources/everything/License.txt'))}`);
for (const file of ['Everything.c', 'Everything.h']) {
  // SDK 静态链接进主程序，发行包也必须保留两份原始版权与许可声明。
  const notice = read(join(root, 'src-tauri/vendor', file)).match(/^(?:\/\/[^\n]*\n)+/)?.[0];
  if (!notice || !notice.includes('Permission is hereby granted')) throw new Error(`Missing SDK license: ${file}`);
  sections.push(`=== Everything SDK ${file} ===\n\n${notice}`);
}
const output = join(root, 'docs/licenses/THIRD-PARTY-LICENSES.txt');
const content = sections.join('\n\n' + '-'.repeat(72) + '\n\n') + '\n';
if (missing.length) throw new Error(`Missing upstream license texts: ${missing.join(', ')}`);
if (process.argv.includes('--check')) {
  if (!existsSync(output) || readFileSync(output, 'utf8') !== content) throw new Error('Dependency notices are stale. Run npm run notices and commit the result.');
} else { mkdirSync(dirname(output), { recursive: true }); writeFileSync(output, content); }
console.log(`Dependency notices ${process.argv.includes('--check') ? 'verified' : 'generated'}: ${packages} packages.`);
