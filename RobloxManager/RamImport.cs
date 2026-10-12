using System.Text.Json;

namespace RobloxManager;

public sealed record ImportedAccount(string Cookie, string Username, long UserId, string Alias, string Note, string Password, string Group, DateTime? LastUse);

/// <summary>
/// Reads a decrypted AccountData.json exported from "Roblox Account Manager" (ic3w0lf22) so its users can move over.
/// RAM keeps that file encrypted; users decrypt it with RAM's own RAMDecrypt tool first, so this app never
/// unlocks another program's protected data itself.
/// </summary>
public static class RamImport
{
    const long MaxFileSize = 50L * 1024 * 1024;

    public static List<ImportedAccount> Read(string path)
    {
        if (new FileInfo(path).Length > MaxFileSize)
            throw new UserError(L.T("This file is too large to be an account list."));
        ReadOnlySpan<byte> data = File.ReadAllBytes(path);
        if (data.StartsWith("﻿"u8))
            data = data[3..];
        data = data.TrimStart(" \t\r\n"u8);
        if (data.Length == 0 || data[0] != (byte)'[')
            throw new UserError(L.T("This file is still encrypted. Decrypt it first with RAM's official RAMDecrypt tool, then import the decrypted file."));

        try
        {
            using var doc = JsonDocument.Parse(data.ToArray());
            var result = new List<ImportedAccount>();
            foreach (var e in doc.RootElement.EnumerateArray())
            {
                if (e.ValueKind != JsonValueKind.Object)
                    continue;
                string Text(string name) => e.TryGetProperty(name, out var v) && v.ValueKind == JsonValueKind.String ? v.GetString() ?? "" : "";
                long userId = e.TryGetProperty("UserID", out var id) && id.ValueKind == JsonValueKind.Number && id.TryGetInt64(out long n) ? n : 0;
                DateTime? lastUse = e.TryGetProperty("LastUse", out var used) && used.ValueKind == JsonValueKind.String
                                    && used.TryGetDateTime(out var d) && d.Year > 2000 ? d : null;
                string group = Text("Group");
                result.Add(new ImportedAccount(Text("SecurityToken"), Text("Username"), userId, Text("Alias"), Text("Description"),
                    Text("Password"), group == "Default" ? "" : group, lastUse));
            }
            return result;
        }
        catch (JsonException)
        {
            throw new UserError(L.T("This file is not an account list from Roblox Account Manager."));
        }
    }
}
