# migrations/

Reserved for on-disk/persisted migration definitions. `obserde-migrate`
(Phase 4) now exists — see `crates/obserde-migrate` — but `Migration`
values are constructed in Rust and held in an in-memory `MigrationGraph`
only; there is no external, on-disk migration format or registry yet, the
same posture Phase 1 took toward `Schema` before an external format
existed. This directory stays empty until that lands.
