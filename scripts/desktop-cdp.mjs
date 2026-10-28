export async function connect(part) {
  const tabs=await(await fetch('http://127.0.0.1:9227/json/list')).json();
  const target=tabs.find(t=>part.startsWith('http')?t.url===part:t.url.includes(part));
  if(!target)throw Error(`missing webview ${part}`);
  const socket=new WebSocket(target.webSocketDebuggerUrl);await new Promise(r=>socket.addEventListener('open',r,{once:true}));
  let next=0;const pending=new Map();socket.addEventListener('message',e=>{const value=JSON.parse(e.data);pending.get(value.id)?.(value);pending.delete(value.id);});
  return {socket,async eval(expression){const id=++next;const reply=new Promise((resolve,reject)=>{const timer=setTimeout(()=>{pending.delete(id);reject(Error('native webview did not respond within 15 seconds'));},15000);pending.set(id,value=>{clearTimeout(timer);resolve(value);});});socket.send(JSON.stringify({id,method:'Runtime.evaluate',params:{expression,awaitPromise:true,returnByValue:true}}));const value=await reply;if(value.result?.exceptionDetails||value.error)throw Error(JSON.stringify(value));return value.result.result.value;}};
}
