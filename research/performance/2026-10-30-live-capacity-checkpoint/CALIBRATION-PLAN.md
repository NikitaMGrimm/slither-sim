# Isolation calibration, frozen before observations

Research-only live checkpoint. The runtime arms are 551cb1030ea2bf84f83bbeb167233b917900929a and current main b750004aeb8f11be61dc483a2ce458c6194f116f. The original checkpoint retained three cohorts per arm; the planned new order is AB/BA/AB, with three fresh cohorts per arm. No optimization or production change.

Before host/client timing, collect twelve consecutive idle 30-second windows after 20 seconds of settling. No builds, hosts, clients, reductions or extra guest commands run during calibration. A read-only collector on vCPU 3 samples /proc once per second and records full process-command inventories at boundaries, redacting the account before writing. Raw process records remain local-only.

Use the original #256 largest-other-user-process CPU metric: only processes present at both boundaries with matching start times, excluding kernel threads and explicit study task paths. Retain summed other CPU, new/exited process counts, host-vCPU IRQ/softirq/steal context and the actual USER_HZ quantum. Boundary snapshots cannot fully attribute a process born and exited between them.

Define R_idle as the maximum, over all twelve fixed windows, of largest-other CPU seconds divided by actual window seconds. Calibration qualifies as idle only if R_idle is at most 0.02 of one CPU and every window's summed other-user CPU rate is at most 0.05. Otherwise retain all calibration data, stop and inspect; do not calibrate away a busy guest or start live capacity collection.

For a selected live window of W seconds, freeze the isolation allowance as:
allowance = W * (R_idle + 0.01) + 2 / USER_HZ.

This uses the measured empirical idle envelope plus the original one-percent excess-activity allowance and two cumulative-counter quanta for endpoint sampling/rounding. It is a predeclared validity screen, not a CPU correction, confidence bound or promise of stationarity. Never subtract background from host CPU, exclude a process by name, change the gate after seeing host timings or retry for a favorable performance outcome. Record the observed largest-other value, calibrated idle component, excess allowance, quantum allowance and final threshold in each live window.

After calibration, commit the measured numeric gate and full live plan before any host timing. Preserve all twelve calibration windows and any failure. All heavy work is on the authorized guest, described publicly only as a 4-vCPU Zen 5 EPYC KVM guest. No system/security/limit change, installs, outside game servers or cloud-resource operations.
