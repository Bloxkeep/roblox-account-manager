// Stands in for Bloxkeep's C# side in the GPUI spike: opens the pipe, starts the UI, sends Home, answers calls,
// pushes the PC's line every 2 s, and reports how long the UI took to draw and how much memory it uses.
using System.Diagnostics;
using System.IO.Pipes;
using System.Text;
using System.Text.Json.Nodes;

var ui = args.Length > 0 ? args[0] : throw new ArgumentException("usage: PipeStub <bloxkeep-ui.exe> [ui args...]");
var assets = Path.Combine(Path.GetDirectoryName(ui)!, "assets");
var home = JsonNode.Parse(File.ReadAllText(Path.Combine(assets, "home.json")))!;
var name = $"bloxkeep-ui-{Environment.ProcessId}";
using var pipe = new NamedPipeServerStream(name, PipeDirection.Out, 1, PipeTransmissionMode.Byte, PipeOptions.Asynchronous);
using var up = new NamedPipeServerStream(name + "-up", PipeDirection.In, 1, PipeTransmissionMode.Byte, PipeOptions.Asynchronous);

var sw = Stopwatch.StartNew();
var psi = new ProcessStartInfo(ui) { UseShellExecute = false };
psi.ArgumentList.Add("--pipe"); psi.ArgumentList.Add(name);
foreach (var a in args.Skip(1)) psi.ArgumentList.Add(a);
var proc = Process.Start(psi)!;
await pipe.WaitForConnectionAsync();
await up.WaitForConnectionAsync();
Console.WriteLine($"[stub] UI connected after {sw.ElapsedMilliseconds} ms");
var w = new StreamWriter(pipe, new UTF8Encoding(false)) { AutoFlush = true, NewLine = "\n" };
var send = (string type, JsonNode data) => { lock (w) w.WriteLine(new JsonObject { ["type"] = type, ["data"] = data.DeepClone() }.ToJsonString()); };
send("home", home);

var r = new StreamReader(up, Encoding.UTF8);
_ = Task.Run(async () => {
    string? line;
    while ((line = await r.ReadLineAsync()) != null) {
        var msg = JsonNode.Parse(line)!;
        var call = (string?)msg["name"];
        if (call == "ready") { Console.WriteLine($"[stub] first frame {sw.ElapsedMilliseconds} ms after start"); continue; }
        Console.WriteLine($"[stub] call {call} {msg["args"]?.ToJsonString()}");
        if (call == "stopGame") {
            // Stop: the game is no longer running, its accounts go off; Home is sent again.
            var place = (long)msg["args"]!["place"]!;
            var g = home["games"]!.AsArray().First(x => (long)x!["place"]! == place)!;
            var gname = (string)g["name"]!;
            g["running"] = null; g["last"] = "Vừa xong";
            foreach (var a in home["accounts"]!.AsArray())
                if ((string?)a!["game"] == gname && (string?)a["state"] != "off") { a["state"] = "off"; a["said"] = "Vừa xong"; a["sinceSecs"] = null; }
            send("home", home);
        }
    }
});

var rnd = new Random(1);
var pc = home["pc"]!.AsObject();
while (!proc.HasExited) {
    await Task.Delay(2000);
    if (proc.HasExited) break;
    pc["ramPct"] = 58 + rnd.Next(10); pc["cpuPct"] = 15 + rnd.Next(20);
    try { send("pc", pc); } catch { break; }
    proc.Refresh();
    if (sw.Elapsed.TotalSeconds > 3)
        Console.WriteLine($"[stub] {sw.Elapsed.TotalSeconds:0}s UI memory: working set {proc.WorkingSet64 / 1048576} MB, private {proc.PrivateMemorySize64 / 1048576} MB, threads {proc.Threads.Count}");
}
Console.WriteLine($"[stub] UI exited ({proc.ExitCode})");
