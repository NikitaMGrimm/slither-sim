# Attempts

- The maintaining agent requested a protocol-19 input-to-explicit-echo study and
  added investigation of #281's accumulated frame excess and short stalls.
  Source inspection found zero default player latency, midpoint steering decode,
  immediate responsive wanted-angle reports, early socket delivery, and a Linux
  delivery-channel wake path rather than an unconditional 1-ms output delay.
  Use the unchanged host CLI stats and generator-side packet capture; no runtime
  hook is proposed or added. Freeze all rows/quotas/definitions before timing.
