"""Post-controller reduction and verified private archive; never deletes sources."""
import hashlib
import json
import os
from pathlib import Path
import select
import subprocess
import sys
import tarfile
from run import digest, emit


def execute(root,study,source_archive):
    root=Path(root).resolve();study=Path(study).resolve();source_archive=Path(source_archive).resolve()
    os.sched_setaffinity(0,{3})
    pid=int((root/'after-primary.pid').read_text())
    try:
        args=(Path('/proc')/str(pid)/'cmdline').read_bytes()
        assert b'after_primary.py' in args
        descriptor=os.pidfd_open(pid)
        wait=select.poll();wait.register(descriptor,select.POLLIN)
        emit('waiting-for-post-count');wait.poll();os.close(descriptor)
    except (FileNotFoundError,ProcessLookupError):pass
    events=[json.loads(line) for line in (root/'after-primary.log').read_text().splitlines() if line.startswith('{')]
    assert events[-1]['event']=='post-count-complete','Controller failed; inspect retained logs'
    if (study/'POSTFIX_PLAN.md').exists():
        pid=int((root/'postfix-331.pid').read_text())
        try:
            args=(Path('/proc')/str(pid)/'cmdline').read_bytes()
            assert b'postfix_331.py' in args
            descriptor=os.pidfd_open(pid);wait=select.poll();wait.register(descriptor,select.POLLIN)
            emit('waiting-for-postfix');wait.poll();os.close(descriptor)
        except (FileNotFoundError,ProcessLookupError):pass
        events=[json.loads(line) for line in (root/'postfix-331.log').read_text().splitlines() if line.startswith('{')]
        assert events[-1]['event']=='postfix-complete','Post-fix supplement failed; inspect retained logs'
    checks=[]
    commands=[('method-tests',[sys.executable,'-B','-m','unittest','discover','-s',str(study),'-p','test_*.py']),
              ('a4-reduction',[sys.executable,str(study/'reduce.py'),str(root)]),
              ('botstorm-reduction',[sys.executable,str(study/'reduce_botstorm.py'),str(root)])]
    for label,command in commands:
        with (root/f'{label}.log').open('wb') as log:
            result=subprocess.run(command,cwd=root,stdout=log,stderr=subprocess.STDOUT)
        checks.append({'check':label,'exit':result.returncode})
        emit('reduction-end',**checks[-1])
    (root/'reduction-status.json').write_text(json.dumps(checks,indent=2)+'\n')
    pins=json.loads((root/'measurement-pins.json').read_text())
    assert digest(source_archive)==pins['sourceArchiveSha256']
    # The archived input source plus execution sources, tools, binaries and raw
    # results reproduce the study without carrying replaceable Cargo targets.
    excluded={'target','.git','__pycache__'}
    omit={'finish-collection.log','archive-ready.json','archive-manifest.json'}
    files=[path for path in sorted(root.rglob('*')) if path.is_file()
           and not path.is_symlink() and not any(part in excluded for part in path.relative_to(root).parts)
           and not path.is_relative_to(root/'postfix-331/source')
           and path.name not in omit]
    records=[{'name':'execution/'+p.relative_to(root).as_posix(),'sha256':digest(p),'bytes':p.stat().st_size} for p in files]
    records.append({'name':'runtime-source.tar','sha256':digest(source_archive),'bytes':source_archive.stat().st_size})
    manifest=root/'archive-manifest.json'
    manifest.write_text(json.dumps(records,indent=2)+'\n')
    archive=root.parent/(root.name+'-results.tar.gz')
    assert not archive.exists(),'Never overwrite an earlier archive'
    with tarfile.open(archive,'w:gz',compresslevel=1) as bundle:
        for path,record in zip(files,records):bundle.add(path,arcname=record['name'],recursive=False)
        bundle.add(source_archive,arcname='runtime-source.tar',recursive=False)
        bundle.add(manifest,arcname='archive-manifest.json',recursive=False)
    expected={r['name']:r for r in records}
    with tarfile.open(archive,'r:gz') as bundle:
        assert {m.name for m in bundle.getmembers()}==set(expected)|{'archive-manifest.json'}
        for member in bundle:
            stream=bundle.extractfile(member);value=hashlib.sha256();length=0
            for chunk in iter(lambda:stream.read(1<<20),b''):value.update(chunk);length+=len(chunk)
            if member.name=='archive-manifest.json':assert value.hexdigest()==digest(manifest)
            else:
                assert value.hexdigest()==expected[member.name]['sha256'],member.name
                assert length==expected[member.name]['bytes'],member.name
    receipt={'archiveSha256':digest(archive),'archiveBytes':archive.stat().st_size,
             'manifestSha256':digest(manifest),'verifiedMembers':len(records)+1,
             'closedAndMemberVerified':True,'sourcePreserved':True,'reductionChecks':checks}
    with (root/'archive-ready.json').open('x') as out:out.write(json.dumps(receipt,indent=2)+'\n')
    emit('archive-ready',**receipt)


if __name__=='__main__':execute(*sys.argv[1:])
