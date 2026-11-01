# P5 legality port plan (M2 scope first: Gen 4 PK4 from D/P/Pt wild slots)

PKHeX flow (`LegalityAnalysis`): `EncounterFinder.FindVerifiedEncounter` (encounter generator →
`EncounterVerifier` → `LearnVerifier` moves → `EvolutionVerifier`) then `UpdateChecks` verifiers.
Parity metric: verdict and the set of failing `CheckIdentifier`s, versus `pkhex-oracle`.

Order:
1. Evolution chain for Gen 4 (`EvolutionChain`, `EvolutionTree` reverse, level bounds).
2. Encounter generation for D/P/Pt wild slots (`EncounterGenerator4`, `EncounterArea4`,
   `EncounterSlot4.IsMatch*`), with the Method J lead check already ported. Test: our matched
   slot == PKHeX `EncounterMatch` on the corpus.
3. Move verification for that scope (`LearnVerifier`, `LearnSource4DP/Pt`: level-up, TM/HM,
   tutors, encounter moves).
4. Verifiers in `UpdateChecks` order: PID/EC, Nickname, Language, Trainer, TrainerID, IVs, EVs,
   Level, Ribbon (Gen 4), Ability, Ball, Form, Misc, MovePP, Gender, Item, Contest, Marking,
   GroundTile, SlotType.
5. Fuzz: mutate generated legal mons; verdict parity on ≥10^5.
6. Widen: statics, gifts, trades, eggs, HGSS (Method K), Pal Park, Gen 5.

## Status (2026-11-01)

Steps 1–5 done for the M2 scope (Gen 4-format PK4 from D/P/Pt wild slots, Unown excluded):
`legality::analysis4::analyze4` matches PKHeX's verdict, failing `CheckIdentifier` set and per-move
validity on all 939 matching corpus PK4 and on 100,000 fuzzed mutations of generated legal mons
(`tests/p5_fuzz.rs`, `p5_parity_1e5`). Not yet covered: statics/gifts/trades/eggs/events,
HG/SS (Method K), Pal Park, Gen 5, Unown forms, the word filter, active-trainer checks
(Korean save, HG/SS walking mood), stored-slot checks (Shaymin Sky in box, PP heal).
