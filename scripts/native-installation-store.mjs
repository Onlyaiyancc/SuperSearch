import assert from 'node:assert/strict';
import { readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { DatabaseSync } from 'node:sqlite';

// 只操作临时托管运行器中的合成数据，禁止在维护者电脑执行。
assert.equal(process.env.GITHUB_ACTIONS, 'true', 'Hosted CI only');
assert.equal(process.env.RUNNER_ENVIRONMENT, 'github-hosted', 'Hosted CI only');
const mode = process.argv[2];
assert(['seed', 'check'].includes(mode), 'Expected seed or check');
const directory = join(process.env.LOCALAPPDATA, 'com.supersearch.desktop');
const config = join(directory, 'config.json');
const settings = JSON.parse(readFileSync(config, 'utf8'));
const db = new DatabaseSync(join(directory, 'state.db'));
const id = 'native-acceptance-fixture';
const aliases = JSON.stringify(['native-acceptance-alias']);
const entry = JSON.stringify({ entityId: id, kind: 'application', title: 'Native acceptance fixture',
  subtitle: 'Synthetic installer fixture', locator: join(process.env.WINDIR, 'System32/notepad.exe'),
  launchKind: 'path', aliases: [], launchKey: id });
try {
  assert.equal(db.prepare('PRAGMA user_version').get().user_version, 1);
  if (mode === 'seed') {
    settings.shortcut = 'Ctrl+Alt+Space';
    settings.theme = 'dark';
    writeFileSync(config, JSON.stringify(settings, null, 2));
    db.exec('BEGIN IMMEDIATE');
    db.prepare('INSERT INTO apps VALUES(?,?,?,1)').run(id, id, entry);
    db.prepare('INSERT INTO aliases VALUES(?,?)').run(id, aliases);
    db.prepare('INSERT INTO pins VALUES(?,?,?,?)').run(id, entry, Buffer.byteLength(entry), 1700000000);
    db.prepare('INSERT INTO usage_stats VALUES(?,?,?,?,?)').run(id, entry, Buffer.byteLength(entry), 7, 1700000000);
    db.exec('COMMIT');
  } else {
    assert.equal(settings.shortcut, 'Ctrl+Alt+Space');
    assert.equal(settings.theme, 'dark');
    assert.equal(db.prepare('SELECT data FROM apps WHERE entity_id=? AND manual=1').get(id)?.data, entry);
    assert.equal(db.prepare('SELECT data FROM aliases WHERE entity_id=?').get(id)?.data, aliases);
    assert.equal(db.prepare('SELECT data FROM pins WHERE entity_id=?').get(id)?.data, entry);
    const usage = db.prepare('SELECT data,success_count,last_success_at FROM usage_stats WHERE entity_id=?').get(id);
    assert.equal(usage?.data, entry);
    assert.equal(usage?.success_count, 7);
    assert.equal(usage?.last_success_at, 1700000000);
    assert.equal(db.prepare('PRAGMA integrity_check').get().integrity_check, 'ok');
  }
} finally { db.close(); }
console.log(`Synthetic upgrade state ${mode === 'seed' ? 'seeded' : 'verified'}.`);
