import {test} from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp,writeFile,readFile,rm} from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import {archiveEvidence,evidenceText} from './latency-rig-storage.mjs';
test('verified archive preserves exact evidence and refuses overwriting',async()=>{
  const dir=await mkdtemp(path.join(os.tmpdir(),'latency-archive-'));
  try{
    const file=path.join(dir,'truth.jsonl'),text='{"ns":"1793512345678912300"}\n';
    await writeFile(file,text);const receipt=await archiveEvidence(file);
    assert.equal(receipt.verifiedRecovery,true);assert.equal(await evidenceText(file),text);
    await assert.rejects(readFile(file),{code:'ENOENT'});
    await writeFile(file,text);await assert.rejects(archiveEvidence(file),{code:'EEXIST'});
    assert.equal(await readFile(file,'utf8'),text);
  }finally{await rm(dir,{recursive:true});}
});
