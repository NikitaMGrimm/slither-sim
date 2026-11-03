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
export function collectorCompletion(child) {
  let log='';
  child.stdout?.on('data',bytes=>log+=bytes);child.stderr?.on('data',bytes=>log+=bytes);
  return new Promise(resolve=>{
    child.once('error',error=>resolve({exitCode:null,error:error.message,log}));
    child.once('close',exitCode=>resolve({exitCode,log}));
  });
}
