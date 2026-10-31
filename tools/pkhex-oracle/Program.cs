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
            checksum_info = sav.ChecksumInfo
        };

        Console.WriteLine(JsonSerializer.Serialize(result, JsonOpts));
        return 0;
    }

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
                    report = la.Report()
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
            slots.Add(new { name, species = pk.Species, pid = pk.PID, checksum_valid = pk.ChecksumValid, legal = new LegalityAnalysis(pk).Valid });
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

    private static int UnknownCommand(string command)
    {
        Console.Error.WriteLine($"Unknown command: {command}");
        PrintUsage();
        return 1;
    }
}
