import assert from 'node:assert/strict';
import {execFileSync} from 'node:child_process';
import {connect} from './desktop-cdp.mjs';
const pause=ms=>new Promise(r=>setTimeout(r,ms));
const probe=()=>{const windows=JSON.parse(execFileSync('powershell.exe',['-NoProfile','-File','scripts/probe-frame.ps1'],{encoding:'utf8'}));assert.equal(windows.length,2,'both search and settings windows must be measured');return windows;};
const main=await connect('http://tauri.localhost/');
await main.eval(`window.__TAURI_INTERNALS__.invoke('open_settings')`);await pause(700);
let settings=await connect('?settings');
for(const radius of [0,40]){
  await settings.eval(`(()=>{document.querySelector('#radius').value='${radius}';document.querySelector('#radius').dispatchEvent(new Event('input',{bubbles:true}));return true})()`);await pause(200);
  assert.equal(await main.eval(`document.documentElement.style.getPropertyValue('--radius')`),`${radius}px`);
  for(const query of ['zzzzzzzz','']){
    await main.eval(`(async()=>{const q=document.querySelector('#query');q.value='${query}';q.dispatchEvent(new Event('input',{bubbles:true}));await window.__TAURI_INTERNALS__.invoke('plugin:window|hide',{label:'main'});await window.__TAURI_INTERNALS__.invoke('plugin:window|show',{label:'main'});return true})()`);await pause(200);
    for(const window of probe()){assert.equal(window.clientX,0);assert.equal(window.clientY,0);assert.equal(window.corner,radius===0);}
  }
}
for(const material of ['blur','acrylic','frosted']){
  await settings.eval(`(()=>{document.querySelector('#material').value='${material}';document.querySelector('#material').dispatchEvent(new Event('input',{bubbles:true}));return true})()`);await pause(200);
  assert.equal(await main.eval(`document.documentElement.classList.contains('solid-fallback')`),false);
  for(const window of probe())assert.equal(window.clientY,0);
}
await main.eval(`window.__TAURI_INTERNALS__.invoke('plugin:window|set_theme',{label:'main',value:'light'})`);await pause(250);
for(const window of probe())assert.equal(window.clientY,0);
await main.eval(`window.__TAURI_INTERNALS__.invoke('plugin:window|set_theme',{label:'main',value:null})`);
await settings.eval(`(()=>{document.querySelector('#material').value='blur';document.querySelector('#radius').value='24';document.querySelector('#density').value='35';document.querySelector('#radius').dispatchEvent(new Event('input',{bubbles:true}));return true})()`);await pause(200);
await settings.eval(`(()=>{setTimeout(()=>window.__TAURI_INTERNALS__.invoke('plugin:window|close',{label:'settings'}),100);return true})()`);
settings.socket.close();await pause(250);
await main.eval(`window.__TAURI_INTERNALS__.invoke('open_settings')`);await pause(700);
settings=await connect('?settings');
assert.equal(await settings.eval(`document.querySelector('#radius').value`),'24');
for(const window of probe()){assert.equal(window.clientY,0);assert.equal(window.corner,false);}
console.log('PASS: event values reach the other window; native corners survive resize/show/theme; both client areas start at 0,0; all material modes apply');
main.socket.close();settings.socket.close();process.exit(0);
