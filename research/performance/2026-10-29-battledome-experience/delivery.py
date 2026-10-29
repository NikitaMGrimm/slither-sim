"""Publish terminal readiness only after closing, verifying and hashing the archive."""
from pathlib import Path
import gzip,hashlib,json,tarfile
def cleanup_allowed(launched,verified):return not launched or verified
def publish(root,summary,paths):
 root=Path(root);summary['status']=summary['outcome']
 (root/'run-summary.json').write_text(json.dumps(summary,indent=2)+'\n')
 assert all(path.exists() for path in paths),'All evidence sources are required before readiness'
 part=root/'results.tar.gz.part'
 with tarfile.open(part,'w:gz') as archive:
  for path in paths:
   if path.exists():archive.add(path,arcname=path.name)
 with gzip.open(part,'rb') as stream:
  while stream.read(1024*1024):pass
 digest=hashlib.file_digest(part.open('rb'),'sha256').hexdigest()
 part.replace(root/'results.tar.gz')
 checksum=root/'results.sha256.tmp';checksum.write_text(digest+'\n');checksum.replace(root/'results.sha256')
 ready=summary|{'archiveReady':True,'archiveSha256':digest,'archiveBytes':(root/'results.tar.gz').stat().st_size}
 status=root/'run-status-final.tmp';status.write_text(json.dumps(ready)+'\n');status.replace(root/'run-status.json')
 return ready
