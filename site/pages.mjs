// The website's own words (build.mjs makes the pages): the landing page and the frame around the guide, in English and
// Vietnamese. Every claim here is one the README and the guide make; keep them in step.
export const SITE = 'https://bloxkeep.github.io/roblox-account-manager/';
export const REPO = 'https://github.com/Bloxkeep/roblox-account-manager';
// Google gives a name to a whole host, never to a folder like /roblox-account-manager/, and reads it from the WebSite
// data on the host's own home page (https://developers.google.com/search/docs/appearance/site-names). That page is the
// repo Bloxkeep/bloxkeep.github.io; the landing pages carry the same WebSite node so the two never disagree.
const HOST = 'https://bloxkeep.github.io/';
const WEBSITE = { '@type': 'WebSite', '@id': `${HOST}#website`, name: 'Bloxkeep', alternateName: ['bloxkeep.github.io'], url: HOST };
const DOWNLOAD = `${REPO}/releases/latest/download/Bloxkeep-Setup.exe`;
const PORTABLE = `${REPO}/releases/latest/download/Bloxkeep-Portable.zip`;

/** The guide's topic pages (docs/guide/<topic>.md and .vi.md), in the guide's own order. */
export const TOPICS = ['getting-started', 'accounts', 'playing', 'multi-roblox', 'presets', 'staying-in-game',
  'windows-and-stats', 'discord', 'settings', 'backup-and-security', 'troubleshooting'];

export const T = {
  en: {
    nav: { guide: 'Guide', download: 'Download', other: 'Tiếng Việt' },
    guideTitle: 'Bloxkeep guide',
    allTopics: 'All topics',
    alerts: { note: 'Note', tip: 'Tip', important: 'Important', warning: 'Warning', caution: 'Caution' },
    topics: {
      'getting-started': 'Getting started', accounts: 'Accounts', playing: 'Playing', 'multi-roblox': 'Multi-Roblox',
      presets: 'Presets and schedules', 'staying-in-game': 'Staying in game', 'windows-and-stats': 'Windows and statistics',
      discord: 'Discord', settings: 'Settings', 'backup-and-security': 'Backup and security', troubleshooting: 'Troubleshooting',
    },
    foot: {
      disclaimer: 'Bloxkeep is an independent project. It is not affiliated with, endorsed or sponsored by Roblox Corporation. Roblox is a trademark of Roblox Corporation.',
      releases: 'All releases', privacy: 'Privacy', signing: 'Code signing policy', license: 'MIT license',
    },
  },
  vi: {
    nav: { guide: 'Hướng dẫn', download: 'Tải về', other: 'English' },
    guideTitle: 'Hướng dẫn Bloxkeep',
    allTopics: 'Các chủ đề',
    alerts: { note: 'Lưu ý', tip: 'Mẹo', important: 'Quan trọng', warning: 'Cảnh báo', caution: 'Cẩn thận' },
    topics: {
      'getting-started': 'Bắt đầu', accounts: 'Tài khoản', playing: 'Vào game', 'multi-roblox': 'Multi-Roblox',
      presets: 'Preset và hẹn giờ', 'staying-in-game': 'Treo game', 'windows-and-stats': 'Cửa sổ và thống kê',
      discord: 'Discord', settings: 'Cài đặt', 'backup-and-security': 'Sao lưu và bảo mật', troubleshooting: 'Gỡ rối',
    },
    foot: {
      disclaimer: 'Bloxkeep là dự án độc lập, không liên kết, không được Roblox Corporation bảo trợ hay tài trợ. Roblox là thương hiệu của Roblox Corporation.',
      releases: 'Mọi phiên bản', privacy: 'Quyền riêng tư', signing: 'Chính sách ký số', license: 'Giấy phép MIT',
    },
  },
};

const L = {
  en: {
    title: 'Bloxkeep: run several Roblox accounts at once on Windows',
    description: 'Free, open-source Roblox account manager for Windows. Play several accounts at once with Multi-Roblox, auto-rejoin, anti-AFK, presets on a schedule and Discord alerts. Your accounts stay encrypted on your PC.',
    h1: 'Run several Roblox accounts at once on Windows',
    lead: 'Bloxkeep keeps your Roblox accounts in one place. Pick a game, pick accounts, press Play, and each one opens in its own window. Free and open source; your accounts stay encrypted on your PC.',
    download: 'Download Bloxkeep',
    portable: 'Portable ZIP',
    all: 'All downloads',
    meta: v => `Version ${v} · Windows 10 or 11, 64-bit · Free`,
    heroAlt: "Bloxkeep's main window: account cards, the game bar at the bottom, Multi-Roblox on in the sidebar",
    proofTitle: 'See it working',
    proofText: "A real screenshot from one Windows PC: five Roblox accounts open at once with Bloxkeep. Four are already in the game, the fifth is joining, and Bloxkeep's sidebar counts 5 windows with Multi-Roblox on.",
    proofAlt: 'Five Roblox windows open at once on one PC, titled Roblox - Account 1 to Roblox - Account 5, four in the same game and the fifth joining, with Bloxkeep behind them',
    proofCaption: 'Names in the game are blurred. Roblox 0.743, October 2026.',
    proofLink: 'How to turn on Multi-Roblox',
    featuresTitle: 'What it does',
    more: 'How it works',
    features: [
      ['multi-roblox', 'multi-roblox-real-detail.jpg', 'Several Roblox windows at once', 'Turn on Multi-Roblox and every account plays in its own window, opened windowed and tiled the way you choose. Each new Roblox build is checked.'],
      ['accounts', 'accounts-list.png', 'All your accounts in one place', 'Add accounts with a code from the Roblox phone app, a password (2-step verification works), the Roblox page or a cookie. See who is online, in a game or needs to sign in.'],
      ['playing', 'game-picker.png', 'Pick a game, pick a server', 'Find a game by name or link. Choose the server with the most players, the fewest, the lowest ping or room for all your accounts, or join a friend.'],
      ['presets', 'presets.png', 'Presets and schedules', 'Save accounts, a game and a server and start them in one click, or on a schedule that also stops them, with timed restarts.'],
      ['staying-in-game', 'card-rejoin.png', 'Stay in game', 'Anti-AFK keeps windows from being kicked for idling. Auto-rejoin goes back in after a disconnect, waiting a little longer each try instead of giving up.'],
      ['windows-and-stats', 'stats.png', 'Game monitor and statistics', "Live pictures of every Roblox window, a mini window, a warning before RAM or Windows' virtual memory runs out, and play time per account and game."],
      ['discord', 'discord-alert.png', 'Discord alerts', 'Alerts, a live status message and play-time summaries in your own Discord channel.'],
      ['backup-and-security', 'lock.png', 'Locked with your password', 'Accounts are encrypted with your master password and stay on your PC. Back them up to a OneDrive or Google Drive folder and restore them on a new PC.'],
    ],
    safeTitle: 'Your accounts stay yours',
    safe: [
      ['On your PC only', 'Bloxkeep has no server of its own. Your accounts are kept in a file on your PC, encrypted with your master password.'],
      ['Open source', 'Every line is public on GitHub, and the downloads are built by GitHub from that source, so anyone can check them.'],
      ['Nothing injected', 'Multi-Roblox injects nothing into Roblox: it only keeps Roblox from closing the other windows.'],
    ],
    caution: 'Only download Bloxkeep from its GitHub Releases page or this site. A copy from anywhere else may have been changed to steal accounts.',
    getTitle: 'Download',
    getText: 'Bloxkeep-Setup.exe is a small installer: it downloads the newest Bloxkeep as it installs (about 60 MB) and installs Microsoft Edge WebView2 if your PC lacks it. You choose the folder, on any drive, and it needs no admin rights. Bloxkeep then updates itself; to get new features earlier, turn on Get beta versions in Settings › Bloxkeep updates.',
    smartscreen: "The files aren't code-signed yet, so Windows SmartScreen may warn the first time: click More info, then Run anyway.",
    faqTitle: 'Questions',
    faq: [
      ['Is Bloxkeep free?', 'Yes. It is free and open source under the MIT license, with no account or subscription.'],
      ['Does it need Microsoft Edge or a particular browser?', 'No. It uses WebView2, a part of Windows, whatever browser you use; Setup installs it if it is missing.'],
      ['Does Multi-Roblox really work?', 'Yes: the screenshot above is real, five accounts at once on one PC. Roblox can change how it blocks several windows in any update, so Bloxkeep checks every new Roblox build and tells you when one may stop it.'],
      ['Is running several Roblox windows allowed?', "Roblox doesn't officially support running several clients, so use Multi-Roblox at your own risk. It is off until you turn it on."],
      ['Where do I get help?', 'Read the guide, use Send feedback in the app (Settings › Bloxkeep updates), or open an issue on GitHub.'],
    ],
    guideLink: 'Read the user guide',
  },
  vi: {
    title: 'Bloxkeep: chơi nhiều tài khoản Roblox cùng lúc trên Windows',
    description: 'App quản lý tài khoản Roblox miễn phí, mã nguồn mở cho Windows. Chơi nhiều tài khoản cùng lúc với Multi-Roblox, tự vào lại game, chống AFK, Preset hẹn giờ và báo về Discord. Tài khoản được mã hóa, chỉ nằm trên máy bạn.',
    h1: 'Chơi nhiều tài khoản Roblox cùng lúc trên Windows',
    lead: 'Bloxkeep giữ các tài khoản Roblox của bạn ở một chỗ. Chọn game, chọn tài khoản, bấm Vào game, mỗi tài khoản mở một cửa sổ riêng. Miễn phí, mã nguồn mở, tài khoản được mã hóa và chỉ nằm trên máy bạn.',
    download: 'Tải Bloxkeep',
    portable: 'Bản portable (ZIP)',
    all: 'Tất cả bản tải',
    meta: v => `Phiên bản ${v} · Windows 10 hoặc 11, 64-bit · Miễn phí`,
    heroAlt: 'Cửa sổ chính của Bloxkeep: các thẻ tài khoản, thanh chọn game ở dưới, Multi-Roblox đang bật ở thanh bên',
    proofTitle: 'Chạy thật trên một máy',
    proofText: 'Ảnh chụp thật trên một máy Windows: năm tài khoản Roblox mở cùng lúc bằng Bloxkeep. Bốn tài khoản đã vào game, tài khoản thứ năm đang vào, thanh bên của Bloxkeep đếm 5 cửa sổ và Multi-Roblox đang bật.',
    proofAlt: 'Năm cửa sổ Roblox mở cùng lúc trên một máy, tiêu đề từ Roblox - Account 1 đến Roblox - Account 5, bốn cửa sổ cùng ở trong một game và cửa sổ thứ năm đang vào, phía sau là Bloxkeep',
    proofCaption: 'Tên trong game đã được làm mờ. Roblox 0.743, tháng 10/2026.',
    proofLink: 'Cách bật Multi-Roblox',
    featuresTitle: 'Bloxkeep làm được gì',
    more: 'Xem cách dùng',
    features: [
      ['multi-roblox', 'multi-roblox-real-detail.jpg', 'Nhiều cửa sổ Roblox cùng lúc', 'Bật Multi-Roblox là mỗi tài khoản chơi trong một cửa sổ riêng, mở ở chế độ cửa sổ và xếp theo ý bạn. Mỗi bản Roblox mới đều được kiểm tra.'],
      ['accounts', 'accounts-list.png', 'Mọi tài khoản ở một chỗ', 'Thêm tài khoản bằng mã từ app Roblox trên điện thoại, bằng mật khẩu (có xác minh 2 bước), qua trang Roblox hoặc bằng cookie. Xem ngay ai đang online, đang trong game hay cần đăng nhập lại.'],
      ['playing', 'game-picker.png', 'Chọn game, chọn server', 'Tìm game theo tên hoặc link. Chọn server đông nhất, vắng nhất, ping thấp nhất hay đủ chỗ cho mọi tài khoản, hoặc vào cùng bạn bè.'],
      ['presets', 'presets.png', 'Preset và hẹn giờ', 'Lưu sẵn tài khoản, game và server rồi mở bằng một cú bấm, hoặc theo lịch tự mở tự tắt, kèm khởi động lại định kỳ.'],
      ['staying-in-game', 'card-rejoin.png', 'Treo game yên tâm', 'Chống AFK giữ cửa sổ khỏi bị kick vì đứng yên. Tự vào lại game sau khi bị văng, mỗi lần chờ lâu hơn một chút chứ không bỏ cuộc.'],
      ['windows-and-stats', 'stats.png', 'Theo dõi game và thống kê', 'Hình trực tiếp của mọi cửa sổ Roblox, cửa sổ nhỏ, cảnh báo trước khi hết RAM hoặc bộ nhớ ảo của Windows, và thời gian chơi theo tài khoản và game.'],
      ['discord', 'discord-alert.png', 'Báo về Discord', 'Cảnh báo, tin trạng thái cập nhật liên tục và tổng kết giờ chơi gửi vào kênh Discord của bạn.'],
      ['backup-and-security', 'lock.png', 'Khóa bằng mật khẩu của bạn', 'Tài khoản được mã hóa bằng mật khẩu chủ và chỉ nằm trên máy bạn. Sao lưu vào thư mục OneDrive hoặc Google Drive, rồi khôi phục trên máy mới.'],
    ],
    safeTitle: 'Tài khoản vẫn là của bạn',
    safe: [
      ['Chỉ nằm trên máy bạn', 'Bloxkeep không có máy chủ riêng. Tài khoản nằm trong một file trên máy bạn, được mã hóa bằng mật khẩu chủ.'],
      ['Mã nguồn mở', 'Toàn bộ mã nguồn công khai trên GitHub, và file tải về do GitHub build từ chính mã nguồn đó, ai cũng kiểm tra được.'],
      ['Không chèn gì vào Roblox', 'Multi-Roblox không chèn gì vào Roblox, chỉ giữ cho Roblox không tự đóng các cửa sổ khác.'],
    ],
    caution: 'Chỉ tải Bloxkeep từ trang Releases trên GitHub hoặc từ trang này. Bản tải ở nơi khác có thể đã bị sửa để lấy cắp tài khoản.',
    getTitle: 'Tải về',
    getText: 'Bloxkeep-Setup.exe là bộ cài nhỏ: khi cài nó tải bản Bloxkeep mới nhất (khoảng 60 MB) và tự cài Microsoft Edge WebView2 nếu máy chưa có. Bạn chọn thư mục cài, ở ổ nào cũng được, không cần quyền admin. Sau đó Bloxkeep tự cập nhật; muốn dùng tính năng mới sớm hơn, bật Nhận bản beta trong Cài đặt › Cập nhật Bloxkeep.',
    smartscreen: 'File chưa có chữ ký số nên lần đầu chạy Windows SmartScreen có thể cảnh báo: bấm More info (Thông tin thêm) rồi Run anyway (Vẫn chạy).',
    faqTitle: 'Hỏi đáp',
    faq: [
      ['Bloxkeep có mất phí không?', 'Không. Bloxkeep miễn phí và mã nguồn mở theo giấy phép MIT, không cần tài khoản hay đăng ký gói nào.'],
      ['Có cần Microsoft Edge hay trình duyệt nào không?', 'Không. Bloxkeep dùng WebView2, một thành phần của Windows, bạn dùng trình duyệt nào cũng được; bộ cài tự cài WebView2 nếu máy chưa có.'],
      ['Multi-Roblox có chạy được thật không?', 'Có: ảnh ở trên là ảnh chụp thật, năm tài khoản cùng lúc trên một máy. Roblox có thể đổi cách chặn nhiều cửa sổ ở bất kỳ bản cập nhật nào, nên Bloxkeep kiểm tra từng bản Roblox mới và báo cho bạn khi có bản có thể chặn.'],
      ['Chạy nhiều cửa sổ Roblox có được phép không?', 'Roblox không chính thức hỗ trợ chạy nhiều cửa sổ cùng lúc, nên bạn tự cân nhắc khi dùng Multi-Roblox. Tính năng này tắt sẵn cho tới khi bạn bật.'],
      ['Cần giúp thì hỏi ở đâu?', 'Đọc hướng dẫn, dùng Gửi góp ý trong app (Cài đặt › Cập nhật Bloxkeep), hoặc mở issue trên GitHub.'],
    ],
    guideLink: 'Đọc hướng dẫn sử dụng',
  },
};

/** Real screenshots of Roblox windows (not Bloxkeep's interface): the same picture in every language. */
const SHARED = new Set(['multi-roblox-real.jpg', 'multi-roblox-real-detail.jpg']);

const esc = s => String(s).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');

/** The landing page of one language: its title and description for the head, its body, and what search engines read. */
export function landing(lang, { version, rel }) {
  const l = L[lang];
  const home = lang === 'vi' ? 'vi/' : '';
  const shot = name => rel(`screenshots/${lang === 'vi' && !SHARED.has(name) ? 'vi/' : ''}${name}`);
  const buttons = `<div class="cta">
      <a class="btn primary" href="${DOWNLOAD}">${esc(l.download)}</a>
      <a class="btn" href="${PORTABLE}">${esc(l.portable)}</a>
      <a class="btn ghost" href="${REPO}/releases/latest">${esc(l.all)}</a>
    </div>
    <p class="meta">${esc(l.meta(version))}</p>`;
  const body = `<section class="hero wrap">
  <div class="hero-text">
    <h1>${esc(l.h1)}</h1>
    <p class="lead">${esc(l.lead)}</p>
    ${buttons}
  </div>
  <img class="shot hero-shot" src="${shot('hero.png')}" width="1320" height="840" alt="${esc(l.heroAlt)}">
</section>

<section class="wrap proof" aria-labelledby="proof">
  <h2 id="proof">${esc(l.proofTitle)}</h2>
  <p>${esc(l.proofText)}</p>
  <figure>
    <a href="${shot('multi-roblox-real.jpg')}"><img class="shot" src="${shot('multi-roblox-real.jpg')}" width="1920" height="1080" alt="${esc(l.proofAlt)}"></a>
    <figcaption>${esc(l.proofCaption)} <a href="${rel(`${home}guide/multi-roblox/`)}">${esc(l.proofLink)} ›</a></figcaption>
  </figure>
</section>

<section class="wrap" aria-labelledby="features">
  <h2 id="features">${esc(l.featuresTitle)}</h2>
  <div class="features">
${l.features.map(([topic, img, title, text]) => `    <article class="feature">
      <a href="${rel(`${home}guide/${topic}/`)}"><img class="shot" src="${shot(img)}" loading="lazy" decoding="async" alt=""></a>
      <h3>${esc(title)}</h3>
      <p>${esc(text)}</p>
      <a class="more" href="${rel(`${home}guide/${topic}/`)}">${esc(l.more)} ›</a>
    </article>`).join('\n')}
  </div>
</section>

<section class="wrap" aria-labelledby="safe">
  <h2 id="safe">${esc(l.safeTitle)}</h2>
  <div class="cards">
${l.safe.map(([title, text]) => `    <div class="card"><h3>${esc(title)}</h3><p>${esc(text)}</p></div>`).join('\n')}
  </div>
  <div class="alert alert-caution"><p>${esc(l.caution)}</p></div>
</section>

<section class="wrap get" id="download" aria-labelledby="get">
  <h2 id="get">${esc(l.getTitle)}</h2>
  <p>${esc(l.getText)}</p>
  ${buttons}
  <p class="small">${esc(l.smartscreen)}</p>
</section>

<section class="wrap" aria-labelledby="faq">
  <h2 id="faq">${esc(l.faqTitle)}</h2>
  <dl class="faq">
${l.faq.map(([q, a]) => `    <dt>${esc(q)}</dt><dd>${esc(a)}</dd>`).join('\n')}
  </dl>
  <p><a class="btn" href="${rel(home + 'guide/')}">${esc(l.guideLink)}</a></p>
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
