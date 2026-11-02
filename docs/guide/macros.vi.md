[English](macros.md)

# Macro (Beta)

Macro là một danh sách bước ngắn, ví dụ "Giữ chuột trái 0.6 giây" hay "Chờ đến khi màu hiện trong Vùng cá", mà Bloxkeep chạy trong các cửa sổ Roblox. Mỗi macro có thể có một **bảng** nhỏ cạnh game, gồm nút Chạy và những cài đặt bạn cho người chơi đổi.

> [!NOTE]
> Macro đang ở bản Beta: có thể chạy khác nhau trên mỗi máy. Gặp lỗi thì báo ở **Cài đặt › Bloxkeep › Gửi góp ý**.

## Macro điều khiển Roblox thế nào

Bloxkeep đưa từng cửa sổ Roblox lên trước trong giây lát, gửi phím và cú bấm, rồi trả lại cửa sổ bạn đang dùng. Mỗi lần một cửa sổ.

- Macro tạm dừng khi bạn dùng chuột hay bàn phím, khi có ứng dụng toàn màn hình và khi máy khóa, rồi tự chạy tiếp.
- Phải tắt gõ tiếng Việt (Unikey và tương tự). Dòng của macro sẽ báo và chờ.
- **Ctrl+Alt+0** dừng mọi macro, ở bất kỳ ứng dụng nào. **Khóa app** cũng dừng.
- Bloxkeep không đọc bộ nhớ Roblox, không sửa file hay can thiệp vào Roblox. Nó chỉ nhìn cửa sổ, như bạn.

## Tạo macro

1. Mở tab **Macro**, bấm **Thêm macro**: **Ghi thao tác**, **Từ mẫu có sẵn**, **Trống** hoặc **Nhập từ file**.
2. Mỗi bước là một câu. Bấm vào bước để mở và đổi giá trị ngay trong câu. **Thêm bước** hiện các bước hay dùng trước, còn lại ở **Bước khác**.
3. Bấm **Chạy thử**: macro chạy một lần trên một cửa sổ và cho biết từng bước ra sao.
4. Bấm **Lưu**, rồi **Chạy** ở dòng của macro. Dòng đó cho biết từng tài khoản đang làm gì.

Mẫu có sẵn có danh sách **Cần làm trước khi chạy**, ví dụ "Đặt Vùng thanh bằng Sửa trên game".

## Điểm và vùng

- Bước bấm chuột có một **điểm** đánh số. **Vùng** là khung trên game nơi một bước tìm kiếm ("trong Vùng cá").
- **Sửa trên game** hiện điểm và vùng trên chính cửa sổ Roblox. Kéo chúng ở đó rồi bấm **Xong**.
- Chưa mở cửa sổ? **Chụp ảnh** hiện điểm và vùng trên ảnh cửa sổ để kéo ngay tại đó.
- Chọn vùng trong câu của bước, hoặc **+ Vùng mới**. Điểm và vùng tự theo khi đổi cỡ cửa sổ.

## Người chơi được đổi

Mở một bước và tích **Người chơi được đổi** dưới câu của bước (phím, thời gian giữ, màu). Giá trị đó sẽ hiện trên bảng của macro, ở phần **Người chơi thấy**. Bấm vào tên để đổi tên hoặc đổi thứ tự. Bộ đếm mà macro cộng vào (ví dụ "fish") cũng hiện trên bảng.

Tích **Điểm** ở bước bấm chuột để người chơi tự chọn chỗ bấm: bảng có nút **Đặt**, bấm rồi bấm vào game. **Thêm** ở phần **Người chơi thấy** để thêm công tắc (bật/tắt), lựa chọn (danh sách) hoặc tiêu đề; bước **Nếu** đọc được công tắc và lựa chọn. **Thêm tab** để chia bảng dài thành nhiều tab.

**Danh sách tích chọn** là danh sách chữ (ví dụ các enchant) để người chơi tích trên bảng. Bước **Chờ chữ** hoặc **Nếu** có thể đọc "chữ được tích trong" danh sách đó: chữ nào được tích cũng tính, chữ dài đọc sai một ký tự vẫn nhận, và tên dài hơn trong danh sách ("Blessed Song") không bị tính nhầm thành tên ngắn ("Blessed").

**Bấm vào màu** có thêm cách bấm **từng chấm một lần, khi nó hiện**: bấm khi chấm đã hiện đủ thời gian bạn đặt (vòng tròn hay mục tiêu hiện lần lượt).

Mở bảng bằng nút **Bảng** ở dòng của macro, trang **Macro** của biểu tượng khay, hoặc Ctrl+K "Hiện bảng". **Tự hiện bảng** mở bảng khi có tài khoản vào game bạn chọn.

> [!IMPORTANT]
> Các bước cần bấm liên tục, như **Giữ thanh trúng đích**, giữ cửa sổ ở trước suốt lúc chạy. Game kiểu này chạy từng tài khoản một, các tài khoản khác chờ lượt.

## Khi macro bị kẹt

Một bảng bật lên hay một lần cá không cắn có thể làm macro chờ mãi.

- Mở dòng dưới tên macro. Ở **Nếu chờ mãi không thấy hoặc bị kẹt**, chọn **Làm lại từ đầu**.
- **Tính là kẹt** là bao lâu không thấy gì (ví dụ 5 phút) thì làm việc đó.
- Ở **Bước khác**, thêm **Khi bị kẹt…**. Bước này nằm cuối; đặt sau nó các bước gỡ kẹt, như đóng bảng bật lên. Chúng chạy trước khi macro làm lại từ đầu.

Làm lại 5 lần liền mà vẫn không thấy gì thì macro dừng trên cửa sổ đó.

## Giới hạn

Tối đa 100 macro, mỗi macro 500 bước, 30 mục cho người chơi, 6 tab và 8 vùng. Một lần chạy dừng sau 10 phút, trừ khi bạn đổi **Dừng sau** (mở dòng dưới tên macro).

## Hỏi đáp

**Hỏi: Macro chạy được khi tôi đang dùng máy không?**
Đáp: Không. Macro tạm dừng khi bạn dùng máy và chạy tiếp khi bạn ngừng.

**Hỏi: Macro có gõ mã đã lưu của tôi vào game không?**
Đáp: Không. Macro chỉ gõ những gì bạn ghi trong bước của nó.

**Hỏi: Chạy thử báo "chưa thấy".**
Đáp: Cho thứ đó hiện trên màn hình (ví dụ chờ cá cắn), rồi bấm **Chạy thử** lại. Vẫn chưa thấy thì dùng **Sửa trên game** để đặt vùng lên nó.

**Hỏi: Một bước bấm sai chỗ.**
Đáp: Bấm **Sửa trên game** và kéo điểm đánh số của bước đó tới đúng chỗ.
