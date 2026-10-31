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
        var sav = SaveUtil.GetSaveFile(data);
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

        byte[] MakeTemplate()
        {
            var b = new byte[0x80000];
            Array.Fill(b, (byte)0xFF);
            return b;
        }

        var saves = new (string filename, SaveFile sav)[]
        {
            ("platinum.sav", new SAV4Pt(MakeTemplate())),
            ("diamond.sav", new SAV4DP(MakeTemplate()) { Version = GameVersion.D }),
            ("pearl.sav", new SAV4DP(MakeTemplate()) { Version = GameVersion.P }),
            ("heartgold.sav", new SAV4HGSS(MakeTemplate()) { Version = GameVersion.HG }),
            ("soulsilver.sav", new SAV4HGSS(MakeTemplate()) { Version = GameVersion.SS }),
            ("black.sav", new SAV5BW(MakeTemplate()) { Version = GameVersion.B }),
            ("white.sav", new SAV5BW(MakeTemplate()) { Version = GameVersion.W }),
            ("black2.sav", new SAV5B2W2(MakeTemplate()) { Version = GameVersion.B2 }),
            ("white2.sav", new SAV5B2W2(MakeTemplate()) { Version = GameVersion.W2 })
        };

        var results = new System.Collections.Generic.List<object>();
        foreach (var (filename, sav) in saves)
        {
            sav.OT = "Verity";
            sav.TID16 = 12345;
            sav.SID16 = 54321;
            var path = Path.Combine(outDir, filename);
            var bytes = sav.Write().ToArray();
            File.WriteAllBytes(path, bytes);
            results.Add(new
            {
                filename,
                version = sav.Version.ToString(),
                generation = (int)sav.Generation,
                size = bytes.Length,
                checksums_valid = sav.ChecksumsValid
            });
        }

        Console.WriteLine(JsonSerializer.Serialize(results, JsonOpts));
        return 0;
    }

    private static int UnknownCommand(string command)
    {
        Console.Error.WriteLine($"Unknown command: {command}");
        PrintUsage();
        return 1;
    }
}
