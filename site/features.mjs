// Every feature of Bloxkeep in one list, in English and Vietnamese: the landing page's "Every feature" section and the
// SoftwareApplication featureList (pages.mjs). The READMEs link here instead of copying it: Google indexed one copy of
// each repeated text and dropped the repo's home page (2026-11-14). `node site/features.mjs en` still prints it as Markdown. Plain words people search for. Every line is something the guide (docs/guide) describes; keep
// them in step, and never name other projects.
export const FEATURES = {
  en: {
    title: 'Every feature',
    intro: 'Everything Bloxkeep does, in one list. The guide explains each one.',
    groups: [
      ['Accounts', [
        'Add Roblox accounts with the Roblox phone app (code or QR), a username and password (2-step verification and captchas work), the roblox.com sign-in page, a .ROBLOSECURITY cookie or another Bloxkeep file',
        'See every account at a glance: online, in a game on this PC or another device, why it disconnected, RAM, play time, needs sign-in, banned',
        'Appeal a ban: a banned account opens Roblox\'s Violations & Appeals page, already signed in',
        'Search by name, username, nickname, group, note, game or user ID; filters, groups, sorting, cards or a list',
        'Save a password and an authenticator key per account: copy the password or the current 2FA code, and the code is filled in at sign-in',
        'A daily sign-in check finds accounts Roblox signed out before you press Play',
        'roblox.com signed in as each account, in tabs side by side, without touching your own browser',
        'Mark your main account, and make your accounts Roblox friends with each other in one click',
        'Roblox VNG accounts too (being tested)',
      ]],
      ['Play', [
        'Pick a game by name, link or Place ID, with each game\'s page, recent games and saved servers',
        'Server browser: most or fewest players, lowest ping, best FPS, or room for all your accounts',
        'Join your private servers, a friend\'s server, any player\'s server or a Job ID',
        'Join this server: bring your other accounts into one account\'s server',
        'Switch an account to another server, or leave crowded servers by itself',
        'Launch order and a wait between accounts; Stop while they open',
        'Solve Roblox\'s bot check in Bloxkeep\'s own window when a join asks for one',
      ]],
      ['Multi-Roblox', [
        'Several Roblox windows at once, one per account, without injecting anything into Roblox',
        'Every new Roblox build is checked for changes that could stop it, even builds not out yet',
        'Open Roblox with Roblox, Bloxstrap, Fishstrap, Voidstrap, Roblox directly, the Microsoft Store Roblox (being tested) or your own launcher',
        'Roblox can always open windowed, so one fullscreen setting never mixes the windows up',
      ]],
      ['Presets and schedules', [
        'Save accounts, a game and a server, and start them in one click',
        'Server for a preset: any, most or fewest players, lowest ping, random, a fixed Job ID or a player\'s server',
        'Schedules on chosen weekdays or a date, stopping after some hours or at a time; missed runs are handled',
        'Per preset: anti-AFK, auto-rejoin, rejoin with another account, Light AFK, restart Roblox every few hours, arrange windows and its own Discord channel',
      ]],
      ['AFK farming and staying in game', [
        'Anti-AFK against idle kicks, also while you use another window',
        'Auto-rejoin after a disconnect, waiting a little longer each try',
        'Modes: AFK farming, Play and farm, or your own graphics, FPS limit and volume for chosen accounts; an FPS cap per account',
        'Light AFK and Fast Flags for lower graphics and less RAM',
        'RAM tools: low RAM warning, lower memory priority and a RAM limit for background or minimized windows, restart a window that grows too heavy',
        'Background Roblox windows run in Windows\' efficiency mode, so the one you play stays smooth',
        'Roblox updates: close, update and reopen in one click, or update by itself while no window is open',
      ]],
      ['Roblox windows', [
        'Arrange windows in a grid picked from pictures, or a size of your own; Ctrl+Alt+A from any app',
        'Game monitor: live pictures of every window, with minimize all, open all and close all',
        'A mini window that stays on top',
        'Hide Roblox windows while their games keep running; hotkeys to hide, minimize, switch and open Bloxkeep',
        'The account name and process ID in each window title, the account\'s picture as the window icon, and a taskbar button per account or per preset',
      ]],
      ['Macros (Beta)', [
        'Steps written as plain sentences: press keys, click, wait for a picture, a colour or text, repeat, If',
        'Record a macro or start from a template, then move its points and areas right on the game',
        'Read numbers and text from the game, a file or a web page; counters on a small panel beside the game',
        'Recovery steps when a macro gets stuck; Ctrl+Alt+1 to 9 runs a macro, Ctrl+Alt+0 stops them all',
      ]],
      ['Stats, alerts and Discord', [
        'Statistics: today on a 24-hour clock, play time by day and hour, why accounts dropped and the time it cost, top games, each account and preset',
        'An activity log you can filter and export as CSV',
        'Discord alerts, a live status message and play-time summaries in your own channel, with pings and quiet hours',
        '/pc in Discord: see your accounts, take a screenshot of their Roblox windows, run a preset, close windows or switch server, from your phone',
        '"Playing Bloxkeep" on your Discord profile, without account names',
        'Windows notifications, and alerts on your phone through ntfy',
      ]],
      ['Games', [
        'A Games tab with popular games\' wikis inside Bloxkeep: search, item tables, compare, recent buffs and nerfs',
        'Live news from the games\' own alerts, such as stock and spawns, with notifications for the words you pick',
        'Blox Fruits values',
      ]],
      ['Safe and yours', [
        'Accounts encrypted with AES-256 and your master password, kept only on your PC; no server of its own',
        'Lock by hand or after a while away; several vaults with a picker at start',
        'Backups to a OneDrive or Google Drive folder, restore on a new PC, import from another Bloxkeep file',
        'Hide account names for screenshots and streams',
        'Nearly 100 themes, some moving, or your own colours, picture or video; themes shared as files',
        'English and Vietnamese, Ctrl+K quick search, a tray menu, start with Windows',
        'Free and open source (MIT), updates itself; installer, offline installer or portable ZIP for Windows 10 and 11',
      ]],
    ],
  },
  vi: {
    title: 'Mọi tính năng',
    intro: 'Mọi thứ Bloxkeep làm được, gom vào một danh sách. Hướng dẫn giải thích từng cái.',
    groups: [
      ['Tài khoản', [
        'Thêm tài khoản Roblox bằng app Roblox trên điện thoại (mã hoặc QR), tên đăng nhập và mật khẩu (có xác minh 2 bước và captcha), trang đăng nhập roblox.com, cookie .ROBLOSECURITY hoặc file Bloxkeep khác',
        'Xem nhanh mọi tài khoản: online, đang trong game ở máy này hay máy khác, vì sao bị văng, RAM, giờ chơi, cần đăng nhập lại, bị ban',
        'Kháng cáo ban: tài khoản bị ban mở trang kháng cáo của Roblox, đã đăng nhập sẵn',
        'Tìm theo tên, username, biệt danh, nhóm, ghi chú, game hay user ID; bộ lọc, nhóm, sắp xếp, dạng thẻ hoặc danh sách',
        'Lưu mật khẩu và khóa 2FA cho từng tài khoản: chép mật khẩu, chép mã 2FA, mã tự điền khi đăng nhập',
        'Kiểm tra đăng nhập mỗi ngày, tìm tài khoản bị Roblox đăng xuất trước khi bấm Vào game',
        'Mở roblox.com đã đăng nhập từng tài khoản, nhiều tab cạnh nhau, không đụng tới trình duyệt của bạn',
        'Đánh dấu tài khoản chính, kết bạn Roblox giữa các tài khoản bằng một lần bấm',
        'Có cả tài khoản Roblox VNG (đang thử)',
      ]],
      ['Vào game', [
        'Chọn game theo tên, link hoặc Place ID, có trang của từng game, game gần đây và server đã lưu',
        'Duyệt server: đông nhất, vắng nhất, ping thấp nhất, FPS tốt nhất, hoặc đủ chỗ cho mọi tài khoản',
        'Vào server riêng của bạn, server của bạn bè, server của bất kỳ người chơi nào hoặc theo Job ID',
        'Vào server này: đưa các tài khoản khác vào cùng server với một tài khoản',
        'Đổi tài khoản sang server khác, hoặc tự rời server quá đông',
        'Thứ tự mở và thời gian chờ giữa các tài khoản; bấm Dừng khi đang mở',
        'Giải bot check của Roblox ngay trong cửa sổ của Bloxkeep khi vào game bị hỏi',
      ]],
      ['Multi-Roblox', [
        'Mở nhiều cửa sổ Roblox cùng lúc, mỗi tài khoản một cửa sổ, không chèn gì vào Roblox',
        'Kiểm tra mọi bản Roblox mới xem có thay đổi làm hỏng Multi-Roblox không, cả bản chưa phát hành',
        'Mở Roblox bằng Roblox, Bloxstrap, Fishstrap, Voidstrap, mở thẳng Roblox, Roblox của Microsoft Store (đang thử) hoặc launcher của bạn',
        'Có thể luôn mở Roblox dạng cửa sổ, để chế độ toàn màn hình không làm lẫn các cửa sổ',
      ]],
      ['Preset và hẹn giờ', [
        'Lưu tài khoản, game và server, mở bằng một lần bấm',
        'Server cho preset: bất kỳ, đông nhất, vắng nhất, ping thấp nhất, ngẫu nhiên, Job ID cố định hoặc server của một người chơi',
        'Hẹn giờ theo thứ trong tuần hoặc một ngày, dừng sau vài giờ hoặc vào một giờ; lần lỡ hẹn cũng được xử lý',
        'Riêng từng preset: chống AFK, tự vào lại, vào lại cùng tài khoản khác, Light AFK, mở lại Roblox sau vài giờ, xếp cửa sổ và kênh Discord riêng',
      ]],
      ['Treo game (AFK farm)', [
        'Chống AFK để không bị kick vì đứng yên, kể cả khi bạn dùng cửa sổ khác',
        'Tự vào lại sau khi bị văng, mỗi lần chờ lâu hơn một chút',
        'Chế độ: Treo game, Vừa chơi vừa treo, hoặc tự chỉnh đồ họa, giới hạn FPS và âm lượng cho tài khoản chọn; giới hạn FPS riêng từng tài khoản',
        'Light AFK và Fast Flags để giảm đồ họa, bớt RAM',
        'Công cụ RAM: cảnh báo thiếu RAM, giảm ưu tiên bộ nhớ và giới hạn RAM cho cửa sổ chạy nền hay thu nhỏ, mở lại cửa sổ quá nặng',
        'Cửa sổ Roblox chạy nền dùng chế độ tiết kiệm của Windows, cửa sổ bạn đang chơi luôn mượt',
        'Cập nhật Roblox: đóng, cập nhật và mở lại bằng một lần bấm, hoặc tự cập nhật khi không có cửa sổ nào mở',
      ]],
      ['Cửa sổ Roblox', [
        'Xếp cửa sổ theo lưới chọn bằng hình, hoặc theo kích thước của bạn; Ctrl+Alt+A từ bất kỳ app nào',
        'Theo dõi game: hình trực tiếp của mọi cửa sổ, có thu nhỏ tất cả, mở lại tất cả và đóng tất cả',
        'Cửa sổ mini luôn nằm trên cùng',
        'Ẩn cửa sổ Roblox mà game vẫn chạy; phím tắt để ẩn, thu nhỏ, chuyển cửa sổ và mở Bloxkeep',
        'Tên tài khoản và PID trên tiêu đề cửa sổ, ảnh tài khoản làm biểu tượng cửa sổ, nút taskbar riêng cho từng tài khoản hoặc preset',
      ]],
      ['Macro (Beta)', [
        'Các bước viết như câu thường: bấm phím, click, chờ một hình, màu hoặc chữ, lặp lại, Nếu',
        'Ghi lại macro hoặc bắt đầu từ mẫu, rồi kéo điểm và vùng ngay trên game',
        'Đọc số và chữ từ game, file hoặc trang web; bộ đếm trên bảng nhỏ cạnh game',
        'Bước gỡ kẹt khi macro bị kẹt; Ctrl+Alt+1 đến 9 chạy macro, Ctrl+Alt+0 dừng tất cả',
      ]],
      ['Thống kê, thông báo và Discord', [
        'Thống kê: hôm nay trên đồng hồ 24 giờ, giờ chơi theo ngày và theo giờ, vì sao bị văng và mất bao lâu, game chơi nhiều, từng tài khoản và preset',
        'Nhật ký hoạt động lọc được và xuất ra CSV',
        'Cảnh báo, tin nhắn trạng thái trực tiếp và tổng kết giờ chơi trong kênh Discord của bạn, có ping và giờ yên lặng',
        '/pc trong Discord: xem tài khoản, chụp ảnh cửa sổ Roblox của chúng, chạy preset, đóng cửa sổ hay đổi server, từ điện thoại',
        '"Đang chơi Bloxkeep" trên hồ sơ Discord của bạn, không lộ tên tài khoản',
        'Thông báo của Windows, và cảnh báo trên điện thoại qua ntfy',
      ]],
      ['Game', [
        'Tab Game có wiki của các game nổi tiếng ngay trong Bloxkeep: tìm kiếm, bảng vật phẩm, so sánh, buff và nerf mới',
        'Tin trực tiếp từ kênh báo của chính game, như stock và spawn, báo cho bạn khi có từ bạn chọn',
        'Bảng giá Blox Fruits',
      ]],
      ['An toàn và của riêng bạn', [
        'Tài khoản mã hóa AES-256 bằng mật khẩu chính, chỉ nằm trên máy bạn; Bloxkeep không có server riêng',
        'Khóa bằng tay hoặc tự khóa khi bạn rời máy; nhiều kho với màn chọn lúc mở',
        'Sao lưu vào thư mục OneDrive hoặc Google Drive, khôi phục trên máy mới, nhập từ file Bloxkeep khác',
        'Ẩn tên tài khoản khi chụp màn hình hay livestream',
        'Gần 100 giao diện, có cái chuyển động, hoặc màu, ảnh, video của riêng bạn; chia sẻ giao diện bằng file',
        'Tiếng Anh và tiếng Việt, tìm nhanh bằng Ctrl+K, menu ở khay hệ thống, mở cùng Windows',
        'Miễn phí, mã nguồn mở (MIT), tự cập nhật; bản cài, bản cài offline hoặc bản portable cho Windows 10 và 11',
      ]],
    ],
  },
};

/** `node site/features.mjs en|vi`: the list as Markdown for the README. */
if (process.argv[1]?.endsWith('features.mjs')) {
  const f = FEATURES[process.argv[2] ?? 'en'];
  console.log(`### ${f.title}\n\n${f.groups.map(([g, items]) => `**${g}**\n\n${items.map(x => `- ${x}`).join('\n')}`).join('\n\n')}`);
}
