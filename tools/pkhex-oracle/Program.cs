using System;
using System.IO;
using System.Linq;
using System.Text.Json;
using System.Text.Json.Serialization;
using PKHeX.Core;

namespace PkhexOracle;

public static class Program
{
    private static readonly JsonSerializerOptions JsonOpts = new()
    {
        WriteIndented = true,
        DefaultIgnoreCondition = JsonIgnoreCondition.WhenWritingNull
    };

    public static int Main(string[] args)
    {
        if (args.Length == 0)
        {
            PrintUsage();
            return 1;
        }

        try
        {
            var command = args[0].ToLowerInvariant();
            return command switch
            {
                "sav-info" => HandleSavInfo(args),
                "sav-roundtrip" => HandleSavRoundtrip(args),
                "pkm-decode" => HandlePkmDecode(args),
                "legality" => HandleLegality(args),
                "legality-batch" => HandleLegalityBatch(args),
                "generate-synthetic" => HandleGenerateSynthetic(args),
                "sav-dump" => HandleSavDump(args),
                "sav-apply" => HandleSavApply(args),
                "export-text4" => HandleExportText4(args),
                "export-data" => HandleExportData(args),
                "export-encounters" => HandleExportEncounters(args),
                "export-gifts4" => HandleExportGifts4(args),
                "check-wild4" => HandleCheckWild4(args),
                "gen-wild4" => HandleGenWild4(args),
                "gen-gift4" => HandleGenGift4(args),
                "gen-egg4" => HandleGenEgg4(args),
                "gen-ranch4" => HandleGenRanch4(args),
                "gen-slot3" => HandleGenSlot3(args),
                "gen-origin5" => HandleGenOrigin5(args),
                "export-gifts5" => HandleExportGifts5(args),
                "version" => HandleVersion(),
                _ => UnknownCommand(command)
            };
        }
        catch (Exception ex)
        {
            Console.Error.WriteLine($"Error: {ex.Message}\n{ex.StackTrace}");
            return 1;
        }
    }

    private static void PrintUsage()
    {
        Console.WriteLine("pkhex-oracle — PKHeX test oracle CLI");
        Console.WriteLine("Commands:");
        Console.WriteLine("  sav-info <sav-path>");
        Console.WriteLine("  sav-roundtrip <sav-path> [--out <out-path>]");
        Console.WriteLine("  pkm-decode <pkm-path>");
        Console.WriteLine("  legality <pkm-path>");
        Console.WriteLine("  legality-batch <dir-path>");
        Console.WriteLine("  generate-synthetic <out-dir>");
        Console.WriteLine("  sav-dump <sav-path> <out-dir>");
        Console.WriteLine("  gen-egg4 <count> <seed> <out-dir>");
        Console.WriteLine("  sav-apply <sav-path> <ops.json> <out-path>");
        Console.WriteLine("  version");
    }

    private static int HandleVersion()
    {
        var asm = typeof(EntityFormat).Assembly.GetName();
        var info = new
        {
            oracle = "pkhex-oracle",
            pkhex_version = asm.Version?.ToString() ?? "unknown"
        };
        Console.WriteLine(JsonSerializer.Serialize(info, JsonOpts));
        return 0;
    }

    private static int HandleSavInfo(string[] args)
    {
        if (args.Length < 2)
        {
            Console.Error.WriteLine("Usage: sav-info <sav-path>");
            return 1;
        }

        var path = args[1];
        if (!File.Exists(path))
        {
            Console.Error.WriteLine($"File not found: {path}");
            return 1;
        }

        var data = File.ReadAllBytes(path);
        var sav = SaveUtil.GetSaveFile(data);
        if (sav == null)
        {
            Console.Error.WriteLine("Unrecognized save file format");
            return 1;
        }

        var result = new
        {
            version = sav.Version.ToString(),
            generation = (int)sav.Generation,
            trainer_name = sav.OT,
            tid = sav.TID16,
            sid = sav.SID16,
            gender = sav.Gender == 0 ? "Male" : "Female",
            language = ((LanguageID)sav.Language).ToString(),
            box_count = sav.BoxCount,
            party_count = sav.PartyCount,
            checksum_info = sav.ChecksumInfo,
            gender_id = sav.Gender,
            language_id = sav.Language,
            money = sav.Money,
            badges = sav switch { SAV4 s4 => s4.Badges, SAV5 s5 => s5.Misc.Badges, _ => -1 },
            played = new[] { sav.PlayedHours, sav.PlayedMinutes, sav.PlayedSeconds },
            inventory = sav.Inventory.Select(p => new
            {
                type = p.Type.ToString(),
                slots = p.Items.Length,
                max_count = p.MaxCount,
                items = p.Items.Select(i => new[] { i.Index, i.Count }).ToArray(),
                legal = p.Info.GetItems(p.Type).ToArray(),
            }).ToArray(),
            event_flags_set = Events(sav) is { } ef ? Enumerable.Range(0, ef.EventFlagCount).Where(ef.GetEventFlag).ToArray() : null,
            event_flag_count = Events(sav)?.EventFlagCount,
            event_work = Events(sav) is { } ew ? Enumerable.Range(0, ew.EventWorkCount).Select(ew.GetWork).ToArray() : null,
            box_names = sav is IBoxDetailName bn ? Enumerable.Range(0, sav.BoxCount).Select(bn.GetBoxName).ToArray() : null,
            mystery = sav is SAV4 m4 ? Mystery4(m4) : null,
        };

        Console.WriteLine(JsonSerializer.Serialize(result, JsonOpts));
        return 0;
    }

    private static object Mystery4(SAV4 sav)
    {
        var m = sav.Mystery;
        var store = (IMysteryGiftStorage)m;
        return new
        {
            unlocked = sav.IsMysteryGiftUnlocked,
            delivery_active = m.IsDeliveryManActive,
            received_flags = Enumerable.Range(0, m.MysteryGiftReceivedFlagMax).Where(m.GetMysteryGiftReceivedFlag).ToArray(),
            sentinels = m is MysteryBlock4DP dp ? Enumerable.Range(0, 11).Select(dp.GetMysteryGiftReceivedSentinel).ToArray() : null,
            pgt = Enumerable.Range(0, 8).Select(i => (PGT)store.GetMysteryGift(i)).Select(g => new
            {
                type = (int)g.CardType,
                slot = (int)g.Slot,
                detail = (int)g.Detail,
                species = g.IsEntity && g.GiftType != GiftType4.ManaphyEgg ? (int)g.Species : g.IsManaphyEgg ? 490 : 0,
                item = g.IsItem ? g.ItemID : 0,
                raw = Convert.ToHexString(g.Data),
            }).ToArray(),
            pcd = Enumerable.Range(8, 3).Select(i => (PCD)store.GetMysteryGift(i)).Select(g => new
            {
                type = (int)g.Gift.CardType,
                card_id = g.CardID,
                title = g.CardTitle,
                compat = (int)g.CardCompatibility,
                raw = Convert.ToHexString(g.Data),
            }).ToArray(),
        };
    }

    private static IEventFlag37? Events(SaveFile sav) => sav switch
    {
        IEventFlagProvider37 p => p.EventWork,
        IEventFlag37 e => e,
        _ => null,
    };

    private static int HandleSavRoundtrip(string[] args)
    {
        if (args.Length < 2)
        {
            Console.Error.WriteLine("Usage: sav-roundtrip <sav-path> [--out <out-path>]");
            return 1;
        }

        var inPath = args[1];
        var outPath = args.Length >= 4 && args[2] == "--out" ? args[3] : inPath + ".roundtrip";

        var data = File.ReadAllBytes(inPath);
        // PKHeX writes into the buffer it was given; keep a pristine copy for comparison.
        var sav = SaveUtil.GetSaveFile(data.ToArray());
        if (sav == null)
        {
            Console.Error.WriteLine("Unrecognized save file format");
            return 1;
        }

        var exported = sav.Write().ToArray();
        File.WriteAllBytes(outPath, exported);

        bool identical = data.AsSpan().SequenceEqual(exported.AsSpan());
        var res = new
        {
            input = inPath,
            output = outPath,
            input_length = data.Length,
            output_length = exported.Length,
            byte_identical = identical
        };

        Console.WriteLine(JsonSerializer.Serialize(res, JsonOpts));
        return 0;
    }

    private static int HandlePkmDecode(string[] args)
    {
        if (args.Length < 2)
        {
            Console.Error.WriteLine("Usage: pkm-decode <pkm-path>");
            return 1;
        }

        var path = args[1];
        var data = File.ReadAllBytes(path);
        var pkm = EntityFormat.GetFromBytes(data);
        if (pkm == null)
        {
            Console.Error.WriteLine("Unrecognized PKM format");
            return 1;
        }

        var info = new
        {
            species = pkm.Species,
            species_name = SpeciesName.GetSpeciesNameGeneration(pkm.Species, (int)LanguageID.English, pkm.Format),
            form = pkm.Form,
            nickname = pkm.Nickname,
            level = pkm.CurrentLevel,
            exp = pkm.EXP,
            nature = ((Nature)pkm.Nature).ToString(),
            gender = pkm.Gender == 0 ? "Male" : (pkm.Gender == 1 ? "Female" : "Genderless"),
            is_shiny = pkm.IsShiny,
            pid = $"0x{pkm.PID:X8}",
            checksum_valid = pkm.ChecksumValid,
            ivs = new { hp = pkm.IV_HP, atk = pkm.IV_ATK, def = pkm.IV_DEF, spa = pkm.IV_SPA, spd = pkm.IV_SPD, spe = pkm.IV_SPE },
            evs = new { hp = pkm.EV_HP, atk = pkm.EV_ATK, def = pkm.EV_DEF, spa = pkm.EV_SPA, spd = pkm.EV_SPD, spe = pkm.EV_SPE },
            moves = pkm.Moves,
            ot_name = pkm.OriginalTrainerName,
            ot_tid = pkm.TID16,
            ot_sid = pkm.SID16,
            format = pkm.Format
        };

        Console.WriteLine(JsonSerializer.Serialize(info, JsonOpts));
        return 0;
    }

    private static int HandleLegality(string[] args)
    {
        if (args.Length < 2)
        {
            Console.Error.WriteLine("Usage: legality <pkm-path>");
            return 1;
        }

        var path = args[1];
        var data = File.ReadAllBytes(path);
        var pkm = EntityFormat.GetFromBytes(data);
        if (pkm == null)
        {
            Console.Error.WriteLine("Unrecognized PKM format");
            return 1;
        }

        var la = new LegalityAnalysis(pkm);
        var checkResults = la.Results.Select(r => new
        {
            valid = r.Valid,
            judgement = r.Judgement.ToString(),
            identifier = r.Identifier.ToString(),
            result = r.Result.ToString()
        });

        var res = new
        {
            valid = la.Valid,
            parsed_species = pkm.Species,
            report = la.Report(),
            encounter_match = la.EncounterMatch?.Name,
            results = checkResults
        };

        Console.WriteLine(JsonSerializer.Serialize(res, JsonOpts));
        return 0;
    }

    private static int HandleLegalityBatch(string[] args)
    {
        if (args.Length < 2)
        {
            Console.Error.WriteLine("Usage: legality-batch <dir-path>");
            return 1;
        }

        var dir = args[1];
        if (!Directory.Exists(dir))
        {
            Console.Error.WriteLine($"Directory not found: {dir}");
            return 1;
        }

        var extensions = new[] { ".pk3", ".pk4", ".pk5", ".pkm" };
        var files = Directory.EnumerateFiles(dir, "*.*", SearchOption.AllDirectories)
            .Where(f => extensions.Contains(Path.GetExtension(f).ToLowerInvariant()));

        foreach (var file in files)
        {
            try
            {
                var data = File.ReadAllBytes(file);
                var pkm = EntityFormat.GetFromBytes(data);
                if (pkm == null) continue;

                var la = new LegalityAnalysis(pkm);
                var entry = new
                {
                    file = Path.GetFileName(file),
                    species = pkm.Species,
                    valid = la.Valid,
                    report = la.Report(),
                    enc_kind = la.EncounterMatch.GetType().Name,
                    enc_index = la.EncounterMatch is PCD pcdm ? Array.IndexOf(EncounterEvent.MGDB_G4, pcdm) : -1,
                    move_valid = la.Info.Moves.Select(m => m.Valid).ToArray(),
                    evo_gen4 = pkm.Format == 4 && pkm.Generation == 4
                        ? la.Info.EvoChainsAllGens.Gen4.Select(e => new[] { (int)e.Species, e.Form, e.LevelMin, e.LevelMax, e.LevelUpRequired, (int)e.Method }).ToArray()
                        : null,
                    invalid = la.Results.Where(r => !r.Valid).Select(r => r.Identifier.ToString()).Distinct().ToArray(),
                };
                Console.WriteLine(JsonSerializer.Serialize(entry));
            }
            catch
            {
                // skip corrupted sample
            }
        }

        return 0;
    }

    private static int HandleGenerateSynthetic(string[] args)
    {
        var outDir = args.Length >= 2 ? args[1] : "corpus/synthetic";
        Directory.CreateDirectory(outDir);

        var games = new (string file, GameVersion ver)[]
        {
            ("diamond.sav", GameVersion.D), ("pearl.sav", GameVersion.P), ("platinum.sav", GameVersion.Pt),
            ("heartgold.sav", GameVersion.HG), ("soulsilver.sav", GameVersion.SS),
            ("black.sav", GameVersion.B), ("white.sav", GameVersion.W),
            ("black2.sav", GameVersion.B2), ("white2.sav", GameVersion.W2),
        };

        var results = new System.Collections.Generic.List<object>();
        foreach (var (file, ver) in games)
        {
            var sav = MakeBlank(ver);
            sav.TID16 = 12345;
            sav.SID16 = 54321;
            var rnd = new Random((int)ver);
            var mons = GenerateLegal(sav, ver, rnd, 6 + 2 * sav.BoxSlotCount);
            for (int i = 0; i < Math.Min(6, mons.Count); i++)
                sav.SetPartySlotAtIndex(mons[i], i);
            for (int i = 6; i < mons.Count; i++)
                sav.SetBoxSlotAtIndex(mons[i], (i - 6) / sav.BoxSlotCount, (i - 6) % sav.BoxSlotCount);

            var path = Path.Combine(outDir, file);
            var bytes = sav.Write().ToArray();
            File.WriteAllBytes(path, bytes);
            var reread = SaveUtil.GetSaveFile(bytes);
            results.Add(new
            {
                file,
                version = sav.Version.ToString(),
                size = bytes.Length,
                pokemon = mons.Count,
                legal = mons.Count(m => new LegalityAnalysis(m).Valid),
                redetected = reread?.Version.ToString(),
            });
        }

        Console.WriteLine(JsonSerializer.Serialize(results, JsonOpts));
        return 0;
    }

    private static SaveFile MakeBlank(GameVersion ver)
    {
        if (ver is not (GameVersion.D or GameVersion.P or GameVersion.Pt or GameVersion.HG or GameVersion.SS))
        {
            var s5 = BlankSaveFile.Get(ver, "Verity", LanguageID.English);
            s5.Language = (int)LanguageID.English;
            return s5;
        }

        // PKHeX's blank Gen 4 saves have no backing buffer; build one with valid block footers.
        var (gSize, sStart, sSize) = ver switch
        {
            GameVersion.D or GameVersion.P => (SAV4DP.GeneralSize, SAV4DP.GeneralSize, 0x121E0),
            GameVersion.Pt => (SAV4Pt.GeneralSize, SAV4Pt.GeneralSize, 0x121E4),
            _ => (SAV4HGSS.GeneralSize, SAV4HGSS.GeneralSize + 0xD8, 0x12310),
        };
        var data = new byte[SaveUtil.SIZE_G4RAW];
        Array.Fill(data, (byte)0xFF);
        for (int part = 0; part < 2; part++)
        {
            int baseOfs = part * 0x40000;
            foreach (var (ofs, len) in new[] { (0, gSize), (sStart, sSize) })
            {
                var block = data.AsSpan(baseOfs + ofs, len);
                block.Clear();
                System.Buffers.Binary.BinaryPrimitives.WriteUInt32LittleEndian(block[^0x14..], (uint)part);
                System.Buffers.Binary.BinaryPrimitives.WriteUInt32LittleEndian(block[^0x10..], (uint)part);
                System.Buffers.Binary.BinaryPrimitives.WriteUInt32LittleEndian(block[^0xC..], (uint)len);
                System.Buffers.Binary.BinaryPrimitives.WriteUInt32LittleEndian(block[^0x8..], SAV4.MAGIC_JAPAN_INTL);
            }
        }
        var sav = SaveUtil.GetSaveFile(data) ?? throw new InvalidOperationException($"blank {ver} not detected");
        sav.Version = ver;
        sav.OT = "Verity";
        sav.Language = (int)LanguageID.English;
        return sav;
    }

    private static System.Collections.Generic.List<PKM> GenerateLegal(SaveFile sav, GameVersion ver, Random rnd, int count)
    {
        var list = new System.Collections.Generic.List<PKM>();
        var max = sav.MaxSpeciesID;
        int attempts = 0;
        while (list.Count < count && attempts++ < count * 50)
        {
            var species = (ushort)rnd.Next(1, max + 1);
            var blank = sav.BlankPKM;
            blank.Species = species;
            blank.Gender = blank.GetSaneGender();
            var encs = EncounterMovesetGenerator.GenerateEncounters(blank, sav, ReadOnlyMemory<ushort>.Empty, ver);
            var enc = encs.Skip(rnd.Next(0, 3)).FirstOrDefault() ?? encs.FirstOrDefault();
            if (enc == null)
                continue;
            var pk = enc.ConvertToPKM(sav);
            var converted = EntityConverter.ConvertToType(pk, sav.PKMType, out _);
            if (converted == null)
                continue;
            if (!new LegalityAnalysis(converted).Valid)
                continue;
            list.Add(converted);
        }
        return list;
    }

    private static int HandleSavDump(string[] args)
    {
        if (args.Length < 3)
        {
            Console.Error.WriteLine("Usage: sav-dump <sav-path> <out-dir>");
            return 1;
        }

        var sav = SaveUtil.GetSaveFile(File.ReadAllBytes(args[1]));
        if (sav == null)
        {
            Console.Error.WriteLine("Unrecognized save file format");
            return 1;
        }
        var outDir = args[2];
        Directory.CreateDirectory(outDir);
        var ext = "." + sav.PKMType.Name.ToLowerInvariant();

        var slots = new System.Collections.Generic.List<object>();
        void Dump(PKM pk, string name, bool party)
        {
            if (pk.Species == 0)
                return;
            var dec = party ? pk.DecryptedPartyData : pk.DecryptedBoxData;
            var enc = party ? pk.EncryptedPartyData : pk.EncryptedBoxData;
            File.WriteAllBytes(Path.Combine(outDir, name + ext), dec);
            File.WriteAllBytes(Path.Combine(outDir, name + ".enc"), enc);
            slots.Add(DescribeSlot(pk, name, party));
        }

        for (int i = 0; i < sav.PartyCount; i++)
            Dump(sav.GetPartySlotAtIndex(i), $"party_{i}", true);
        for (int b = 0; b < sav.BoxCount; b++)
            for (int s = 0; s < sav.BoxSlotCount; s++)
                Dump(sav.GetBoxSlotAtIndex(b, s), $"box{b:D2}_{s:D2}", false);

        Console.WriteLine(JsonSerializer.Serialize(new { version = sav.Version.ToString(), slots }, JsonOpts));
        return 0;
    }

    // ops: [{"op":"copy"|"swap","from":"box00_00"|"party_0","to":...}]
    private static object DescribeSlot(PKM pk, string name, bool party)
    {
        var pi = pk.PersonalInfo;
        var stats = pk.GetStats(pi);
        var pidiv = MethodFinder.Analyze(pk);
        var la = new LegalityAnalysis(pk);
        var enc = la.EncounterMatch;
        var lead = la.Info.PIDIV;
        static int[][] Evos(System.Collections.Generic.IEnumerable<EvoCriteria> c) =>
            c.Select(e => new[] { (int)e.Species, e.Form, e.LevelMin, e.LevelMax, e.LevelUpRequired, (int)e.Method }).ToArray();
        var gen4 = pk.Format == 4 && pk.Generation == 4;
        return new
        {
            name,
            evo_origin = gen4 ? Evos(EncounterOrigin.GetOriginChain(pk, 4, EntityContext.Gen4)) : null,
            evo_gen4 = gen4 ? Evos(la.Info.EvoChainsAllGens.Gen4) : null,
            move_valid = la.Info.Moves.Select(m => m.Valid).ToArray(),
            move_method = la.Info.Moves.Select(m => m.Info.Method.ToString()).ToArray(),
            enc_kind = enc.GetType().Name,
            enc_version = (int)enc.Version,
            enc_location = enc.Location,
            enc_species = enc.Species,
            enc_form = enc.Form,
            enc_level_min = enc.LevelMin,
            enc_level_max = enc.LevelMax,
            enc_slot_number = enc is EncounterSlot4 s4 ? s4.SlotNumber : -1,
            enc_slot_type = enc is EncounterSlot4 t4 ? (int)t4.Type : -1,
            enc_can_radar = enc is EncounterSlot4 r4 && r4.CanUseRadar,
            enc_pressure = enc is EncounterSlot4 p4 ? p4.PressureLevel : 0,
            enc_ground = enc is EncounterSlot4 g4 ? (long)g4.GroundTile : 0,
            enc_magnet = enc is EncounterSlot4 m4 ? new[] { (int)m4.MagnetPullIndex, m4.MagnetPullCount } : null,
            enc_static = enc is EncounterSlot4 x4 ? new[] { (int)x4.StaticIndex, x4.StaticCount } : null,
            enc_rate = enc is EncounterSlot4 a4 ? a4.AreaRate : 0,
            lead = (int)lead.Lead,
            lead_type = (int)lead.Type,
            lead_seed = lead.EncounterSeed,
            pidiv_type = (int)pidiv.Type,
            pidiv_seed = pidiv.OriginSeed,
            party,
            legal = new LegalityAnalysis(pk).Valid,
            checksum_valid = pk.ChecksumValid,
            pid = pk.PID,
            species = pk.Species,
            form = pk.Form,
            held_item = pk.HeldItem,
            tid = pk.TID16,
            sid = pk.SID16,
            exp = pk.EXP,
            level = pk.CurrentLevel,
            friendship = pk.OriginalTrainerFriendship,
            ability = pk.Ability,
            language = pk.Language,
            evs = new[] { pk.EV_HP, pk.EV_ATK, pk.EV_DEF, pk.EV_SPE, pk.EV_SPA, pk.EV_SPD },
            ivs = new[] { pk.IV_HP, pk.IV_ATK, pk.IV_DEF, pk.IV_SPE, pk.IV_SPA, pk.IV_SPD },
            moves = new[] { pk.Move1, pk.Move2, pk.Move3, pk.Move4 },
            pp = new[] { pk.Move1_PP, pk.Move2_PP, pk.Move3_PP, pk.Move4_PP },
            pp_ups = new[] { pk.Move1_PPUps, pk.Move2_PPUps, pk.Move3_PPUps, pk.Move4_PPUps },
            is_egg = pk.IsEgg,
            is_nicknamed = pk.IsNicknamed,
            fateful = pk.FatefulEncounter,
            gender = pk.Gender,
            nature = (int)pk.Nature,
            version = (int)pk.Version,
            egg_location = pk.EggLocation,
            met_location = pk.MetLocation,
            ball = pk.Ball,
            met_level = pk.MetLevel,
            ot_gender = pk.OriginalTrainerGender,
            is_shiny = pk.IsShiny,
            hp_type = pk.HPType,
            hp_power = pk.HPPower,
            nickname = pk.Nickname,
            ot_name = pk.OriginalTrainerName,
            stored_stats = party ? new[] { pk.Stat_HPMax, pk.Stat_ATK, pk.Stat_DEF, pk.Stat_SPE, pk.Stat_SPA, pk.Stat_SPD } : null,
            stored_level = party ? (int?)pk.Stat_Level : null,
            calc_stats = stats,
            base_stats = new[] { pi.HP, pi.ATK, pi.DEF, pi.SPE, pi.SPA, pi.SPD },
            gender_ratio = pi.Gender,
            exp_growth = pi.EXPGrowth,
            abilities = new[] { pi.GetAbilityAtIndex(0), pi.GetAbilityAtIndex(1), pi.AbilityCount > 2 ? pi.GetAbilityAtIndex(2) : 0 },
        };
    }

    private static int HandleSavApply(string[] args)
    {
        if (args.Length < 4)
        {
            Console.Error.WriteLine("Usage: sav-apply <sav-path> <ops.json> <out-path>");
            return 1;
        }
        var sav = SaveUtil.GetSaveFile(File.ReadAllBytes(args[1]));
        if (sav == null)
        {
            Console.Error.WriteLine("Unrecognized save file format");
            return 1;
        }

        PKM Get(string slot) => slot.StartsWith("party_")
            ? sav.GetPartySlotAtIndex(int.Parse(slot[6..]))
            : sav.GetBoxSlotAtIndex(int.Parse(slot[3..5]), int.Parse(slot[6..]));
        void Set(string slot, PKM pk)
        {
            if (slot.StartsWith("party_"))
                sav.SetPartySlotAtIndex(pk, int.Parse(slot[6..]), EntityImportSettings.None);
            else
                sav.SetBoxSlotAtIndex(pk, int.Parse(slot[3..5]), int.Parse(slot[6..]), EntityImportSettings.None);
        }

        using var doc = JsonDocument.Parse(File.ReadAllBytes(args[2]));
        int applied = 0;
        foreach (var op in doc.RootElement.EnumerateArray())
        {
            var kind = op.GetProperty("op").GetString();
            if (kind == "dex")
            {
                var pk = EntityFormat.GetFromBytes(File.ReadAllBytes(op.GetProperty("pkm").GetString()!))!;
                if (sav is SAV4 s4) s4.Dex.SetDex(pk);
                else if (sav is SAV5 s5) s5.Zukan.SetDex(pk);
                applied++;
                continue;
            }
            if (kind == "set")
            {
                var slot = op.GetProperty("slot").GetString()!;
                var pk = Get(slot);
                foreach (var f in op.GetProperty("fields").EnumerateObject())
                {
                    var v = f.Value;
                    switch (f.Name)
                    {
                        case "held_item": pk.HeldItem = v.GetInt32(); break;
                        case "exp": pk.EXP = v.GetUInt32(); break;
                        case "friendship": pk.OriginalTrainerFriendship = v.GetByte(); break;
                        case "ivs": pk.SetIVs(v.EnumerateArray().Select(x => x.GetInt32()).ToArray()); break;
                        case "evs": pk.SetEVs(v.EnumerateArray().Select(x => x.GetInt32()).ToArray()); break;
                        case "moves": pk.SetMoves(v.EnumerateArray().Select(x => (ushort)x.GetInt32()).ToArray()); break;
                        case "nickname": pk.IsNicknamed = true; pk.Nickname = v.GetString()!; break;
                        default: throw new ArgumentException($"unknown field {f.Name}");
                    }
                }
                if (slot.StartsWith("party_"))
                    pk.ResetPartyStats();
                Set(slot, pk);
                applied++;
                continue;
            }
            var from = op.GetProperty("from").GetString()!;
            var to = op.GetProperty("to").GetString()!;
            var a = Get(from);
            var b = Get(to);
            if (kind == "copy")
                Set(to, a);
            else if (kind == "swap")
            {
                Set(to, a);
                Set(from, b);
            }
            else
                throw new ArgumentException($"unknown op {kind}");
            applied++;
        }

        File.WriteAllBytes(args[3], sav.Write().ToArray());
        Console.WriteLine(JsonSerializer.Serialize(new { applied }));
        return 0;
    }

    private static readonly System.Collections.Generic.Dictionary<GameVersion, SaveFile> Blanks = new();
    private static SaveFile Sav(GameVersion v) => Blanks.TryGetValue(v, out var s) ? s : Blanks[v] = MakeBlank(v);

    private static int[] Raw(PersonalInfo pi) => pi.Write().Select(b => (int)b).ToArray();

    private static object ExportGame(string key, byte gen, IPersonalTable table, ushort maxSpecies,
        Func<ushort, byte, Learnset> levelup, Func<ushort, byte, ReadOnlySpan<ushort>> eggs, ReadOnlySpan<ushort> held)
    {
        var personal = new System.Collections.Generic.List<int[]>();
        var tutors = new System.Collections.Generic.List<int[]>();
        for (int i = 0; i < table.Count; i++)
        {
            var pi = table[i];
            personal.Add(Raw(pi));
            if (pi is PersonalInfo4 p4)
                tutors.Add(p4.TypeTutors.Select(b => b ? 1 : 0).ToArray());
        }
        var forms = new System.Collections.Generic.List<int[]>();
        var learn = new System.Collections.Generic.Dictionary<int, int[][]>();
        var egg = new System.Collections.Generic.List<object>();
        for (ushort sp = 1; sp <= maxSpecies; sp++)
        {
            var fc = table.GetFormEntry(sp, 0).FormCount;
            for (byte f = 0; f < Math.Max((byte)1, fc); f++)
            {
                var idx = table.GetFormIndex(sp, f);
                forms.Add(new[] { sp, f, idx });
                if (!learn.ContainsKey(idx))
                {
                    var ls = levelup(sp, f);
                    var mv = ls.GetAllMoves(); var lv = ls.GetAllLevels();
                    var arr = new int[mv.Length][];
                    for (int k = 0; k < mv.Length; k++) arr[k] = new int[] { lv[k], mv[k] };
                    learn[idx] = arr;
                }
                var e = eggs(sp, f).ToArray();
                if (e.Length > 0)
                    egg.Add(new { species = sp, form = f, moves = e });
            }
        }
        return new
        {
            game = key,
            generation = gen,
            max_species = maxSpecies,
            personal_size = personal[0].Length,
            personal,
            type_tutors = tutors.Count > 0 ? tutors : null,
            forms,
            levelup = learn.OrderBy(kv => kv.Key).Select(kv => new { index = kv.Key, moves = kv.Value }).ToArray(),
            egg_moves = egg,
            held_items = held.ToArray(),
        };
    }

    private static int HandleExportData(string[] args)
    {
        var outDir = args.Length > 1 ? args[1] : "data/exports";
        Directory.CreateDirectory(outDir);
        var games = new (string key, Func<object> make)[]
        {
            ("dp", () => ExportGame("dp", 4, PersonalTable.DP, Sav(GameVersion.D).MaxSpeciesID, LearnSource4DP.Instance.GetLearnset, (s, f) => LearnSource4DP.Instance.GetEggMoves(s, f), Sav(GameVersion.D).HeldItems)),
            ("pt", () => ExportGame("pt", 4, PersonalTable.Pt, Sav(GameVersion.Pt).MaxSpeciesID, LearnSource4Pt.Instance.GetLearnset, (s, f) => LearnSource4Pt.Instance.GetEggMoves(s, f), Sav(GameVersion.Pt).HeldItems)),
            ("hgss", () => ExportGame("hgss", 4, PersonalTable.HGSS, Sav(GameVersion.HG).MaxSpeciesID, LearnSource4HGSS.Instance.GetLearnset, (s, f) => LearnSource4HGSS.Instance.GetEggMoves(s, f), Sav(GameVersion.HG).HeldItems)),
            ("bw", () => ExportGame("bw", 5, PersonalTable.BW, Sav(GameVersion.B).MaxSpeciesID, LearnSource5BW.Instance.GetLearnset, (s, f) => LearnSource5BW.Instance.GetEggMoves(s, f), Sav(GameVersion.B).HeldItems)),
            ("b2w2", () => ExportGame("b2w2", 5, PersonalTable.B2W2, Sav(GameVersion.B2).MaxSpeciesID, LearnSource5B2W2.Instance.GetLearnset, (s, f) => LearnSource5B2W2.Instance.GetEggMoves(s, f), Sav(GameVersion.B2).HeldItems)),
        };
        var manifest = new System.Collections.Generic.Dictionary<string, object>
        {
            ["pkhex_version"] = typeof(EntityFormat).Assembly.GetName().Version?.ToString() ?? "?",
            ["timestamp"] = DateTime.UtcNow.ToString("o"),
        };
        File.WriteAllText(Path.Combine(outDir, "g3.json"), JsonSerializer.Serialize(ExportG3()));
        foreach (var (key, make) in games)
        {
            var data = make();
            File.WriteAllText(Path.Combine(outDir, $"{key}.json"), JsonSerializer.Serialize(data));
            manifest[key] = "ok";
        }

        foreach (var (ctx, gen) in new[] { (EntityContext.Gen4, 4), (EntityContext.Gen5, 5) })
        {
            var max = (gen == 4 ? Sav(GameVersion.HG) : Sav(GameVersion.B2)).MaxMoveID;
            var moves = Enumerable.Range(0, max + 1).Select(m => new { pp = MoveInfo.GetPP(ctx, (ushort)m), type = MoveInfo.GetType((ushort)m, ctx) }).ToArray();
            File.WriteAllText(Path.Combine(outDir, $"moves{gen}.json"), JsonSerializer.Serialize(new { generation = gen, max_move = max, moves }));
        }

        File.WriteAllText(Path.Combine(outDir, "move_tables.json"), JsonSerializer.Serialize(new
        {
            g4_tutor_moves = PersonalInfo4.TutorMoves.ToArray(),
            g4_tm = PersonalInfo4.MachineMovesTechnical.ToArray(),
            g4_hm_dppt = PersonalInfo4.MachineMovesHiddenDPPt.ToArray(),
            g4_hm_hgss = PersonalInfo4.MachineMovesHiddenHGSS.ToArray(),
            g4_special_blast_burn = PersonalInfo4.SpecialTutorBlastBurn.ToArray(),
            g4_special_hydro_cannon = PersonalInfo4.SpecialTutorHydroCannon.ToArray(),
            g4_special_frenzy_plant = PersonalInfo4.SpecialTutorFrenzyPlant.ToArray(),
            g4_special_draco_meteor = PersonalInfo4.SpecialTutorDracoMeteor.ToArray(),
            g5_tmhm = PersonalInfo5BW.MachineMoves.ToArray(),
            g5_type_tutor_moves = PersonalInfo5BW.TypeTutorMoves.ToArray(),
            g5_hatch = Enumerable.Range(0, 256).Select(loc => new[] { GameVersion.W, GameVersion.B, GameVersion.W2, GameVersion.B2 }
                .Select((v, i) => EggHatchLocation5.IsValidMet5((ushort)loc, v) ? 1 << i : 0).Sum()).ToArray(),
            g4_released_held = Enumerable.Range(1, 1000).Where(i => ItemRestrictions.IsHeldItemAllowed(i, EntityContext.Gen4)).ToArray(),
            g5_released_held = Enumerable.Range(1, 1000).Where(i => ItemRestrictions.IsHeldItemAllowed(i, EntityContext.Gen5)).ToArray(),
            b2w2_tutor_moves = new[] { PersonalInfo5B2W2.TutorMoves1.ToArray(), PersonalInfo5B2W2.TutorMoves2.ToArray(), PersonalInfo5B2W2.TutorMoves3.ToArray(), PersonalInfo5B2W2.TutorMoves4.ToArray() },
        }));

        var langs = new[] { "ja", "en", "fr", "it", "de", "es", "ko" };
        foreach (var lang in langs)
        {
            var gs = GameInfo.GetStrings(lang);
            File.WriteAllText(Path.Combine(outDir, $"strings_{lang}.json"), JsonSerializer.Serialize(new
            {
                lang,
                species = gs.specieslist.Take(Sav(GameVersion.B2).MaxSpeciesID + 1),
                moves = gs.movelist.Take(Sav(GameVersion.B2).MaxMoveID + 1),
                abilities = gs.abilitylist.Take(Sav(GameVersion.B2).MaxAbilityID + 1),
                natures = gs.natures,
                types = gs.types.Take(17),
                items4 = gs.GetItemStrings(EntityContext.Gen4, GameVersion.HGSS).Take(Sav(GameVersion.HG).MaxItemID + 1),
                items5 = gs.GetItemStrings(EntityContext.Gen5, GameVersion.B2W2).Take(Sav(GameVersion.B2).MaxItemID + 1),
                loc4_0 = gs.Gen4.Met0.ToArray(),
                loc4_2000 = gs.Gen4.Met2.ToArray(),
                loc4_3000 = gs.Gen4.Met3.ToArray(),
                loc5_0 = gs.Gen5.Met0.ToArray(),
                loc5_30000 = gs.Gen5.Met3.ToArray(),
                loc5_40000 = gs.Gen5.Met4.ToArray(),
                loc5_60000 = gs.Gen5.Met6.ToArray(),
            }));
        }
        var names5 = new System.Collections.Generic.Dictionary<string, string[]>();
        foreach (var lang in new[] { 1, 2, 3, 4, 5, 7, 8 })
            names5[lang.ToString()] = Enumerable.Range(0, 650).Select(sp => SpeciesName.GetSpeciesNameGeneration((ushort)sp, lang, 5)).ToArray();
        File.WriteAllText(Path.Combine(outDir, "names5.json"), JsonSerializer.Serialize(names5));
        var names4 = new System.Collections.Generic.Dictionary<string, string[]>();
        foreach (var lang in Enumerable.Range(0, 11))
            names4[lang.ToString()] = Enumerable.Range(0, Sav(GameVersion.HG).MaxSpeciesID + 1)
                .Select(sp => SpeciesName.GetSpeciesNameGeneration((ushort)sp, lang, 4)).ToArray();
        File.WriteAllText(Path.Combine(outDir, "names4.json"), JsonSerializer.Serialize(names4));

        File.WriteAllText(Path.Combine(outDir, "manifest.json"), JsonSerializer.Serialize(manifest));
        Console.WriteLine(JsonSerializer.Serialize(manifest));
        return 0;
    }

    private static object? Simple(object? v) => v switch
    {
        null => null,
        string str => str,
        bool b => b,
        Enum e => Convert.ToInt64(e),
        byte or sbyte or short or ushort or int or uint or long or ulong => Convert.ToInt64(v),
        System.Collections.IEnumerable list when v.GetType().IsArray && v.GetType().GetElementType() is { } et && (et.IsPrimitive || et.IsEnum)
            => list.Cast<object>().Select(Simple).ToArray(),
        _ => SimpleStruct(v),
    };

    private static object? SimpleStruct(object v)
    {
        var t = v.GetType();
        if (!t.IsValueType || t.Namespace != "PKHeX.Core")
            return null;
        // Small PKHeX value types (e.g. Moveset, IndividualValueSet): flatten their simple properties.
        var d = new System.Collections.Generic.Dictionary<string, object?>();
        foreach (var p in t.GetProperties(System.Reflection.BindingFlags.Public | System.Reflection.BindingFlags.Instance))
        {
            if (p.GetIndexParameters().Length != 0) continue;
            object? x;
            try { x = p.GetValue(v); } catch { continue; }
            if (x is null || x.GetType().IsValueType && x.GetType().Namespace == "PKHeX.Core" && !x.GetType().IsEnum) continue;
            var sv = Simple(x);
            if (sv != null) d[p.Name] = sv;
        }
        return d.Count > 0 ? d : null;
    }

    private static System.Collections.Generic.Dictionary<string, object?> Describe(object o)
    {
        var d = new System.Collections.Generic.Dictionary<string, object?> { ["kind"] = o.GetType().Name };
        if (o is EncounterGift3 g3)
            d["VersionMask"] = Enumerable.Range(1, 5).Sum(v => g3.Version.Contains((GameVersion)v) ? 1 << (v - 1) : 0);
        foreach (var p in o.GetType().GetProperties(System.Reflection.BindingFlags.Public | System.Reflection.BindingFlags.Instance))
        {
            if (p.GetIndexParameters().Length != 0 || p.Name is "Parent" or "EqualityContract" or "Name" or "LongName")
                continue;
            object? v;
            try { v = p.GetValue(o); } catch { continue; }
            var sv = Simple(v);
            if (sv != null) d[p.Name] = sv;
        }
        foreach (var f in o.GetType().GetFields(System.Reflection.BindingFlags.Public | System.Reflection.BindingFlags.Instance))
        {
            var sv = Simple(f.GetValue(o));
            if (sv != null) d[f.Name] = sv;
        }
        // In-game trade OT names and nicknames by language (private ReadOnlyMemory<string>).
        foreach (var name in new[] { "TrainerNames", "Nicknames" })
        {
            var f = o.GetType().GetField(name, System.Reflection.BindingFlags.NonPublic | System.Reflection.BindingFlags.Instance);
            if (f?.GetValue(o) is ReadOnlyMemory<string> m)
                d[name] = o is EncounterTrade3 ? m.ToArray().Select((n, lang) => Gen3NameIn4(n, lang)).ToArray() : m.ToArray();
        }
        return d;
    }

    /// Same conversion `EncounterTrade3.IsTrainerMatch` applies before comparing in the Gen 4 format.
    private static string Gen3NameIn4(string name, int language)
    {
        Span<char> tmp = stackalloc char[name.Length];
        StringConverter345.TransferGlyphs34(name, language, tmp);
        return new string(tmp);
    }

    private static int HandleExportEncounters(string[] args)
    {
        var outDir = args.Length > 1 ? args[1] : "data/exports";
        Directory.CreateDirectory(outDir);
        var asm = typeof(EntityFormat).Assembly;
        const System.Reflection.BindingFlags all = System.Reflection.BindingFlags.Static | System.Reflection.BindingFlags.Public | System.Reflection.BindingFlags.NonPublic;
        var summary = new System.Collections.Generic.Dictionary<string, object>();
        foreach (var cls in new[] { "Encounters3RSE", "Encounters3FRLG", "EncountersWC3", "Encounters4DPPt", "Encounters4HGSS", "Encounters5BW", "Encounters5B2W2", "Encounters5DR" })
        {
            var t = asm.GetType("PKHeX.Core." + cls) ?? throw new InvalidOperationException(cls);
            var groups = new System.Collections.Generic.Dictionary<string, object>();
            foreach (var f in t.GetFields(all))
            {
                if (f.GetValue(null) is not Array arr || arr.Length == 0)
                    continue;
                if (cls == "EncountersWC3" && f.Name is not ("Encounter_WC3" or "PCNY" or "PCJP"))
                    continue; // the rest are the parts Encounter_WC3 is built from
                var et = arr.GetType().GetElementType()!;
                if (typeof(IEncounterArea<IEncounterable>).IsAssignableFrom(et) || et.GetProperty("Slots") is { } slotsProp && et.GetInterfaces().Any(i => i.Name.StartsWith("IEncounterArea")))
                {
                    var areas = new System.Collections.Generic.List<object>();
                    foreach (var area in arr)
                    {
                        var ad = Describe(area);
                        var slots = (Array)area.GetType().GetProperty("Slots")!.GetValue(area)!;
                        ad["slots"] = slots.Cast<object>().Select(Describe).ToArray();
                        areas.Add(ad);
                    }
                    groups[f.Name] = new { type = "areas", items = areas };
                }
                else if (typeof(IEncounterable).IsAssignableFrom(et))
                {
                    groups[f.Name] = new { type = "templates", items = arr.Cast<object>().Select(Describe).ToArray() };
                }
            }
            File.WriteAllText(Path.Combine(outDir, $"enc_{cls}.json"), JsonSerializer.Serialize(groups));
            summary[cls] = groups.ToDictionary(g => g.Key, g => (int)((System.Collections.ICollection)((dynamic)g.Value).items).Count);
        }

        foreach (var (ctx, gen, max) in new[] { (EntityContext.Gen3, 3, 386), (EntityContext.Gen4, 4, (int)Sav(GameVersion.HG).MaxSpeciesID), (EntityContext.Gen5, 5, (int)Sav(GameVersion.B2).MaxSpeciesID) })
        {
            var tree = EvolutionTree.GetEvolutionTree(ctx);
            var table = gen == 3 ? (IPersonalTable)PersonalTable.E : gen == 4 ? PersonalTable.HGSS : PersonalTable.B2W2;
            var evos = new System.Collections.Generic.List<object>();
            for (ushort sp = 1; sp <= max; sp++)
            {
                var fc = Math.Max((byte)1, table.GetFormEntry(sp, 0).FormCount);
                for (byte f = 0; f < fc; f++)
                {
                    var fw = tree.Forward.GetForward(sp, f).ToArray();
                    if (fw.Length == 0) continue;
                    evos.Add(new { species = sp, form = f, evolutions = fw.Select(e => new { e.Species, e.Form, method = (int)e.Method, e.Argument, e.Level, e.LevelUp }) });
                }
            }
            File.WriteAllText(Path.Combine(outDir, $"evolutions{gen}.json"), JsonSerializer.Serialize(new { generation = gen, evolutions = evos }));
            summary[$"evolutions{gen}"] = evos.Count;
        }
        Console.WriteLine(JsonSerializer.Serialize(summary, JsonOpts));
        return 0;
    }

    // Input: JSON array of {group, location, slot_type, slot_number, species, form, level_min, level_max,
    // tid, sid, pid, iv32, level}. Builds each with PKHeX from the matching slot and reports legality.
    private static int HandleCheckWild4(string[] args)
    {
        var asm = typeof(EntityFormat).Assembly;
        const System.Reflection.BindingFlags all = System.Reflection.BindingFlags.Static | System.Reflection.BindingFlags.Public | System.Reflection.BindingFlags.NonPublic;
        var t = asm.GetType("PKHeX.Core.Encounters4DPPt")!;
        using var doc = JsonDocument.Parse(File.ReadAllBytes(args[1]));
        var tr = Sav(GameVersion.Pt);
        int valid = 0, total = 0;
        var failures = new System.Collections.Generic.List<object>();
        foreach (var r in doc.RootElement.EnumerateArray())
        {
            int I(string k) => r.GetProperty(k).GetInt32();
            var areas = (EncounterArea4[])t.GetField(r.GetProperty("group").GetString()!, all)!.GetValue(null)!;
            var slot = areas.SelectMany(a => a.Slots).First(sl =>
                sl.Location == I("location") && (int)sl.Type == I("slot_type") && sl.SlotNumber == I("slot_number")
                && sl.Species == I("species") && sl.Form == I("form") && sl.LevelMin == I("level_min") && sl.LevelMax == I("level_max")
                && sl.GroundTile == (GroundTileAllowed)r.GetProperty("ground").GetInt64() && sl.AreaRate == I("rate"));
            tr.TID16 = (ushort)I("tid");
            tr.SID16 = (ushort)I("sid");
            var pk = slot.ConvertToPKM(tr);
            var pid = r.GetProperty("pid").GetUInt32();
            var level = (byte)I("level");
            pk.PID = pid;
            pk.IV32 = r.GetProperty("iv32").GetUInt32();
            pk.Gender = EntityGender.GetFromPIDAndRatio(pid, pk.PersonalInfo.Gender);
            var pi = (PersonalInfo4)pk.PersonalInfo;
            pk.Ability = (pid & 1) == 0 ? pi.Ability1 : pi.Ability2;
            pk.MetLevel = level;
            pk.CurrentLevel = level;
            Span<ushort> moves = stackalloc ushort[4];
            LearnSource4Pt.Instance.GetLearnset(pk.Species, pk.Form).SetEncounterMoves(level, moves);
            pk.SetMoves(moves);
            pk.RefreshChecksum();
            if (args.Length > 2)
            {
                Directory.CreateDirectory(args[2]);
                File.WriteAllBytes(Path.Combine(args[2], $"{total:D6}.pk4"), pk.DecryptedBoxData);
            }
            var la = new LegalityAnalysis(pk);
            total++;
            if (la.Valid) valid++;
            else if (failures.Count < 20)
                failures.Add(new { index = total - 1, report = la.Report(), lead = (int)la.Info.PIDIV.Lead });
        }
        Console.WriteLine(JsonSerializer.Serialize(new { total, valid, failures }, JsonOpts));
        return 0;
    }

    // gen-wild4 <EncountersClass> <group> <count> <seed> <out-dir>: PKHeX-generated, PKHeX-legal
    // wild PK4s from random slots of a group (used to fuzz verity's checker on HG/SS).
    private static int HandleGenWild4(string[] args)
    {
        var asm = typeof(EntityFormat).Assembly;
        const System.Reflection.BindingFlags all = System.Reflection.BindingFlags.Static | System.Reflection.BindingFlags.Public | System.Reflection.BindingFlags.NonPublic;
        var t = asm.GetType("PKHeX.Core." + args[1])!;
        var areas = (EncounterArea4[])t.GetField(args[2], all)!.GetValue(null)!;
        var slots = areas.SelectMany(a => a.Slots).ToArray();
        int count = int.Parse(args[3]);
        var rng = new Random(int.Parse(args[4]));
        Directory.CreateDirectory(args[5]);
        var hgss = args[2].Contains("HG") || args[2].Contains("SS");
        var tr = Sav(hgss ? (args[2].Contains("HG") ? GameVersion.HG : GameVersion.SS) : GameVersion.Pt);
        int made = 0, tried = 0;
        while (made < count && tried < count * 20)
        {
            tried++;
            var slot = slots[rng.Next(slots.Length)];
            tr.TID16 = (ushort)rng.Next(65536);
            tr.SID16 = (ushort)rng.Next(65536);
            PK4 pk;
            try { pk = slot.ConvertToPKM(tr); } catch { continue; }
            if (!new LegalityAnalysis(pk).Valid)
                continue;
            File.WriteAllBytes(Path.Combine(args[5], $"{made:D6}.pk4"), pk.DecryptedBoxData);
            made++;
        }
        Console.WriteLine(JsonSerializer.Serialize(new { made, tried }));
        return 0;
    }

    /// M6: `gen-gift4 <count> <seed> <out-dir>`: PKHeX-generated legal Gen 4 gift PK4s (random
    /// Wonder Cards from `MGDB_G4`, plus Ranger Manaphy eggs) via `ConvertToPKM`.
    /// My Pokémon Ranch gifts on Diamond, in each language with a trainer name.
    private static int HandleGenRanch4(string[] args)
    {
        int count = int.Parse(args[1]);
        var rng = new Random(int.Parse(args[2]));
        Directory.CreateDirectory(args[3]);
        const System.Reflection.BindingFlags all = System.Reflection.BindingFlags.Static | System.Reflection.BindingFlags.Public | System.Reflection.BindingFlags.NonPublic;
        var gifts = (EncounterTrade4RanchGift[])typeof(EncounterGenerator4).Assembly.GetType("PKHeX.Core.Encounters4DPPt")!.GetField("RanchGifts", all)!.GetValue(null)!;
        int[] languages = [1, 2, 3, 4, 5, 7];
        int made = 0, tried = 0;
        while (made < count && tried < count * 20)
        {
            tried++;
            var tr = Sav(GameVersion.D);
            tr.Language = languages[rng.Next(languages.Length)];
            var pk = gifts[rng.Next(gifts.Length)].ConvertToPKM(tr);
            pk.RefreshChecksum();
            if (!new LegalityAnalysis(pk).Valid)
                continue;
            File.WriteAllBytes(Path.Combine(args[3], $"{made:D6}.pk4"), pk.DecryptedBoxData);
            made++;
        }
        Console.WriteLine(JsonSerializer.Serialize(new { made, tried }));
        return made > 0 ? 0 : 1;
    }

    /// Gen 3 origin data: Emerald's species table, the TM/HM and tutor bits, and the level-up moves of each game.
    private static object ExportG3()
    {
        var table = PersonalTable.E;
        var personal = new System.Collections.Generic.List<int[]>();
        var tutors = new System.Collections.Generic.List<int[]>();
        var forms = new System.Collections.Generic.List<int[]>();
        for (int i = 0; i < table.Count; i++)
        {
            var pi = table[i];
            var raw = new int[0x2C];
            Raw(pi).CopyTo(raw, 0);
            // Lay it out as a Gen 4 entry so the reader is shared: TM/HM bits at 0x1C, no forms.
            Array.Clear(raw, 0x1C, 0x10);
            for (int b = 0; b < pi.TMHM.Length; b++)
                if (pi.TMHM[b]) raw[0x1C + (b >> 3)] |= 1 << (b & 7);
            raw[0x29] = pi.FormCount;
            raw[0x2A] = pi.FormStatsIndex & 0xFF;
            raw[0x2B] = pi.FormStatsIndex >> 8;
            personal.Add(raw);
            tutors.Add(pi.TypeTutors.Select(b => b ? 1 : 0).ToArray());
        }
        for (ushort sp = 1; sp <= 386; sp++)
        {
            var fc = table.GetFormEntry(sp, 0).FormCount;
            for (byte f = 0; f < Math.Max((byte)1, fc); f++)
                forms.Add(new[] { (int)sp, f, table.GetFormIndex(sp, f) });
        }
        object Levels(Func<ushort, byte, Learnset> get)
        {
            var o = new System.Collections.Generic.List<object>();
            for (ushort sp = 1; sp <= 386; sp++)
            {
                var ls = get(sp, 0);
                var mv = ls.GetAllMoves(); var lv = ls.GetAllLevels();
                var arr = new int[mv.Length][];
                for (int k = 0; k < mv.Length; k++) arr[k] = new int[] { lv[k], mv[k] };
                o.Add(new { index = (int)sp, moves = arr });
            }
            return o;
        }
        var egg = new System.Collections.Generic.List<object>();
        for (ushort sp = 1; sp <= 386; sp++)
        {
            var e = LearnSource3E.Instance.GetEggMoves(sp, 0).ToArray();
            if (e.Length > 0) egg.Add(new { species = sp, form = 0, moves = e });
        }
        return new
        {
            game = "g3",
            generation = 3,
            max_species = 386,
            personal_size = 0x2C,
            personal,
            type_tutors = tutors,
            forms,
            levelup = Levels(LearnSource3E.Instance.GetLearnset),
            levelup_rs = Levels(LearnSource3RS.Instance.GetLearnset),
            levelup_fr = Levels(LearnSource3FR.Instance.GetLearnset),
            levelup_lg = Levels(LearnSource3LG.Instance.GetLearnset),
            egg_moves = egg,
            held_items = Array.Empty<int>(),
        };
    }

    /// The Gen 5 event database as hex records (`PGF` is 0xCC bytes).
    private static int HandleExportGifts5(string[] args)
    {
        var list = EncounterEvent.MGDB_G5.Select(g => Convert.ToHexString(g.Data)).ToArray();
        File.WriteAllText(args[1], JsonSerializer.Serialize(list));
        Console.WriteLine(list.Length);
        return 0;
    }

    /// PKHeX-legal Pokémon from a Gen 5 game: random trainer, encounter, language and level gain.
    private static int HandleGenOrigin5(string[] args)
    {
        int count = int.Parse(args[1]);
        var rng = new Random(int.Parse(args[2]));
        Directory.CreateDirectory(args[3]);
        string mode = args.Length > 4 ? args[4] : "slot";
        const System.Reflection.BindingFlags all = System.Reflection.BindingFlags.Static | System.Reflection.BindingFlags.Public | System.Reflection.BindingFlags.NonPublic;
        var asm = typeof(EntityFormat).Assembly;
        IEnumerable<IEncounterConvertible> Field(string cls, string f) => ((Array)asm.GetType("PKHeX.Core." + cls)!.GetField(f, all)!.GetValue(null)!).Cast<IEncounterConvertible>();
        IEnumerable<IEncounterConvertible> Areas(string cls, string f) => ((Array)asm.GetType("PKHeX.Core." + cls)!.GetField(f, all)!.GetValue(null)!).Cast<EncounterArea5>().SelectMany(a => a.Slots).Cast<IEncounterConvertible>();
        const string bw = "Encounters5BW", b2 = "Encounters5B2W2", dr = "Encounters5DR";
        IEncounterConvertible[] Eggs(GameVersion v) => Enumerable.Range(1, 649).Where(sp => Breeding.CanHatchAsEgg((ushort)sp) && EvolutionTree.Evolves5.GetBaseSpeciesForm((ushort)sp, 0).Item1 == sp)
            .Select(sp => (IEncounterConvertible)new EncounterEgg5((ushort)sp, 0, v)).ToArray();
        (GameVersion V, IEncounterConvertible[] L) Game(GameVersion v)
        {
            bool isBw = v is GameVersion.B or GameVersion.W;
            string cls = isBw ? bw : b2;
            IEnumerable<IEncounterConvertible> list = mode switch
            {
                "static" => Field(cls, "Static" + v).Concat(Field(cls, isBw ? "Encounter_BW" : "Encounter_B2W2_Regular")),
                "entree" => Field(cls, isBw ? "DreamWorld_BW" : "DreamWorld_B2W2").Concat(Field(dr, "DreamWorld_Common")),
                "radar" => isBw ? Array.Empty<IEncounterConvertible>() : Field(dr, "Encounter_DreamRadar"),
                "n" => isBw ? Array.Empty<IEncounterConvertible>() : Field(b2, "Encounter_B2W2_N"),
                "trade" => Field(cls, "TradeGift_" + v).Concat(Field(cls, isBw ? "TradeGift_BW" : "TradeGift_B2W2")),
                "egg" or "unhatched" => Eggs(v),
                "gift" => EncounterEvent.MGDB_G5.Where(g => g.IsEntity).Cast<IEncounterConvertible>(),
                _ => Areas(cls, "Slots" + v),
            };
            return (v, list.ToArray());
        }
        var games = new[] { GameVersion.W, GameVersion.B, GameVersion.W2, GameVersion.B2 }.Select(Game).Where(g => g.L.Length > 0).ToArray();
        int[] languages = [1, 2, 3, 4, 5, 7, 8];
        var giftEggs = EncounterEvent.MGDB_G5.Where(g => g.IsEntity && g.IsEgg).Cast<IEncounterConvertible>().ToArray();
        int made = 0, tried = 0;
        while (made < count && tried < count * 20)
        {
            tried++;
            var (v, list) = games[rng.Next(games.Length)];
            var enc = list[rng.Next(list.Length)];
            var tr = Sav(v);
            tr.Language = languages[rng.Next(languages.Length)];
            tr.TID16 = (ushort)rng.Next(65536);
            tr.SID16 = (ushort)rng.Next(65536);
            if (mode == "unhatched")
                enc = rng.Next(5) == 0 && giftEggs.Length != 0 ? giftEggs[rng.Next(giftEggs.Length)] : enc;
            if (enc.ConvertToPKM(tr) is not PK5 pk)
                continue;
            if (mode == "unhatched")
            {
                pk.IsEgg = true;
                pk.IsNicknamed = true;
                pk.Nickname = SpeciesName.GetEggName(pk.Language, 5);
                pk.OriginalTrainerFriendship = (byte)rng.Next(pk.PersonalInfo.HatchCycles + 1);
                pk.EggLocation = enc is PGF g5 ? g5.EggLocation : Locations.Daycare5;
                pk.EggMetDate = new DateOnly(2012, 3, 4);
                pk.MetLevel = 1;
                pk.MetDate = null;
                pk.MetLocation = 0;
                if (enc is EncounterEgg5 egg5)
                {
                    var learn = GameData.GetLearnSource(v).GetLearnset(egg5.Species, 0);
                    var wanted = learn.GetBaseEggMoves(1).ToArray().ToList();
                    wanted.AddRange(LearnSource5BW.Instance.GetEggMoves(egg5.Species, 0).ToArray().OrderBy(_ => rng.Next()).Take(rng.Next(0, 3)));
                    var request = wanted.TakeLast(4).Concat(Enumerable.Repeat((ushort)0, 4)).Take(4).ToArray();
                    var expected = new ushort[4];
                    MoveBreed.GetExpectedMoves(request, egg5, expected);
                    pk.SetMoves(new Moveset(expected[0], expected[1], expected[2], expected[3]));
                    pk.HealPP();
                }
            }
            else if (!pk.IsEgg)
                pk.CurrentLevel = (byte)Math.Min(100, pk.CurrentLevel + rng.Next(3) * rng.Next(20));
            pk.RefreshChecksum();
            if (!new LegalityAnalysis(pk).Valid)
                continue;
            File.WriteAllBytes(Path.Combine(args[3], $"{made:D6}.pk5"), pk.DecryptedBoxData);
            made++;
        }
        Console.WriteLine(JsonSerializer.Serialize(new { made, tried }));
        return made > 0 ? 0 : 1;
    }

    /// PKHeX-legal Gen 3 wild Pokémon sent through Pal Park: random trainer, slot, language and level gain.
    private static int HandleGenSlot3(string[] args)
    {
        int count = int.Parse(args[1]);
        var rng = new Random(int.Parse(args[2]));
        Directory.CreateDirectory(args[3]);
        const System.Reflection.BindingFlags all = System.Reflection.BindingFlags.Static | System.Reflection.BindingFlags.Public | System.Reflection.BindingFlags.NonPublic;
        var asm = typeof(EntityFormat).Assembly;
        EncounterArea3[] Get(string cls, string field) => (EncounterArea3[])asm.GetType("PKHeX.Core." + cls)!.GetField(field, all)!.GetValue(null)!;
        T[] Field<T>(string cls, string field) => (T[])asm.GetType("PKHeX.Core." + cls)!.GetField(field, all)!.GetValue(null)!;
        IEncounterConvertible[] Stat(string cls, params string[] fields) => fields.SelectMany(f => Field<EncounterStatic3>(cls, f)).Cast<IEncounterConvertible>().ToArray();
        IEncounterConvertible[] Trade(string cls, params string[] fields) => fields.SelectMany(f => Field<EncounterTrade3>(cls, f)).Cast<IEncounterConvertible>().ToArray();
        IEncounterConvertible[] Slots(EncounterArea3[] areas) => areas.SelectMany(a => a.Slots).Cast<IEncounterConvertible>().ToArray();
        IEncounterConvertible[] Eggs(GameVersion v) => Enumerable.Range(1, 386).Where(sp => Breeding.CanHatchAsEgg((ushort)sp) && EvolutionTree.Evolves3.GetBaseSpeciesForm((ushort)sp, 0).Item1 == sp)
            .Select(sp => (IEncounterConvertible)new EncounterEgg3((ushort)sp, v)).ToArray();
        IEncounterConvertible[] Gifts() => Field<EncounterGift3>("EncountersWC3", "Encounter_WC3").Cast<IEncounterConvertible>()
            .Concat(Field<EncounterGift3NY>("EncountersWC3", "PCNY")).Concat(Field<EncounterGift3JPN>("EncountersWC3", "PCJP"))
            .Concat(Field<EncounterGift3Colo>("Encounters3RSE", "ColoGiftsR")).Concat(Field<EncounterGift3Colo>("Encounters3RSE", "ColoGiftsS")).ToArray();
        string mode = args.Length > 4 ? args[4] : "slot";
        (GameVersion V, IEncounterConvertible[] Slots) Game(GameVersion v, string rse, string frlg)
        {
            bool isRse = v is GameVersion.R or GameVersion.S or GameVersion.E;
            var cls = isRse ? "Encounters3RSE" : "Encounters3FRLG";
            var shared = isRse ? "StaticRSE" : "StaticFRLG";
            return (v, mode switch
            {
                "static" => Stat(cls, rse, shared),
                "trade" => isRse
                    ? Trade(cls, v == GameVersion.E ? new[] { "TradeGift_E" } : new[] { "TradeGift_RS", "TradeGift_E" })
                    : Trade(cls, frlg, "TradeGift_FRLG"),
                "egg" => Eggs(v),
                "gift" => Gifts(),
                _ => Slots(Get(cls, "Slots" + v)),
            });
        }
        var games = new[]
        {
            Game(GameVersion.R, "StaticR", ""), Game(GameVersion.S, "StaticS", ""), Game(GameVersion.E, "StaticE", ""),
            Game(GameVersion.FR, "StaticFR", "TradeGift_FR"), Game(GameVersion.LG, "StaticLG", "TradeGift_LG"),
        };
        int[] languages = [1, 2, 3, 4, 5, 7];
        int made = 0, tried = 0;
        while (made < count && tried < count * 20)
        {
            tried++;
            var (v, slots) = games[rng.Next(games.Length)];
            var slot = slots[rng.Next(slots.Length)];
            var tr = Sav(v);
            tr.Language = languages[rng.Next(languages.Length)];
            tr.TID16 = (ushort)rng.Next(65536);
            tr.SID16 = (ushort)rng.Next(65536);
            var made3 = slot.ConvertToPKM(tr);
            if (!made3.Valid) continue;
            var pk3 = made3 is CK3 ck ? ck.ConvertToPK3() : (PK3)made3;
            pk3.CurrentLevel = (byte)Math.Min(100, Math.Max(pk3.MetLevel, (byte)5) + rng.Next(3) * rng.Next(20));
            var pk = pk3.ConvertToPK4();
            pk.RefreshChecksum();
            if (!new LegalityAnalysis(pk).Valid)
                continue;
            File.WriteAllBytes(Path.Combine(args[3], $"{made:D6}.pk4"), pk.DecryptedBoxData);
            made++;
        }
        Console.WriteLine(JsonSerializer.Serialize(new { made, tried }));
        return made > 0 ? 0 : 1;
    }

    /// Unhatched Gen 4 eggs: bred (random legal moves from MoveBreed), gift eggs and Ranger Manaphy.
    private static int HandleGenEgg4(string[] args)
    {
        int count = int.Parse(args[1]);
        var rng = new Random(int.Parse(args[2]));
        Directory.CreateDirectory(args[3]);
        const System.Reflection.BindingFlags all = System.Reflection.BindingFlags.Static | System.Reflection.BindingFlags.Public | System.Reflection.BindingFlags.NonPublic;
        var ranger = (PGT)typeof(EncounterGenerator4).GetField("RangerManaphy", all)!.GetValue(null)!;
        GameVersion[] versions = [GameVersion.D, GameVersion.P, GameVersion.Pt, GameVersion.HG, GameVersion.SS];
        var core = typeof(EncounterGenerator4).Assembly;
        EncounterStatic4[] Table(string type, string field) => (EncounterStatic4[])core.GetType("PKHeX.Core." + type)!.GetField(field, all)!.GetValue(null)!;
        var statics = Table("Encounters4DPPt", "StaticDP").Concat(Table("Encounters4DPPt", "StaticPt")).Concat(Table("Encounters4HGSS", "Encounter_HGSS")).Where(e => e.IsEgg).ToArray();
        int made = 0, tried = 0, statsBred = 0, statsGift = 0, statsRanger = 0;
        while (made < count && tried < count * 40)
        {
            tried++;
            var v = versions[rng.Next(versions.Length)];
            var tr = Sav(v);
            tr.TID16 = (ushort)rng.Next(65536);
            tr.SID16 = (ushort)rng.Next(65536);
            PK4 pk;
            int kind = rng.Next(10);
            try
            {
                if (kind == 0)
                {
                    pk = (PK4)ranger.ConvertToPKM(tr);
                    if (!pk.IsEgg) continue;
                    pk.MetDate = pk.MetLocation == 0 ? null : new DateOnly(2010, 3, 5);
                }
                else if (kind <= 2)
                {
                    pk = statics[rng.Next(statics.Length)].ConvertToPKM(tr);
                    pk.IsEgg = true;
                    MakeUnhatched(pk, tr, rng, pk.EggLocation);
                }
                else
                {
                    var species = (ushort)rng.Next(1, 491);
                    if (!Breeding.CanHatchAsEgg(species) || !PersonalTable.HGSS.IsPresentInGame(species, 0))
                        continue;
                    if (EvolutionTree.Evolves4.GetBaseSpeciesForm(species, 0).Species != species && !Breeding.IsSplitBreedNotBabySpecies4(species))
                        continue;
                    var egg = new EncounterEgg4(species, v);
                    pk = egg.ConvertToPKM(tr);
                    pk.IsEgg = true;
                    MakeUnhatched(pk, tr, rng, Locations.Daycare4);
                    var learn = GameData.GetLearnSource(v).GetLearnset(species, 0);
                    var eggMoves = egg.Learn.GetEggMoves(species, 0);
                    var wanted = learn.GetBaseEggMoves(1).ToArray().ToList();
                    wanted.AddRange(eggMoves.ToArray().OrderBy(_ => rng.Next()).Take(rng.Next(0, 3)));
                    var request = wanted.TakeLast(4).Concat(Enumerable.Repeat((ushort)0, 4)).Take(4).ToArray();
                    var expected = new ushort[4];
                    MoveBreed.GetExpectedMoves(request, egg, expected);
                    pk.SetMoves(new Moveset(expected[0], expected[1], expected[2], expected[3]));
                    pk.HealPP();
                }
            }
            catch { continue; }
            pk.RefreshChecksum();
            if (!new LegalityAnalysis(pk).Valid)
                continue;
            if (kind == 0) statsRanger++; else if (kind <= 2) statsGift++; else statsBred++;
            File.WriteAllBytes(Path.Combine(args[3], $"{made:D6}.pk4"), pk.DecryptedBoxData);
            made++;
        }
        Console.WriteLine(JsonSerializer.Serialize(new { made, tried, bred = statsBred, gift = statsGift, ranger = statsRanger }));
        return made > 0 ? 0 : 1;
    }

    private static void MakeUnhatched(PK4 pk, ITrainerInfo tr, Random rng, ushort eggLocation)
    {
        pk.IsNicknamed = false;
        pk.Nickname = SpeciesName.GetEggName(pk.Language, 4);
        pk.OriginalTrainerFriendship = (byte)rng.Next(pk.PersonalInfo.HatchCycles + 1);
        pk.EggLocation = eggLocation;
        pk.EggMetDate = new DateOnly(2010, 3, 4);
        pk.MetLevel = 0;
        pk.MetDate = null;
        pk.MetLocation = 0;
    }

    private static int HandleGenGift4(string[] args)
    {
        int count = int.Parse(args[1]);
        var rng = new Random(int.Parse(args[2]));
        Directory.CreateDirectory(args[3]);
        const System.Reflection.BindingFlags all = System.Reflection.BindingFlags.Static | System.Reflection.BindingFlags.Public | System.Reflection.BindingFlags.NonPublic;
        var ranger = (PGT)typeof(EncounterGenerator4).GetField("RangerManaphy", all)!.GetValue(null)!;
        var gifts = EncounterEvent.MGDB_G4.Where(g => g.IsEntity).ToArray();
        GameVersion[] versions = [GameVersion.D, GameVersion.P, GameVersion.Pt, GameVersion.HG, GameVersion.SS];
        int made = 0, tried = 0;
        while (made < count && tried < count * 20)
        {
            tried++;
            var tr = Sav(versions[rng.Next(versions.Length)]);
            tr.TID16 = (ushort)rng.Next(65536);
            tr.SID16 = (ushort)rng.Next(65536);
            PK4 pk;
            try
            {
                pk = (PK4)(rng.Next(8) == 0 ? ranger.ConvertToPKM(tr) : gifts[rng.Next(gifts.Length)].ConvertToPKM(tr));
            }
            catch { continue; }
            if (!new LegalityAnalysis(pk).Valid)
                continue;
            File.WriteAllBytes(Path.Combine(args[3], $"{made:D6}.pk4"), pk.DecryptedBoxData);
            made++;
        }
        Console.WriteLine(JsonSerializer.Serialize(new { made, tried }));
        return 0;
    }

    /// M6: PKHeX's Gen 4 event database (`EncounterEvent.MGDB_G4`, wc4.pkl) as raw PCD records
    /// with the inner PK4 decrypted, plus parsed fields for cross-checking.
    private static int HandleExportGifts4(string[] args)
    {
        var outPath = args.Length > 1 ? args[1] : "data/exports/gifts4.json";
        var list = new System.Collections.Generic.List<object>();
        var db = EncounterEvent.MGDB_G4;
        for (int i = 0; i < db.Length; i++)
        {
            var pcd = db[i];
            var raw = pcd.Data.ToArray();
            var g = pcd.Gift;
            if (g.IsEntity && !g.IsManaphyEgg)
                g.PK.Data.CopyTo(raw.AsSpan(8));
            Span<int> ivs = stackalloc int[6];
            pcd.GetIVs(ivs);
            list.Add(new
            {
                index = i,
                card_id = pcd.CardID,
                gift_type = (int)pcd.GiftType,
                entity = pcd.IsEntity,
                raw = Convert.ToHexString(raw),
                species = pcd.Species,
                form = pcd.Form,
                level = pcd.Level,
                ball = pcd.Ball,
                is_egg = pcd.IsEgg,
                version = (int)pcd.Version,
                shiny = pcd.Shiny.ToString(),
                fateful = pcd.FatefulEncounter,
                location = pcd.Location,
                egg_location = pcd.EggLocation,
                fixed_ivs = pcd.HasFixedIVs,
                ivs = ivs.ToArray(),
                random_pid = pcd.IsEntity && pcd.IsRandomPID(),
                moves = new[] { pcd.Moves.Move1, pcd.Moves.Move2, pcd.Moves.Move3, pcd.Moves.Move4 },
                tid = pcd.TID16,
                sid = pcd.SID16,
                ot = pcd.OriginalTrainerName,
                title = pcd.CardTitle,
                compat = pcd.CardCompatibility,
            });
        }
        File.WriteAllText(outPath, JsonSerializer.Serialize(list));
        Console.WriteLine($"{db.Length} gifts -> {outPath}");
        return 0;
    }

    private static int HandleExportText4(string[] args)
    {
        var res = new
        {
            pkhex_version = typeof(EntityFormat).Assembly.GetName().Version?.ToString(),
            table_int = StringConverter4Util.TableINT.ToArray().Select(c => (int)c).ToArray(),
            table_kor_start = 0x400,
            table_kor = StringConverter4Util.TableKOR.ToArray().Select(c => (int)c).ToArray(),
        };
        File.WriteAllText(args[1], JsonSerializer.Serialize(res));
        return 0;
    }

    private static int UnknownCommand(string command)
    {
        Console.Error.WriteLine($"Unknown command: {command}");
        PrintUsage();
        return 1;
    }
}
