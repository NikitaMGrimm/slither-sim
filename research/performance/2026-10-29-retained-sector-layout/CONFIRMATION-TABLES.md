# Separate confirmation tables

Ten reported pairs per row, plus the retained discarded AB in
[confirmation-results.json](confirmation-results.json). These observations
are not pooled with the original five-pair series. Arm values are medians of
window means/p99s; paired ratios are medians of individual ratios and need not
equal ratios of arm medians. Ranges are descriptive, not confidence intervals.
MPKI means misses per thousand instructions. Background CPU covers each
controller request wall span, not only the measured advances. Its raw residual
can be negative because of counter timing/10 ms quantization. No window is
excluded or corrected using background CPU. Computed by
[confirmation-tables.py](confirmation-tables.py).

| Row / tick | CPU ms base → candidate | Paired CPU ratio [range] | Instructions ratio | Cycles ratio |
| --- | ---: | ---: | ---: | ---: |
| mature-stock / all | 1.6197 → 1.5659 | 0.9875 [0.8780, 1.0553] | 0.9886 [0.9886, 0.9886] | 0.9818 [0.8838, 1.0490] |
| mature-stock / ordinary | 1.5900 → 1.5436 | 0.9913 [0.8813, 1.0597] | 0.9902 [0.9902, 0.9902] | 0.9857 [0.8887, 1.0535] |
| mature-stock / maintenance | 3.7619 → 3.1498 | 0.8590 [0.7368, 0.9298] | 0.9310 [0.9310, 0.9310] | 0.8540 [0.7393, 0.9224] |
| mature-legacy / all | 1.6835 → 1.6416 | 0.9572 [0.8914, 1.0611] | 0.9893 [0.9893, 0.9893] | 0.9586 [0.9042, 1.0398] |
| mature-legacy / ordinary | 1.6536 → 1.6181 | 0.9607 [0.8931, 1.0662] | 0.9908 [0.9908, 0.9908] | 0.9626 [0.9060, 1.0446] |
| mature-legacy / maintenance | 3.7798 → 3.2636 | 0.8505 [0.7890, 0.9137] | 0.9351 [0.9351, 0.9351] | 0.8484 [0.7840, 0.9010] |
| short-500-stock / all | 1.1130 → 1.0989 | 0.9715 [0.9103, 1.0726] | 0.9820 [0.9820, 0.9820] | 0.9699 [0.9057, 1.0703] |
| short-500-stock / ordinary | 1.0946 → 1.0831 | 0.9736 [0.9127, 1.0767] | 0.9834 [0.9834, 0.9834] | 0.9723 [0.9080, 1.0742] |
| short-500-stock / maintenance | 2.4398 → 2.2442 | 0.9033 [0.8340, 0.9609] | 0.9390 [0.9390, 0.9390] | 0.9022 [0.8334, 0.9541] |
| short-500-legacy / all | 1.2400 → 1.2376 | 0.9978 [0.9449, 1.0713] | 0.9830 [0.9830, 0.9830] | 0.9968 [0.9461, 1.0676] |
| short-500-legacy / ordinary | 1.2200 → 1.2201 | 0.9996 [0.9472, 1.0744] | 0.9843 [0.9843, 0.9843] | 0.9989 [0.9485, 1.0708] |
| short-500-legacy / maintenance | 2.6759 → 2.5289 | 0.9398 [0.8597, 0.9745] | 0.9431 [0.9431, 0.9431] | 0.9417 [0.8605, 0.9758] |

| Row / tick | M instructions | M cycles | L1d MPKI | dTLB MPKI | CPU p99 ms |
| --- | ---: | ---: | ---: | ---: | ---: |
| mature-stock / ordinary | 10.4453 → 10.3424 | 6.8110 → 6.6374 | 27.3298 → 27.9788 | 0.3983 → 0.3675 | 2.9646 → 2.8108 |
| mature-stock / maintenance | 20.0328 → 18.6499 | 16.2884 → 13.5666 | 19.0343 → 19.9092 | 0.4991 → 0.4246 | 4.7781 → 3.9755 |
| mature-legacy / ordinary | 10.7649 → 10.6662 | 7.1715 → 6.9524 | 26.8607 → 27.5448 | 0.4621 → 0.4594 | 3.1539 → 3.1777 |
| mature-legacy / maintenance | 21.3062 → 19.9242 | 16.4875 → 14.0923 | 18.2345 → 19.0985 | 0.5094 → 0.4799 | 4.4806 → 4.1466 |
| short-500-stock / ordinary | 6.7120 → 6.6006 | 4.7886 → 4.7511 | 30.4444 → 31.2994 | 0.4261 → 0.4454 | 2.2283 → 2.2143 |
| short-500-stock / maintenance | 15.0062 → 14.0905 | 10.6936 → 9.8926 | 18.7173 → 19.8309 | 0.3617 → 0.3768 | 3.1383 → 2.6774 |
| short-500-legacy / ordinary | 6.9951 → 6.8850 | 5.3381 → 5.3651 | 29.9685 → 30.5533 | 0.5614 → 0.5608 | 2.5234 → 2.6701 |
| short-500-legacy / maintenance | 15.9582 → 15.0501 | 11.8281 → 11.1898 | 18.3358 → 19.1371 | 0.4631 → 0.4683 | 3.5537 → 3.1360 |

| Row | Whole-window improving pairs | Median whole CPU ratio | Median maintenance ratio |
| --- | ---: | ---: | ---: |
| mature-stock | 5/10 | 0.987482 | 0.859030 |
| mature-legacy | 8/10 | 0.957239 | 0.850463 |

| Young row | Median paired CPU cost µs/tick | Range | Qualifies |
| --- | ---: | ---: | --- |
| short-500-stock | -31.986611 | [-110.523897, 79.981259] | True |
| short-500-legacy | -2.689671 | [-70.388379, 86.954306] | True |

| Row | Busy-minus-study CPU ms/window, median [range] | Visible non-study CPU ms/window, median [range] |
| --- | ---: | ---: |
| mature-stock | 55.0 [0.0, 150.0] | 30.0 [0.0, 50.0] |
| mature-legacy | 60.0 [-30.0, 170.0] | 30.0 [10.0, 40.0] |
| short-500-stock | 55.0 [10.0, 140.0] | 20.0 [0.0, 40.0] |
| short-500-legacy | 90.0 [20.0, 140.0] | 20.0 [0.0, 40.0] |
