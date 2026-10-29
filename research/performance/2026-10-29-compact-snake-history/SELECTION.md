# Pilot selection and pre-primary search bound

The three stock pilot pairs select the single sorted vector with linear lookup
under the frozen ranking rule. Its median whole-window CPU ratio is
0.800889 (range 0.740271–0.863048), versus 0.869622 (0.775949–0.893694) for
separate keys/records with binary lookup. Instruction ratios are 1.021633 and
1.029076 respectively. The ranges overlap: this is selection evidence, not a
claim that one compact design is generally faster than the other.

Both prototypes pass the old/new randomized oracle and existing snake tests.
Wrong-removal, lost-replacement and reversed-enumeration mutations each fail at
runtime for both; restored sources pass. Their short stock/13 streams match.
All three complete stock warm streams compare byte-for-byte through tick
193,000: 5,876,276,676 bytes per arm. Two child continuations per arm and all
three direct-parent continuations also agree. The stream-byte mutation is
detected. All pilot observations, including discarded warmups, remain retained.

Before any primary data, source review identifies an unbounded-work issue for
the unrestricted linear prototype: dense views can hold many peers. Amend the
selected design to use linear lookup for at most eight records and binary
lookup above that. Eight is an implementation work bound, not a game rule or
a CPU/cache-size condition. Sorted unique keys make both searches return the
same index. Record values, insertion/removal, enumeration and ownership are
unchanged. Keep the original pilot design/results as recorded; its CPU ratio
does not measure this bounded version.

Freeze and freshly build this version before primary measurement. Run its
focused oracle/mutations, full Rust group, 27 traces, complete stock/13 warm
streams and mature continuation proofs. Primary evidence uses fresh worlds and
fresh five-pair observations; no pilot data is substituted. The original CPU
adoption gate and short controls stay unchanged. No further search-threshold or
layout tuning follows primary outcomes; if the CPU bar fails, reject it.
