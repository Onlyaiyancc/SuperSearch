import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

function versionParts(version) {
  const match = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/.exec(version);
  if (!match || match[0] !== version || match.slice(1).some(part => Number(part) > 65535)) {
    throw new Error('Expected a Windows-compatible X.Y.Z version.');
  }
  return match.slice(1).map(Number);
}

export function releaseOptions(version, tag) {
  versionParts(version);
  if (tag === `v${version}`) return { draft: true, prerelease: false };
  const match = /^v(.+)-beta\.([1-9]\d*)$/.exec(tag);
  if (!match || match[0] !== tag || match[1] !== version) throw new Error('Tag and source version differ or tag format is invalid.');
  return { draft: false, prerelease: true };
}

export function candidateInstaller(version, baseline) {
  const candidate = versionParts(version);
  const previous = versionParts(baseline);
  const different = candidate.findIndex((part, i) => part !== previous[i]);
  if (different < 0 || candidate[different] < previous[different]) throw new Error('Candidate must be newer than the baseline.');
  return `SuperSearch_${version}_x64-setup.exe`;
}

// 安装验收和工作流调用同一策略，避免各自维护版本规则。
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    const [mode, version, value] = process.argv.slice(2);
    if (mode === 'tag') console.log(JSON.stringify(releaseOptions(version, value)));
    else if (mode === 'candidate') console.log(JSON.stringify({ installer: candidateInstaller(version, value) }));
    else throw new Error('Expected tag or candidate mode.');
  } catch (error) { console.error(error.message); process.exitCode = 1; }
}
