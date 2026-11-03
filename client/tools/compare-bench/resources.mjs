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
