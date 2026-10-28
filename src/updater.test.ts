import { it, expect } from 'vitest';
import { createUpdater } from './updater';
it('installs a verified available update and requests restart', async () => {
  const calls:string[]=[];
  const run=createUpdater(async()=>({version:'0.2.0',downloadAndInstall:async(_event,options)=>{expect(options?.restartAfterInstall).toBe(true);calls.push('installed');},close:async()=>{calls.push('closed');}}),()=>{},()=>calls.push('prepare'));
  await run();expect(calls).toEqual(['prepare','installed','closed']);
});
it('coalesces concurrent checks', async()=>{
  let checks=0;
  const run=createUpdater(async()=>{checks++;await new Promise(r=>setTimeout(r,10));return null;},()=>{});
  await Promise.all([run(),run()]);expect(checks).toBe(1);
});
it('reports network failure without preparing an install',async()=>{
  let prepared=false,last='';const run=createUpdater(async()=>{throw Error('offline');},text=>{last=text;},()=>{prepared=true;});
  await run();expect(prepared).toBe(false);expect(last).toContain('offline');
});
it('does not check or install when another window owns the update',async()=>{
  let checked=false;
  const run=createUpdater(async()=>{checked=true;return null;},()=>{},()=>{},{acquire:async()=>false,release:async()=>{throw Error('not owned');}});
  await run();expect(checked).toBe(false);
});
