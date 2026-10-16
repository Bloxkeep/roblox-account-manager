// Text helpers the React parts share with app.js, written the same way.

/** Accents folded and case ignored, as app.js's fold ("man hinh" finds "Màn hình"). */
export const fold = (s: string | null | undefined) =>
  String(s || '').normalize('NFD').replace(/\p{M}/gu, '').replace(/đ/g, 'd').replace(/Đ/g, 'D').toLowerCase();
