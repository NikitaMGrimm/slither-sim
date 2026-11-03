// Acquisition owns listeners and children before it writes any evidence.
export function resourceScope() {
  const disposers=[]; let closing;
  return {
    add(dispose) { disposers.push(dispose); },
    close() { return closing ??= (async()=>{
      const errors=[];
      for(const dispose of disposers.reverse())try{await dispose();}catch(error){errors.push(error);}
      if(errors.length)throw new AggregateError(errors,'Acquisition resource cleanup failed');
    })(); },
  };
}
export async function listen(server) {
  await new Promise((resolve,reject)=>{server.once('error',reject);server.listen(0,'127.0.0.1',resolve);});
}
export async function closeServer(server) {
  server.closeAllConnections?.();
  if(server.listening)await new Promise((resolve,reject)=>server.close(error=>error?reject(error):resolve()));
}
// Resolves failures as data so an early spawn error cannot become an unhandled
// rejection while a caller is still preparing browser observations.
export function collectorCompletion(child,onProgress=()=>{}) {
  let log='';
  const append=bytes=>{log+=bytes;onProgress(log);};
  child.stdout?.on('data',append);child.stderr?.on('data',append);
  return new Promise(resolve=>{
    child.once('error',error=>resolve({exitCode:null,error:error.message,log}));
    child.once('close',exitCode=>resolve({exitCode,log}));
  });
}
export async function recoverCollectorLog(completed,existing='',timeoutMs=2000) {
  if(existing||!completed)return existing;
  let timer;
  try{
    const done=await Promise.race([completed,new Promise(resolve=>{timer=setTimeout(()=>resolve(null),timeoutMs);})]);
    return done?.log??existing;
  }finally{clearTimeout(timer);}
}
export function cleanupPresentMon(child,session,terminate) {
  if(!child||child.exitCode===0||!session)return null;
  try{return {status:'stopped',output:terminate(session)};}
  catch(error){
    // PresentMon 2.6 returns 7 for ERROR_WMI_INSTANCE_NOT_FOUND. Match only its
    // exact missing-session diagnostic for this owned name; other errors fail.
    const output=String(error.stdout??'')+String(error.stderr??'');
    const errors=output.split(/\r?\n/).map(s=>s.trim()).filter(s=>/^error:/i.test(s));
    if(error.status===7&&errors.length===1&&errors[0]===`error: no existing sessions found: ${session}`)return {status:'already-gone',output};
    throw error;
  }
}
