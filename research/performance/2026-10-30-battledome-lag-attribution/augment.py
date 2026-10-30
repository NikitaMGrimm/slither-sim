"""Append the exploratory small-catchup diagnostic to already verified reductions.

This saves re-decoding millions of unchanged scheduler events. A normal full
reduce.py run produces the same field. Both paths call catchup_diagnostics.
Remote use: python3 augment.py ROOT ORIGINAL_JSON; follow-up caches are in ROOT.
"""
from pathlib import Path
import json
import sys
import tarfile
import tempfile

from reduce import verify_archive,read_ticks,catchup_diagnostics


def apply(root,row):
    label=row["label"];receipt=json.loads((root/f"{label}-ready.json").read_text())
    assert receipt["archiveSha256"]==row["sourceArchiveSha256"]
    path=root/f"{label}.tar.gz";verify_archive(path,receipt)
    # Read only the necessary closed members in physical order.
    with tempfile.TemporaryDirectory(dir=root) as temporary:
        work=Path(temporary)
        with tarfile.open(path,"r|gz") as archive:
            for member in archive:
                if member.name in ["window.json","probe.jsonl","server.tar.gz","server-retrieval.json"]:
                    (work/member.name).write_bytes(archive.extractfile(member).read())
        receipt=json.loads((work/"server-retrieval.json").read_text())
        verify_archive(work/"server.tar.gz",receipt)
        with tarfile.open(work/"server.tar.gz","r|gz") as archive:
            for member in archive:
                if member.name in ["ticks.bin","ticks.meta.json"]:
                    (work/member.name).write_bytes(archive.extractfile(member).read())
        window=json.loads((work/"window.json").read_text())
        meta=json.loads((work/"ticks.meta.json").read_text())
        ticks,sleeps,_=read_ticks(work/"ticks.bin",int(window["startTargetMono"]*1e9),int(window["endTargetMono"]*1e9))
        records=[json.loads(x) for x in (work/"probe.jsonl").read_text().splitlines()]
        row["smallCatchups"]=catchup_diagnostics(records,ticks,sleeps,
            meta["anchorUnixNs"]-meta["anchorMonotonicNs"],window)


def main():
    root,original=map(Path,sys.argv[1:])
    data=json.loads(original.read_text())
    for row in data["rows"]:
        apply(root,row);print(row["label"],flush=True)
    original.write_text(json.dumps(data,indent=2)+"\n")
    for label in ["11-light","12-sched","13-sched","14-light","15-followup","16-followup","17-followup"]:
        path=root/"followup/analysis"/f"{label}.json";row=json.loads(path.read_text())
        apply(root/"followup",row);path.write_text(json.dumps(row,indent=2)+"\n");print(label,flush=True)


if __name__=="__main__":main()
