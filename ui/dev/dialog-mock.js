'use strict';
// Fake app for the message box's page (dialog.html) in a normal browser, with the app's own questions as Dialog.cs gets
// them (already in the app's language). ?case=ask (Close all, with "Don't ask again"), choices (the uninstaller's
// question), error, signout (a heading and two buttons), long (a long error with a folder path); ?theme=light a theme,
// ?lang=vi Vietnamese. What is clicked shows in the page's calls (Tests/ui/page-hooks.js).
(() => {
  const params = new URLSearchParams(location.search);
  const vi = params.get('lang') === 'vi';
  const L = (en, vn) => (vi ? vn : en);
  const cases = {
    ask: {
      text: L('Close ALL Roblox running on this PC (including Roblox in the system tray)?',
        'Đóng TẤT CẢ Roblox đang chạy trên máy (kể cả Roblox chạy ngầm ở khay hệ thống)?'),
      heading: null, kind: 'warning', def: 1, cancel: 1, footnote: null, remember: L("Don't ask again", 'Không hỏi lại'),
      choices: [{ text: L('Close all', 'Đóng tất cả'), detail: null, danger: true }, { text: L('Cancel', 'Hủy'), detail: null, danger: false }],
    },
    choices: {
      text: L('Bloxkeep is being removed from this PC. Your accounts and settings stay unless you delete them.',
        'Đang gỡ Bloxkeep khỏi máy này. Tài khoản và cài đặt vẫn giữ nguyên, trừ khi bạn chọn xóa.'),
      heading: L('Delete your accounts too?', 'Xóa luôn tài khoản?'), kind: 'warning', def: 0, cancel: 0,
      footnote: L("Kept if you don't choose within 20 seconds.", 'Không chọn trong 20 giây thì sẽ giữ lại tất cả.'), remember: null,
      choices: [
        { text: L('Keep my accounts and settings', 'Giữ tài khoản và cài đặt'), detail: L('Installing Bloxkeep again picks up where you left off.', 'Cài lại Bloxkeep là dùng tiếp như cũ.'), danger: false },
        { text: L('Delete my accounts and settings', 'Xóa tài khoản và cài đặt'),
          detail: L("Deletes the vault with your saved accounts, your settings and logs in C:\\Users\\Minh\\AppData\\Local\\Bloxkeep. This can't be undone.",
            'Xóa vault chứa tài khoản đã lưu, cài đặt và nhật ký trong C:\\Users\\Minh\\AppData\\Local\\Bloxkeep. Không thể hoàn tác.'), danger: true },
      ],
    },
    error: {
      text: L("Microsoft Edge WebView2 isn't installed. Install it from Microsoft's website, then reopen Bloxkeep.",
        'Máy chưa có Microsoft Edge WebView2. Hãy cài từ trang web của Microsoft rồi mở lại Bloxkeep.'),
      heading: null, kind: 'error', def: 0, cancel: 0, footnote: null, remember: null, choices: [{ text: 'OK', detail: null, danger: false }],
    },
    signout: {
      text: L("Signing out here would also sign this account out of Bloxkeep, and you'd have to add it again. When you're done, just close this tab.",
        'Đăng xuất ở đây cũng sẽ đăng xuất tài khoản khỏi Bloxkeep và bạn phải thêm lại. Xem xong chỉ cần đóng tab này.'),
      heading: L('Sign-out stopped', 'Đã chặn đăng xuất'), kind: 'info', def: 0, cancel: 1, footnote: null, remember: null,
      choices: [{ text: L('Close tab', 'Đóng tab'), detail: null, danger: false }, { text: L('Keep browsing', 'Xem tiếp'), detail: null, danger: false }],
    },
    long: {
      text: L("Couldn't back up the vault to D:\\Backups\\Bloxkeep\\a-very-long-folder-name-that-goes-on-and-on\\vault-2026-11-03.rmv: Access to the path is denied.",
        'Không sao lưu được vault vào D:\\Backups\\Bloxkeep\\a-very-long-folder-name-that-goes-on-and-on\\vault-2026-11-03.rmv: Không có quyền ghi vào thư mục này.'),
      heading: null, kind: 'error', def: 0, cancel: 0, footnote: null, remember: null, choices: [{ text: 'OK', detail: null, danger: false }],
    },
    confirm: {
      text: L("Couldn't save your changes. Quit anyway and lose them?", 'Không lưu được thay đổi. Vẫn thoát và bỏ thay đổi?'),
      heading: null, kind: 'warning', def: 1, cancel: 1, footnote: null, remember: null,
      choices: [{ text: L('Quit anyway', 'Vẫn thoát'), detail: null, danger: true }, { text: L('Cancel', 'Hủy'), detail: null, danger: false }],
    },
  };
  const data = cases[params.get('case')] || cases.ask;
  let emit = () => {};
  window.__mockBridge = {
    attach(fn) { emit = fn; },
    async call(cmd) {
      if (cmd === 'ready') setTimeout(() => emit('dialog', { ...data, theme: params.get('theme') || 'dark', customTheme: null, language: vi ? 'vi' : 'en' }), 0);
      return null;
    },
  };
})();
