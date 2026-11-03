# Macro: mỗi nút một mục bước riêng ("Chức năng")

Ngày: 2026-11-04. Trạng thái: **bản thiết kế, chưa làm**. Chờ chủ dự án duyệt.

---

## 1. Hiện trạng (đọc từ code, không đoán)

### 1.1 Dữ liệu (`RobloxManager/Macro.cs`, `MacroPanel.cs`, `ui/src/macros.ts`)

- Một macro có **một danh sách bước phẳng** `Steps` (tối đa 500).
- Khối lồng nhau không lưu dạng cây. Chúng là các bước đánh dấu nằm trong danh sách:
  - `if` / `else` / `endIf`
  - `loop` / `loopUntil` / `endLoop`
  - `break`
- `MacroRules.Jumps()` tính bước nhảy một lần lúc lưu và lúc chạy, sâu tối đa 20 tầng.
- Có thể có **một** bước `recover` ("Nếu bị kẹt…"). Các bước sau nó chỉ chạy khi gỡ kẹt, và nó không được nằm trong khối.
- `Options` là những gì người chơi chỉnh trên bảng. Loại:
  - giá trị: `number`, `colour`, `key`, `point`, `list`
  - lựa chọn: `check` (công tắc, lưu 1/0), `choice` (chữ)
  - trang trí: `heading`
- Bước tham chiếu option bằng `"$o1"`, vùng bằng `"@a1"`.
- `MacroPanel` gồm `AutoShow`, `PlaceId` và `Tabs` (2–6 tab).
- `OnFail` (`stop` / `skip` / `restart` / `rejoin` / `alert`), `StuckMinutes`, `MaxMinutes` và `Trigger` đều **áp cho cả macro**.

### 1.2 Công tắc nối với bước như thế nào

- Công tắc **không sở hữu bước nào**. Nó chỉ là một giá trị 1/0.
- Muốn công tắc có tác dụng, người làm phải tự thêm khối `Nếu … {tên} đang bật` (test `var`) vào danh sách chính, rồi kẹp các bước vào giữa `Nếu` và `Hết nếu`.
- Editor chỉ dùng `readers()` (`MacroEditor.tsx:250`) để **chặn xoá** công tắc còn được đọc. Ngoài lúc đó, không chỗ nào cho thấy công tắc điều khiển bước nào.
- **Kết luận:** vấn đề người dùng gặp ("làm sao biết bước nào thuộc nút nào") là thật, và nằm ở cấu trúc chứ không chỉ ở cách hiển thị.

### 1.3 Engine (`MacroPlayer.cs`)

- **Một lần chạy** (`MacroRun`) = một macro × một tài khoản, với một con trỏ bước `Pc`.
- Hết danh sách bước thì lần chạy **kết thúc** (`done`). Macro "chạy mãi" là nhờ bọc mọi bước trong `Lặp lại 0 lần` (= mãi mãi); cả 3 mẫu có sẵn đều làm vậy.
- **Chỉ một cửa sổ có input tại một thời điểm** (khoá `InputTurn`). Các lần chạy thay phiên nhau theo lượt:
  - Một lượt dài tối đa 8 s khi có cửa sổ khác đang chờ.
  - Lượt nhường ngay khi gặp chờ dài (trên 1,5 s) hoặc chờ hình.
  - Sau 30 s, lượt nhường cho anti-AFK.
- Phím đang giữ được thả khi hết lượt và giữ lại khi lượt sau bắt đầu (`Held`). `Pairs()` bắt mỗi phím đã giữ phải được thả.
- Đổi công tắc trên bảng áp **ngay** vào bản sao macro của các lần chạy đang chạy (`AppController.Panels.cs:152`). Giá trị mới có tác dụng ở lần kế tiếp một bước đọc nó.
- Cửa sổ đổi tiến trình (rejoin, crash) → `StartOver`: chạy lại từ bước 1, không giữ phím nào.
- **Rủi ro có sẵn (đọc từ code, chưa thử thật):** `Queue()` chỉ chặn trùng *cùng macro + cùng tài khoản*. Hai macro **khác nhau** trên cùng một tài khoản sẽ chạy xen kẽ theo lượt trên cùng một cửa sổ. Đây chính là kiểu xung đột input mà yêu cầu muốn tránh.

### 1.4 Những thứ chưa có

- Nhóm bước có tên.
- Gọi một nhóm bước từ chỗ khác (chương trình con).
- Chạy theo hẹn giờ bên trong một lần chạy.
- Ưu tiên giữa các phần.
- Trạng thái riêng cho từng công tắc trên bảng (bảng chỉ có trạng thái chung của macro).

---

## 2. Các mô hình đã xét

Mọi lựa chọn bị ràng buộc bởi một sự thật: **một cửa sổ Roblox chỉ có một chuột và một bàn phím**. Hai nhóm bước không thể thực sự chạy song song trên cùng một tài khoản. Vì vậy bất kỳ mô hình "song song" nào cuối cùng cũng phải xếp hàng các bước, và chỗ xếp hàng chính là nơi sinh ra xung đột.

| # | Mô hình | Lấy từ | Ưu | Nhược với Bloxkeep | Kết luận |
|---|---|---|---|---|---|
| A | Mỗi nút một kịch bản chạy song song | Scratch ("khi bấm cờ"), Node-RED | Dễ hiểu lúc đầu | Hai kịch bản tranh chuột/phím, xen nhau giữa chừng (bấm Bán khi đang quăng cần). Kết quả phụ thuộc thời điểm, không đoán trước được | Loại |
| B | Ngắt ưu tiên: nút quan trọng cắt ngang vòng chính rồi quay lại chỗ cũ | Luồng ngắt của AutoHotkey, BPMN *interrupting event sub-process* | Phản ứng nhanh | Quay lại giữa chừng thì game đã khác (dây câu đã đứt, menu đã đóng). Phải lưu cả trạng thái chờ và phím giữ. Người mới khó lường | Để giai đoạn sau, chỉ là lựa chọn nâng cao |
| C | Cây hành vi: mỗi nhịp chọn nhánh ưu tiên cao nhất đang hợp lệ | AI trong game (Behavior Tree, Selector/Sequence) | Xác định, mạnh | Mất khái niệm "vòng chính" quen thuộc, khó với người mới | Loại cho người mới, chỉ lấy ý "thứ tự cố định" |
| D | Máy trạng thái | Statecharts (XState) | Chặt chẽ | Quá trừu tượng cho người chơi Roblox | Loại |
| E | Bước "Chạy chức năng X" do người làm tự đặt | Chương trình con, Tasker "Perform Task" | Linh hoạt, xác định | Người mới phải hiểu "gọi", và quên đặt thì nút không có tác dụng | Giữ làm **lựa chọn nâng cao** |
| F | **Điểm kiểm tra cố định**: vòng chính chạy hết một vòng, đến điểm an toàn thì xét các chức năng đang bật theo thứ tự cố định, chạy trọn từng cái rồi quay lại | Vòng quét PLC (scan cycle), BPMN *non-interrupting event sub-process*, Home Assistant `mode: queued` | Không bao giờ xen giữa hai nhóm bước. Thứ tự đoán trước được. Người mới không phải đặt gì | Phản ứng chậm tối đa một vòng | **Chọn làm mặc định** |

Ba ý được giữ lại từ các hệ khác:

- **Home Assistant**: mỗi tự động hoá ghi rõ chế độ khi bị gọi lúc đang chạy (`single`, `queued`, `restart`, `parallel`). Bloxkeep cố định là `queued` theo thứ tự, không cho `parallel`.
- **Tasker**: một hồ sơ = điều kiện + việc làm. Tương ứng ở đây: chức năng = **khi nào** + **các bước**.
- **PLC**: thứ tự quét cố định từ trên xuống. Chức năng nằm trên được xét trước.

---

## 3. Thiết kế được chọn

### 3.1 Khái niệm cho người dùng

- **Vòng chính**: các bước hiện có. Nếu không thêm chức năng nào thì macro y như bây giờ.
- **Chức năng**: một nút bật/tắt trên bảng, kèm **các bước riêng** và **khi nào chạy**. Ví dụ: "Tự bán cá", "Tự ăn", "Tự nâng cấp".
- **Điểm dừng an toàn**: chỗ Bloxkeep xét các chức năng. Người dùng không phải đặt nó.
- **Nút giá trị** (số, phím, màu, điểm, danh sách tích chọn) vẫn như cũ. Chúng là dữ liệu cho bước đọc, không có bước riêng.
- **Công tắc tự do** hiện có (dùng với khối Nếu) vẫn giữ cho người dùng nâng cao. Có thêm nút "Đổi thành chức năng" ở giai đoạn 2.

### 3.2 Dữ liệu (đề xuất)

```jsonc
// Macro: thêm một trường, các trường cũ không đổi
"features": [
  {
    "id": "f1",                 // do trang tạo, không hiện ra
    "option": "o3",             // công tắc trên bảng (kind "check") mà chức năng sở hữu
    "when": {                   // cả hai trường đều có thể bỏ trống; có cả hai thì phải thoả cả hai
      "everyMs": 600000,        // không quá 1 lần mỗi 10 phút cho mỗi tài khoản; null = mỗi vòng
      "test": { "kind": "image", "image": "…", "region": "@a2" }  // MacroTest dùng lại nguyên bản: hình / chữ / màu / bộ đếm
    },
    "onFail": "skip",           // "skip": bỏ lần này, về vòng chính | "macro": theo OnFail của macro
    "steps": [ /* MacroStep[], cùng định dạng với vòng chính */ ]
  }
]
```

Quy tắc kiểm tra (thêm vào `MacroRules.Check`):

- Tối đa **10** chức năng.
- Tổng số bước của vòng chính và mọi chức năng không quá 500. Giới hạn ảnh 4 MB vẫn tính chung.
- Mỗi danh sách bước tự qua `Jumps()` và `Pairs()` riêng: khối phải đóng trong chính chức năng đó, phím giữ phải thả trong chính chức năng đó.
- `recover` chỉ được có trong vòng chính.
- Trong chức năng, `stop` / `rejoin` vẫn tác động lên **cả lần chạy**, giống hiện nay.
- `option` phải trỏ tới một option `check` có thật. Xoá chức năng thì xoá luôn công tắc, và ngược lại (một thao tác, có hoàn tác).
- Chuỗi JSON dư (`Extra`) giữ nguyên như mọi chỗ khác, nên bản cũ của Bloxkeep đọc macro mới mà không vỡ. Bản cũ chỉ bỏ qua `features`.

### 3.3 Cách chạy (execution model)

Thêm vào `MacroRun`:

- `Section`: `null` là vòng chính, còn lại là id chức năng.
- `MainPc`: chỗ quay về trong vòng chính.
- `Due`: hàng chờ các chức năng sẽ chạy ở điểm dừng này.
- `LastRan[featureId]`: dùng cho `everyMs`.
- Jumps tính sẵn cho từng danh sách.

**Điểm dừng an toàn** của vòng chính là một trong hai chỗ:

1. Cuối danh sách vòng chính (trước `recover` nếu có).
2. Mỗi lần đi qua `Hết lặp` của một khối Lặp **ở tầng ngoài cùng** (tầng 0).

Chỗ thứ hai giúp ba mẫu có sẵn và macro cũ (bọc trong `Lặp mãi`) dùng được ngay, không phải sửa.

Tại điểm dừng an toàn:

1. Nếu vòng chính đang giữ phím (`Held` khác rỗng) thì **không xét**, chờ điểm dừng sau. Editor cảnh báo trước (xem 4.6).
2. Duyệt các chức năng **theo thứ tự trên bảng**. Một chức năng vào `Due` khi thoả cả ba điều kiện:
   - công tắc đang bật,
   - đã đủ `everyMs` kể từ `LastRan`,
   - `test` đúng (nhìn ảnh hiện tại, chỉ một lần chụp cho tất cả).
3. Lần lượt chạy **trọn** từng chức năng trong `Due`, mỗi cái một lần. Không chức năng nào cắt ngang chức năng khác.
4. Hết `Due` thì quay về `MainPc`: bước sau `Hết lặp` hoặc đầu vòng chính. Nếu vòng chính đã hết mà không có Lặp mãi: có chức năng đã chạy thì đi tiếp vòng mới, nếu không thì lần chạy kết thúc như cũ.

Các tính chất bảo đảm:

- **Không xen kẽ, không race trên một tài khoản:** mỗi lần chạy chỉ có một con trỏ bước, nên mọi bước của mọi chức năng xếp thành một hàng duy nhất.
- **Xác định:** cùng trạng thái công tắc, cùng đồng hồ và cùng hình thì luôn ra cùng thứ tự.
- **Không bỏ đói:** mỗi điểm dừng chạy *mọi* chức năng đã sẵn sàng, nên chức năng nằm dưới vẫn chạy trong cùng điểm dừng, chỉ sau những cái nằm trên.
- **Giữa các tài khoản:** cơ chế lượt và khoá `InputTurn` hiện có không đổi. Đồng hồ `everyMs` tính riêng cho từng tài khoản.
- **Hai macro trên cùng một tài khoản** (rủi ro ở 1.3): đề xuất chặn khi Queue, kèm thông báo "Tài khoản này đang chạy macro X". Chức năng làm cho việc chạy hai macro cùng lúc không còn cần thiết. Đây là sửa riêng, có thể làm trước.

Các trường hợp biên:

- **Tắt công tắc khi chức năng đang chạy:** dừng ở **ranh giới bước kế tiếp**, thả mọi phím chức năng đang giữ, rồi về vòng chính như khi chức năng chạy xong. Không chạy nốt, vì người dùng tắt là muốn tắt.
- **Bật công tắc giữa vòng:** được xét ở điểm dừng an toàn kế tiếp.
- **Chức năng thất bại** (chờ quá hạn, không thấy hình):
  - `onFail: "skip"` (mặc định): bỏ lần này, đặt `LastRan = bây giờ` để không lặp lại ngay, rồi về vòng chính.
  - `"macro"`: theo `OnFail` của cả macro.
- **Kẹt (`StuckMinutes`):** thời gian trong chức năng vẫn tính. Một lần thấy hình trong chức năng cũng được tính là có tiến triển.
- **Rejoin hoặc crash (`StartOver`):** về đầu vòng chính và xoá `Due`. **Giữ** `LastRan`, để bán mỗi 10 phút không bị dồn lại sau mỗi lần rejoin.
- **Thời gian tối đa (`MaxMinutes`):** tính chung cả lần chạy, như hiện nay.
- **Lượt nhường giữa chức năng** (chờ dài, anti-AFK): giống vòng chính. `Section` và `Pc` được giữ, lượt sau chạy tiếp đúng chỗ.

### 3.4 Lựa chọn nâng cao (giai đoạn 2, không làm trước)

1. **"Chạy chức năng ở đây"** (mô hình E): một bước đặt trong vòng chính. Khi đã có bước này, chức năng ghi "Chạy khi: khi được gọi" và không tự xét ở điểm dừng an toàn nữa.
2. **"Khẩn"** (mô hình B):
   - Xét ở *mọi ranh giới bước* khi không giữ phím, chứ không chỉ ở điểm dừng an toàn.
   - Chạy xong thì **làm lại vòng chính từ đầu**, không quay lại giữa chừng (an toàn hơn khi game đã thay đổi).
   - Dùng cho các việc như "Bị đánh thì chạy trốn".
3. **Lựa chọn có bước riêng cho từng mục:** ví dụ "Đảo: A / B / C", mỗi mục có một danh sách bước, mục đang chọn sẽ chạy. Dữ liệu là `cases: { "A": steps, … }` trong một feature mà `option` là kiểu `choice`.

---

## 4. UX

### 4.1 Bố cục editor

```
┌ Các bước ───────────────────────────────────────────┐
│ [Vòng chính] [Tự bán cá ●] [Tự ăn ○] [+ Chức năng]  │  ← thanh chọn nhóm (● bật, ○ tắt)
├─────────────────────────────────────────────────────┤
│ Tự bán cá                               [ bật ]     │  ← tên = nhãn trên bảng, sửa tại chỗ
│ Chạy khi: [mỗi 10 phút ▾]  và  [thấy hình… ▾]       │  ← câu, giống cách ghi bước hiện nay
│ Nếu lỗi: [bỏ qua lần này ▾]                         │
│ ─────────────────────────────────────────────────── │
│ 1  Bấm vào nút Bán                                  │  ← cùng StepRow như vòng chính:
│ 2  Chờ 1 giây                                       │     kéo thả, thêm, nhân đôi, xoá, ghi chú,
│ 3  Bấm phím Escape                                  │     If/Repeat, Thử
│ + Thêm bước                                         │
└─────────────────────────────────────────────────────┘
```

- **Chưa có chức năng nào:** thanh chọn nhóm **ẩn**, editor y như hôm nay. Chỉ có một nút nhỏ "+ Chức năng" cạnh "+ Thêm bước" (progressive disclosure).
- Tab "Vòng chính" hiện **dấu điểm dừng an toàn** ở cuối danh sách và trên `Hết lặp` ngoài cùng: một dòng mảnh có chữ "Chức năng đang bật chạy ở đây". Người dùng nhìn thấy *khi nào* chức năng chen vào.
- Số bước đánh lại từ 1 trong mỗi chức năng. Lỗi và thông báo ghi kèm tên: "Tự bán cá · bước 2: …".

### 4.2 Tạo, sửa, xoá, sắp xếp

- **Tạo:**
  - "+ Chức năng" tạo cùng lúc công tắc trên bảng và nhóm bước rỗng, rồi mở nhóm đó.
  - Nhãn mặc định là "Chức năng 1", sửa ngay tại chỗ.
  - Trên bảng xem trước, "+ Công tắc" hỏi thêm: "Có bước riêng?". Chọn Có thì tạo chức năng, chọn Không thì tạo công tắc tự do như cũ.
- **Sửa bước:** dùng nguyên `StepRow` và `StepBody`, không viết lại.
- **Chuyển bước giữa các nhóm:**
  - Menu bước có "Chuyển sang… ▸ Vòng chính / Tự bán cá / …". Khối If/Repeat được chuyển cả khối.
  - Không kéo thả giữa hai tab: kéo qua tab dễ thả nhầm.
- **Sắp thứ tự chức năng:** kéo trên thanh chọn nhóm, hoặc ↑↓ trên bảng xem trước. Thứ tự này **cũng là thứ tự chạy**, và dòng chú thích dưới thanh ghi rõ "Chạy từ trái sang phải".
- **Xoá chức năng:**
  - Hỏi xác nhận nếu còn bước: "Xoá Tự bán cá và 3 bước?".
  - Xoá cả công tắc, có hoàn tác như khi xoá khối hiện nay.

### 4.3 Bảng của người chơi

Mỗi chức năng là một dòng công tắc, như hiện nay. Thêm một dòng trạng thái nhỏ bên dưới khi macro đang chạy:

| Trạng thái | Chữ |
|---|---|
| Tắt | (không hiện gì) |
| Bật, chờ điểm dừng | Chờ hết vòng |
| Bật, chờ hẹn giờ | Lần sau: 6 phút |
| Bật, chờ hình | Chờ thấy hình |
| Đang chạy | Đang chạy · bước 2/3 |
| Lần trước lỗi | Lần trước: không thấy nút Bán |

Chạy nhiều tài khoản thì hiện trạng thái của tài khoản đang có lượt, hoặc "2/3 tài khoản đang chạy".

### 4.4 Tránh cấu hình sai (editor báo trước khi lưu)

- Chức năng **không có bước:** cảnh báo "Tự ăn chưa có bước nào".
- Vòng chính **không có điểm dừng an toàn nào đi tới được** (ví dụ: một `Lặp mãi` nằm trong khối Nếu, hoặc một Lặp-đến-khi không bao giờ thoát): "Chức năng sẽ không bao giờ chạy: vòng chính không có lúc dừng". Kèm nút sửa "Bọc vòng chính trong Lặp mãi".
- **Giữ phím qua điểm dừng an toàn** (Giữ W trước Lặp, Thả sau Lặp): "Đang giữ W ở cuối vòng nên chức năng không chen vào được".
- Chức năng có `Chạy khi: thấy hình` nhưng chưa cắt hình: báo lỗi như bước hiện nay.
- Hai chức năng **có cùng lúc giữ và thả phím** là việc của mỗi danh sách, do `Pairs()` riêng bắt.

### 4.5 Theo dõi và debug

- **Thử** trong editor:
  - Tab Vòng chính: chạy cả macro như hiện nay.
  - Tab chức năng: "Thử chức năng này" chỉ chạy riêng nhóm bước đó một lần.
  - Dòng đang chạy được tô sáng như `runningAt` hiện nay, kèm `Section` để tô đúng tab.
- **Nhật ký** (activity) ghi:
  - "Tự bán cá: xong (12 giây)"
  - "Tự bán cá: bỏ qua, không thấy nút Bán (bước 1)"
  - "Tự ăn: chưa đến giờ" chỉ hiện trong chế độ chi tiết.
- **Thẻ macro** trong danh sách hiện "3 chức năng · 2 bật".

### 4.6 Người mới và người dùng nâng cao

- **Người mới:**
  - Thêm chức năng, kéo bước vào, chọn "Chạy khi: mỗi vòng" là xong.
  - Không phải viết Nếu, không phải đặt điểm gọi.
- **Người dùng nâng cao:**
  - Kết hợp hẹn giờ với điều kiện hình hoặc bộ đếm.
  - Dùng bộ đếm giữa vòng chính và chức năng (biến dùng chung theo tài khoản, như hiện nay).
  - Giai đoạn 2: "Chạy chức năng ở đây", "Khẩn", lựa chọn có bước riêng cho từng mục.

---

## 5. Mô phỏng (theo đúng luật ở 3.3)

Macro câu cá. Vòng chính bọc trong `Lặp mãi`: Quăng cần → Chờ cá cắn (≤ 20 s) → Kéo (follow) → Hết lặp. Mỗi vòng mất khoảng 25 giây.

| Chức năng | Thứ tự | Chạy khi | Bước |
|---|---|---|---|
| Tự ăn | 1 | thấy màu đỏ ở thanh đói | 4 bước |
| Tự bán cá | 2 | mỗi 10 phút | 3 bước |

**S1. Một tài khoản, cả hai bật.**

- t = 0: vòng 1 bắt đầu.
- t = 25 s: tới `Hết lặp` (điểm dừng an toàn). Chụp một hình: thanh đói chưa đỏ. Tự bán cá chưa từng chạy nên `LastRan` rỗng, coi như đến hạn. `Due = [Tự bán cá]`. Chạy 3 bước, xong ở t ≈ 31 s, đặt `LastRan = 31 s`. Quay lại `Lặp`.
- t = 31 s đến 10 phút 31 s: mỗi điểm dừng chỉ xét Tự ăn.
- t ≈ 7 phút: thanh đói đỏ giữa vòng. Không ngắt. Ở điểm dừng kế (≤ 25 s sau), Tự ăn chạy.
- Độ trễ tối đa bằng **một vòng chính**. Đây là cái giá có chủ đích để đổi lấy việc không bao giờ bấm Bán khi đang kéo cá.

**S2. Cả hai cùng đến hạn ở một điểm dừng.** `Due = [Tự ăn, Tự bán cá]`, theo thứ tự trên bảng. Tự ăn chạy trọn, rồi Tự bán cá chạy trọn, rồi về vòng chính. Lặp lại 100 lần vẫn ra đúng thứ tự này.

**S3. Tắt Tự bán cá khi nó đang ở bước 2/3.** Bước 2 (Chờ 1 giây) dừng ở lát chờ 250 ms kế tiếp. Không còn phím giữ nên không phải thả gì. `LastRan` vẫn được đặt. Lần chạy về bước sau `Hết lặp`, vòng chính tiếp tục.

**S4. Vòng chính giữ W suốt vòng** (Giữ W → Lặp mãi { … } → Thả W). Ở `Hết lặp`, `Held = {W}`, nên không xét chức năng nào, ở mọi vòng. Editor đã cảnh báo lúc lưu (4.4). Nếu người dùng vẫn lưu, bảng hiện "Chờ hết vòng" mãi, và nhật ký ghi một lần "Đang giữ W nên chức năng chưa chạy được".

**S5. Tự bán cá không thấy nút Bán** (`onFail: skip`). Bước 1 chờ quá hạn 10 s, bỏ lần này. `LastRan = bây giờ`, nên 10 phút sau mới thử lại, không lặp mỗi vòng. Nhật ký ghi lỗi, bảng hiện "Lần trước: không thấy nút Bán". Vòng chính không bị ảnh hưởng.

**S6. Ba tài khoản cùng chạy.** Mỗi tài khoản có `Pc`, `Section`, `Due` và `LastRan` riêng. Các lượt vẫn thay phiên như hiện nay, và chỉ một cửa sổ có input tại một thời điểm. Tài khoản 1 đang Tự bán cá thì tài khoản 2 vẫn được lượt khi tài khoản 1 chờ dài. Không có chia sẻ trạng thái chức năng giữa các tài khoản; chỉ giá trị công tắc là chung, vì nó thuộc macro.

**S7. Rejoin giữa Tự ăn.** Tiến trình cửa sổ đổi, `StartOver` chạy: về đầu vòng chính, xoá `Due`. `LastRan` được giữ, nên Tự bán cá không chạy lại ngay chỉ vì rejoin. Tự ăn sẽ được xét lại ở điểm dừng đầu tiên.

**S8. Bật Tự ăn giữa vòng.** Đổi công tắc áp ngay vào bản sao macro (cơ chế hiện có). Tự ăn được xét ở điểm dừng kế tiếp. Không có chuyện chạy nửa chừng.

---

## 6. Rủi ro và điều chưa chắc

| Rủi ro | Mức | Cách xử lý |
|---|---|---|
| Hai macro khác nhau trên cùng tài khoản chạy xen kẽ (có sẵn, đọc từ `Queue`) | Cao | Chặn khi Queue (sửa riêng, trước tiên). Cần một test MacroSuite chứng minh trước khi sửa |
| Vòng chính không có `Lặp` ngoài cùng, chỉ có Lặp lồng sâu | Trung bình | Điểm dừng vẫn có ở cuối danh sách. Editor cảnh báo nếu không đi tới được |
| Độ trễ một vòng là quá lâu với vòng chính rất dài (vài phút) | Trung bình | Ghi rõ trong UI. "Khẩn" ở giai đoạn 2 |
| Nhìn ảnh ở điểm dừng (test `image`/`text`) làm vòng chậm thêm | Thấp | Một lần chụp cho mọi chức năng. OCR chỉ chạy khi có chức năng dùng chữ |
| Macro mới mở bằng Bloxkeep cũ: chức năng bị bỏ qua lặng lẽ | Thấp | Ghi trong What's new. Bản cũ vẫn chạy vòng chính bình thường |
| Giới hạn 500 bước và 4 MB ảnh, nay chia cho nhiều nhóm | Thấp | Tính tổng, báo lỗi giống hiện nay |

**Chưa kiểm chứng trên Roblox thật:**

- Độ trễ khi chụp và nhìn ảnh ở mỗi điểm dừng.
- Cảm nhận của người dùng về độ trễ một vòng.

Cả hai cần đo bằng FakeDesk (thời gian logic) và một lần chạy thật do chủ dự án làm.

---

## 7. Kiểm chứng và thứ tự làm

1. **Test trước (MacroSuite, FakeDesk có đồng hồ riêng, không mở cửa sổ):** viết S1–S8 thành test có nhịp thời gian, kiểm tra thứ tự bước được bấm. Viết thêm test cho rủi ro hai macro trên một tài khoản.
2. **Engine và kiểm tra dữ liệu:**
   - `Macro.Features`, `MacroRules.Check` (Jumps/Pairs từng danh sách).
   - `MacroRun.Section/MainPc/Due/LastRan`, điểm dừng an toàn, tắt giữa chừng.
3. **Editor:**
   - Thanh chọn nhóm, đầu mục "Chạy khi / Nếu lỗi".
   - Dùng lại `StepRow` cho mỗi danh sách, "Chuyển sang…".
   - Dấu điểm dừng an toàn, cảnh báo ở 4.4.
4. **Bảng:** trạng thái từng chức năng (4.3).
5. **Nhật ký và Thử chức năng.**
6. **UI tests:** chỉ chạy các kịch bản bị chạm (`--only macro`), cùng ảnh chụp editor có và không có chức năng.

Giai đoạn 2, làm sau khi giai đoạn 1 được dùng thật: "Chạy chức năng ở đây", "Khẩn", lựa chọn có bước riêng cho từng mục, "Đổi công tắc thành chức năng".
