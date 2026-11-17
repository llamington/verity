//! Building UI strings without `core::fmt`: `t!("Box ", n + 1)` concatenates pieces into a
//! stack buffer. Text is UTF-8 bytes; `\x01` toggles the muted shade (`MUTED`).

use core::ops::Deref;

/// Text of at most `N` bytes (longer text is cut). Most UI text fits `Text`; paths and move
/// descriptions use `LongText`. They live on the stack, which is small (16 KB of DTCM shared with
/// the core's legality checks), so keep them no bigger than the C buffers they replace.
#[derive(Clone, Copy)]
pub struct TextN<const N: usize> {
    buf: [u8; N],
    len: usize,
}

pub type Text = TextN<128>;
pub type LongText = TextN<200>;

impl<const N: usize> TextN<N> {
    pub const fn new() -> Self {
        Self {
            buf: [0; N],
            len: 0,
        }
    }

    pub fn from(bytes: &[u8]) -> Self {
        let mut t = Self::new();
        t.push_bytes(bytes);
        t
    }

    pub fn push<P: Piece>(&mut self, p: P) {
        p.put(self);
    }

    pub fn push_bytes(&mut self, bytes: &[u8]) {
        Out::push_bytes(self, bytes);
    }

    pub fn push_byte(&mut self, b: u8) {
        Out::push_bytes(self, &[b]);
    }

    pub fn truncate(&mut self, len: usize) {
        self.len = self.len.min(len);
    }

    /// Cuts to what a C buffer of `size` bytes would hold (`size - 1` bytes and a nul).
    pub fn cap(mut self, size: usize) -> Self {
        self.truncate(size.saturating_sub(1));
        self
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.buf[..self.len]
    }
}

impl<const N: usize> Default for TextN<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> Deref for TextN<N> {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        self.as_bytes()
    }
}

/// Where pieces go.
pub trait Out {
    fn push_bytes(&mut self, bytes: &[u8]);

    fn push_byte(&mut self, b: u8) {
        self.push_bytes(&[b]);
    }
}

impl<const N: usize> Out for TextN<N> {
    fn push_bytes(&mut self, bytes: &[u8]) {
        let n = bytes.len().min(N - self.len);
        self.buf[self.len..self.len + n].copy_from_slice(&bytes[..n]);
        self.len += n;
    }
}

/// Something `t!` can append.
pub trait Piece {
    fn put(&self, t: &mut dyn Out);
}

impl Piece for &str {
    fn put(&self, t: &mut dyn Out) {
        t.push_bytes(self.as_bytes());
    }
}

impl Piece for &[u8] {
    fn put(&self, t: &mut dyn Out) {
        t.push_bytes(self);
    }
}

impl<const N: usize> Piece for &[u8; N] {
    fn put(&self, t: &mut dyn Out) {
        t.push_bytes(&self[..]);
    }
}

impl<const N: usize> Piece for &TextN<N> {
    fn put(&self, t: &mut dyn Out) {
        t.push_bytes(self.as_bytes());
    }
}

impl<const N: usize> Piece for TextN<N> {
    fn put(&self, t: &mut dyn Out) {
        t.push_bytes(self.as_bytes());
    }
}

fn put_u32(t: &mut dyn Out, mut v: u32, width: usize, fill: u8) {
    let mut digits = [0u8; 10];
    let mut n = 0;
    loop {
        digits[n] = b'0' + (v % 10) as u8;
        n += 1;
        v /= 10;
        if v == 0 {
            break;
        }
    }
    for _ in n..width {
        t.push_byte(fill);
    }
    for i in (0..n).rev() {
        t.push_byte(digits[i]);
    }
}

macro_rules! unsigned {
    ($($ty:ty),*) => {$(
        impl Piece for $ty {
            fn put(&self, t: &mut dyn Out) {
                put_u32(t, *self as u32, 0, b' ');
            }
        }
    )*};
}
unsigned!(u8, u16, u32, usize);

macro_rules! signed {
    ($($ty:ty),*) => {$(
        impl Piece for $ty {
            fn put(&self, t: &mut dyn Out) {
                let v = *self as i32;
                if v < 0 {
                    t.push_byte(b'-');
                }
                put_u32(t, v.unsigned_abs(), 0, b' ');
            }
        }
    )*};
}
signed!(i8, i16, i32);

/// `%0Nu`: zero-padded to `width` digits.
pub struct Zero(pub u32, pub usize);

impl Piece for Zero {
    fn put(&self, t: &mut dyn Out) {
        put_u32(t, self.0, self.1, b'0');
    }
}

/// `%Nu`: space-padded on the left to `width`.
pub struct Pad(pub u32, pub usize);

impl Piece for Pad {
    fn put(&self, t: &mut dyn Out) {
        put_u32(t, self.0, self.1, b' ');
    }
}

/// `%.Ns`: at most `n` bytes of text.
pub struct Prec<'a>(pub &'a [u8], pub usize);

impl Piece for Prec<'_> {
    fn put(&self, t: &mut dyn Out) {
        t.push_bytes(&self.0[..self.0.len().min(self.1)]);
    }
}

/// `%*s`: text right-aligned in `width` bytes.
pub struct Right<'a>(pub &'a [u8], pub usize);

impl Piece for Right<'_> {
    fn put(&self, t: &mut dyn Out) {
        for _ in self.0.len()..self.1 {
            t.push_byte(b' ');
        }
        t.push_bytes(self.0);
    }
}

/// Text built from pieces: `t!("Box ", n + 1)`.
#[macro_export]
macro_rules! t {
    ($($p:expr),* $(,)?) => {{
        #[allow(unused_mut)]
        let mut t = $crate::text::Text::new();
        $( t.push($p); )*
        t
    }};
}

/// `t!` for paths and other text up to 200 bytes.
#[macro_export]
macro_rules! long {
    ($($p:expr),* $(,)?) => {{
        #[allow(unused_mut)]
        let mut t = $crate::text::LongText::new();
        $( t.push($p); )*
        t
    }};
}

/// The bytes of a nul-terminated buffer before its nul.
pub fn cstr(buf: &[u8]) -> &[u8] {
    &buf[..buf.iter().position(|&b| b == 0).unwrap_or(buf.len())]
}

/// The part of a path after its last `/` (all of it without one).
pub fn file_name(path: &[u8]) -> &[u8] {
    path.iter()
        .rposition(|&b| b == b'/')
        .map_or(path, |i| &path[i + 1..])
}

pub fn eq_ignore_case(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.eq_ignore_ascii_case(y))
}
