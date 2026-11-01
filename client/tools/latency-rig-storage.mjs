// Recoverable archives for manual scratch evidence; never used for acquisition.
import {createReadStream,createWriteStream} from 'node:fs';
import {access,writeFile,unlink} from 'node:fs/promises';
import {createGzip,createGunzip} from 'node:zlib';
import {pipeline} from 'node:stream/promises';
import {createHash} from 'node:crypto';
import path from 'node:path';
import {pathToFileURL} from 'node:url';

export async function evidenceStream(file) {
  try {await access(file);return createReadStream(file);} catch(e) {
    if(e.code!=='ENOENT')throw e;
    await access(`${file}.gz`);
    const input=createReadStream(`${file}.gz`),output=createGunzip();
    input.on('error',e=>output.destroy(e));return input.pipe(output);
  }
}
export async function evidenceText(file) {
  let text='';const input=await evidenceStream(file);input.setEncoding('utf8');
  for await(const chunk of input)text+=chunk;return text;
}
async function digest(stream) {
  const hash=createHash('sha256');let bytes=0;
  for await(const chunk of stream){hash.update(chunk);bytes+=chunk.length;}
  return {sha256:hash.digest('hex'),bytes};
}
export async function archiveEvidence(file) {
  if(!file.endsWith('.jsonl'))throw Error('archive only explicit JSONL evidence');
  const original=await digest(createReadStream(file));
  const archived=`${file}.gz`;
  await pipeline(createReadStream(file),createGzip(),createWriteStream(archived,{flags:'wx'}));
  const recovered=await digest(createReadStream(archived).pipe(createGunzip()));
  if(original.sha256!==recovered.sha256||original.bytes!==recovered.bytes)throw Error('archive recovery mismatch; original retained');
  const receipt={format:'latency-rig-archive-v1',file:path.basename(file),...original,archive:path.basename(archived),verifiedRecovery:true};
  await writeFile(`${archived}.receipt.json`,JSON.stringify(receipt,null,2),{flag:'wx'});
  // Both copies and a durable verification receipt exist before reclaiming space.
  await unlink(file);
  return receipt;
}
if(process.argv[1]&&import.meta.url===pathToFileURL(process.argv[1]).href){
  try {for(const file of process.argv.slice(2))console.log(JSON.stringify(await archiveEvidence(path.resolve(file))));}
  catch(e){console.error(String(e));process.exitCode=1;}
}
