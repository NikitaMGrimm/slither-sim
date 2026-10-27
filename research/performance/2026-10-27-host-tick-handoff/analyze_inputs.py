"""Match sampled unchanged steering bytes at client enqueue, read and ingest."""
from bisect import bisect_left
from collections import defaultdict, deque
import json
import math
from pathlib import Path


def stats(values):
    values = sorted(values)
    if not values:
        return {'count': 0}
    return {'count': len(values), **{
        name: values[math.ceil(len(values)*rank)-1]
        for name, rank in [('p50', .5), ('p95', .95), ('p99', .99), ('max', 1)]
    }, 'min': values[0]}


def match_input(samples, reads, ingests, tick_starts):
    """FIFO by client/byte; report loss instead of inventing a latency sample."""
    pending_reads, pending_ingests = defaultdict(deque), defaultdict(deque)
    for event in sorted(reads, key=lambda e: e[3]):
        pending_reads[event[1]].append(event)
    for event in sorted(ingests, key=lambda e: e[3]):
        pending_ingests[event[1]].append(event)
    send_read, send_ingest, read_ingest, tick_delays, enqueue_tick_delays = [], [], [], [], []
    missing_read = missing_ingest = 0
    for client, _, byte, sent in sorted(samples, key=lambda e: e[3]):
        queue = pending_reads[client]
        while queue and queue[0][3] < sent-.002:
            queue.popleft()
        if not queue or queue[0][2] != byte or queue[0][3] > sent+2:
            missing_read += 1
            continue
        read = queue.popleft()
        queue = pending_ingests[client]
        while queue and queue[0][3] < read[3]-.00001:
            queue.popleft()
        if not queue or queue[0][2] != byte or queue[0][3] > read[3]+2:
            missing_ingest += 1
            continue
        ingest = queue.popleft()
        send_read.append((read[3]-sent)*1000)
        send_ingest.append((ingest[3]-sent)*1000)
        read_ingest.append((ingest[3]-read[3])*1000)
        # Tick i starts before consuming inputs and physics for virtual tick i+1.
        first_after_read = bisect_left(tick_starts, read[3])+1
        consumed_tick = round(ingest[4]/.007)
        tick_delays.append(consumed_tick-first_after_read)
        enqueue_tick_delays.append(consumed_tick-(bisect_left(tick_starts, sent)+1))
    return {
        'sentSamples': len(samples), 'matchedSamples': len(send_ingest),
        'unmatchedRead': missing_read, 'unmatchedIngest': missing_ingest,
        'clientEnqueueToServerReadMs': stats(send_read),
        'clientEnqueueToIngestMs': stats(send_ingest),
        'serverReadToIngestMs': stats(read_ingest),
        'extraTicksAfterFirstStartedTick': stats(tick_delays),
        'ingestedNoLaterThanNextStartedTick': sum(x <= 0 for x in tick_delays),
        'extraTicksAfterFirstStartedTickSinceClientEnqueue': stats(enqueue_tick_delays),
        'ingestedNoLaterThanNextStartedTickSinceClientEnqueue': sum(x <= 0 for x in enqueue_tick_delays),
    }


def reduce(host, load):
    host, load = Path(host), Path(load)
    samples, events = [], []
    for path in load.glob('inputs-*.inputs.json'):
        epoch_ms, records = json.loads(path.read_text(encoding='utf-8'))
        samples.extend((client, connection, byte, (epoch_ms+at)/1000)
                       for client, connection, byte, at in records)
    for path in host.glob('input-slither-*.json'):
        events.extend(json.loads(path.read_text(encoding='utf-8')))
    origin, ticks = json.loads((host/'ticks.json').read_text(encoding='utf-8'))
    return match_input(samples, [e for e in events if e[0] == 0],
                       [e for e in events if e[0] == 1],
                       [origin+tick[0]*1e-9 for tick in ticks])


if __name__ == '__main__':
    import argparse
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('host', type=Path)
    parser.add_argument('load', type=Path)
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    args.output.write_text(json.dumps(reduce(args.host, args.load), indent=2)+'\n', encoding='utf-8')
