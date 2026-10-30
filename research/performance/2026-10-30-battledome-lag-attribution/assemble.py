"""Bind both frozen series to their own archives and one verified native build.

Remote use: python3 assemble.py ROOT OUTPUT. Reductions are produced by reduce.py;
the follow-up row reductions live in ROOT/followup/analysis/LABEL.json.
"""
from pathlib import Path
import collections
import hashlib
import json
import sys
import tarfile

from bundle import sha
from reduce import verify_archive


def main():
    root,output=map(Path,sys.argv[1:])
    master=json.loads((root/"pins.json").read_text())
    native=json.loads((root/"native-ready.json").read_text())
    verify_archive(root/"native.tar.gz",native)
    with tarfile.open(root/"native.tar.gz") as archive:
        assert json.load(archive.extractfile("pins.json"))==master
    follow_tools=json.loads((root/"followup-tools-ready.json").read_text())
    verify_archive(root/"followup-tools.tar.gz",follow_tools)
    original=json.loads((root/"selected-original-complete.json").read_text())
    assert original["pins"]==dict(master,nativeArchive=native)
    groups=[]
    for name,directory,labels in [
        ("original",root,["01-off","02-light","03-trace","04-trace","05-light","06-off",
                          "07-primary","08-primary","09-primary"]),
        ("scheduler-only",root/"followup",["11-light","12-sched","13-sched","14-light",
                                         "15-followup","16-followup","17-followup"]),
    ]:
        rows=[]
        for label in labels:
            row=(next(v for v in original["rows"] if v["label"]==label) if name=="original" else
                 json.loads((directory/"analysis"/f"{label}.json").read_text()))
            receipt=json.loads((directory/f"{label}-ready.json").read_text())
            assert receipt["complete"] and receipt["allMembersVerified"] and receipt["requiredSourcesVerified"]
            path=directory/f"{label}.tar.gz"
            assert path.stat().st_size==receipt["archiveBytes"]
            assert sha(path)==receipt["archiveSha256"]==row["sourceArchiveSha256"]
            # The reducer already verified manifests and matching native pins.
            # Cached results are accepted only for this exact component digest.
            row["evidenceStatus"]="qualified" if row["generatorQualified"] else "unverified"
            rows.append(row)
        groups.append({"name":name,"calibration":json.loads((directory/"calibration.json").read_text()),"rows":rows})
    primary=[r for g in groups for r in g["rows"] if r["kind"] in ["primary","followup-diagnostic"]]
    episodes=[e for r in primary if r["generatorQualified"] for e in r["episodes"]
              if not e["leftCensored"] and not e["rightCensored"]]
    tags=collections.Counter(tag for e in episodes for tag in e["tags"])
    sources=["prepare.py","study_lag.rs","server.py","collect.py","bundle.py","followup.py",
             "reduce.py","tables.py","test_reduce.py","assemble.py","augment.py"]
    manifest={name:sha(Path(__file__).with_name(name)) for name in sources}
    source_path=root/"analysis-source-manifest.json"
    source_path.write_text(json.dumps(manifest,sort_keys=True,indent=2)+"\n")
    result={"pins":dict(master,nativeArchive=native,followupToolsArchive=follow_tools,
                         analysisSourceManifestSha256=sha(source_path)),
            "series":groups,"episodeAttribution":{
                "completeQualifiedEpisodes":len(episodes),
                "censoredFragments":sum(e["leftCensored"] or e["rightCensored"] for r in primary for e in r["episodes"]),
                "overlappingTagCounts":dict(tags),
                "overlappingEpisodeFractions":{k:v/len(episodes) for k,v in tags.items()} if episodes else None,
                "scope":"Fractions overlap; no qualifying episodes means shares are not estimable"}}
    output.write_text(json.dumps(result,indent=2)+"\n")
    print(json.dumps({"rows":sum(len(g["rows"]) for g in groups),"primaryWindows":len(primary),
                      "completeEpisodes":len(episodes),"allGeneratorGatesPass":all(r["generatorQualified"] for r in primary)}))


if __name__=="__main__":main()
