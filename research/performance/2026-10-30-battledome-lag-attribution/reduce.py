"""Remote-only scheduling attribution. Raw scheduler names/IDs never enter output.

python3 reduce.py ROOT OUTPUT
Stage #286's reduce.py as prior_reduce.py beside this file, by pinned reference.
"""
from pathlib import Path
import bisect
import collections
import hashlib
import json
import math
import re
import struct
import subprocess
import sys
import tarfile
import tempfile

import prior_reduce as prior
from bundle import sha

stats=prior.stats
MS=1_000_000
DT=7*MS


class Intervals:
    """Disjoint, sorted intervals with logarithmic overlap queries."""
    def __init__(self, values=()):
        merged=[]
        for a,b in sorted(values):
            if b<=a:continue
            if merged and a<=merged[-1][1]:merged[-1][1]=max(merged[-1][1],b)
            else:merged.append([a,b])
        self.values=merged
        self.starts=[a for a,b in merged];self.ends=[b for a,b in merged]
        self.prefix=[0]
        for a,b in merged:self.prefix.append(self.prefix[-1]+b-a)

    def overlap(self,a,b):
        if b<=a:return 0
        left=bisect.bisect_right(self.ends,a);right=bisect.bisect_left(self.starts,b)
        if left>=right:return 0
        return (self.prefix[right]-self.prefix[left]
                -max(0,a-self.starts[left])-max(0,self.ends[right-1]-b))


def at_ns(value):
    whole,fraction=value.split(".")
    return int(whole)*1_000_000_000+int(fraction.ljust(9,"0")[:9])


class Trace:
    def __init__(self,path,sim,socket):
        self.usable=False;self.reason="No scheduler capture"
        self.running=collections.defaultdict(list)
        self.wait=collections.defaultdict(list)
        self.names={};self.wakes=[];self.dispatch=[];self.blocks=[]
        self.irq=Intervals();self.first=self.last=0;self.events=0
        if not path.exists():return
        output=path.with_suffix(".script-private.txt")
        errors=path.with_suffix(".script-private.log")
        with output.open("w") as stream,errors.open("w") as err:
            result=subprocess.run(["perf","script","--ns","-i",str(path)],stdout=stream,stderr=err)
        error=errors.read_text()
        if result.returncode or re.search(r"\bLOST\b|lost .*events",error,re.I):
            self.reason="Trace decode failure or recorded event loss";return
        pattern=re.compile(r"^\s*.*?\s+(\d+)\s+\[\d+\]\s+(\d+\.\d+):\s+(\S+):\s+(.*)$")
        switch=re.compile(r"(.*):(\d+) \[\d+\] (\S+) ==> (.*):(\d+) \[\d+\]")
        wake=re.compile(r"(.*):(\d+) \[\d+\]")
        running=None;previous=None;state="unknown";irq_depth=0;irq_start=None;irqs=[]
        with output.open() as trace_stream:
            for line in trace_stream:
                if re.search(r"\bLOST\b",line):self.reason="Recorded trace loss";return
                match=pattern.match(line)
                if not match:continue
                timestamp=at_ns(match[2]);event,detail=match[3],match[4]
                if not self.first:self.first=timestamp
                self.last=timestamp;self.events+=1
                if event=="sched:sched_switch":
                    value=switch.fullmatch(detail)
                    assert value,detail
                    prev_pid,next_pid=int(value[2]),int(value[5]);prev_state=value[3]
                    self.names[prev_pid]=value[1];self.names[next_pid]=value[4]
                    if previous is not None:
                        # A wake event can split the current running slice.
                        self.running[running].append((previous,timestamp))
                        if state=="runnable":self.wait[running].append((previous,timestamp))
                    if prev_pid==sim:
                        state="runnable" if prev_state.startswith("R") else "blocked"
                        if state=="blocked":self.blocks.append(timestamp)
                    if next_pid==sim:state="running";self.dispatch.append(timestamp)
                    running=next_pid;previous=timestamp
                elif event in ["sched:sched_wakeup","sched:sched_wakeup_new"]:
                    value=wake.match(detail)
                    if value and int(value[2])==sim:
                        if previous is not None:
                            self.running[running].append((previous,timestamp))
                            if state=="runnable":self.wait[running].append((previous,timestamp))
                        self.wakes.append(timestamp)
                        if state!="running":state="runnable"
                        previous=timestamp
                elif event in ["irq:irq_handler_entry","irq:softirq_entry"]:
                    if irq_depth==0:irq_start=timestamp
                    irq_depth+=1
                elif event in ["irq:irq_handler_exit","irq:softirq_exit"]:
                    if irq_depth:
                        irq_depth-=1
                        if irq_depth==0:irqs.append((irq_start,timestamp))
        self.irq=Intervals(irqs)
        self.running={pid:Intervals(value) for pid,value in self.running.items()}
        # IRQ time is reported separately, not silently charged to the interrupted task.
        self.wait={pid:Intervals(value) for pid,value in self.wait.items()}
        self.usable=self.events>0
        self.reason=None if self.usable else "No parsed trace events"
        self.socket=socket;self.sim=sim

    def wait_parts(self,a,b):
        parts=collections.Counter()
        for pid,index in self.wait.items():
            amount=index.overlap(a,b)
            if not amount:continue
            # Intersect IRQ intervals only with this pid's waiting slices.
            left=bisect.bisect_right(index.ends,a);right=bisect.bisect_left(index.starts,b)
            irq=sum(self.irq.overlap(max(a,x),min(b,y)) for x,y in index.values[left:right])
            amount=max(0,amount-irq);parts["irq"]+=irq
            if pid==self.socket:key="socket"
            elif pid==0:key="idleOrSteal"
            elif self.names.get(pid,"").startswith(("perf","python","sshd","sudo")):key="observerOrController"
            elif self.names.get(pid,"").startswith("slither"):key="otherHostThread"
            else:key="otherGuest"
            parts[key]+=amount
        return dict(parts)


def read_ticks(path,start,end):
    data=path.read_bytes();assert len(data)%128==0,"Incomplete observer record"
    records=list(struct.iter_unpack("<16Q",data))
    assert all(a[1]<b[1] and a[2]<=b[2] for a,b in zip(records,records[1:])),"Observer sequence/clock"
    ticks=[];pending={};sleeps=[];sleep=None
    for value in records:
        if value[0]==0:pending[value[4]]=value
        elif value[0]==1:
            before=pending.pop(value[4],None)
            if before is not None and start<=before[2]<end:
                assert value[6]>=before[6] and value[8]>=before[8] and value[9]>=before[9]
                ticks.append({"a":before,"b":value,"lag":before[2]-before[3],
                              "cpu":value[6]-before[6],"wall":value[2]-before[2],
                              "socket":value[9]-before[9],"wait":value[8]-before[8]})
        elif value[0]==2:sleep=value
        elif value[0]==3 and sleep is not None:
            if start<=sleep[2]<end:sleeps.append((sleep,value))
            sleep=None
    return ticks,sleeps,records


def wake_details(before,after,trace):
    result={"overshootMs":max(0,after[2]-before[3])/MS}
    if not trace.usable:return result
    left=bisect.bisect_left(trace.blocks,before[2]);right=bisect.bisect_right(trace.blocks,after[2])
    if left<right:
        block=trace.blocks[right-1];wi=bisect.bisect_left(trace.wakes,block)
        if wi<len(trace.wakes) and trace.wakes[wi]<=after[2]:
            wake=trace.wakes[wi];di=bisect.bisect_left(trace.dispatch,wake)
            result["readyDelayMs"]=max(0,wake-before[3])/MS
            if di<len(trace.dispatch) and trace.dispatch[di]<=after[2]:
                result["dispatchDelayMs"]=(trace.dispatch[di]-wake)/MS
    return result


def episodes(ticks):
    result=[];active=None;recovered=0;baseline=False
    for i,value in enumerate(ticks):
        if active is None and value["lag"]>20*MS:
            begin=max(0,i-1)
            active={"first":begin,"peak":i,"last":i,"leftCensored":not baseline}
            recovered=0
        if active is not None:
            active["last"]=i
            if value["lag"]>ticks[active["peak"]]["lag"]:active["peak"]=i
            recovered=recovered+1 if value["lag"]<=DT else 0
            if recovered>=5:
                active["rightCensored"]=False;result.append(active);active=None;recovered=0;baseline=True
        else:
            recovered=recovered+1 if value["lag"]<=DT else 0
            if recovered>=5:baseline=True
    if active is not None:active["rightCensored"]=True;result.append(active)
    return result


def qualify(clients,window):
    start,end=window["startUnix"],window["endUnix"]
    result=[]
    for worker in range(1,7):
        reports=[r for r in clients if r["worker"]==worker and start<=r["unix"]<=end]
        if not reports:
            result.append({"worker":worker,"passed":False,"reason":"No reports"});continue
        cpu=stats([r["cpuFraction"] for r in reports]);late=max(r["lateP99Nanos"] for r in reports)/MS
        actions=sum(r["actions"] for r in reports)
        boost=sum(r["boostActions"] for r in reports)/actions if actions else 0
        coverage=(reports[0]["unix"]-start<2.5 and end-reports[-1]["unix"]<2.5
                  and max((b["unix"]-a["unix"] for a,b in zip(reports,reports[1:])),default=0)<2.5)
        result.append({"worker":worker,"reports":len(reports),"cpuFraction":cpu,
                       "worstInputP99Ms":late,"boostActionShare":boost,"wholeWindowReportCoverage":coverage,
                       "passed":coverage and not any(r["thin"] for r in reports)
                       and cpu["p50"]<.7 and cpu["p99"]<.8 and late<42 and boost>=.99})
    return result


def phase_frames(records,episode_rows,window):
    buckets={key:{"gaps":[],"residuals":[],"bunched":0} for key in ["growth","repayment","control"]}
    previous={}
    for value in records:
        if value["type"]!="frame":continue
        before=previous.get(value["life"]);previous[value["life"]]=value
        if before is None or not value["playing"] or not value["wasPlaying"]:continue
        if not window["startUnix"]<=value["unix"]<window["endUnix"]:continue
        phase="control"
        for ep in episode_rows:
            if ep["startUnix"]<=value["unix"]<=ep["endUnix"]:
                phase="growth" if value["unix"]<=ep["peakUnix"] else "repayment";break
        gap=(value["at"]-before["at"])*1000;header=value["headerMs"]-before["headerMs"]
        buckets[phase]["gaps"].append(gap);buckets[phase]["residuals"].append(gap-header)
        buckets[phase]["bunched"]+=gap<1
    return {key:{"gapMs":stats(v["gaps"]),"residualMs":stats(v["residuals"]),
                 "bunchedUnder1Ms":v["bunched"],
                 "residualStalls":{str(t):sum(x>t for x in v["residuals"]) for t in [20,100,250]}}
            for key,v in buckets.items()}


def catchup_diagnostics(records,ticks,sleeps,anchor,window):
    """Exploratory small-debt adjacency; never changes the frozen episode count."""
    starts=[a[2] for a,b in sleeps]
    selected=[(a,b) for a,b in zip(ticks,ticks[1:])
              if bisect.bisect_left(starts,b["a"][2])==bisect.bisect_left(starts,a["a"][2])]
    ranges=[{"startUnix":(a["b"][2]-DT+anchor)/1e9,
             "peakUnix":(b["b"][2]+DT+anchor)/1e9,
             "endUnix":(b["b"][2]+DT+anchor)/1e9} for a,b in selected]
    frames=phase_frames(records,ranges,window)
    return {"scope":"Exploratory: gap endpoint within 7 ms of adjacent no-pacing ticks' completions",
            "pairs":len(selected),
            "handoverToNextStartMs":stats([(b["a"][2]-a["b"][2])/MS for a,b in selected]),
            "completionSpacingMs":stats([(b["b"][2]-a["b"][2])/MS for a,b in selected]),
            "adjacentFrames":frames["growth"],"otherFrames":frames["control"]}


def verify_archive(path,receipt):
    assert receipt["complete"] and receipt["allMembersVerified"] and receipt["requiredSourcesVerified"]
    assert path.stat().st_size==receipt["archiveBytes"] and sha(path)==receipt["archiveSha256"]
    with tarfile.open(path) as archive:manifest=json.load(archive.extractfile("manifest.json"))
    with tarfile.open(path,"r|gz") as archive:
        seen=set()
        for member in archive:
            assert member.name not in seen;seen.add(member.name)
            if member.name!="manifest.json":
                assert hashlib.file_digest(archive.extractfile(member),"sha256").hexdigest()==manifest[member.name]
        assert seen==set(manifest)|{"manifest.json"}


def reduce_row(root,label,master):
    receipt=json.loads((root/f"{label}-ready.json").read_text());archive=root/f"{label}.tar.gz"
    verify_archive(archive,receipt)
    with tempfile.TemporaryDirectory(dir=root) as temporary:
        work=Path(temporary)
        with tarfile.open(archive) as stream:stream.extractall(work,filter="data")
        assert json.loads((work/"pins.json").read_text())==master
        window=json.loads((work/"window.json").read_text())
        host=work/"target";host.mkdir()
        target_receipt=json.loads((work/"server-retrieval.json").read_text())
        verify_archive(work/"server.tar.gz",target_receipt)
        with tarfile.open(work/"server.tar.gz") as stream:stream.extractall(host,filter="data")
        assert json.loads((host/"window.json").read_text())==window
        meta=json.loads((host/"ticks.meta.json").read_text())
        start=int(window["startTargetMono"]*1e9);end=int(window["endTargetMono"]*1e9)
        ticks,sleeps,raw=read_ticks(host/"ticks.bin",start,end)
        sim=meta["simulationTid"];socket=next((r[14] for r in raw if r[14]),0)
        trace=Trace(host/f"{label}-perf.data",sim,socket)
        counter_check=None;parts_window={}
        observed=[r for r in raw if start<=r[2]<=end]
        if trace.usable and len(observed)>1:
            first,last=observed[0],observed[-1]
            parts_window=trace.wait_parts(first[2],last[2])
            traced=sum(parts_window.values())
            counter=last[8]-first[8]
            difference=abs(traced-counter)
            counter_check={"procRunnableWaitMs":counter/MS,"traceRunnableWaitMs":traced/MS,
                           "differenceMs":difference/MS,
                           "pass":difference<=max(MS,counter*.05)}
            if not counter_check["pass"]:
                trace.usable=False;trace.reason="Trace/proc runnable-wait crosscheck failed"
        clients=[json.loads(line[7:]) for line in (work/"clients.jsonl").read_text().splitlines() if line.startswith("CLIENT ")]
        probe=prior.summarize_row(work,prior.host_intervals(host/"host.jsonl"),
                                  prior.icmp_records(work/"icmp.log"),clients)
        # #286's hardcoded 300 report minimum was for 360-second rows. These
        # 60/300-second windows retain its actual headroom and coverage gates.
        probe["generator"]=qualify(clients,window)
        probe["generatorQualified"]=all(x["passed"] for x in probe["generator"])
        probe.pop("minimumSampleQuotaPass",None)
        records=[json.loads(x) for x in (work/"probe.jsonl").read_text().splitlines()]
        telemetry=[json.loads(x) for x in (host/"telemetry.jsonl").read_text().splitlines()]
        selected=[x for x in telemetry if start/1e9<=x["mono"]<=end/1e9]
        cpu=lambda x:sum(t["schedstat"][0] for t in x["threads"])
        host_cpu=(cpu(selected[-1])-cpu(selected[0]))/1e9
        cpu_delta=[b-a for a,b in zip(selected[0]["cpu"],selected[-1]["cpu"])]
        anchor=meta["anchorUnixNs"]-meta["anchorMonotonicNs"]
        ep_rows=[]
        for number,ep in enumerate(episodes(ticks),1):
            section=ticks[ep["first"]:ep["last"]+1]
            a=section[0]["a"][2];b=section[-1]["b"][2]
            peak=ticks[ep["peak"]]["a"][2]
            parts=trace.wait_parts(a,b) if trace.usable else {}
            wake_delays=[];dispatch_delays=[];overshoots=[]
            for before,after in sleeps:
                if before[2]<a or after[2]>b:continue
                overshoots.append(max(0,after[2]-before[3]))
                if trace.usable:
                    left=bisect.bisect_left(trace.blocks,before[2]);right=bisect.bisect_right(trace.blocks,after[2])
                    if left<right:
                        block=trace.blocks[right-1];wi=bisect.bisect_left(trace.wakes,block)
                        if wi<len(trace.wakes) and trace.wakes[wi]<=after[2]:
                            wake=trace.wakes[wi];di=bisect.bisect_left(trace.dispatch,wake)
                            wake_delays.append(max(0,wake-before[3]))
                            if di<len(trace.dispatch) and trace.dispatch[di]<=after[2]:
                                dispatch_delays.append(trace.dispatch[di]-wake)
            period_first=section[0]["a"];period_last=section[-1]["b"]
            wait_delta=period_last[8]-period_first[8]
            cpu_over=sum(max(0,t["cpu"]-DT) for t in section)
            socket_cpu=period_last[9]-period_first[9]
            off_cpu=max(0,b-a-(period_last[6]-period_first[6]))
            blocked=max(0,off_cpu-wait_delta)
            lower=max(0,socket_cpu-blocked);upper=min(socket_cpu,wait_delta)
            tags=[]
            if cpu_over>MS:tags.append("a-world-work")
            if parts.get("socket",lower)>MS:tags.append("b-socket-runnable-contention")
            if sum(wake_delays)>MS:tags.append("c-wakeup-ready-delay")
            if sum(v for k,v in parts.items() if k!="socket")>MS:tags.append("d-other-preemption")
            if not tags:tags.append("unattributed")
            before_samples=[x for x in telemetry if x["mono"]<=a/1e9]
            after_samples=[x for x in telemetry if x["mono"]>=b/1e9]
            steal_bracket=None
            if before_samples and after_samples:
                left,right=before_samples[-1],after_samples[0]
                steal_bracket={"jiffies":right["cpu"][7]-left["cpu"][7],
                               "bracketMs":(right["mono"]-left["mono"])*1000}
                if steal_bracket["jiffies"] and "d-other-preemption" not in tags:
                    tags.append("d-steal-in-coarse-bracket")
            ep_rows.append({"id":f"{label}-{number}","leftCensored":ep["leftCensored"],
                "rightCensored":ep["rightCensored"],"startSeconds":(a-start)/1e9,
                "durationMs":(b-a)/MS,"peakStartLagMs":ticks[ep["peak"]]["lag"]/MS,
                "startUnix":(a+anchor)/1e9,"peakUnix":(peak+anchor)/1e9,"endUnix":(b+anchor)/1e9,
                "tickCount":len(section),"worldCPUOverBudgetMs":cpu_over/MS,
                "worldCPUMaxMs":max(t["cpu"] for t in section)/MS,
                "runnableWaitMs":wait_delta/MS,"traceRunnableWaitPartsMs":{k:v/MS for k,v in parts.items()},
                "socketContentionProcBoundsMs":[lower/MS,upper/MS],
                "pacingOvershootMs":stats([x/MS for x in overshoots]),
                "pacingReadyDelayTotalMs":sum(wake_delays)/MS,
                "pacingReadyDelayMs":stats([x/MS for x in wake_delays]),
                "pacingDispatchDelayMs":stats([x/MS for x in dispatch_delays]),
                "guestStealBracket":steal_bracket,
                "deferredIncrement":section[-1]["a"][5]-section[0]["a"][5],
                "tags":tags})
        pairs=list(zip(ticks,ticks[1:]))
        intervals=[b["a"][2]-a["a"][2] for a,b in pairs]
        aligned_sim=[b["a"][6]-a["a"][6] for a,b in pairs]
        aligned_socket=[b["a"][9]-a["a"][9] for a,b in pairs]
        aligned_sim_wait=[b["a"][8]-a["a"][8] for a,b in pairs]
        aligned_socket_wait=[b["a"][10]-a["a"][10] for a,b in pairs]
        sleep_starts=[a[2] for a,b in sleeps]
        no_sleep=[bisect.bisect_left(sleep_starts,b["a"][2])==bisect.bisect_left(sleep_starts,a["a"][2])
                  for a,b in pairs]
        longest=run=0
        for value in no_sleep:
            run=run+1 if value else 0;longest=max(longest,run)
        wakes=[wake_details(a,b,trace) for a,b in sleeps]
        completion=[t["b"][2]-t["a"][3] for t in ticks]
        census=[t["b"][2]-t["a"][3]+t["a"][5]*DT for t in ticks if t["a"][4]%100==0]
        census_min=min(census,default=0)
        slow=sorted(ticks,key=lambda t:t["cpu"],reverse=True)[:math.ceil(len(ticks)*.01)]
        phases=collections.Counter(t["a"][4]%72 for t in slow)
        record_kinds=collections.Counter(r[0] for r in raw if start<=r[2]<=end)
        largest=[]
        sleep_ends=[b[2] for a,b in sleeps]
        for i in sorted(range(1,len(ticks)),key=lambda i:ticks[i]["lag"],reverse=True)[:10]:
            before,current=ticks[i-1],ticks[i];a,b=before["a"][2],current["a"][2]
            si=bisect.bisect_right(sleep_ends,b)-1
            wake=(wake_details(*sleeps[si],trace) if si>=0 and sleeps[si][0][2]>=a else None)
            largest.append({"seconds":(b-start)/1e9,"startLagMs":current["lag"]/MS,
                "precedingWorldCPUMs":before["cpu"]/MS,
                "precedingPeriodSimCPUMs":aligned_sim[i-1]/MS,
                "precedingPeriodSocketCPUMs":aligned_socket[i-1]/MS,
                "precedingPeriodSimRunnableWaitMs":aligned_sim_wait[i-1]/MS,
                "precedingPeriodTraceWaitPartsMs":{k:v/MS for k,v in trace.wait_parts(a,b).items()} if trace.usable else {},
                "lastPacingWake":wake})
        result={"label":label,"kind":window["kind"],"mode":window["mode"],
                "sourceArchiveSha256":receipt["archiveSha256"],"seconds":window["endUnix"]-window["startUnix"],
                "hostProcessCPUSeconds":host_cpu,"guestStealJiffies":cpu_delta[7],
                "guestIRQJiffies":cpu_delta[5],"guestSoftIRQJiffies":cpu_delta[6],
                "trace":{"usable":trace.usable,"reason":trace.reason,"events":trace.events,
                         "coversWindow":trace.first<=start and trace.last>=end if trace.usable else False,
                         "counterCrosscheck":counter_check},
                "ticks":len(ticks),"startLagMs":stats([t["lag"]/MS for t in ticks]),
                "virtualStartLagMs":stats([(t["lag"]+t["a"][5]*DT)/MS for t in ticks]),
                "completionProgressLagMs":stats([v/MS for v in completion]),
                "censusComparableCompletionRelativeMs":stats([(v-census_min)/MS for v in census]),
                "observerRecordKinds":{str(k):v for k,v in sorted(record_kinds.items())},
                "worldSlowest1Percent":{"ticks":len(slow),"cpuMs":stats([t["cpu"]/MS for t in slow]),
                    "topModulo72Phases":[{"phase":p,"ticks":n,"fraction":n/len(slow)}
                                         for p,n in phases.most_common(4)]},
                "worldCPUMs":stats([t["cpu"]/MS for t in ticks]),
                "alignedPeriodSimulationCPUMs":stats([v/MS for v in aligned_sim]),
                "alignedPeriodSocketCPUMs":stats([v/MS for v in aligned_socket]),
                "alignedPeriodTotalHostCPUMs":stats([(a+b)/MS for a,b in zip(aligned_sim,aligned_socket)]),
                "alignedPeriodSimulationRunnableWaitMs":stats([v/MS for v in aligned_sim_wait]),
                "alignedPeriodSocketRunnableWaitMs":stats([v/MS for v in aligned_socket_wait]),
                "alignedPeriodSimulationOffCPUMs":stats([max(0,a-b)/MS for a,b in zip(intervals,aligned_sim)]),
                "alignedPeriodSocketOffCPUMs":stats([max(0,a-b)/MS for a,b in zip(intervals,aligned_socket)]),
                "worldCPUOverBudgetTickCount":sum(t["cpu"]>DT for t in ticks),
                "alignedTotalCPUOverBudgetPeriodCount":sum(a+b>DT for a,b in zip(aligned_sim,aligned_socket)),
                "socketCPUWithinTickMs":stats([t["socket"]/MS for t in ticks]),
                "simRunnableWithinTickMs":stats([t["wait"]/MS for t in ticks]),
                "tickStartIntervalMs":stats([x/MS for x in intervals]),
                "backToBackStartIntervalsUnder7Ms":sum(x<DT for x in intervals),
                "noPacingWaitPeriods":sum(no_sleep),"longestNoPacingWaitStreak":longest,
                "noPacingWaitStartIntervalMs":stats([x/MS for x,v in zip(intervals,no_sleep) if v]),
                "observedDeferredIncrement":sum(max(0,b["a"][5]-a["a"][5]) for a,b in pairs),
                "traceWindowRunnableWaitPartsMs":{k:v/MS for k,v in parts_window.items()},
                "pacingWakeMs":{key:stats([w[key] for w in wakes if key in w])
                                 for key in ["overshootMs","readyDelayMs","dispatchDelayMs"]},
                "largestTenStartDelays":largest,
                "lateTickCounts":{str(t):sum(v["lag"]>t*MS for v in ticks) for t in [7,20,50,100]},
                "completeEpisodes":sum(not e["leftCensored"] and not e["rightCensored"] for e in ep_rows),
                "episodes":ep_rows,"clientByEpisodePhase":phase_frames(records,ep_rows,window),
                "smallCatchups":catchup_diagnostics(records,ticks,sleeps,anchor,window),
                "probe":probe,"generatorQualified":probe["generatorQualified"]}
        return result


def main():
    root,output=map(Path,sys.argv[1:]);master=json.loads((root/"pins.json").read_text())
    native=json.loads((root/"native-ready.json").read_text());verify_archive(root/"native.tar.gz",native)
    with tarfile.open(root/"native.tar.gz") as archive:
        assert json.load(archive.extractfile("pins.json"))==master
    rows=[]
    for label in ["01-off","02-light","03-trace","04-trace","05-light","06-off",
                  "07-primary","08-primary","09-primary","11-light","12-sched",
                  "13-sched","14-light","15-followup","16-followup","17-followup"]:
        if (root/f"{label}-ready.json").exists():
            row=reduce_row(root,label,master);rows.append(row)
            print(json.dumps({"label":label,"ticks":row["ticks"],"episodes":row["completeEpisodes"],
                              "qualified":row["generatorQualified"]}),flush=True)
    output.write_text(json.dumps({"pins":dict(master,nativeArchive=native),"rows":rows},indent=2)+"\n")


if __name__=="__main__":main()
