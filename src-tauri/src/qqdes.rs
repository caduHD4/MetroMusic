//! Cifra DES de uma passada usada pelo QQ Music nos blobs de letra (QRC).
//!
//! Não é DES padrão: a S-box 4 tem um erro de digitação (`10, 10` onde o DES tem `10, 1`), que o cliente do QQ
//! reproduz; por isso nenhuma biblioteca de DES serve. Porte fiel de `QQMusicDes.kt` (MetroFuse), que por sua vez
//! vem de github.com/wangqr/QQMusicDES (MIT).

const SBOX: [[u8; 64]; 8] = [
    [14, 4, 13, 1, 2, 15, 11, 8, 3, 10, 6, 12, 5, 9, 0, 7, 0, 15, 7, 4, 14, 2, 13, 1, 10, 6, 12, 11, 9, 5, 3, 8, 4, 1, 14, 8, 13, 6, 2, 11, 15, 12, 9, 7, 3, 10, 5, 0, 15, 12, 8, 2, 4, 9, 1, 7, 5, 11, 3, 14, 10, 0, 6, 13],
    [15, 1, 8, 14, 6, 11, 3, 4, 9, 7, 2, 13, 12, 0, 5, 10, 3, 13, 4, 7, 15, 2, 8, 15, 12, 0, 1, 10, 6, 9, 11, 5, 0, 14, 7, 11, 10, 4, 13, 1, 5, 8, 12, 6, 9, 3, 2, 15, 13, 8, 10, 1, 3, 15, 4, 2, 11, 6, 7, 12, 0, 5, 14, 9],
    [10, 0, 9, 14, 6, 3, 15, 5, 1, 13, 12, 7, 11, 4, 2, 8, 13, 7, 0, 9, 3, 4, 6, 10, 2, 8, 5, 14, 12, 11, 15, 1, 13, 6, 4, 9, 8, 15, 3, 0, 11, 1, 2, 12, 5, 10, 14, 7, 1, 10, 13, 0, 6, 9, 8, 7, 4, 15, 14, 3, 11, 5, 2, 12],
    [7, 13, 14, 3, 0, 6, 9, 10, 1, 2, 8, 5, 11, 12, 4, 15, 13, 8, 11, 5, 6, 15, 0, 3, 4, 7, 2, 12, 1, 10, 14, 9, 10, 6, 9, 0, 12, 11, 7, 13, 15, 1, 3, 14, 5, 2, 8, 4, 3, 15, 0, 6, 10, 10, 13, 8, 9, 4, 5, 11, 12, 7, 2, 14],
    [2, 12, 4, 1, 7, 10, 11, 6, 8, 5, 3, 15, 13, 0, 14, 9, 14, 11, 2, 12, 4, 7, 13, 1, 5, 0, 15, 10, 3, 9, 8, 6, 4, 2, 1, 11, 10, 13, 7, 8, 15, 9, 12, 5, 6, 3, 0, 14, 11, 8, 12, 7, 1, 14, 2, 13, 6, 15, 0, 9, 10, 4, 5, 3],
    [12, 1, 10, 15, 9, 2, 6, 8, 0, 13, 3, 4, 14, 7, 5, 11, 10, 15, 4, 2, 7, 12, 9, 5, 6, 1, 13, 14, 0, 11, 3, 8, 9, 14, 15, 5, 2, 8, 12, 3, 7, 0, 4, 10, 1, 13, 11, 6, 4, 3, 2, 12, 9, 5, 15, 10, 11, 14, 1, 7, 6, 0, 8, 13],
    [4, 11, 2, 14, 15, 0, 8, 13, 3, 12, 9, 7, 5, 10, 6, 1, 13, 0, 11, 7, 4, 9, 1, 10, 14, 3, 5, 12, 2, 15, 8, 6, 1, 4, 11, 13, 12, 3, 7, 14, 10, 15, 6, 8, 0, 5, 9, 2, 6, 11, 13, 8, 1, 4, 10, 7, 9, 5, 0, 15, 14, 2, 3, 12],
    [13, 2, 8, 4, 6, 15, 11, 1, 10, 9, 3, 14, 5, 0, 12, 7, 1, 15, 13, 8, 10, 3, 7, 4, 12, 5, 6, 11, 0, 14, 9, 2, 7, 11, 4, 1, 9, 12, 14, 2, 0, 6, 10, 13, 15, 3, 5, 8, 2, 1, 14, 7, 4, 10, 8, 13, 15, 12, 9, 0, 3, 5, 6, 11],
];

fn bitnum(a: &[u8], b: usize, c: u32) -> u32 {
    let idx = (b / 32) * 4 + 3 - (b % 32) / 8;
    (((a[idx] >> (7 - (b % 8))) & 1) as u32) << c
}
fn bit_r(a: u32, b: u32, c: u32) -> u32 { ((a >> (31 - b)) & 1) << c }
fn bit_l(a: u32, b: u32, c: u32) -> u32 { ((a << b) & 0x8000_0000) >> c }
fn sbox_bit(a: u32) -> usize { ((a & 0x20) | ((a & 0x1f) >> 1) | ((a & 0x01) << 4)) as usize }

fn ip(inb: &[u8]) -> [u32; 2] {
    let (mut s0, mut s1) = (0u32, 0u32);
    for g in 0..4usize {
        for k in 0..8usize {
            let c = (31 - 8 * g - k) as u32;
            let b = 57 + 2 * g - 8 * k;
            s0 |= bitnum(inb, b, c);
            s1 |= bitnum(inb, b - 1, c);
        }
    }
    [s0, s1]
}

fn inv_ip(st: [u32; 2]) -> [u8; 8] {
    let (s0, s1) = (st[0], st[1]);
    let mut out = [0u8; 8];
    for (idx, n) in [(3usize, 7u32), (2, 6), (1, 5), (0, 4), (7, 3), (6, 2), (5, 1), (4, 0)] {
        let v = bit_r(s1, n, 7) | bit_r(s0, n, 6) | bit_r(s1, n + 8, 5) | bit_r(s0, n + 8, 4)
            | bit_r(s1, n + 16, 3) | bit_r(s0, n + 16, 2) | bit_r(s1, n + 24, 1) | bit_r(s0, n + 24, 0);
        out[idx] = (v & 0xff) as u8;
    }
    out
}

const FINAL: [(u32, u32); 32] = [
    (15, 0), (6, 1), (19, 2), (20, 3), (28, 4), (11, 5), (27, 6), (16, 7), (0, 8), (14, 9), (22, 10), (25, 11), (4, 12), (17, 13), (30, 14), (9, 15),
    (1, 16), (7, 17), (23, 18), (13, 19), (31, 20), (26, 21), (2, 22), (8, 23), (18, 24), (12, 25), (29, 26), (5, 27), (21, 28), (10, 29), (3, 30), (24, 31),
];

fn feistel(s: u32, key: &[u8; 6]) -> u32 {
    let t1 = bit_l(s, 31, 0) | ((s & 0xf000_0000) >> 1) | bit_l(s, 4, 5) | bit_l(s, 3, 6) | ((s & 0x0f00_0000) >> 3) | bit_l(s, 8, 11) | bit_l(s, 7, 12)
        | ((s & 0x00f0_0000) >> 5) | bit_l(s, 12, 17) | bit_l(s, 11, 18) | ((s & 0x000f_0000) >> 7) | bit_l(s, 16, 23);
    let t2 = bit_l(s, 15, 0) | ((s & 0x0000_f000) << 15) | bit_l(s, 20, 5) | bit_l(s, 19, 6) | ((s & 0x0000_0f00) << 13) | bit_l(s, 24, 11) | bit_l(s, 23, 12)
        | ((s & 0x0000_00f0) << 11) | bit_l(s, 28, 17) | bit_l(s, 27, 18) | ((s & 0x0000_000f) << 9) | bit_l(s, 0, 23);
    let mut l = [((t1 >> 24) & 0xff) as u8, ((t1 >> 16) & 0xff) as u8, ((t1 >> 8) & 0xff) as u8, ((t2 >> 24) & 0xff) as u8, ((t2 >> 16) & 0xff) as u8, ((t2 >> 8) & 0xff) as u8];
    for i in 0..6 { l[i] ^= key[i]; }
    let l: Vec<u32> = l.iter().map(|&x| x as u32).collect();
    let sb = |n: usize, v: u32| SBOX[n][sbox_bit(v)] as u32;
    let r = (sb(0, l[0] >> 2) << 28) | (sb(1, ((l[0] & 0x03) << 4) | (l[1] >> 4)) << 24) | (sb(2, ((l[1] & 0x0f) << 2) | (l[2] >> 6)) << 20) | (sb(3, l[2] & 0x3f) << 16)
        | (sb(4, l[3] >> 2) << 12) | (sb(5, ((l[3] & 0x03) << 4) | (l[4] >> 4)) << 8) | (sb(6, ((l[4] & 0x0f) << 2) | (l[5] >> 6)) << 4) | sb(7, l[5] & 0x3f);
    FINAL.iter().fold(0u32, |acc, &(b, c)| acc | bit_l(r, b, c))
}

const KEY_SHIFT: [u32; 16] = [1, 1, 2, 2, 2, 2, 2, 2, 1, 2, 2, 2, 2, 2, 2, 1];
const PERM_C: [usize; 28] = [56, 48, 40, 32, 24, 16, 8, 0, 57, 49, 41, 33, 25, 17, 9, 1, 58, 50, 42, 34, 26, 18, 10, 2, 59, 51, 43, 35];
const PERM_D: [usize; 28] = [62, 54, 46, 38, 30, 22, 14, 6, 61, 53, 45, 37, 29, 21, 13, 5, 60, 52, 44, 36, 28, 20, 12, 4, 27, 19, 11, 3];
const COMPRESS: [u32; 48] = [
    13, 16, 10, 23, 0, 4, 2, 27, 14, 5, 20, 9, 22, 18, 11, 3, 25, 7, 15, 6, 26, 19, 12, 1,
    40, 51, 30, 36, 46, 54, 29, 39, 50, 44, 32, 47, 43, 48, 38, 55, 33, 52, 45, 41, 49, 35, 28, 31,
];

fn key_setup(key8: &[u8], decrypt: bool) -> [[u8; 6]; 16] {
    let (mut c, mut d) = (0u32, 0u32);
    for i in 0..28 { c |= bitnum(key8, PERM_C[i], 31 - i as u32); d |= bitnum(key8, PERM_D[i], 31 - i as u32); }
    let mut sched = [[0u8; 6]; 16];
    for i in 0..16 {
        let sh = KEY_SHIFT[i];
        c = ((c << sh) | (c >> (28 - sh))) & 0xffff_fff0;
        d = ((d << sh) | (d >> (28 - sh))) & 0xffff_fff0;
        let at = if decrypt { 15 - i } else { i };
        let mut s = [0u8; 6];
        for j in 0..24 { s[j / 8] |= bit_r(c, COMPRESS[j], 7 - (j % 8) as u32) as u8; }
        for j in 24..48 { s[j / 8] |= bit_r(d, COMPRESS[j] - 27, 7 - (j % 8) as u32) as u8; }
        sched[at] = s;
    }
    sched
}

fn crypt_block(inb: &[u8], sched: &[[u8; 6]; 16]) -> [u8; 8] {
    let mut st = ip(inb);
    for k in sched.iter().take(15) {
        let t = st[1];
        st[1] = feistel(st[1], k) ^ st[0];
        st[0] = t;
    }
    st[0] = feistel(st[1], &sched[15]) ^ st[0];
    inv_ip(st)
}

/// DES de uma passada, ECB, em blocos independentes de 8 bytes, com os 8 primeiros bytes de `key`.
pub fn crypt(data: &[u8], key: &[u8], decrypt: bool) -> Vec<u8> {
    let sched = key_setup(&key[..8], decrypt);
    let mut out = Vec::with_capacity(data.len());
    for chunk in data.chunks(8) {
        let mut block = [0u8; 8];
        block[..chunk.len()].copy_from_slice(chunk);
        out.extend_from_slice(&crypt_block(&block, &sched)[..chunk.len()]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ida_e_volta() {
        let data: Vec<u8> = (0..64u8).collect();
        let enc = crypt(&data, b"123ZXC!@#)(*$%^&", false);
        assert_ne!(enc, data);
        assert_eq!(crypt(&enc, b"123ZXC!@#)(*$%^&", true), data);
    }
}
