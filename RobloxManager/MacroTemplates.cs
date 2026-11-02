using System.Text.Json;

namespace RobloxManager;

/// <summary>
/// The built-in starting points for a new macro: generic (no game's name), each running as it is once its marked steps
/// are done on the game (an area moved, a colour picked). Each comes with the settings the player changes on its panel
/// ("$o1") and its areas ("@a1"). Kept as data: the page lists them and opens a copy in the editor.
/// </summary>
static class MacroTemplates
{
    /// <summary>A template: its macro and what it does.</summary>
    /// <param name="Icon">A plain word the page draws an icon for: keyboard, pointer, bar, blank.</param>
    /// <param name="Macro">The macro a new macro starts from (Blank: no steps).</param>
    public sealed record Template(string Id, string Name, string About, string Icon, Macro Macro);

    /// <summary>
    /// For the page: { macros: (Macro &amp; {about, icon})[] }, each new (its id ""). A step the person still has to
    /// finish on the game carries "todo" (its checklist).
    /// </summary>
    public static object All() => new
    {
        macros = Build().Select(t =>
        {
            var node = JsonSerializer.SerializeToNode(t.Macro, MacroRules.PageJson)!;
            node["id"] = "";
            node["about"] = t.About;
            node["icon"] = t.Icon;
            return node;
        }).ToList(),
    };

    /// <summary>The person's to-do on a step (the editor's checklist): kept with the step like any field it doesn't know.</summary>
    static void Todo(MacroStep step, params string[] todo)
    {
        step.Extra ??= new();
        step.Extra["todo"] = JsonSerializer.SerializeToElement(todo);
    }

    public static Template? Find(string? id) => Build().FirstOrDefault(t => t.Id == id);

    /// <summary>Built each time (names in the language now; nothing shared to change by mistake).</summary>
    internal static List<Template> Build() =>
    [
        Keys(),
        Click(),
        Cast(),
        Blank(),
    ];

    static Macro MacroOf(string name, int maxMinutes, string steps, string options, string areas)
    {
        using var json = JsonDocument.Parse($$"""
            {"name":{{S(name)}},"targets":"selected","maxMinutes":{{maxMinutes}},"steps":{{steps}},"options":{{options}},"areas":{{areas}},
             "panel":{"autoShow":false,"placeId":null} }
            """);
        return MacroRules.Read(json.RootElement);
    }

    static string S(string text) => JsonSerializer.Serialize(text);

    static Template Keys()
    {
        string name = L.T("Press keys on repeat");
        var macro = MacroOf(name, 60, """
            [
              {"kind":"loop","times":0},
              {"kind":"key","key":"$o2"},
              {"kind":"wait","ms":"$o1"},
              {"kind":"endLoop"}
            ]
            """, $$"""
            [
              {"name":"o1","label":{{S(L.T("Every"))}},"kind":"number","value":1000,"default":1000,"min":100,"max":600000,"unit":"s"},
              {"name":"o2","label":{{S(L.T("Key"))}},"kind":"key","value":"E","default":"E"}
            ]
            """, "[]");
        return new Template("keys", name, L.T("Presses a key again and again in the game, as often as you set."), "keyboard", macro);
    }

    static Template Click()
    {
        string name = L.T("Click a button when it shows up");
        var macro = MacroOf(name, 120, """
            [
              {"kind":"loop","times":0},
              {"kind":"waitColour","colour":"$o1","tolerance":24,"region":"@a1","timeoutMs":600000},
              {"kind":"clickColour","colour":"$o1","tolerance":24,"region":"@a1","minSize":6,"button":"left","timeoutMs":2000},
              {"kind":"addVar","name":"clicks","value":1},
              {"kind":"wait","ms":"$o2"},
              {"kind":"endLoop"}
            ]
            """, $$"""
            [
              {"name":"o1","label":{{S(L.T("Button colour"))}},"kind":"colour","value":"#22c55e","default":"#22c55e"},
              {"name":"o2","label":{{S(L.T("Then wait"))}},"kind":"number","value":500,"default":500,"min":0,"max":60000,"unit":"s"}
            ]
            """, $$"""
            [ {"name":"a1","label":{{S(L.T("Button area"))}},"colour":"#22c55e","rect":{"x":0.25,"y":0.25,"w":0.5,"h":0.5} } ]
            """);
        Todo(macro.Steps[1], L.T("Move the button area on the game"), L.T("Pick the button's colour"));
        return new Template("click", name, L.T("Waits for a button of a colour to show up in an area, clicks it, and waits for the next one."), "pointer", macro);
    }

    static Template Cast()
    {
        string name = L.T("Hold to cast, then keep the bar on the target");
        var macro = MacroOf(name, 240, """
            [
              {"kind":"loop","times":0},
              {"kind":"click","x":0.5,"y":0.5,"button":"left","holdMs":"$o1"},
              {"kind":"follow","region":"@a1","target":"$o2","marker":"$o3","tolerance":24,
               "button":"left","mode":"smooth","kp":"$o4","kd":0.4,"predict":true,"fps":60,"untilGoneMs":1000,"timeoutMs":"$o5"},
              {"kind":"addVar","name":"catches","value":1},
              {"kind":"wait","ms":1500},
              {"kind":"endLoop"}
            ]
            """, $$"""
            [
              {"name":"o1","label":{{S(L.T("Hold to cast"))}},"kind":"number","value":800,"default":800,"min":0,"max":10000,"unit":"s"},
              {"name":"o2","label":{{S(L.T("Target colour"))}},"kind":"colour","value":"#22c55e","default":"#22c55e"},
              {"name":"o3","label":{{S(L.T("Marker colour"))}},"kind":"colour","value":"#ffffff","default":"#ffffff"},
              {"name":"o4","label":{{S(L.T("Strength"))}},"kind":"number","value":1,"default":1,"min":0,"max":10,"unit":""},
              {"name":"o5","label":{{S(L.T("Wait for the bar"))}},"kind":"number","value":60000,"default":60000,"min":1000,"max":600000,"unit":"s"}
            ]
            """, $$"""
            [ {"name":"a1","label":{{S(L.T("Bar area"))}},"colour":"#3b82f6","rect":{"x":0.3,"y":0.75,"w":0.4,"h":0.06} } ]
            """);
        Todo(macro.Steps[1], L.T("Drag dot 2 to where you cast"));
        Todo(macro.Steps[2], L.T("Move the bar area on the game"), L.T("Pick the target's colour"), L.T("Pick the marker's colour"));
        return new Template("cast", name, L.T("Holds the mouse to cast, waits for the bar, then keeps the marker on the target until the bar goes."), "bar", macro);
    }

    static Template Blank()
    {
        var macro = new Macro { Name = L.T("New macro"), Targets = "selected" };
        return new Template("blank", L.T("Blank"), L.T("Start from nothing and build your own."), "blank", macro);
    }
}
