# Tin game: gom tin từ bot Discord của các game lên app

Ngày: 2026-11-04. Trạng thái: **bản thiết kế, chưa làm**. Chờ chủ dự án duyệt.

---

## 0. Mục tiêu

- **Hôm nay:** muốn biết món gì spawn, event gì đang chạy, người chơi phải vào Discord của từng game rồi tìm bot báo tin.
- **Bloxkeep làm trung gian.** Chủ dự án tự thêm nguồn (một kênh của một server Discord) sau khi đã hỏi chủ nguồn. Bot Bloxkeep đọc tin, chuẩn hoá và đẩy lên app.
- **Hiện trong app, kiểu app.** Người dùng không bao giờ thấy hay đụng vào bot. Bot chỉ là đường ống ẩn.
- **Phải mở rộng ra nhiều game.** Thêm một game hay một bot mới là việc **cấu hình**, không phải viết code và không cần ra bản app mới.
- **Thử nghiệm đầu tiên:** Shindo Life (Shindo Spawn Alert), sau đó là Steal An Egg (SenZ V2).

## 1. Hiện trạng (đọc từ code, không đoán)

- **Bot là một Cloudflare Worker có một Durable Object `Gateway`** (`services/DiscordBot/worker/worker.js`).
  - Nó giữ kết nối Gateway với `INTENTS = 1 << 9` (dòng 57), tức là chỉ nhận sự kiện tin nhắn trong server. **Chưa có Message Content**, nên chưa đọc được embed.
  - Dữ liệu được lưu bằng SQLite của DO (`ctx.storage.sql`, các bảng `events`, `subs`, `online`…).
- **Đã có sẵn đường WebSocket từ app lên bot:**
  - Bên bot: `/remote/connect`, với `ctx.acceptWebSocket(server, [pc])` và socket ngủ được (hibernate).
  - Bên app: `RobloxManager/RemoteLink.cs` (`Url = wss://bloxkeep-bot…/remote/connect`), tự nối lại khi rớt.
  - Tin game sẽ dùng **cùng cách làm này** với một đường riêng.
- **App đã có thông báo Windows:** `ShellForm` gọi `NotifyIcon.ShowBalloonTip` (dòng 284).
- **Giao diện là React** (`ui/src`). Thanh bên (`Sidebar.tsx`) có các mục `nav.<view>`. Tin game thêm một mục mới ở đó.

## 2. Đường đi của một tin

```
Kênh bot của game (server khác)
   │  Discord "Follow" (hoặc webhook do chủ nguồn đặt)
   ▼
Kênh ẩn trong server Bloxkeep (mỗi nguồn một kênh)
   │  MESSAGE_CREATE (cần Message Content)
   ▼
Bot Bloxkeep: tra bản đồ của kênh → tin chuẩn → lưu 20 tin gần nhất
   │  WebSocket /feeds/connect (chỉ những app đang theo dõi game đó)
   ▼
App: trang "Tin game" + thông báo Windows
```

## 3. Nguồn

- **Một nguồn là một kênh, không phải một bot.** Ví dụ, SenZ V2 có hai kênh: "Event Alert" (bướm) và "Notifier" (trứng). Mỗi kênh là một nguồn và có bản đồ riêng. Một game có thể có nhiều nguồn.
- **Cách lấy tin, theo thứ tự ưu tiên:**
  1. **Follow.** Trong server của họ, bấm Follow kênh rồi chọn kênh ẩn của Bloxkeep. Chỉ làm được khi kênh đó là kênh Announcement.
  2. **Webhook.** Nếu kênh không phải Announcement, chủ dự án đưa chủ nguồn một webhook trỏ vào kênh ẩn, và nhờ họ bảo bot gửi thêm vào đó. Đi theo đường này thì bên Bloxkeep không phải làm gì thêm.
  3. Không có cách nào khác. Bloxkeep **không** dùng selfbot hay tài khoản người thật để đọc kênh.
- **Message Content:**
  - Thêm `1 << 15` vào `INTENTS`.
  - Bật "Message Content Intent" trong Developer Portal. Bot đang ở dưới 100 server nên không cần Discord duyệt.
  - Bot **chỉ đọc nội dung ở các kênh đã đăng ký làm nguồn** và bỏ qua mọi kênh khác.
- **Ghi công:** mỗi nguồn lưu tên bot, tên server và link mời (nếu chủ nguồn muốn). Thẻ trong app luôn ghi "Theo {bot}".

## 4. Ba kiểu thẻ

App chỉ biết vẽ ba kiểu thẻ. Mọi nguồn đều phải quy về một trong ba kiểu này.

| Kiểu | Khi nào | Dòng lớn trên thẻ | Ví dụ thật |
|---|---|---|---|
| `timed`: sự kiện có giờ | một món spawn, sắp spawn hoặc vừa spawn | "Spawn sau 4:12" / "Spawn 1 phút trước", hoặc con số chính nếu có | Shindo, trứng Steal An Egg |
| `live`: đang diễn ra | event, thời tiết đã bắt đầu | "Đang diễn ra · 3 phút", hoặc "Còn 12 phút" khi biết thời lượng | Butterfly Bloom |
| `list`: danh sách | shop/stock gồm nhiều dòng "tên – số" | tên shop + "đổi sau 3:40" | chưa có ví dụ thật |

**Các ô của mỗi thẻ đều giống nhau:**

- ảnh;
- tên lớn;
- dòng phụ (game · loại);
- dòng lớn;
- tối đa 3 nhãn;
- một dòng ghi chú ngắn;
- "Theo {bot}";
- nút "Vào game" (chỉ khi có link).

Màu "hiếm" (nền vàng) bật khi tin khớp điều kiện hiếm mà bản đồ đặt ra.

Kiểu `list` để sẵn trong thiết kế nhưng **chỉ làm khi có một bot thật dạng danh sách**.

## 5. Bản đồ của một nguồn

**Phần khác nhau giữa các game nằm hết ở đây.** Bản đồ là JSON, lưu trong bảng `feeds` của DO.

### 5.1 Chỗ lấy dữ liệu trong embed

| Ký hiệu | Lấy từ |
|---|---|
| `title` | tiêu đề embed |
| `field:Location` | field có tên "Location" |
| `line:Where` | dòng `**Where:** …` trong phần mô tả |
| `thumb` | ảnh thumbnail |
| `link:Join Game` | link nằm trên dòng hoặc field "Join Game" |

Khi so tên nhãn, bot bỏ qua emoji, dấu `*` và hoa/thường. Vì vậy "👟**Recommended Speed:**" vẫn khớp với `Recommended Speed`.

### 5.2 Phép biến đổi

| Phép | Làm gì |
|---|---|
| `cut:" - "` | bỏ phần sau dấu này (bỏ giờ dính ở tiêu đề) |
| `split:"·"` | tách thành nhiều nhãn |
| `time` | đọc `<t:1730700000…>` thành giờ thật |
| `number` | đọc `$1.3b/s`, `20B` thành số; hiểu k/M/B/T |
| `match:"(\w+) Egg"` | lấy phần trong ngoặc |

### 5.3 Ví dụ cho ba nguồn đã thấy

```json
{ "id": "shindo-spawn", "game": "Shindo Life", "kind": "timed",
  "name": "title", "icon": "thumb",
  "at": "field:Spawn Time|time",
  "chips": ["field:Location", "field:Rarity", "field:Category"],
  "rare": { "from": "field:Rarity|number", "max": 2 },
  "life": 15, "credit": "Shindo Spawn Alert" }

{ "id": "sae-egg", "game": "Steal An Egg", "kind": "timed",
  "name": "line:Egg", "icon": "thumb", "tier": "title|match:\"(\\w+) Egg\"",
  "at": "line:Spawned|time", "big": "line:Money|number",
  "chips": ["line:Location", "line:Recommended Speed"],
  "link": "link:Join Game", "life": 10, "credit": "SenZ V2" }

{ "id": "sae-event", "game": "Steal An Egg", "kind": "live",
  "name": "title|cut:\" - \"", "icon": "thumb",
  "at": "line:Started|time", "chips": ["line:Where|split:\"·\""],
  "note": "Butterfly Net", "lasts": null,
  "link": "link:Join Game", "life": 30, "credit": "SenZ V2" }
```

**Ý nghĩa từng khoá:**

- `life`: số phút tin còn giá trị. Quá thời gian này, thẻ mờ đi và app không thông báo nữa.
- `lasts`: thời lượng event (phút), khi chủ dự án biết. Có giá trị này thì thẻ đổi thành "Còn X phút".
- `note`: một dòng chữ cố định do chủ dự án viết, vì đoạn quảng cáo của bot bị bỏ.
- `big`: con số chính. Nó cũng dùng làm bộ lọc theo ngưỡng (mục 7.3).

**Những phần luôn bị bỏ:** dòng ping role ở đầu tin, nút "Add me to your server", đoạn văn mô tả, mục "Why…".

## 6. Lệnh `/feed` (chỉ chủ dự án)

| Lệnh | Làm gì |
|---|---|
| `/feed add kênh game` | Đọc tin mới nhất trong kênh, **tự đoán bản đồ** (tiêu đề → tên, thumbnail → ảnh, field/dòng có `<t:>` → giờ, các field/dòng còn lại → nhãn, link "Join" → nút), rồi hiện thẻ xem trước đúng như trong app. Bấm Lưu hoặc Sửa. |
| `/feed edit nguồn` | Mở bản đồ JSON trong một modal để sửa, xem trước lại rồi lưu. |
| `/feed test nguồn` | Chạy bản đồ trên 5 tin gần nhất và cho xem 5 thẻ. |
| `/feed list` | Liệt kê các nguồn kèm giờ nhận tin gần nhất và số tin không khớp. |
| `/feed remove nguồn` | Xoá nguồn. App sẽ không thấy game đó nữa nếu game không còn nguồn nào. |

- Thẻ xem trước trong Discord chỉ là embed mô phỏng thẻ app, để chủ dự án biết các ô được lấy đúng chưa.
- Đổi bản đồ có hiệu lực ngay với tin mới. App không cần cập nhật.

## 7. App: trang "Tin game"

### 7.1 Chỗ đặt

- Thêm một mục mới ở thanh bên, tên "Tin game" / "Game feeds".
- Trang này chỉ có hai phần, theo thứ tự:
  1. dải game đang theo dõi;
  2. dòng thẻ mới nhất.

### 7.2 Danh mục và theo dõi

- Bấm "Thêm game" để mở danh mục: mỗi game một ô (ảnh, tên, số nguồn). Bấm vào là theo dõi.
- **Chưa theo dõi game nào thì app không mở kết nối.** Như vậy người không dùng tính năng này không tốn gì.
- Danh mục lấy từ bot, nên game mới hiện ra mà không cần bản app mới.

### 7.3 Bộ lọc ("Báo cho tôi khi")

Mỗi game có bộ lọc riêng. Chỉ hiện những dòng mà nguồn của game đó có dữ liệu:

- **Tên:** chọn trong danh sách tên đã gặp (Pegasus, Butterfly Bloom…), hoặc "tất cả".
- **Loại:** từ `tier` (Eternal…).
- **Con số:** "từ 1B" khi nguồn có `big`.
- **Chỉ món hiếm:** khi nguồn có `rare`.
- **Ẩn tin cũ hơn:** mặc định bằng `life` của nguồn.

Bộ lọc quyết định **thông báo**. Dòng thẻ trong trang vẫn hiện mọi tin, còn tin không khớp bộ lọc thì nhạt đi.

### 7.4 Thẻ

- Vẽ đúng ba kiểu ở mục 4.
- Giờ đếm từng giây ngay trong app, nên bot không phải gửi lại tin.
- Trạng thái của kiểu `timed`: "Spawn sau m:ss", tới giờ thì đổi sang "Đã spawn · x phút trước", quá `life` thì thẻ mờ.
- Bấm "Vào game" là mở link của nguồn. Cách mở phụ thuộc vào loại link (xem mục 9).

### 7.5 Thông báo

- Tin mới khớp bộ lọc thì hiện thông báo Windows (dùng `ShowBalloonTip` có sẵn), có tên, dòng lớn và game. Bấm vào sẽ mở trang Tin game.
- Với kiểu `timed` có giờ trong tương lai: có thể bật thêm "Nhắc lúc spawn" cho từng game.

## 8. Tin chuẩn và đường truyền

**Một tin chuẩn trông như sau:**

```json
{ "id": "sae-egg:1302…", "source": "sae-egg", "game": "Steal An Egg", "kind": "timed",
  "name": "Pegasus", "tier": "Eternal", "icon": "https://…", "at": 1730700000,
  "big": { "text": "$1.3B/s", "value": 1.3e9 }, "chips": ["Angels", "20B"],
  "note": null, "link": "https://…", "rare": false, "life": 10, "credit": "SenZ V2", "raw": null }
```

- **Bot:**
  - Mỗi nguồn giữ 20 tin gần nhất trong SQLite.
  - Đường `/feeds/connect` dùng socket ngủ được, gắn tag theo game (`acceptWebSocket(server, ["game:Steal An Egg", …])`).
  - Có tin mới thì bot gửi cho `getWebSockets("game:…")`.
- **App:**
  - Một lớp `FeedLink.cs`, chép cách nối lại của `RemoteLink.cs`.
  - Khi kết nối, app gửi danh sách game đang theo dõi, và bot trả về 20 tin gần nhất của mỗi game.
  - Đổi danh sách theo dõi thì gửi lại danh sách đó, không cần kết nối lại.
- **Ảnh:** bot gửi đúng URL ảnh trong embed. Ảnh do Discord lưu (`cdn.discordapp.com/attachments`) có hạn dùng. Nếu một nguồn dùng loại ảnh này thì bot tải ảnh về và tự lưu một bản, vì app có thể mở trang sau vài giờ.

## 9. Lỗi, tin lạ và điều chưa biết

- **Tin không khớp bản đồ** (thiếu tên hoặc thiếu giờ):
  - Bot vẫn đẩy tin xuống với `kind: "raw"`, `raw` là tiêu đề cộng mô tả dạng chữ thường. App hiện một thẻ chữ đơn giản.
  - Bot đếm số tin lỗi, và khi một nguồn có 3 tin lỗi liên tiếp thì nhắn riêng cho chủ dự án.
- **Nguồn im lặng quá lâu** (ví dụ 24 giờ không có tin): `/feed list` đánh dấu nguồn đó. App vẫn hiện tin cũ kèm giờ thật của tin.
- **Chưa biết:**
  - Link "Join Game" của SenZ V2 dẫn tới đâu: thẳng vào Roblox (`roblox://`, trang game) hay qua web của họ. Phải mở thử trước khi làm nút. Nếu link qua web lạ thì nút chỉ mở trang game Roblox theo `placeId` mà chủ dự án ghi trong nguồn.
  - Kênh của Shindo và SenZ có phải kênh Announcement không (Follow được hay phải dùng webhook).
  - Có bot dạng `list` thật nào không.
- **Giới hạn:** tin chỉ nhanh bằng bot nguồn. Nếu bot nguồn báo trễ 19 phút thì app cũng trễ 19 phút; `life` sẽ cho thẻ đó mờ ngay.

## 10. Kiểm thử

- **Mẫu thật:** lưu JSON của 3 tin đã thấy (Shindo, trứng, bướm) vào `services/DiscordBot/test/feeds/`. Test chạy bản đồ trên các mẫu này và so với tin chuẩn mong đợi.
- **`features.mjs`:**
  - `/feed add` đoán bản đồ đúng từ mẫu;
  - một tin mới đi tới socket gắn tag đúng game và không tới game khác;
  - tin lỗi đi theo đường `raw`;
  - người không phải chủ dự án bị từ chối `/feed`.
- **UI:** một kịch bản `--only feeds`: theo dõi một game, nhận 3 tin giả, kiểm tra đồng hồ đếm, tin mờ khi quá `life`, và bộ lọc con số chặn thông báo.
- **Không chạy** toàn bộ test C#, website hay main-browser.

## 11. Thứ tự làm

1. Bot: Message Content, bảng `feeds`, đọc embed và bản đồ, `/feed add|test|list|remove`, có test với mẫu thật.
2. Chủ dự án nối nguồn Shindo (Follow hoặc webhook) và kiểm tra bằng `/feed test`.
3. Bot: `/feeds/connect` và gửi tin theo game.
4. App: `FeedLink.cs`, trang Tin game, thẻ `timed` và `live`, bộ lọc, thông báo.
5. Thêm hai nguồn SenZ V2.
6. Kiểu `list`: chỉ làm khi có bot thật.
