import { invoke, isTauri } from '@tauri-apps/api/core';
import { emit, listen } from '@tauri-apps/api/event';
import { LogicalSize, getCurrentWindow } from '@tauri-apps/api/window';
import { search, type Program } from './search';
import { parsePreferences, preferencesKey, type Preferences } from './preferences';
import './styles.css';
import { check } from '@tauri-apps/plugin-updater';
import { createUpdater } from './updater';
import { getVersion } from '@tauri-apps/api/app';

const native = isTauri();
const updateGate={acquire:()=>invoke<boolean>('reserve_update'),release:()=>invoke<void>('release_update')};
const settings = new URLSearchParams(location.search).has('settings');
const svg = (paths: string) => `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${paths}</svg>`;
const searchIcon = svg('<circle cx="10.8" cy="10.8" r="6.8"/><path d="m16 16 4.5 4.5"/>');
const appIcon = svg('<rect x="3" y="3" width="7" height="7" rx="2"/><rect x="14" y="3" width="7" height="7" rx="2"/><rect x="3" y="14" width="7" height="7" rx="2"/><rect x="14" y="14" width="7" height="7" rx="2"/>');
const read = (key: string) => { try { return localStorage.getItem(key); } catch { return null; } };
const save = (key: string, value: string) => { try { localStorage.setItem(key, value); } catch { /* the launcher remains usable without persistence */ } };
const $ = <T extends HTMLElement = HTMLElement>(id: string) => document.getElementById(id) as T;
let preferences = parsePreferences(read(preferencesKey));
let theme = matchMedia('(prefers-color-scheme: light)').matches ? 'light' : 'dark';

async function applyGlass() {
  document.documentElement.dataset.theme = theme;
  // Acrylic supplies the blurred material; only a light tint belongs above it.
  document.documentElement.style.setProperty('--density', String(.03 + preferences.density * .002));
  document.documentElement.style.setProperty('--radius', `${preferences.radius}px`);
  document.documentElement.classList.toggle('no-blur', !preferences.blur);
  if (!native) return;
  try {
    await invoke('set_glass',{material:preferences.blur?preferences.material:'none',dark:theme==='dark',density:preferences.density});
    await invoke('set_rounding', { radius: preferences.radius });
    document.documentElement.classList.remove('solid-fallback');
  } catch { document.documentElement.classList.add('solid-fallback'); }
}

async function initializeAppearance() {
  if (native) {
    const window = getCurrentWindow();
    await window.setTheme(null);
    theme = await window.theme() ?? theme;
    await window.onThemeChanged(event => { theme = event.payload; void applyGlass(); });
    await listen<Preferences>('appearance-changed', event => {
      preferences = parsePreferences(JSON.stringify(event.payload));
      save(preferencesKey,JSON.stringify(preferences));
      void applyGlass();
      if (!settings) render();
    });
  }
  matchMedia('(prefers-color-scheme: light)').addEventListener('change', event => { theme = event.matches ? 'light' : 'dark'; void applyGlass(); });
  await applyGlass();
}

let programs: Program[] = [], results: Program[] = [], selected = 0, indexing = false, launching = false;
let recent: string[] = [];
let usage: Record<string, number> = {};
try { const value: unknown = JSON.parse(read('kpax:recent') ?? '[]'); if (Array.isArray(value)) recent = value.filter((id): id is string => typeof id === 'string').slice(0, 50); } catch { /* discard malformed history */ }
try {
  const value = JSON.parse(read('kpax:usage') ?? '{}');
  if (value && typeof value === 'object' && !Array.isArray(value)) usage = Object.fromEntries(Object.entries(value).filter(([, count]) => typeof count === 'number' && Number.isFinite(count) && count > 0)) as Record<string, number>;
} catch { /* discard malformed counts */ }

const root = document.querySelector<HTMLDivElement>('#app')!;
if (settings) {
  root.innerHTML = `<main class="settings glass">
    <header data-tauri-drag-region><h1 data-tauri-drag-region>настройки</h1><button id="close-settings" class="icon-button" aria-label="закрыть настройки">${svg('<path d="m7 7 10 10M17 7 7 17"/>')}</button></header>
    <label class="setting-row"><span>матовое стекло</span><input id="blur" type="checkbox" role="switch"/><span class="switch" aria-hidden="true"></span></label>
    <label class="slider-label" for="material"><span>материал</span><select id="material"><option value="acrylic">матовый акрил</option><option value="blur">мягкое стекло</option><option value="frosted">плотный акрил</option></select></label>
    <label class="slider-label" for="density"><span>плотность стекла</span><output id="density-value"></output></label>
    <input id="density" type="range" min="0" max="100"/>
    <p>мягкое стекло сохраняет больше фона, акрил сильнее его приглушает. плотность меняет заливку поверх размытия.</p>
    <label class="slider-label" for="radius"><span>округлость окна</span><output id="radius-value"></output></label>
    <input id="radius" type="range" min="0" max="40"/>
    <label class="slider-label" for="rows"><span>программ в списке</span><output id="rows-value"></output></label>
    <input id="rows" type="range" min="4" max="10"/>
    <label class="setting-row startup"><span>запускать вместе с windows</span><input id="startup" type="checkbox" role="switch"/><span class="switch" aria-hidden="true"></span></label>
    <div class="settings-note">тема меняется вместе с windows.<br/>частые программы определяются по запускам через поиск.</div>
    <div class="settings-bottom"><button id="refresh" class="text-button">обновить программы</button><span id="shortcut"></span></div>
    <p id="status" role="status"></p>
    <div class="updates"><div class="slider-label"><span>обновления</span><span id="version"></span></div><p id="update-status" role="status"></p><button id="update" class="text-button">проверить и установить</button></div>
  </main>`;
  $<HTMLInputElement>('blur').checked = preferences.blur;
  $<HTMLInputElement>('density').value = String(preferences.density);
  $<HTMLInputElement>('rows').value = String(preferences.rows);
  $<HTMLInputElement>('radius').value = String(preferences.radius);
  $<HTMLSelectElement>('material').value = preferences.material;
  function values() { $('density-value').textContent = `${preferences.density}%`; $('rows-value').textContent = String(preferences.rows); $('radius-value').textContent = `${preferences.radius} px`; }
  values();
  for (const id of ['blur', 'density', 'rows','radius','material']) $(id).addEventListener('input', () => {
    preferences = parsePreferences(JSON.stringify({ blur: $<HTMLInputElement>('blur').checked, density: Number($<HTMLInputElement>('density').value), rows: Number($<HTMLInputElement>('rows').value),radius:Number($<HTMLInputElement>('radius').value),material:$<HTMLSelectElement>('material').value }));
    save(preferencesKey, JSON.stringify(preferences)); values();
    void applyGlass();
    if (native) void emit('appearance-changed',preferences);
  });
  $('startup').addEventListener('change', async () => {
    try { await invoke('set_startup',{enabled:$<HTMLInputElement>('startup').checked}); }
    catch(error) { $('status').textContent=String(error); $<HTMLInputElement>('startup').checked=await invoke<boolean>('get_startup'); }
  });
  const updater = createUpdater(check, text => { $('update-status').textContent = text; },()=>{},updateGate);
  $('update').addEventListener('click', async () => {
    $<HTMLButtonElement>('update').disabled=true;
    try { await updater(); } finally { $<HTMLButtonElement>('update').disabled=false; }
  });
  $('close-settings').addEventListener('click', () => { if (native) void getCurrentWindow().close(); });
  $('refresh').addEventListener('click', async () => {
    if (!native) return;
    const button = $<HTMLButtonElement>('refresh'); button.disabled = true; $('status').textContent = 'обновляем…';
    try { const list = await invoke<Program[]>('refresh_programs'); await emit('catalog-changed', list); $('status').textContent = `найдено программ: ${list.length}`; }
    catch (error) { $('status').textContent = String(error); }
    finally { button.disabled = false; }
  });
  document.addEventListener('keydown', event => { if (event.key === 'Escape' && native) void getCurrentWindow().close(); });
  void initializeAppearance().then(async () => { if (native) { await getCurrentWindow().show();await getCurrentWindow().setFocus();$('shortcut').textContent = await invoke<string>('shortcut_label'); $('version').textContent=await getVersion(); $<HTMLInputElement>('startup').checked=await invoke<boolean>('get_startup'); $('update-status').textContent='автоматическая проверка при запуске'; } });
} else {
  root.innerHTML = `<main class="launcher glass" aria-label="поиск программ">
    <div class="searchbox"><span class="search-icon" data-tauri-drag-region>${searchIcon}</span>
      <input id="query" type="text" placeholder="что открыть?" autocomplete="off" spellcheck="false" aria-label="найти программу" role="combobox" aria-autocomplete="list" aria-expanded="true" aria-controls="results"/>
    </div><div id="results" role="listbox" aria-label="программы"></div><div id="status" class="sr-only" role="status"></div>
  </main>`;
  const input = $<HTMLInputElement>('query');
  input.addEventListener('input', () => { input.placeholder = 'что открыть?'; input.removeAttribute('aria-invalid'); render(); });
  document.addEventListener('keydown', event => {
    if (event.isComposing) return;
    if (event.key === 'ArrowDown' || event.key === 'ArrowUp') { event.preventDefault(); choose(selected + (event.key === 'ArrowDown' ? 1 : -1)); }
    if (event.key === 'Enter') { event.preventDefault(); void launch(); }
    if (event.key === 'Escape' && native) { event.preventDefault(); void getCurrentWindow().hide(); }
    if (event.key.toLowerCase() === 'k' && (event.ctrlKey || event.metaKey)) { event.preventDefault(); input.focus(); input.select(); }
  });
  void (async () => {
    await initializeAppearance();
    if (native) {
      await listen('launcher-open', opened);
      await listen<Program[]>('catalog-changed', event => { programs = event.payload; render(); });
      programs = await invoke<Program[]>('cached_programs');
      opened();
      const background = await invoke<boolean>('background_start') || read('kpax:background-next-start') === '1';
      save('kpax:background-next-start','0');
      if (!background) { await getCurrentWindow().show(); await getCurrentWindow().setFocus(); input.focus(); }
      void refresh();
      // One background check at startup; a sleeping/offline computer keeps its current version.
      setTimeout(() => { const updater=createUpdater(check,text=>{if(text.startsWith('не удалось'))save('kpax:background-next-start','0');},()=>save('kpax:background-next-start','1'),updateGate); void updater(); },5000);
    } else opened();
  })();
}

let windowHeight = 0;
function render() {
  if (settings) return;
  const input = $<HTMLInputElement>('query');
  results = search(programs, input.value, recent, usage).slice(0, 60);
  const fragment = document.createDocumentFragment();
  for (const [index, program] of results.entries()) {
    const row = document.createElement('div'); row.id = `result-${index}`; row.className = 'result'; row.role = 'option'; row.setAttribute('aria-label', program.name);
    const icon = document.createElement('span'); icon.className = 'app-icon';
    if (program.icon.startsWith('data:image/png;base64,')) {
      const image = document.createElement('img'); image.src = program.icon; image.alt = ''; image.draggable = false; icon.append(image);
    } else icon.innerHTML = appIcon;
    const name = document.createElement('span'); name.className = 'app-name'; name.textContent = program.name;
    row.append(icon, name);
    row.addEventListener('pointermove', () => choose(index, false));
    row.addEventListener('click', () => { choose(index, false); void launch(); });
    fragment.append(row);
  }
  $('results').replaceChildren(fragment); $('results').scrollTop = 0; $('results').hidden = !results.length;
  input.setAttribute('aria-expanded', String(results.length > 0));
  if (indexing && !programs.length) input.placeholder = 'ищем программы…';
  const height = 56 + (results.length ? Math.min(preferences.rows, results.length) * 44 + 8 : 0);
  if (native && height !== windowHeight) {
    windowHeight = height;
    void getCurrentWindow().setSize(new LogicalSize(620, height)).then(()=>invoke('set_rounding',{radius:preferences.radius})).catch(error => { $('status').textContent = String(error); });
  }
  selected = 0; choose(0, false);
}

function choose(index: number, scroll = true) {
  selected = Math.max(0, Math.min(index, results.length - 1));
  const rows = $('results').children;
  for (let i = 0; i < rows.length; i++) { rows[i].classList.toggle('selected', i === selected); rows[i].setAttribute('aria-selected', String(i === selected)); }
  $<HTMLInputElement>('query').setAttribute('aria-activedescendant', results.length ? `result-${selected}` : '');
  if (scroll) rows[selected]?.scrollIntoView({ block: 'nearest' });
}

async function launch() {
  const program = results[selected];
  if (!program || launching || !native) return;
  launching = true;
  try {
    await invoke('launch_program', { id: program.id });
    recent = [program.id, ...recent.filter(id => id !== program.id)].slice(0, 50);
    const previous = Object.hasOwn(usage, program.id) ? usage[program.id] : 0;
    usage = { ...usage, [program.id]: Math.min(Number.MAX_SAFE_INTEGER, previous + 1) };
    save('kpax:recent', JSON.stringify(recent)); save('kpax:usage', JSON.stringify(usage));
  } catch (error) {
    const input = $<HTMLInputElement>('query'); input.value = ''; input.placeholder = String(error); input.setAttribute('aria-invalid', 'true'); $('status').textContent = String(error);
  } finally { launching = false; }
}

async function refresh() {
  if (indexing || !native) return;
  indexing = true; render();
  try { programs = await invoke<Program[]>('refresh_programs'); }
  catch (error) { $('status').textContent = String(error); if (!programs.length) $<HTMLInputElement>('query').placeholder = 'не удалось получить программы'; }
  finally { indexing = false; if (programs.length) $<HTMLInputElement>('query').placeholder = 'что открыть?'; render(); }
}

function opened() {
  const input = $<HTMLInputElement>('query'); input.value = ''; input.placeholder = 'что открыть?'; input.removeAttribute('aria-invalid'); render(); input.focus();
  const panel = document.querySelector<HTMLElement>('.launcher')!; panel.classList.remove('opening'); void panel.offsetWidth; panel.classList.add('opening');
}
