import type { Update } from '@tauri-apps/plugin-updater';
type AvailableUpdate = Pick<Update, 'version' | 'downloadAndInstall' | 'close'>;
export function createUpdater(check: (options?: {timeout:number}) => Promise<AvailableUpdate | null>, report: (text:string) => void, prepare: () => void = () => {}, gate?: {acquire:()=>Promise<boolean>;release:()=>Promise<void>}) {
  let running: Promise<void> | null = null;
  async function run() {
    let update: AvailableUpdate | null = null;
    let owned = false;
    try {
      if (gate) { owned=await gate.acquire(); if(!owned){report('обновление уже проверяется');return;} }
      report('проверяем обновления…');
      update = await check({ timeout:20000 });
      if (!update) { report('установлена последняя версия'); return; }
      report(`устанавливаем ${update.version}…`);
      prepare();
      await update.downloadAndInstall(undefined, { timeout:300000, restartAfterInstall:true });
    } catch(error) { report(`не удалось обновиться: ${String(error)}`); }
    finally { await update?.close().catch(()=>{}); if(owned)await gate?.release().catch(()=>{}); }
  }
  return () => running ?? (running=run().finally(()=>{running=null;}));
}
