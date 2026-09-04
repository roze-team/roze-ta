# Statistical coefficient notices

`src/analysis/inference/calibration_tables.rs` contains a selected subset of
numerical coefficients adapted from statsmodels v0.14.6 `tsa/adfvalues.py` and
`tsa/coint_tables.py`. The original BSD-3-Clause terms and copyright holders
are reproduced in `LICENSE-STATSMODELS`, which must accompany redistributed
source and binary distributions as required by that license.

Copyright (C) 2006 Jonathan E. Taylor; Copyright (c) 2006-2008 Scipy Developers;
Copyright (c) 2009-2018 statsmodels Developers. All rights reserved.
The cointegration table source credits Josef Perktold and James P. LeSage.

Roze changes: selected ADF N=1 (none/constant/linear trend), intercept EG N=2,
and Johansen no-deterministic/constant dimensions 1..4; coefficients converted
to fixed-size Rust arrays. Probability and rank-selection adapters are Roze
code. Original source hashes and retrieval URLs are recorded in
`docs/evidence/statistical-table-sources.json` in the repository.

No Yata source or original migration notice is altered by this addition.
The combined crate now declares `MIT AND Apache-2.0 AND BSD-3-Clause`;
`THIRD-PARTY-NOTICES.md` describes the preserved earlier Yata migration layer.
Original Roze code remains MIT and Yata-derived files remain Apache-2.0.
