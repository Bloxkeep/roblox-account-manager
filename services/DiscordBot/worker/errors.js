// /error: what a Roblox error code means for someone running several accounts, what Bloxkeep does when a window
// gets it, and what the person can do. The codes people ask about most are written out here; any other code of
// Roblox's list answers with Roblox's own name for it (error_names.js, from the app's table). What Bloxkeep is said
// to do is what the app does (ui-gpui/src/core/app/rejoin.rs, roblox_codes.rs): change both together.
import { NAMES } from './error_names.js';
import { COLOR, LOGO, SITE } from './texts.js';


/** After which codes Bloxkeep rejoins by itself (auto-rejoin on): the app's rule, said in one place. */
const REJOINS = {
  en: 'With Auto-rejoin on (Settings › Keeping accounts going), Bloxkeep closes the window and opens the account in the game again.',
  vi: 'Khi bật Tự vào lại (Cài đặt › Giữ tài khoản trong game), Bloxkeep đóng cửa sổ rồi mở lại tài khoản vào game.',
};
const STAYS = {
  en: "Bloxkeep leaves the window as it is and doesn't rejoin: Roblox refuses a reconnect after this code, so a new window would only get it again.",
  vi: 'Bloxkeep để nguyên cửa sổ và không vào lại: sau mã này Roblox từ chối kết nối lại, mở cửa sổ mới cũng chỉ gặp lại lỗi đó.',
};

/** code → [title, what it is, what Bloxkeep does, what to do (lines)] in each language. */
const KNOWN = {
  256: {
    en: ['Lost connection, no reason given', "Roblox's catch-all: the connection to the game ended and Roblox names no cause. People meet it most while a game moves them to another server.", REJOINS.en,
      ['Nothing to fix in the account: turn Auto-rejoin on and let it come back.', 'If several accounts get it at the same moment, look at the network, or at RAM and CPU when many windows are open.']],
    vi: ['Mất kết nối, không nêu lý do', 'Mã "chung chung" của Roblox: kết nối tới game bị ngắt mà Roblox không nói vì sao. Hay gặp nhất lúc game chuyển bạn sang server khác.', REJOINS.vi,
      ['Tài khoản không có gì phải sửa: bật Tự vào lại để nó tự vào lại.', 'Nếu nhiều tài khoản bị cùng lúc, xem lại mạng, hoặc RAM và CPU khi mở nhiều cửa sổ.']],
  },
  264: {
    en: ['The account is already in another game', 'The same account joined a game somewhere else, and this window was put out.', STAYS.en, ['Close the other session, then press Play again.']],
    vi: ['Tài khoản đang ở game khác', 'Chính tài khoản này vừa vào game ở nơi khác nên cửa sổ này bị đẩy ra.', STAYS.vi, ['Thoát phiên kia rồi nhấn Play lại.']],
  },
  267: {
    en: ['Kicked by the game', "The game's own script removed the account, often with a message (an anti-AFK kick, a full team, a data lock).", "Bloxkeep shows the game's message and rejoins: many games ask you to rejoin after a kick. A ban by the game is told apart and never rejoined.", ['Read the message on the account\'s card: it is the game\'s reason.']],
    vi: ['Bị game kick', 'Script của chính game đưa tài khoản ra, thường kèm lời nhắn (chống AFK, đội đầy, khoá dữ liệu).', 'Bloxkeep hiện lời nhắn của game và vào lại, vì nhiều game yêu cầu vào lại sau khi kick. Ban của game được nhận riêng và không vào lại.', ['Đọc lời nhắn trên thẻ tài khoản: đó là lý do của game.']],
  },
  268: {
    en: ['Kicked for unexpected client behaviour', "Roblox's server ended the session because the client did something it didn't expect.", STAYS.en, ['Remove anything that changes the Roblox client (injectors, modified files), update Roblox, then Play again.']],
    vi: ['Bị kick vì client hoạt động bất thường', 'Server Roblox ngắt phiên vì client làm điều nó không mong đợi.', STAYS.vi, ['Gỡ mọi thứ can thiệp vào client Roblox (injector, file bị sửa), cập nhật Roblox rồi Play lại.']],
  },
  273: {
    en: ['Signed in on another device', 'The same account joined a game from another device, which takes the place of this window.', "Bloxkeep doesn't fight the other device: rejoining would only put that one out in turn.", ['Leave the game on the other device, then press Play here.']],
    vi: ['Đăng nhập ở thiết bị khác', 'Chính tài khoản này vào game từ thiết bị khác nên chiếm chỗ của cửa sổ này.', 'Bloxkeep không giành lại với thiết bị kia: vào lại chỉ làm bên đó bị đẩy ra theo.', ['Thoát game ở thiết bị kia rồi nhấn Play ở đây.']],
  },
  277: {
    en: ['Connection lost', "The window stopped hearing from the game's server: your network, the server, or a window too starved to answer in time.", REJOINS.en, ['Check the connection if several accounts drop at once.', 'With many windows open, give them a little more FPS or CPU.']],
    vi: ['Mất kết nối', 'Cửa sổ không còn nhận được phản hồi từ server game: do mạng, do server, hoặc cửa sổ thiếu tài nguyên nên trả lời không kịp.', REJOINS.vi, ['Kiểm tra mạng nếu nhiều tài khoản rớt cùng lúc.', 'Khi mở nhiều cửa sổ, cho chúng thêm chút FPS hoặc CPU.']],
  },
  278: {
    en: ['Kicked for being idle', 'Roblox removes a player who sends no input for 20 minutes.', REJOINS.en, ['Turn on Anti-AFK (Settings › Keeping accounts going): Bloxkeep keeps each window from going idle.']],
    vi: ['Bị kick vì treo máy', 'Roblox đưa người chơi ra sau 20 phút không có thao tác.', REJOINS.vi, ['Bật Chống AFK (Cài đặt › Giữ tài khoản trong game): Bloxkeep giữ cho từng cửa sổ không bị tính là treo.']],
  },
  279: {
    en: ["Couldn't reach the server", 'Roblox was sent to a server and got no answer from it: the server just closed, or the connection to it is blocked.', REJOINS.en, ['If every join fails, check a VPN, a firewall or the router.']],
    vi: ['Không kết nối được server', 'Roblox được chỉ tới một server nhưng không nhận được trả lời: server vừa đóng, hoặc đường tới nó bị chặn.', REJOINS.vi, ['Nếu lần vào nào cũng lỗi, kiểm tra VPN, tường lửa hoặc router.']],
  },
  280: {
    en: ['Roblox needs an update', "The game's servers run a newer Roblox than this window.", STAYS.en, ['Update Roblox, then press Play again. "Update automatically" for Roblox in Bloxkeep\'s Settings keeps it current.']],
    vi: ['Roblox cần cập nhật', 'Server game chạy bản Roblox mới hơn cửa sổ này.', STAYS.vi, ['Cập nhật Roblox rồi nhấn Play lại. Bật "Tự cập nhật" cho Roblox trong Cài đặt của Bloxkeep để luôn có bản mới.']],
  },
  288: {
    en: ['Server shut down', "The game's server closed: an update of the game, or its developer shutting servers down.", REJOINS.en, ['Nothing to do: it is the game, not your accounts.']],
    vi: ['Server đóng', 'Server game đóng lại: game vừa cập nhật, hoặc nhà phát triển tắt server.', REJOINS.vi, ['Không cần làm gì: do game, không phải do tài khoản.']],
  },
  292: {
    en: ['The PC is out of memory', 'Roblox closed the game because the PC ran low on memory.', REJOINS.en, ['Open fewer windows, or turn on "Save RAM on background windows" (Settings › RAM & CPU).']],
    vi: ['Máy hết RAM', 'Roblox đóng game vì máy sắp hết bộ nhớ.', REJOINS.vi, ['Mở ít cửa sổ hơn, hoặc bật "Tiết kiệm RAM cho cửa sổ chạy nền" (Cài đặt › RAM & CPU).']],
  },
  524: {
    en: ['No permission to join', 'The server is not open to this account: a private server it has no access to, or a game that is not public.', STAYS.en, ['Ask the owner for access, or join a public server.']],
    vi: ['Không có quyền vào', 'Server không mở cho tài khoản này: server riêng mà nó không có quyền, hoặc game chưa công khai.', STAYS.vi, ['Xin chủ server cấp quyền, hoặc vào server công khai.']],
  },
  529: {
    en: ['Roblox answered the join with an error', "Roblox's own service failed the join. It also shows when Roblox wants a bot check from the account.", REJOINS.en,
      ['Use /robloxstatus to see whether Roblox is down.', "If one account keeps getting it, open that account's Roblox website from Bloxkeep and finish any check Roblox asks for."]],
    vi: ['Roblox trả lỗi khi vào game', 'Dịch vụ của Roblox xử lý lượt vào bị lỗi. Cũng gặp khi Roblox đòi tài khoản xác minh không phải bot.', REJOINS.vi,
      ['Dùng /robloxstatus để xem Roblox có đang lỗi không.', 'Nếu một tài khoản bị mãi, mở trang Roblox của tài khoản đó từ Bloxkeep và làm xong bước xác minh Roblox yêu cầu.']],
  },
  600: {
    en: ['Banned by the game', 'The game itself banned this account (not Roblox): its join is refused.', "Bloxkeep marks the account as banned by the game and doesn't rejoin it there.", ['Only the game can lift it. Other games are not affected.']],
    vi: ['Bị game ban', 'Chính game ban tài khoản này (không phải Roblox): lượt vào bị từ chối.', 'Bloxkeep đánh dấu tài khoản bị game ban và không cho vào lại game đó.', ['Chỉ game mới gỡ được. Các game khác không bị ảnh hưởng.']],
  },
  769: {
    en: ['Teleport failed', "The game tried to move the account to another place or server and Roblox couldn't do it.", "The account stays where it was: Roblox's box only has OK. If the window ends up out of the game, Bloxkeep rejoins.", ['Usually the game: try again, or join the place directly.']],
    vi: ['Teleport thất bại', 'Game định chuyển tài khoản sang nơi khác hoặc server khác nhưng Roblox không làm được.', 'Tài khoản ở nguyên chỗ cũ: hộp báo của Roblox chỉ có OK. Nếu cửa sổ bị ra khỏi game, Bloxkeep vào lại.', ['Thường do game: thử lại, hoặc vào thẳng nơi đó.']],
  },
  773: {
    en: ['Teleport failed: the place is restricted', "The place the game teleports to doesn't accept this account.", "The account stays where it was: Roblox's box only has OK.", ['Often a private or unfinished place of the game: nothing to fix on your side.']],
    vi: ['Teleport thất bại: nơi đến bị hạn chế', 'Nơi game chuyển tới không nhận tài khoản này.', 'Tài khoản ở nguyên chỗ cũ: hộp báo của Roblox chỉ có OK.', ['Thường là nơi riêng hoặc chưa mở của game: phía bạn không sửa được.']],
  },
  // Roblox's web answers, as Bloxkeep says them when a launch or a sign-in is refused.
  401: {
    en: ['Signed out (HTTP 401)', "Roblox no longer accepts the account's saved sign-in.", 'Bloxkeep marks the account "Needs sign-in".', ['Sign in to the account again in Bloxkeep.']],
    vi: ['Bị đăng xuất (HTTP 401)', 'Roblox không còn nhận phiên đăng nhập đã lưu của tài khoản.', 'Bloxkeep đánh dấu tài khoản "Cần đăng nhập".', ['Đăng nhập lại tài khoản trong Bloxkeep.']],
  },
  403: {
    en: ['Refused by Roblox (HTTP 403)', 'Roblox refused the request for this account. Most often it wants a check from the account first, or the account is restricted.', 'Bloxkeep says that Roblox refused, and for which account.', ["Open the account's Roblox website from Bloxkeep and finish any check Roblox asks for.", 'If one account alone keeps getting it, sign in to it again.']],
    vi: ['Roblox từ chối (HTTP 403)', 'Roblox từ chối yêu cầu của tài khoản này. Thường là Roblox đòi tài khoản xác minh trước, hoặc tài khoản bị hạn chế.', 'Bloxkeep báo Roblox từ chối, và từ chối tài khoản nào.', ['Mở trang Roblox của tài khoản từ Bloxkeep và làm xong bước xác minh Roblox yêu cầu.', 'Nếu chỉ một tài khoản bị mãi, đăng nhập lại tài khoản đó.']],
  },
  429: {
    en: ['Too many requests (HTTP 429)', 'Roblox is slowing this network down: too many asks in a short time, often from opening many accounts at once.', 'Bloxkeep says Roblox is limiting requests. It passes by itself, usually within a minute.', ['Wait a minute, then open accounts a few at a time. A longer "Wait between accounts" (Settings › How Roblox opens, and waiting times) helps.']],
    vi: ['Quá nhiều yêu cầu (HTTP 429)', 'Roblox đang hãm mạng này: quá nhiều lượt hỏi trong thời gian ngắn, thường do mở nhiều tài khoản cùng lúc.', 'Bloxkeep báo Roblox đang giới hạn yêu cầu. Thường tự hết trong khoảng một phút.', ['Chờ một phút rồi mở từng nhóm vài tài khoản. Tăng "Chờ giữa các tài khoản" (Cài đặt › Cách mở Roblox & thời gian chờ) sẽ đỡ hơn.']],
  },
};
// Codes that mean the same to the person as one written out above.
const SAME = { 266: 277, 276: 273, 271: 288, 274: 288, 275: 288, 517: 288, 523: 524, 770: 769, 771: 769, 772: 769, 774: 769 };

const W = {
  en: { error: 'Error', what: 'What Bloxkeep does', todo: 'What you can do', roblox: "Roblox's name for it", unknown: "That isn't one of Roblox's error codes I know.", unknownTodo: "Check the number in Roblox's error box (\"Error Code: 277\"), or ask in Bloxkeep's Discord with a screenshot.", other: 'Bloxkeep has no special handling for this code: a window that drops is rejoined when Auto-rejoin is on, unless Roblox refuses a reconnect after it.', guide: 'Guide' },
  vi: { error: 'Lỗi', what: 'Bloxkeep làm gì', todo: 'Bạn làm được gì', roblox: 'Tên Roblox đặt', unknown: 'Tôi không biết mã lỗi Roblox nào như vậy.', unknownTodo: 'Xem lại con số trong hộp báo lỗi của Roblox ("Error Code: 277"), hoặc hỏi trong Discord của Bloxkeep kèm ảnh chụp.', other: 'Bloxkeep không xử lý riêng mã này: cửa sổ bị rớt sẽ được mở lại khi bật Tự vào lại, trừ khi sau mã đó Roblox từ chối kết nối lại.', guide: 'Hướng dẫn' },
};

/** The answer to /error for a code, in language `l` ('en' or 'vi'). */
export function errorEmbed(l, code) {
  const w = W[l] ?? W.en, n = Number(code);
  const known = KNOWN[n] ?? KNOWN[SAME[n]];
  const name = NAMES[n]?.[0];
  const base = { thumbnail: { url: LOGO }, footer: { text: 'Bloxkeep', icon_url: LOGO } };
  if (known) {
    const [title, what, does, todo] = known[l] ?? known.en;
    return {
      ...base, color: COLOR.yellow, title: `${w.error} ${n} · ${title}`, description: what,
      fields: [
        { name: w.what, value: does },
        { name: w.todo, value: todo.map((t, i) => (todo.length > 1 ? `${i + 1}. ${t}` : t)).join('\n') },
        ...(name ? [{ name: w.roblox, value: `\`${name}\``, inline: true }] : []),
        { name: w.guide, value: `[bloxkeep.github.io](${SITE})`, inline: true },
      ],
    };
  }
  if (name)
    return { ...base, color: COLOR.grey, title: `${w.error} ${n} · ${NAMES[n][1]}`, description: w.other, fields: [{ name: w.roblox, value: `\`${name}\``, inline: true }] };
  return { ...base, color: COLOR.grey, title: `${w.error} ${Number.isFinite(n) ? n : '?'}`, description: `${w.unknown}\n${w.unknownTodo}`, fields: [] };
}
