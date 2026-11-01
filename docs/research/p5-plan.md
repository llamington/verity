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
