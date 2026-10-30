"""Prospectively frozen scheduler-only series after the all-IRQ overhead failure.

Uses collect.py's unchanged per-window driver. ROOT is a separate session tree
with bin/ and pins.json referencing the original verified native build.
"""
from pathlib import Path
import json
import os
import signal
import subprocess

import collect as c


def main():
    ping_log=(c.root/"icmp.log").open("w")
    c.ping=subprocess.Popen(["stdbuf","-oL","ping","-n","-D","-i","0.2",c.server],
                            stdout=ping_log,stderr=subprocess.STDOUT,start_new_session=True)
    c.api("start",bind=f"{c.server}:8766")
    c.prior.atomic_text(c.load_dir/"target","79\n")
    load_log=(c.root/"clients.jsonl").open("w")
    c.load=subprocess.Popen([str(c.root/"bin/clients"),str(c.load_dir),"8766","79",c.server],
                            stdout=subprocess.DEVNULL,stderr=load_log,start_new_session=True)
    first=c.start_probe("11-light")
    try:
        c.wait(300,"followup-warm-80")
        c.prior.atomic_text(c.load_dir/"target","159\n");c.wait(60,"followup-warm-160")
        c.prior.atomic_text(c.load_dir/"target","319\n");c.wait(60,"followup-warm-320")
        for label,mode in [("11-light","light"),("12-sched","sched"),
                           ("13-sched","sched"),("14-light","light")]:
            c.measure(label,mode,60,"followup-calibration",first if label=="11-light" else None)
        costs={label:c.process_cpu_per_tick(label) for label in ["11-light","12-sched","13-sched","14-light"]}
        light=(costs["11-light"]+costs["14-light"])/2
        sched=(costs["12-sched"]+costs["13-sched"])/2
        passed=sched/light<=1.05 and sched-light<=.15
        (c.root/"calibration.json").write_text(json.dumps({"hostCPUPerTickMs":costs,
            "lightMedianMs":light,"schedMedianMs":sched,"overheadGatePass":passed,
            "descriptiveEcologyDriftNotFixedWork":True},indent=2)+"\n")
        if passed:
            for label in ["15-followup","16-followup","17-followup"]:
                c.measure(label,"sched",300,"followup-diagnostic")
    finally:
        c.prior.atomic_text(c.load_dir/"STOP","stop\n")
        c.load.wait(timeout=30);load_log.close()
        os.killpg(c.ping.pid,signal.SIGINT);c.ping.wait(timeout=5);ping_log.close()
        c.api("stop")
    assert c.load.returncode==0
    c.progress("collection-complete")


if __name__=="__main__":
    try:main()
    except Exception as error:c.progress("failed",error=repr(error));raise
