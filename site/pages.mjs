// The website's own words (build.mjs makes the pages): the landing page and the frame around the guide, in English and
// Vietnamese. Every claim here is one the README and the guide make; keep them in step.
export const SITE = 'https://bloxkeep.github.io/roblox-account-manager/';
export const REPO = 'https://github.com/Bloxkeep/roblox-account-manager';
// Google gives a name to a whole host, never to a folder like /roblox-account-manager/, and reads it from the WebSite
// data on the host's own home page (https://developers.google.com/search/docs/appearance/site-names). That page is the
// repo Bloxkeep/bloxkeep.github.io; the landing pages carry the same WebSite node so the two never disagree.
const HOST = 'https://bloxkeep.github.io/';
const WEBSITE = { '@type': 'WebSite', '@id': `${HOST}#website`, name: 'Bloxkeep', alternateName: ['bloxkeep.github.io'], url: HOST };
export const DOWNLOAD = `${REPO}/releases/latest/download/Bloxkeep-Setup.exe`;
const PORTABLE = `${REPO}/releases/latest/download/Bloxkeep-Portable.zip`;
/** Bloxkeep's community server: the bot sends this address on to an invite that never expires (worker.js, /discord). */
export const DISCORD = 'https://bloxkeep-bot.bloxkeep-app.workers.dev/discord';
/** Discord's mark (its own logo, as Discord asks it to be used for a link to a server), 24 by 24. */
export const DISCORD_MARK = 'M20.317 4.3698a19.7913 19.7913 0 00-4.8851-1.5152.0741.0741 0 00-.0785.0371c-.211.3753-.4447.8648-.6083 1.2495-1.8447-.2762-3.68-.2762-5.4868 0-.1636-.3933-.4058-.8742-.6177-1.2495a.077.077 0 00-.0785-.037 19.7363 19.7363 0 00-4.8852 1.515.0699.0699 0 00-.0321.0277C.5334 9.0458-.319 13.5799.0992 18.0578a.0824.0824 0 00.0312.0561c2.0528 1.5076 4.0413 2.4228 5.9929 3.0294a.0777.0777 0 00.0842-.0276c.4616-.6304.8731-1.2952 1.226-1.9942a.076.076 0 00-.0416-.1057c-.6528-.2476-1.2743-.5495-1.8722-.8923a.077.077 0 01-.0076-.1277c.1258-.0943.2517-.1923.3718-.2914a.0743.0743 0 01.0776-.0105c3.9278 1.7933 8.18 1.7933 12.0614 0a.0739.0739 0 01.0785.0095c.1202.099.246.1981.3728.2924a.077.077 0 01-.0066.1276 12.2986 12.2986 0 01-1.873.8914.0766.0766 0 00-.0407.1067c.3604.698.7719 1.3628 1.225 1.9932a.076.076 0 00.0842.0286c1.961-.6067 3.9495-1.5219 6.0023-3.0294a.077.077 0 00.0313-.0552c.5004-5.177-.8382-9.6739-3.5485-13.6604a.061.061 0 00-.0312-.0286zM8.02 15.3312c-1.1825 0-2.1569-1.0857-2.1569-2.419 0-1.3332.9555-2.4189 2.157-2.4189 1.2108 0 2.1757 1.0952 2.1568 2.419 0 1.3332-.9555 2.4189-2.1569 2.4189zm7.9748 0c-1.1825 0-2.1569-1.0857-2.1569-2.419 0-1.3332.9554-2.4189 2.1569-2.4189 1.2108 0 2.1757 1.0952 2.1568 2.419 0 1.3332-.946 2.4189-2.1568 2.4189Z';

/** The guide's topic pages (docs/guide/<topic>.md and .vi.md), in the guide's own order. */
export const TOPICS = ['run-multiple-accounts', 'getting-started', 'accounts', 'playing', 'multi-roblox', 'presets', 'staying-in-game',
  'windows-and-stats', 'macros', 'discord', 'settings', 'backup-and-security', 'troubleshooting'];

export const T = {
  en: {
    nav: { guide: 'Guide', news: "What's new", demo: 'Try it', discord: 'Discord', download: 'Download', other: 'Tiếng Việt' },
    demo: {
      title: 'Try Bloxkeep in your browser',
      description: "Bloxkeep's real interface on made-up accounts: click around before you download. Nothing is installed.",
      bar: 'A demo with made-up accounts. Nothing is installed and nothing reaches Roblox.',
      download: 'Download Bloxkeep',
      small: 'Looks best on a computer.',
      frame: 'Bloxkeep demo',
    },
    guideTitle: 'Bloxkeep guide',
    allTopics: 'All topics',
    alerts: { note: 'Note', tip: 'Tip', important: 'Important', warning: 'Warning', caution: 'Caution' },
    topics: {
      'run-multiple-accounts': 'Run multiple accounts', 'getting-started': 'Getting started', accounts: 'Accounts', playing: 'Playing', 'multi-roblox': 'Multi-Roblox',
      presets: 'Presets and schedules', 'staying-in-game': 'Staying in game', 'windows-and-stats': 'Windows and statistics', macros: 'Macros',
      discord: 'Discord', settings: 'Settings', 'backup-and-security': 'Backup and security', troubleshooting: 'Troubleshooting',
    },
    foot: {
      disclaimer: 'Bloxkeep is an independent project. It is not affiliated with, endorsed or sponsored by Roblox Corporation. Roblox is a trademark of Roblox Corporation.',
      releases: 'All releases', privacy: 'Privacy', signing: 'Code signing policy', license: 'MIT license',
      tagline: 'The free Roblox account manager for Windows. Your accounts stay encrypted on your PC.',
      get: 'Get it', learn: 'Learn', community: 'Community', download: 'Download', start: 'Getting started', help: 'Troubleshooting',
      issue: 'Report a problem', source: 'Source code',
    },
    news: {
      title: "What's new in Bloxkeep",
      intro: 'What each version brought, newest first. Bloxkeep updates itself, so these arrive on their own.',
      kinds: { new: 'New', improved: 'Better', fixed: 'Fixed', removed: 'Removed', en: 'Changes' },
      date: d => new Date(d + 'T12:00:00Z').toLocaleDateString('en-US', { year: 'numeric', month: 'long', day: 'numeric', timeZone: 'UTC' }),
      jump: 'Versions',
    },
  },
  vi: {
    nav: { guide: 'Hướng dẫn', news: 'Có gì mới', demo: 'Dùng thử', discord: 'Discord', download: 'Tải về', other: 'English' },
    demo: {
      title: 'Dùng thử Bloxkeep trên trình duyệt',
      description: 'Giao diện thật của Bloxkeep với tài khoản giả: bấm thử trước khi tải. Không cần cài gì.',
      bar: 'Bản thử với tài khoản giả. Không cài gì, không kết nối Roblox.',
      download: 'Tải Bloxkeep',
      small: 'Xem trên máy tính sẽ rõ hơn.',
      frame: 'Bản thử Bloxkeep',
    },
    guideTitle: 'Hướng dẫn Bloxkeep',
    allTopics: 'Các chủ đề',
    alerts: { note: 'Lưu ý', tip: 'Mẹo', important: 'Quan trọng', warning: 'Cảnh báo', caution: 'Cẩn thận' },
    topics: {
      'run-multiple-accounts': 'Chạy nhiều tài khoản', 'getting-started': 'Bắt đầu', accounts: 'Tài khoản', playing: 'Vào game', 'multi-roblox': 'Multi-Roblox',
      presets: 'Preset và hẹn giờ', 'staying-in-game': 'Treo game', 'windows-and-stats': 'Cửa sổ và thống kê', macros: 'Macro',
      discord: 'Discord', settings: 'Cài đặt', 'backup-and-security': 'Sao lưu và bảo mật', troubleshooting: 'Gỡ rối',
    },
    foot: {
      disclaimer: 'Bloxkeep là dự án độc lập, không liên kết, không được Roblox Corporation bảo trợ hay tài trợ. Roblox là thương hiệu của Roblox Corporation.',
      releases: 'Mọi phiên bản', privacy: 'Quyền riêng tư', signing: 'Chính sách ký số', license: 'Giấy phép MIT',
      tagline: 'App quản lý tài khoản Roblox miễn phí cho Windows. Tài khoản được mã hóa, chỉ nằm trên máy bạn.',
      get: 'Tải về', learn: 'Tìm hiểu', community: 'Cộng đồng', download: 'Tải Bloxkeep', start: 'Bắt đầu', help: 'Gỡ rối',
      issue: 'Báo lỗi', source: 'Mã nguồn',
    },
    news: {
      title: 'Bloxkeep có gì mới',
      intro: 'Mỗi phiên bản thêm gì, mới nhất ở trên. Bloxkeep tự cập nhật nên bạn không cần tải lại.',
      kinds: { new: 'Mới', improved: 'Tốt hơn', fixed: 'Đã sửa', removed: 'Đã bỏ', vi: 'Thay đổi' },
      date: d => d.split('-').reverse().map(Number).join('/'),
      jump: 'Phiên bản',
    },
  },
};

const L = {
  en: {
    title: 'Bloxkeep: run several Roblox accounts at once on Windows',
    description: 'Free, open-source Roblox account manager for Windows. Play several accounts at once with Multi-Roblox, auto-rejoin, anti-AFK, presets on a schedule and Discord alerts. Your accounts stay encrypted on your PC.',
    h1: 'Run several Roblox accounts at once on Windows',
    lead: 'Every Roblox account in one place. Pick a game, pick accounts, press Play, and each one opens in its own window.',
    download: 'Download Bloxkeep',
    portable: 'Portable ZIP',
    tryLink: 'Or try it in your browser first',
    all: 'All downloads',
    meta: v => `Free and open source · Version ${v} · Windows 10 or 11, 64-bit`,
    heroAlt: "Bloxkeep's main window: account cards, the game bar at the bottom, Multi-Roblox on in the sidebar",
    farmAlt: 'Four Roblox windows open at once on one PC, titled Roblox - Account 7, 4, 2 and 1, all in the same game',
    farmTag: 'Your Roblox windows',
    appTag: 'Bloxkeep, beside them',
    proofTitle: 'See it working',
    proofText: 'A real screenshot from one Windows PC: six Roblox accounts open at once with Bloxkeep, all in the same game, tiled side by side with one click.',
    proofAlt: 'Six Roblox windows open at once on one PC, tiled three by two, titled Roblox - Account 1 to Roblox - Account 7, all in the same game',
    proofCaption: 'Account names are hidden. Roblox 0.745, November 2026.',
    proofLink: 'How to turn on Multi-Roblox',
    featuresTitle: 'What it does',
    more: 'How it works',
    features: [
      ['multi-roblox', 'multi-roblox-real-detail.jpg', 'Several Roblox windows at once', 'Turn on Multi-Roblox and every account plays in its own window, opened windowed and tiled the way you choose. Each new Roblox build is checked.'],
      ['accounts', 'accounts-list.png', 'All your accounts in one place', 'Add accounts with a code from the Roblox phone app, a password (2-step verification works), the Roblox page or a cookie. See who is online, in a game or needs to sign in.'],
      ['playing', 'demo-servers.webp', 'Pick a game, pick a server', 'Find a game by name or link. Choose the server with the most players, the fewest, the lowest ping or room for all your accounts, or join a friend.'],
      ['presets', 'demo-presets.webp', 'Presets and schedules', 'Save accounts, a game and a server and start them in one click, or on a schedule that also stops them, with timed restarts.'],
      ['staying-in-game', 'demo-rejoin.webp', 'Stay in game', 'Anti-AFK keeps windows from being kicked for idling. Auto-rejoin goes back in after a disconnect, waiting a little longer each try instead of giving up.'],
      ['windows-and-stats#game-monitor', 'game-monitor-real.jpg', 'Game monitor', "Live pictures of every Roblox window, a mini window, and a warning before RAM or Windows' virtual memory runs out."],
      ['windows-and-stats#statistics', 'stats.png', 'Statistics', 'A timeline of what each account played, why windows dropped and how much time it cost, and play time by account and game.'],
      ['discord#control-from-discord', 'discord-control.png', 'Control from your phone', 'Type /pc in Discord to see your accounts, run a preset, close windows or switch server, from anywhere.'],
      ['discord', 'discord-alert.png', 'Discord alerts', 'Alerts, a live status message and play-time summaries in your own Discord channel.'],
      ['windows-and-stats#arrange-windows', 'arrange-layouts.png', 'Arrange windows', "Pick a layout from pictures, like Windows' snap layouts, or a size of your own. Ctrl+Alt+A arranges them from any app."],
      ['settings', 'roblox-updates.png', 'Roblox your way', 'Graphics, FPS and volume for each account. Open Roblox with Bloxstrap, Fishstrap, Voidstrap or your own launcher.'],
      ['troubleshooting#keyboard-shortcuts', 'palette.png', 'Find anything with Ctrl+K', 'Type a few letters to find an account, a game, a setting or an action.'],
      ['settings#appearance', 'theme-photos.jpg', 'Themes with real photos', 'Nearly 100 themes, some of them real photos that move, or a picture or video of your own behind the page.'],
      ['backup-and-security', 'lock.png', 'Locked with your password', 'Accounts are encrypted with your master password and stay on your PC. Back them up to a OneDrive or Google Drive folder and restore them on a new PC.'],
      ['getting-started#updates', 'lock-update.png', 'Updates itself', "New versions install by themselves while you aren't using Bloxkeep. Your Roblox windows keep running."],
    ],
    startTitle: 'Start in three steps',
    start: [
      ['Set a master password', "It unlocks Bloxkeep and keeps your accounts encrypted. It can't be recovered, so pick one you'll remember."],
      ['Add your accounts', 'Sign in with your phone (recommended), a username and password, or a browser window.'],
      ['Pick a game and press Play', 'Type a game in the bar at the bottom, click accounts to choose them, press Play.'],
    ],
    startLink: 'Getting started, step by step',
    demoAlt: 'Choosing two accounts, switching the game, looking at the server list, then changing the theme',
    safeTitle: 'Your accounts stay yours',
    safe: [
      ['On your PC only', 'Bloxkeep has no server of its own. Your accounts are kept in a file on your PC, encrypted with your master password.'],
      ['Open source', 'Every line is public on GitHub, and the downloads are built by GitHub from that source, so anyone can check them.'],
      ['Nothing injected', 'Multi-Roblox injects nothing into Roblox: it only keeps Roblox from closing the other windows.'],
    ],
    caution: 'Only download Bloxkeep from its GitHub Releases page or this site. A copy from anywhere else may have been changed to steal accounts.',
    getTitle: 'Download',
    getText: [
      'A small installer: it downloads the newest Bloxkeep (about 60 MB) as it installs.',
      'Any folder on any drive, no admin rights. It installs Microsoft Edge WebView2 if your PC lacks it.',
      'Bloxkeep then updates itself. To get new features earlier, turn on Get beta versions in Settings › Bloxkeep updates.',
    ],
    smartscreen: "The files aren't code-signed yet, so Windows SmartScreen may warn the first time: click More info, then Run anyway.",
    faqTitle: 'Questions',
    faq: [
      ['Is Bloxkeep free?', 'Yes. It is free and open source under the MIT license, with no account or subscription.'],
      ['Does it need Microsoft Edge or a particular browser?', 'No. It uses WebView2, a part of Windows, whatever browser you use; Setup installs it if it is missing.'],
      ['Does Multi-Roblox really work?', 'Yes: the screenshot above is real, six accounts at once on one PC. Roblox can change how it blocks several windows in any update, so Bloxkeep checks every new Roblox build and tells you when one may stop it.'],
      ['Is running several Roblox windows allowed?', "Roblox doesn't officially support running several clients, so use Multi-Roblox at your own risk. It is off until you turn it on."],
      ['Will my account get banned?', "No one can promise that. Roblox doesn't officially support several clients, and some games forbid alt accounts. Bloxkeep signs in and opens games the way roblox.com does and never touches the game's memory."],
      ['Are macros cheating?', "A macro only presses keys and clicks in the Roblox window and looks at its picture, like a person would. It never reads or changes the game's memory. Some games still forbid macros: check the game's rules."],
      ['I forgot my master password.', "It can't be recovered. Restore a backup whose password you remember, or start over."],
      ['Is there a Mac or Linux version?', 'No, Windows 10 and 11 only.'],
      ['Where do I get help?', "Ask in #help on Bloxkeep's Discord, read the guide, use Send feedback in the app (Settings › Bloxkeep updates), or open an issue on GitHub."],
    ],
    guideLink: 'Read the user guide',
    newsTitle: v => `New in ${v}`,
    newsLink: 'Everything new, version by version',
    discord: {
      title: 'Join the Bloxkeep Discord',
      text: 'Ask for help, share an idea, and hear about Roblox updates and new Bloxkeep versions as they happen.',
      button: 'Join the server',
      note: 'Free. A quick check when you join keeps scam bots out. English everywhere, Vietnamese in #general-vn.',
      server: 'Bloxkeep',
      groups: [
        ['Info', [['announcements', 'New Bloxkeep versions and news'], ['roblox-updates', 'Roblox builds coming and live, and outages']]],
        ['Support', [['help', 'One question per post: what you did, what happened, your version'], ['bug-reports', 'Something broke? The steps and what happened'], ['ideas', 'One idea per post. React to the ones you want']]],
        ['Community', [['general', 'Anything about Bloxkeep and Roblox'], ['general-vn', 'Trò chuyện bằng tiếng Việt']]],
      ],
      command: ['/multi', 'Does Multi-Roblox work on the current Roblox, and the one coming next?'],
      commandWho: 'Ask the bot',
    },
  },
  vi: {
    title: 'Bloxkeep: chơi nhiều tài khoản Roblox cùng lúc trên Windows',
    description: 'App quản lý tài khoản Roblox miễn phí, mã nguồn mở cho Windows. Chơi nhiều tài khoản cùng lúc với Multi-Roblox, tự vào lại game, chống AFK, Preset hẹn giờ và báo về Discord. Tài khoản được mã hóa, chỉ nằm trên máy bạn.',
    h1: 'Chơi nhiều tài khoản Roblox cùng lúc trên Windows',
    lead: 'Mọi tài khoản Roblox ở một chỗ. Chọn game, chọn tài khoản, bấm Vào game, mỗi tài khoản mở một cửa sổ riêng.',
    download: 'Tải Bloxkeep',
    portable: 'Bản portable (ZIP)',
    tryLink: 'Hoặc dùng thử trên trình duyệt trước',
    all: 'Tất cả bản tải',
    meta: v => `Miễn phí, mã nguồn mở · Phiên bản ${v} · Windows 10 hoặc 11, 64-bit`,
    heroAlt: 'Cửa sổ chính của Bloxkeep: các thẻ tài khoản, thanh chọn game ở dưới, Multi-Roblox đang bật ở thanh bên',
    farmAlt: 'Bốn cửa sổ Roblox mở cùng lúc trên một máy, tiêu đề Roblox - Account 7, 4, 2 và 1, cùng ở trong một game',
    farmTag: 'Các cửa sổ Roblox',
    appTag: 'Bloxkeep ở bên cạnh',
    proofTitle: 'Chạy thật trên một máy',
    proofText: 'Ảnh chụp thật trên một máy Windows: sáu tài khoản Roblox mở cùng lúc bằng Bloxkeep, cùng ở trong một game, xếp cạnh nhau chỉ bằng một cú bấm.',
    proofAlt: 'Sáu cửa sổ Roblox mở cùng lúc trên một máy, xếp thành lưới ba nhân hai, tiêu đề từ Roblox - Account 1 đến Roblox - Account 7, cùng ở trong một game',
    proofCaption: 'Tên tài khoản đã được ẩn. Roblox 0.745, tháng 11/2026.',
    proofLink: 'Cách bật Multi-Roblox',
    featuresTitle: 'Bloxkeep làm được gì',
    more: 'Xem cách dùng',
    features: [
      ['multi-roblox', 'multi-roblox-real-detail.jpg', 'Nhiều cửa sổ Roblox cùng lúc', 'Bật Multi-Roblox là mỗi tài khoản chơi trong một cửa sổ riêng, mở ở chế độ cửa sổ và xếp theo ý bạn. Mỗi bản Roblox mới đều được kiểm tra.'],
      ['accounts', 'accounts-list.png', 'Mọi tài khoản ở một chỗ', 'Thêm tài khoản bằng mã từ app Roblox trên điện thoại, bằng mật khẩu (có xác minh 2 bước), qua trang Roblox hoặc bằng cookie. Xem ngay ai đang online, đang trong game hay cần đăng nhập lại.'],
      ['playing', 'demo-servers.webp', 'Chọn game, chọn server', 'Tìm game theo tên hoặc link. Chọn server đông nhất, vắng nhất, ping thấp nhất hay đủ chỗ cho mọi tài khoản, hoặc vào cùng bạn bè.'],
      ['presets', 'demo-presets.webp', 'Preset và hẹn giờ', 'Lưu sẵn tài khoản, game và server rồi mở bằng một cú bấm, hoặc theo lịch tự mở tự tắt, kèm khởi động lại định kỳ.'],
      ['staying-in-game', 'demo-rejoin.webp', 'Treo game yên tâm', 'Chống AFK giữ cửa sổ khỏi bị kick vì đứng yên. Tự vào lại game sau khi bị văng, mỗi lần chờ lâu hơn một chút chứ không bỏ cuộc.'],
      ['windows-and-stats#theo-dõi-game', 'game-monitor-real.jpg', 'Theo dõi game', 'Hình trực tiếp của mọi cửa sổ Roblox, cửa sổ nhỏ, và cảnh báo trước khi hết RAM hoặc bộ nhớ ảo của Windows.'],
      ['windows-and-stats#thống-kê', 'stats.png', 'Thống kê', 'Dòng thời gian từng tài khoản chơi gì, vì sao rớt và mất bao lâu, cùng giờ chơi theo tài khoản và game.'],
      ['discord#điều-khiển-từ-discord', 'discord-control.png', 'Điều khiển từ điện thoại', 'Gõ /pc trong Discord để xem tài khoản, chạy preset, đóng cửa sổ hay đổi server, ở đâu cũng được.'],
      ['discord', 'discord-alert.png', 'Báo về Discord', 'Cảnh báo, tin trạng thái cập nhật liên tục và tổng kết giờ chơi gửi vào kênh Discord của bạn.'],
      ['windows-and-stats#xếp-cửa-sổ', 'arrange-layouts.png', 'Xếp cửa sổ', 'Chọn kiểu xếp bằng hình mẫu như snap layouts của Windows, hoặc kích thước tự chọn. Ctrl+Alt+A xếp ngay từ app nào cũng được.'],
      ['settings', 'roblox-updates.png', 'Roblox theo ý bạn', 'Đồ họa, FPS và âm lượng riêng cho từng tài khoản. Mở Roblox bằng Bloxstrap, Fishstrap, Voidstrap hoặc launcher của bạn.'],
      ['troubleshooting#phím-tắt', 'palette.png', 'Tìm mọi thứ bằng Ctrl+K', 'Gõ vài chữ là ra tài khoản, game, cài đặt hay thao tác cần tìm.'],
      ['settings#giao-diện', 'theme-photos.jpg', 'Theme ảnh thật', 'Gần 100 theme, có theme là ảnh thật chuyển động, hoặc đặt ảnh, video của riêng bạn làm nền.'],
      ['backup-and-security', 'lock.png', 'Khóa bằng mật khẩu của bạn', 'Tài khoản được mã hóa bằng mật khẩu chủ và chỉ nằm trên máy bạn. Sao lưu vào thư mục OneDrive hoặc Google Drive, rồi khôi phục trên máy mới.'],
      ['getting-started#cập-nhật', 'lock-update.png', 'Tự cập nhật', 'Bản mới tự cài khi bạn không dùng Bloxkeep. Các cửa sổ Roblox vẫn chạy bình thường.'],
    ],
    startTitle: 'Bắt đầu trong 3 bước',
    start: [
      ['Đặt mật khẩu chủ', 'Dùng để mở Bloxkeep và mã hóa tài khoản. Quên là không lấy lại được, nên chọn cái bạn nhớ.'],
      ['Thêm tài khoản', 'Đăng nhập bằng điện thoại (nên dùng), tên đăng nhập và mật khẩu, hoặc cửa sổ trình duyệt.'],
      ['Chọn game, bấm Vào game', 'Gõ tên game ở thanh dưới, bấm vào các tài khoản để chọn, rồi bấm Vào game.'],
    ],
    startLink: 'Xem từng bước có hình',
    demoAlt: 'Chọn hai tài khoản, đổi game, xem danh sách server rồi đổi theme',
    safeTitle: 'Tài khoản vẫn là của bạn',
    safe: [
      ['Chỉ nằm trên máy bạn', 'Bloxkeep không có máy chủ riêng. Tài khoản nằm trong một file trên máy bạn, được mã hóa bằng mật khẩu chủ.'],
      ['Mã nguồn mở', 'Toàn bộ mã nguồn công khai trên GitHub, và file tải về do GitHub build từ chính mã nguồn đó, ai cũng kiểm tra được.'],
      ['Không chèn gì vào Roblox', 'Multi-Roblox không chèn gì vào Roblox, chỉ giữ cho Roblox không tự đóng các cửa sổ khác.'],
    ],
    caution: 'Chỉ tải Bloxkeep từ trang Releases trên GitHub hoặc từ trang này. Bản tải ở nơi khác có thể đã bị sửa để lấy cắp tài khoản.',
    getTitle: 'Tải về',
    getText: [
      'Bộ cài nhỏ: khi cài sẽ tải bản Bloxkeep mới nhất (khoảng 60 MB).',
      'Cài vào thư mục nào, ổ nào cũng được, không cần quyền admin. Tự cài Microsoft Edge WebView2 nếu máy chưa có.',
      'Sau đó Bloxkeep tự cập nhật. Muốn có tính năng mới sớm hơn, bật Nhận bản beta trong Cài đặt › Cập nhật Bloxkeep.',
    ],
    smartscreen: 'File chưa có chữ ký số nên lần đầu chạy Windows SmartScreen có thể cảnh báo: bấm More info (Thông tin thêm) rồi Run anyway (Vẫn chạy).',
    faqTitle: 'Hỏi đáp',
    faq: [
      ['Bloxkeep có mất phí không?', 'Không. Bloxkeep miễn phí và mã nguồn mở theo giấy phép MIT, không cần tài khoản hay đăng ký gói nào.'],
      ['Có cần Microsoft Edge hay trình duyệt nào không?', 'Không. Bloxkeep dùng WebView2, một thành phần của Windows, bạn dùng trình duyệt nào cũng được; bộ cài tự cài WebView2 nếu máy chưa có.'],
      ['Multi-Roblox có chạy được thật không?', 'Có: ảnh ở trên là ảnh chụp thật, sáu tài khoản cùng lúc trên một máy. Roblox có thể đổi cách chặn nhiều cửa sổ ở bất kỳ bản cập nhật nào, nên Bloxkeep kiểm tra từng bản Roblox mới và báo cho bạn khi có bản có thể chặn.'],
      ['Chạy nhiều cửa sổ Roblox có được phép không?', 'Roblox không chính thức hỗ trợ chạy nhiều cửa sổ cùng lúc, nên bạn tự cân nhắc khi dùng Multi-Roblox. Tính năng này tắt sẵn cho tới khi bạn bật.'],
      ['Tài khoản có bị ban không?', 'Không ai hứa được điều đó. Roblox không chính thức hỗ trợ chạy nhiều cửa sổ, và có game cấm dùng tài khoản phụ. Bloxkeep đăng nhập và mở game giống cách roblox.com làm, không bao giờ đụng vào bộ nhớ của game.'],
      ['Macro có phải hack không?', 'Không. Macro chỉ bấm phím, bấm chuột trong cửa sổ Roblox và nhìn hình của nó, như người chơi làm. Nó không bao giờ đọc hay sửa bộ nhớ của game. Dù vậy, có game vẫn cấm dùng macro: hãy xem luật của game.'],
      ['Quên mật khẩu chủ thì sao?', 'Không lấy lại được. Khôi phục một bản sao lưu mà bạn còn nhớ mật khẩu, hoặc làm lại từ đầu.'],
      ['Dùng trên Mac hay Linux được không?', 'Không, chỉ Windows 10 và 11.'],
      ['Cần giúp thì hỏi ở đâu?', 'Hỏi trong kênh #help trên Discord của Bloxkeep, đọc hướng dẫn, dùng Gửi góp ý trong app (Cài đặt › Cập nhật Bloxkeep), hoặc mở issue trên GitHub.'],
    ],
    guideLink: 'Đọc hướng dẫn sử dụng',
    newsTitle: v => `Có gì mới ở bản ${v}`,
    newsLink: 'Xem mọi thay đổi theo từng bản',
    discord: {
      title: 'Vào Discord của Bloxkeep',
      text: 'Hỏi khi cần giúp, góp ý tưởng, và biết ngay khi Roblox cập nhật hay Bloxkeep ra bản mới.',
      button: 'Vào server',
      note: 'Miễn phí. Xác minh nhanh lúc vào để chặn bot lừa đảo. Có kênh #general-vn nói tiếng Việt.',
      server: 'Bloxkeep',
      groups: [
        ['Info', [['announcements', 'Bloxkeep ra bản mới và tin tức'], ['roblox-updates', 'Bản Roblox sắp ra, đã ra, và khi Roblox lỗi']]],
        ['Support', [['help', 'Mỗi câu hỏi một bài: bạn làm gì, chuyện gì xảy ra, bản nào'], ['bug-reports', 'Gặp lỗi? Ghi các bước và chuyện gì xảy ra'], ['ideas', 'Mỗi ý tưởng một bài, thả cảm xúc cho ý bạn thích']]],
        ['Community', [['general', 'Chuyện về Bloxkeep và Roblox (tiếng Anh)'], ['general-vn', 'Trò chuyện bằng tiếng Việt']]],
      ],
      command: ['/multi', 'Multi-Roblox có chạy trên bản Roblox hiện tại và bản sắp ra không'],
      commandWho: 'Hỏi bot',
    },
  },
};

/** Real screenshots of Roblox windows (not Bloxkeep's interface): the same picture in every language. */
const SHARED = new Set(['multi-roblox-real.jpg', 'multi-roblox-real-detail.jpg', 'game-monitor-real.jpg']);

const esc = s => String(s).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');

/** The landing page of one language: its title and description for the head, its body, and what search engines read. */
export function landing(lang, { version, rel, releases = [] }) {
  const l = L[lang];
  const home = lang === 'vi' ? 'vi/' : '';
  const shot = name => rel(`screenshots/${lang === 'vi' && !SHARED.has(name) ? 'vi/' : ''}${name}`);
  const link = page => { const [topic, part] = page.split('#'); return rel(`${home}guide/${topic}/`) + (part ? `#${part}` : ''); };
  const buttons = `<div class="cta">
      <a class="btn primary big" href="${DOWNLOAD}">${esc(l.download)}</a>
      <a class="btn big" href="${PORTABLE}">${esc(l.portable)}</a>
    </div>
    <p class="meta">${esc(l.meta(version))} · <a href="${REPO}/releases/latest">${esc(l.all)}</a></p>`;
  // Multi-Roblox is the real screenshot above it; of the rest, the first six get a row each, picture beside words, and
  // the others share rows of four.
  // The newest release's highlights, from the app's own What's new (whatsnew.json).
  const top = releases.find(r => r.highlights?.length);
  const news = top && { version: top.version, date: top.date, highlights: top.highlights.map(h => h[lang].slice(0, 2)) };
  const rest = l.features.filter(([page]) => page !== 'multi-roblox');
  const rows = rest.slice(0, 6), small = rest.slice(6);
  const body = `<section class="hero">
  <div class="wrap">
    <div class="hero-text">
      <h1>${esc(l.h1)}</h1>
      <p class="lead">${esc(l.lead)}</p>
      ${buttons}
      <p class="try"><a href="${rel(home + 'demo/')}">${esc(l.tryLink)} ›</a></p>
    </div>
    <div class="stage">
      <figure class="stage-farm"><img class="shot" src="${shot('multi-roblox-real-detail.jpg')}" width="1136" height="1020" alt="${esc(l.farmAlt)}"><figcaption>${esc(l.farmTag)}</figcaption></figure>
      <figure class="stage-app"><a href="${rel(home + 'demo/')}" title="${esc(l.tryLink)}"><img class="shot" src="${shot('hero.png')}" width="1920" height="1200" alt="${esc(l.heroAlt)}"></a><figcaption>${esc(l.appTag)}</figcaption></figure>
    </div>
  </div>
</section>

<section class="band alt proof" aria-labelledby="proof">
  <div class="wrap">
    <h2 id="proof">${esc(l.proofTitle)}</h2>
    <p class="intro">${esc(l.proofText)}</p>
    <figure>
      <a href="${shot('multi-roblox-real.jpg')}"><img class="shot" src="${shot('multi-roblox-real.jpg')}" width="1702" height="1019" loading="lazy" decoding="async" alt="${esc(l.proofAlt)}"></a>
      <figcaption>${esc(l.proofCaption)} <a href="${rel(`${home}guide/multi-roblox/`)}">${esc(l.proofLink)} ›</a></figcaption>
    </figure>
  </div>
</section>

<section class="band" aria-labelledby="start">
  <div class="wrap start">
    <div>
      <h2 id="start">${esc(l.startTitle)}</h2>
      <ol class="start-steps">
${l.start.map(([title, text]) => `        <li><h3>${esc(title)}</h3><p>${esc(text)}</p></li>`).join('\n')}
      </ol>
      <a class="more" href="${rel(`${home}guide/getting-started/`)}">${esc(l.startLink)} ›</a>
    </div>
    <img class="shot" src="${shot('demo.gif')}" width="960" height="611" loading="lazy" decoding="async" alt="${esc(l.demoAlt)}">
  </div>
</section>

<section class="band alt" aria-labelledby="features">
  <div class="wrap">
    <h2 id="features">${esc(l.featuresTitle)}</h2>
${rows.map(([page, img, title, text]) => `    <article class="row">
      <a class="row-shot" href="${link(page)}" tabindex="-1"><img class="shot" src="${shot(img)}" loading="lazy" decoding="async" alt=""></a>
      <div class="row-text">
        <h3>${esc(title)}</h3>
        <p>${esc(text)}</p>
        <a class="more" href="${link(page)}">${esc(l.more)} ›</a>
      </div>
    </article>`).join('\n')}
    <div class="minis">
${small.map(([page, img, title, text]) => `      <article class="mini">
        <a class="mini-shot" href="${link(page)}" tabindex="-1"><img src="${shot(img)}" loading="lazy" decoding="async" alt=""></a>
        <h3>${esc(title)}</h3>
        <p>${esc(text)}</p>
        <a class="more" href="${link(page)}">${esc(l.more)} ›</a>
      </article>`).join('\n')}
    </div>
  </div>
</section>

${news ? `<section class="band news" aria-labelledby="news">
  <div class="wrap">
    <div class="news-head">
      <h2 id="news">${esc(l.newsTitle(news.version))}</h2>
      <p class="news-date"><time datetime="${news.date}">${esc(T[lang].news.date(news.date))}</time></p>
    </div>
    <ol class="news-list">
${news.highlights.map(([title, text]) => `      <li><h3>${esc(title)}</h3><p>${esc(text)}</p></li>`).join('\n')}
    </ol>
    <a class="more" href="${rel(`${home}whats-new/`)}">${esc(l.newsLink)} ›</a>
  </div>
</section>

` : ''}<section class="band alt" aria-labelledby="safe">
  <div class="wrap">
    <h2 id="safe">${esc(l.safeTitle)}</h2>
    <div class="cards">
${l.safe.map(([title, text]) => `      <div class="card"><h3>${esc(title)}</h3><p>${esc(text)}</p></div>`).join('\n')}
    </div>
    <div class="alert alert-caution"><p>${esc(l.caution)}</p></div>
  </div>
</section>

<section class="community" id="discord" aria-labelledby="community">
  <div class="wrap community-in">
    <div class="community-text">
      <svg class="discord-mark" viewBox="0 0 24 24" width="44" height="44" aria-hidden="true"><path fill="currentColor" d="${DISCORD_MARK}"/></svg>
      <h2 id="community">${esc(l.discord.title)}</h2>
      <p class="community-lead">${esc(l.discord.text)}</p>
      <a class="btn big on-blurple" href="${DISCORD}">${esc(l.discord.button)}</a>
      <p class="community-note">${esc(l.discord.note)}</p>
    </div>
    <div class="server">
      <p class="server-name"><img src="${rel('logo.svg')}" width="20" height="20" alt="">${esc(l.discord.server)}</p>
${l.discord.groups.map(([group, chans]) => `      <p class="server-group">${esc(group)}</p>
      <ul>
${chans.map(([name, topic]) => `        <li><span class="hash">#</span><b>${esc(name)}</b><span>${esc(topic)}</span></li>`).join('\n')}
      </ul>`).join('\n')}
      <p class="server-cmd"><span class="who">${esc(l.discord.commandWho)}</span><code>${esc(l.discord.command[0])}</code><span>${esc(l.discord.command[1])}</span></p>
    </div>
  </div>
</section>

<section class="band get" id="download" aria-labelledby="get">
  <div class="wrap">
    <h2 id="get">${esc(l.getTitle)}</h2>
    <ul class="steps">
${l.getText.map(x => `      <li>${esc(x)}</li>`).join('\n')}
    </ul>
    ${buttons}
    <p class="small">${esc(l.smartscreen)}</p>
  </div>
</section>

<section class="band alt" aria-labelledby="faq">
  <div class="wrap narrow">
    <h2 id="faq">${esc(l.faqTitle)}</h2>
    <div class="faq">
${l.faq.map(([q, a]) => `      <details><summary>${esc(q)}</summary><p>${esc(a)}</p></details>`).join('\n')}
    </div>
    <p><a class="btn" href="${rel(home + 'guide/')}">${esc(l.guideLink)} ›</a></p>
  </div>
</section>`;
  const jsonLd = {
    '@context': 'https://schema.org',
    '@graph': [WEBSITE, {
      '@type': 'SoftwareApplication',
      name: 'Bloxkeep',
      description: l.description,
      applicationCategory: 'UtilitiesApplication',
      operatingSystem: 'Windows 10, Windows 11',
      softwareVersion: version,
      downloadUrl: DOWNLOAD,
      url: SITE + home,
      license: `${REPO}/blob/main/LICENSE`,
      inLanguage: lang,
      offers: { '@type': 'Offer', price: '0', priceCurrency: 'USD' },
    }],
  };
  return { title: l.title, description: l.description, body, jsonLd };
}

/** The What's new page of one language: every regular version from the app's own notes (whatsnew.json), newest first. */
export function whatsNew(lang, releases) {
  const n = T[lang].news;
  const list = (title, items) => items?.length ? `<h3>${esc(title)}</h3>\n<ul>${items.map(x => `<li>${esc(x)}</li>`).join('')}</ul>\n` : '';
  const id = v => 'v' + v.replace(/\./g, '-');
  const body = releases.map(r => {
    const highlights = (r.highlights ?? []).map(h => h[lang]).filter(Boolean)
      .map(h => Array.isArray(h) ? `<div class="hl"><h3>${esc(h[0])}</h3>${h[1] ? `<p>${esc(h[1])}</p>` : ''}</div>` : `<div class="hl"><p>${esc(h)}</p></div>`).join('\n');
    return `<section class="release" id="${id(r.version)}" aria-labelledby="${id(r.version)}-h">
<h2 id="${id(r.version)}-h">${esc(r.version)} <time datetime="${r.date}">${esc(n.date(r.date))}</time></h2>
${highlights ? `<div class="hls">${highlights}</div>\n` : ''}${list(n.kinds[lang] ?? '', r[lang])}${['new', 'improved', 'fixed', 'removed'].map(k => list(n.kinds[k], r[k]?.[lang])).join('')}</section>`;
  }).join('\n');
  const jump = releases.map(r => `<a href="#${id(r.version)}">${esc(r.version)}</a>`).join('');
  return { title: n.title, description: n.intro, body: `<article class="wrap doc news-page">
<h1>${esc(n.title)}</h1>
<p class="intro">${esc(n.intro)}</p>
<nav class="versions" aria-label="${esc(n.jump)}">${jump}</nav>
${body}
</article>` };
}
