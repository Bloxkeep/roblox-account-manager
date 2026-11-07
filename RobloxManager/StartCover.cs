using System.Diagnostics;
using System.Drawing.Drawing2D;

namespace RobloxManager;

/// <summary>
/// The window's start logo while the page loads (ShellForm.ShowStartCover): still while the window opens (it grows in with
/// it, WindowOpening), then it hops in place, squashing as it lands, until the page is ready (owner, 2026-11-08: the moment before the password card should move too). Drawn here
/// because nothing else can be yet: the browser is still starting. It comes off only between hops (<see cref="AtRestAsync"/>),
/// where it sits as the page's own logo starts (app.js lockIntro), so one logo moves all the way, without a jump.
/// </summary>
sealed class StartCover : Control
{
    const double HopMs = 760;

    /// <summary>One hop: (time 0–1, lift in logical px, width scale, height scale), standing on its bottom edge. The last
    /// stretch is still: the moments it may come off.</summary>
    static readonly (double T, double Lift, double X, double Y)[] Hop =
    [
        (0, 0, 1, 1), (.12, 0, 1.1, .88), (.4, 18, .95, 1.06), (.62, 0, 1.08, .92), (.76, 4, .99, 1.01), (.88, 0, 1, 1), (1, 0, 1, 1),
    ];
    const double RestFrom = .88;

    readonly Bitmap _logo;
    readonly float _unit;
    readonly Stopwatch _clock = new(); // runs from Start
    readonly System.Windows.Forms.Timer _timer = new() { Interval = 15 };
    TaskCompletionSource? _rest;

    /// <param name="logo">The logo at its size on screen (72 logical px); disposed with the cover.</param>
    /// <param name="unit">Device pixels per logical pixel.</param>
    public StartCover(Bitmap logo, float unit, Color back)
    {
        SetStyle(ControlStyles.AllPaintingInWmPaint | ControlStyles.UserPaint | ControlStyles.OptimizedDoubleBuffer | ControlStyles.ResizeRedraw, true);
        _logo = logo;
        _unit = unit;
        BackColor = back;
        Dock = DockStyle.Fill;
        _timer.Tick += (_, _) =>
        {
            if (_clock.IsRunning)
                Invalidate();
            if (_rest is { } rest && AtRest())
            {
                _rest = null;
                rest.TrySetResult();
            }
        };
        _timer.Start();
    }

    /// <summary>The window is open: the hops begin.</summary>
    public void Start() => _clock.Start();

    /// <summary>Done once the logo stands still in the middle, as the page's logo starts: between hops, a hop at most.</summary>
    public Task AtRestAsync()
    {
        if (AtRest())
            return Task.CompletedTask;
        _rest ??= new TaskCompletionSource();
        return _rest.Task;
    }

    bool AtRest() => !_clock.IsRunning || _clock.Elapsed.TotalMilliseconds % HopMs / HopMs >= RestFrom;

    /// <summary>Lift and scale at <paramref name="ms"/> since the cover went up.</summary>
    static (double Lift, double X, double Y) Pose(double ms)
    {
        double p = ms % HopMs / HopMs;
        for (int i = 1; i < Hop.Length; i++)
        {
            if (p > Hop[i].T)
                continue;
            var (a, b) = (Hop[i - 1], Hop[i]);
            double u = (p - a.T) / (b.T - a.T);
            u = u * u * (3 - 2 * u); // eased at both ends
            return (a.Lift + (b.Lift - a.Lift) * u, a.X + (b.X - a.X) * u, a.Y + (b.Y - a.Y) * u);
        }
        return (0, 1, 1);
    }

    protected override void OnPaint(PaintEventArgs e) => DrawLogo(e.Graphics, ClientRectangle);

    /// <summary>The logo as it is now, in the middle of <paramref name="area"/> (also the window's stand-in, WindowOpening).</summary>
    public void DrawLogo(Graphics g, Rectangle area)
    {
        var (lift, sx, sy) = Pose(_clock.Elapsed.TotalMilliseconds);
        float w = (float)(_logo.Width * sx), h = (float)(_logo.Height * sy);
        // Centred as the still logo was, standing on its bottom edge: a squash stays on the ground.
        float bottom = area.Top + (area.Height + _logo.Height) / 2f - (float)(lift * _unit);
        g.InterpolationMode = InterpolationMode.HighQualityBicubic;
        g.PixelOffsetMode = PixelOffsetMode.HighQuality;
        g.DrawImage(_logo, area.Left + (area.Width - w) / 2f, bottom - h, w, h);
    }

    protected override void Dispose(bool disposing)
    {
        if (disposing)
        {
            _timer.Dispose();
            _logo.Dispose();
            _rest?.TrySetResult();
        }
        base.Dispose(disposing);
    }
}
