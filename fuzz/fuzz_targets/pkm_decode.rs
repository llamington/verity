#![no_main]

use libfuzzer_sys::fuzz_target;
use verity::pkm::{BaseInfo, Format, Pkm};
use verity::text;

fuzz_target!(|data: &[u8]| {
    let Some((&sel, rest)) = data.split_first() else { return };
    let format = if sel & 1 == 0 { Format::Pk4 } else { Format::Pk5 };
    let Some(mut pk) = Pkm::from_encrypted(format, rest) else { return };
    let info = BaseInfo {
        base: [sel as u16; 6],
        gender_ratio: sel,
        growth: sel % 6,
    };
    let _ = pk.calc_stats(&info);
    let _ = pk.level(&info);
    let _ = pk.hidden_power_type();
    let _ = pk.hidden_power_power();
    let _ = pk.egg_location();
    let _ = pk.met_location();
    let mut out = [0u16; 16];
    let n = text::decode4(&pk.nickname_raw(), &mut out);
    let _ = text::decode5(&pk.ot_name_raw(), &mut out);
    let units = out;
    pk.set_nickname(&units[..n]);
    if pk.is_party_format() {
        pk.reset_party_stats(&info);
    }
    let (enc, len) = pk.encrypted();
    let back = Pkm::from_encrypted(format, &enc[..len]).unwrap();
    assert_eq!(back.decrypted(), pk.decrypted());
});
