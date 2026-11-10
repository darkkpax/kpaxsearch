import assert from 'node:assert/strict';
import { execFileSync, spawn } from 'node:child_process';
import { copyFileSync, mkdtempSync, rmSync, rmdirSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { connect } from './desktop-cdp.mjs';

const launcher = resolve(process.argv[2]);
const main = await connect('http://tauri.localhost/');
const directory = mkdtempSync(join(tmpdir(), 'kpax-catalog-'));
const name = `Kpax Catalog Probe ${Date.now()}`;
const programs = execFileSync('powershell.exe', ['-NoProfile', '-Command', "[Environment]::GetFolderPath('Programs')"], { encoding: 'utf8' }).trim();
const shortcut = join(programs, `${name}.lnk`);
const target = join(directory, `${name}.exe`);
const searchForProbe = `(() => {
  const input = document.querySelector('#query');
  input.value = ${JSON.stringify(name)};
  input.dispatchEvent(new Event('input', { bubbles: true }));
})()`;
const hasProbe = `[...document.querySelectorAll('#results [role="option"]')].some(row => row.ariaLabel === ${JSON.stringify(name)})`;
async function expectProbe() {
  for (let attempt = 0; attempt < 24; attempt++) {
    if (await main.eval(hasProbe)) return;
    await delay(500);
  }
  assert.fail('new Start menu shortcut did not appear in search');
}

try {
  await main.eval("window.__TAURI_INTERNALS__.invoke('refresh_programs')");
  await main.eval("window.__TAURI_INTERNALS__.invoke('plugin:window|hide', { label: 'main' })");
  copyFileSync(join(process.env.WINDIR, 'System32', 'winver.exe'), target);
  execFileSync('powershell.exe', ['-NoProfile', '-Command', `
    $ErrorActionPreference = 'Stop'
    $link = (New-Object -ComObject WScript.Shell).CreateShortcut($env:KPAX_PROBE_SHORTCUT)
    $link.TargetPath = $env:KPAX_PROBE_TARGET
    $link.Save()
  `], { env: { ...process.env, KPAX_PROBE_SHORTCUT: shortcut, KPAX_PROBE_TARGET: target } });
  spawn(launcher, [], { stdio: 'ignore' }).unref();
  await delay(750);
  await main.eval(searchForProbe);
  await expectProbe();
  console.log('PASS: an added Start menu shortcut appears after reopening search');
} finally {
  rmSync(shortcut, { force: true });
  rmSync(target, { force: true });
  rmdirSync(directory);
  main.socket.close();
}
