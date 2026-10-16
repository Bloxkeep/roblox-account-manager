[English](backup-and-security.md)

# Sao lưu và bảo mật

Mật khẩu chủ và những gì không lấy lại được, sao lưu tự động, khôi phục, chuyển sang máy mới, nhập từ file Bloxkeep khác, dữ liệu nằm ở đâu, những gì rời khỏi máy và Ẩn tên tài khoản.

<img src="../screenshots/vi/settings-backup.png" width="760" alt="Cài đặt › Sao lưu & dữ liệu: Sao lưu vào thư mục với thư mục OneDrive đã chọn và nút Sao lưu ngay, Khôi phục, Nhập tài khoản từ file dữ liệu, và nơi lưu dữ liệu">

## Làm nhanh

1. Mở **Cài đặt › Sao lưu & dữ liệu**.
2. Ở dòng **Sao lưu vào thư mục**, bấm **Chọn…** và chọn một thư mục nằm ngoài thư mục dữ liệu của Bloxkeep. Nên chọn thư mục Google Drive hoặc OneDrive: **Dùng Google Drive** và **Dùng OneDrive** hiện ra khi Bloxkeep tìm thấy.
3. Bấm **Sao lưu ngay**. Dòng bên dưới sẽ ghi "Sao lưu gần nhất: …".
4. Sang máy mới, ở màn hình đầu tiên bấm **Khôi phục từ bản sao lưu** và mở khóa bằng mật khẩu dùng lúc sao lưu.

## Mật khẩu chủ

Lần đầu mở, Bloxkeep yêu cầu **Đặt mật khẩu chủ**. Mọi thứ bạn lưu trong Bloxkeep đều được mã hóa bằng mật khẩu này.

- Ít nhất 6 ký tự và không được chỉ toàn dấu cách. Thanh **Yếu / Tạm được / Mạnh** chỉ để tham khảo.
- Mỗi lần khởi động, Bloxkeep hiện **Chào mừng trở lại**: nhập mật khẩu chủ rồi bấm **Mở khóa**. Nếu đang bật Caps Lock, app sẽ nhắc.
- Muốn đổi mật khẩu: **Cài đặt › Bảo mật**. Các bản sao lưu làm trước khi đổi vẫn chỉ mở được bằng mật khẩu cũ.
- **Tự khóa** (**Cài đặt › Bảo mật**): khi không ai dùng bàn phím hay chuột của máy trong khoảng thời gian bạn chọn (từ 5 phút tới 2 giờ), Bloxkeep hiện lại màn hình này. Roblox, tự vào lại game, preset hẹn giờ và Discord vẫn chạy phía sau. Còn nút **Khóa** thì khóa hẳn Bloxkeep: các việc đó tạm dừng cho tới khi bạn mở khóa.

> [!WARNING]
> Mật khẩu chủ không lấy lại được. Bloxkeep không lưu nó ở đâu cả, nên quên là mất hết tài khoản đã lưu. Cách duy nhất để lấy lại là một bản sao lưu mà bạn còn nhớ mật khẩu.

<img src="../screenshots/vi/lock.png" width="420" alt="Màn hình Chào mừng trở lại: ô Mật khẩu chủ và nút Mở khóa">

## Sao lưu tự động

**Sao lưu vào thư mục** chép kho dữ liệu đã mã hóa vào một thư mục, để không mất tài khoản khi máy hỏng.

- **Thư mục**: **Chọn…**, hoặc thư mục đồng bộ mà Bloxkeep tìm thấy. **Mở thư mục** để mở ra xem. Thư mục phải nằm ngoài thư mục dữ liệu của Bloxkeep.
- **Sao lưu ngay** bị mờ khi chưa chọn thư mục. Dòng trạng thái ghi "Sao lưu gần nhất: …" hoặc "Chưa có bản sao lưu".
- Bloxkeep sao lưu sau mỗi thay đổi (tối đa 10 phút một lần), khi khóa hoặc thoát nếu có gì thay đổi, và ngay sau khi bạn đổi mật khẩu chủ hoặc chọn thư mục mới.
- Giữ 10 bản mới nhất, cộng bản mới nhất của mỗi ngày trong 30 ngày. App OneDrive hay Google Drive sẽ tự tải chúng lên.
- Nếu mật khẩu chủ dễ đoán mà thư mục lại là thư mục đám mây, app sẽ nhắc bạn đổi mật khẩu dài hơn.

## Khôi phục và chuyển sang máy mới

Bản sao lưu mở bằng mật khẩu dùng lúc sao lưu.

| Cách | Dùng khi | Kết quả |
|---|---|---|
| **Khôi phục từ bản sao lưu** (màn hình đầu) hoặc **Cài đặt › Sao lưu & dữ liệu › Khôi phục › Khôi phục…** | Sang máy mới, hoặc muốn quay về một bản sao lưu. | Luôn hỏi trước. Dữ liệu hiện tại được giữ lại một bản, sau đó Bloxkeep khóa lại: mở khóa bằng mật khẩu của bản sao lưu. |
| **Cài đặt › Sao lưu & dữ liệu › Nhập tài khoản từ file dữ liệu Bloxkeep › Nhập…** | Muốn lấy tài khoản từ một file dữ liệu hay bản sao lưu khác. | Thêm tài khoản; tài khoản đang có vẫn giữ nguyên. |

## Nhập từ file Bloxkeep khác

Dùng để lấy tài khoản từ file `vault.rmv` của máy khác, hoặc từ file `Bloxkeep-backup-…`. Có ba chỗ mở: **Thêm tài khoản › Cách thêm khác › Nhập từ file dữ liệu Bloxkeep khác…**, **Nhập từ: File Bloxkeep** trong hộp **Đăng nhập Roblox**, và **Cài đặt › Sao lưu & dữ liệu › Nhập…**.

1. Bấm **Chọn file…** và chọn file `.rmv`.
2. Nhập **Mật khẩu chủ của file này** rồi bấm **Mở**.
3. Tích những tài khoản muốn lấy. Mỗi dòng ghi **Mới** hay **Đã có**, và tài khoản còn đăng nhập, cần đăng nhập lại, chưa kiểm tra hay đã lưu mật khẩu. **Chọn hết** / **Bỏ chọn** để tích hoặc bỏ tất cả.
4. Muốn lấy cả preset thì tích **Nhập cả preset (N)**.
5. Bấm **Nhập**.

- File gốc vẫn giữ nguyên. Tài khoản lấy sang được khóa bằng mật khẩu chủ của Bloxkeep trên máy này.
- Với tài khoản đã có, Bloxkeep chỉ thay phiên đăng nhập nếu phiên cũ không còn dùng được. Tên, nhóm và ghi chú bạn đặt vẫn giữ nguyên.

<details>
<summary>Chi tiết thêm</summary>

Preset nhập vào có hẹn giờ bị tắt, và tài khoản chưa có ở đây bị bỏ ra khỏi preset. Lưu được tối đa 50 preset. Nhập sai mật khẩu ba lần thì nút **Mở** đếm ngược rồi mới cho thử tiếp. Không nhập được chính file dữ liệu bạn đang dùng.

</details>

## Dữ liệu nằm ở đâu

**Cài đặt › Sao lưu & dữ liệu › Nơi lưu dữ liệu › Mở thư mục** sẽ mở đúng chỗ. Bản cài đặt dùng `%LOCALAPPDATA%\Bloxkeep`; bản portable dùng thư mục `data` nằm cạnh `Bloxkeep.exe`.

| File | Là gì |
|---|---|
| `vault.rmv` | Tài khoản của bạn, đã mã hóa. |
| `vault.rmv.bak` | Bản lưu trước đó. |
| `error.log` | Ghi lỗi, không có thông tin tài khoản. |

> [!WARNING]
> Đừng bao giờ gửi `vault.rmv` hay file sao lưu cho ai. Danh sách đầy đủ có trong [README](../../README.vi.md#dữ-liệu-và-bảo-mật).

## Những gì rời khỏi máy

Bloxkeep không có máy chủ, tài khoản hay công cụ phân tích riêng. Tài khoản, cài đặt và lịch sử của bạn chỉ nằm trên máy, trong một kho dữ liệu được mã hóa bằng mật khẩu chủ, và mật khẩu chủ không được lưu ở đâu cả. Dữ liệu chỉ rời khỏi máy cho những việc bạn dùng:

| Nơi nhận | Gửi gì |
|---|---|
| Roblox | Đăng nhập, trạng thái, game, server. |
| GitHub | Cập nhật Bloxkeep, không kèm dữ liệu tài khoản. |
| Thư mục sao lưu bạn chọn | Kho dữ liệu đã mã hóa. |
| Webhook Discord của bạn | Chỉ khi bạn bật. Không bao giờ có cookie hay mật khẩu. |
| Người làm Bloxkeep | Chỉ những góp ý bạn tự gửi. |

Danh sách đầy đủ có trong [chính sách quyền riêng tư](../../PRIVACY.md) (tiếng Anh).

## Ẩn tên tài khoản

**Cài đặt › Giao diện › Ẩn tên tài khoản** thay tên thật bằng "Tài khoản 1", "Tài khoản 2"… (theo vị trí trong danh sách) ở mọi chỗ, kể cả tiêu đề cửa sổ Roblox.

- Tên đăng nhập @ cũng ẩn đi, và mục **Sửa** không hiện ID người dùng.
- Ô **Tên gợi nhớ** không sửa được trong lúc này.
- Ô tìm kiếm vẫn tìm được theo tên thật.
- Tin nhắn Discord có công tắc riêng: **Ẩn tên tài khoản trong tin nhắn** (xem [Discord](discord.vi.md)).

> [!TIP]
> Bật trước khi chụp màn hình hay livestream.

← [Hướng dẫn](../guide.vi.md) · Tiếp: [Gỡ rối](troubleshooting.vi.md) →
