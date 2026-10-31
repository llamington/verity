use std::fs;
use std::path::PathBuf;

use verity::data::encounters::EncounterData;
use verity::data::GameData;
use verity::gen::wild4::{generate, Lead, Rand, Request};
use verity::rng::method_j::Slot4;

#[test]
fn generated_pt_grass_validates() {
    let b = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data/blobs");
    let (Ok(enc), Ok(pt)) = (fs::read(b.join("enc_dppt.bin")), fs::read(b.join("pt.bin"))) else {
        return;
    };
    let ed = EncounterData::load(&enc).unwrap();
    let gd = GameData::load(&pt).unwrap();
    let ratio = |s: u16| gd.personal(s as usize).gender_ratio();
    let mut rand = Rand::new(42);
    let mut ok = [0u32; 3];
    for (i, e) in ed
        .group(b"SlotsPt")
        .filter(|e| e.slot_type() == 0 && e.species() != 201)
        .take(60)
        .enumerate()
    {
        let slot = Slot4::from_encounter(&e).unwrap();
        for (k, (shiny, lead)) in [
            (false, Lead::None),
            (true, Lead::None),
            (true, Lead::Synchronize(3)),
        ]
        .into_iter()
        .enumerate()
        {
            let req = Request {
                slot,
                tid: 12345,
                sid: 54321,
                species: e.species(),
                gender_ratio: ratio(e.species()),
                met_level: None,
                nature: None,
                gender: None,
                ability_bit: None,
                shiny,
                lead,
                hidden_power_type: None,
            };
            let g = generate(&req, &mut rand, 20_000, ratio)
                .unwrap_or_else(|err| panic!("slot {i} case {k}: {err:?}"));
            let x = (g.pid >> 16) ^ (g.pid & 0xFFFF) ^ (12345 ^ 54321);
            assert_eq!(x < 8, shiny);
            if let Lead::Synchronize(n) = lead {
                assert_eq!(g.pid % 25, n as u32);
            }
            ok[k] += 1;
        }
    }
    eprintln!("generated {ok:?}");
}
