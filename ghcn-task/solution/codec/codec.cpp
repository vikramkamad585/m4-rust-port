// codec: lossless compressor for GHCN-Daily .dly station files.
//
//   codec compress   IN OUT
//   codec decompress IN OUT
//
// Structured mode parses the fixed-width record layout (station id, year,
// month, element, 31 x [value mflag qflag sflag]) and codes every field with
// a context-mixing binary arithmetic coder. Anything that does not parse
// exactly is coded with a generic order-1..4 byte model.

#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <string>
#include <vector>
#include <map>

typedef uint32_t U32;
typedef uint16_t U16;

// ------------------------------------------------------------ squash/stretch

static int squash(int d) {  // 1/(1+e^-d), d scaled by 8 bits, output 12 bits
  static const int t[33] = {1, 2, 3, 6, 10, 16, 27, 45, 73, 120, 194, 310, 488, 747, 1101, 1546, 2047,
                            2549, 2994, 3348, 3607, 3785, 3901, 3975, 4022, 4050, 4068, 4079, 4085,
                            4089, 4092, 4093, 4094};
  if (d > 2047) return 4095;
  if (d < -2047) return 1;
  int w = d & 127;
  d = (d >> 7) + 16;
  return (t[d] * (128 - w) + t[d + 1] * w + 64) >> 7;
}

static short stretch_tab[4096];
static void init_tables() {
  int pi = 0;
  for (int x = -2047; x <= 2047; x++) {
    int v = squash(x);
    for (int j = pi; j <= v; j++) stretch_tab[j] = (short)x;
    pi = v + 1;
  }
  for (int j = pi; j < 4096; j++) stretch_tab[j] = 2047;
}
static inline int stretch(int p) { return stretch_tab[p]; }

// ------------------------------------------------------------ arithmetic coder

struct Coder {
  bool enc;
  std::vector<uint8_t>* out;
  const uint8_t* in;
  size_t in_len, in_pos;
  U32 x1 = 0, x2 = 0xffffffff, x = 0;

  void init_enc(std::vector<uint8_t>* o) { enc = true; out = o; }
  void init_dec(const uint8_t* p, size_t n) {
    enc = false; in = p; in_len = n; in_pos = 0;
    for (int i = 0; i < 4; i++) x = (x << 8) | next_byte();
  }
  int next_byte() { return in_pos < in_len ? in[in_pos++] : 0; }

  // p = P(bit == 1), 12 bits
  int code(int bit, int p) {
    U32 xmid = x1 + (U32)(((uint64_t)(x2 - x1) * (U32)p) >> 12);
    int y;
    if (enc) y = bit; else y = x <= xmid;
    if (y) x2 = xmid; else x1 = xmid + 1;
    while (((x1 ^ x2) & 0xff000000) == 0) {
      if (enc) out->push_back((uint8_t)(x2 >> 24));
      x1 <<= 8;
      x2 = (x2 << 8) | 255;
      if (!enc) x = (x << 8) | next_byte();
    }
    return y;
  }
  void flush() {
    for (int i = 0; i < 4; i++) { out->push_back((uint8_t)(x1 >> 24)); x1 <<= 8; }
  }
};

// ------------------------------------------------------------ model

static inline U32 hash3(U32 a, U32 b, U32 c) {
  U32 h = a * 0x9E3779B1u ^ (b + 0x7F4A7C15u) * 0x85EBCA77u ^ (c + 0x165667B1u) * 0xC2B2AE3Du;
  h ^= h >> 15; h *= 0x2C1B3C6Du; h ^= h >> 12;
  return h;
}

struct Model {
  static const int TBITS = 22;
  static const int NIN_MAX = 16;
  std::vector<U16> fast, slow;
  std::vector<int> weights;  // [mixsel][NIN_MAX]
  std::vector<U16> apm;      // [mixsel][33]
  int nsel;
  Coder* coder;
  // per-bit scratch
  U32 idx[8];
  int st[NIN_MAX];
  int nin;

  Model(Coder* c, int nsel_) : fast(1u << TBITS, 32768), slow(1u << TBITS, 32768), nsel(nsel_), coder(c) {
    weights.assign((size_t)nsel * NIN_MAX, 0);
    apm.resize((size_t)nsel * 33);
    for (int sl = 0; sl < nsel; sl++)
      for (int j = 0; j < 33; j++) apm[(size_t)sl * 33 + j] = (U16)(squash((j - 16) * 128) * 16);
    for (int s = 0; s < nsel; s++)
      for (int i = 0; i < NIN_MAX; i++) weights[(size_t)s * NIN_MAX + i] = (1 << 16) / 4;
  }

  // Code one bit given up to 7 context hashes; returns the bit.
  int bit(int y, const U32* ctx, int n, int sel) {
    const U32 mask = (1u << TBITS) - 1;
    nin = 0;
    for (int i = 0; i < n; i++) {
      idx[i] = ctx[i] & mask;
      st[nin++] = stretch(fast[idx[i]] >> 4);
      st[nin++] = stretch(slow[idx[i]] >> 4);
    }
    st[nin++] = 256;  // bias
    int* w = &weights[(size_t)(sel % nsel) * NIN_MAX];
    long long dot = 0;
    for (int i = 0; i < nin; i++) dot += (long long)st[i] * w[i];
    int pm = squash((int)(dot >> 16));
    // APM: refine the mixed probability by (sel, quantised stretch(pm)).
    int sx = stretch(pm) + 2048;            // 0..4095
    int lo = sx >> 7, wt = sx & 127;        // 33 buckets
    U16* a = &apm[(size_t)(sel % nsel) * 33];
    int pa = (a[lo] * (128 - wt) + a[lo + 1] * wt) >> 11;   // 16-bit -> 12-bit
    int p = (pm + 3 * pa) >> 2;
    if (p < 1) p = 1;
    if (p > 4095) p = 4095;
    y = coder->code(y, p);
    {
      int tgt = y ? 65535 : 0;
      a[lo] += (tgt - a[lo]) * (128 - wt) >> 13;
      a[lo + 1] += (tgt - a[lo + 1]) * wt >> 13;
    }
    int err = ((y << 12) - pm) * 4;
    for (int i = 0; i < nin; i++) w[i] += (st[i] * err) >> 13;
    for (int i = 0; i < n; i++) {
      U16& f = fast[idx[i]];
      U16& s = slow[idx[i]];
      if (y) { f += (65535 - f) >> 3; s += (65535 - s) >> 6; }
      else { f -= f >> 3; s -= s >> 6; }
    }
    return y;
  }
};

// Helper: code an n-bit unsigned value MSB first with a context family.
static U32 code_bits(Model& m, U32 v, int nbits, U32 c1, U32 c2, int sel) {
  U32 acc = 1;
  for (int i = nbits - 1; i >= 0; i--) {
    int b = (v >> i) & 1;
    U32 ctx[2] = {hash3(c1, acc, nbits), hash3(c2, acc, nbits + 77)};
    b = m.bit(b, ctx, 2, sel);
    acc = (acc << 1) | b;
  }
  return acc & ((nbits == 32) ? 0xffffffffu : ((1u << nbits) - 1));
}

// Code a non-negative integer with an Elias-gamma style binarisation.
static U32 code_uint(Model& m, U32 v, U32 c1, U32 c2, U32 c3, int sel) {
  U32 vp = v + 1;
  int nb = 0;
  while ((vp >> nb) > 1) nb++;
  // unary length
  int k = 0;
  for (;; k++) {
    int b = (k < nb);
    U32 ctx[3] = {hash3(c1, k, 1), hash3(c2, k, 2), hash3(c3, k, 3)};
    b = m.bit(b, ctx, 3, sel);
    if (!b) break;
    if (k > 30) break;
  }
  nb = k;
  U32 acc = 1;
  for (int i = nb - 1; i >= 0; i--) {
    int b = (vp >> i) & 1;
    U32 ctx[3] = {hash3(c1, acc, 100 + nb), hash3(c2, acc, 200 + nb), hash3(c3, (U32)i, 300 + nb)};
    b = m.bit(b, ctx, 3, sel);
    acc = (acc << 1) | b;
  }
  return acc - 1;
}

// ------------------------------------------------------------ structured mode

static const int LINE = 269;

// fmt: 0 = "%5d", 1 = " %04d" (zero padded, e.g. HHMM times), 2 = raw text
struct Day { int v; uint8_t f[3]; uint8_t fmt; char raw[5]; };
struct Line {
  char id[11];
  int year, month;
  char elem[4];
  Day d[31];
};

static bool parse_int5(const char* s, int* out) {
  // right aligned, optional '-', digits; must round-trip through %5d
  char buf[6];
  memcpy(buf, s, 5);
  buf[5] = 0;
  char* end;
  long v = strtol(buf, &end, 10);
  if (*end != 0) return false;
  char chk[16];
  snprintf(chk, sizeof chk, "%5ld", v);
  if (strlen(chk) != 5 || memcmp(chk, s, 5) != 0) return false;
  *out = (int)v;
  return true;
}

static bool parse_file(const std::vector<uint8_t>& data, std::vector<Line>& lines) {
  if (data.empty() || data.size() % (LINE + 1) != 0) return false;
  size_t n = data.size() / (LINE + 1);
  lines.resize(n);
  for (size_t i = 0; i < n; i++) {
    const char* s = (const char*)&data[i * (LINE + 1)];
    if (s[LINE] != '\n') return false;
    for (int k = 0; k < LINE; k++)
      if ((unsigned char)s[k] < 32 || (unsigned char)s[k] > 126) return false;
    Line& L = lines[i];
    memcpy(L.id, s, 11);
    for (int k = 11; k < 17; k++) if (s[k] < '0' || s[k] > '9') return false;
    L.year = atoi(std::string(s + 11, 4).c_str());
    L.month = atoi(std::string(s + 15, 2).c_str());
    if (L.month < 1 || L.month > 12) return false;
    memcpy(L.elem, s + 17, 4);
    for (int d = 0; d < 31; d++) {
      const char* p = s + 21 + d * 8;
      if (parse_int5(p, &L.d[d].v)) L.d[d].fmt = 0;
      else {
        char chk[16];
        int v = atoi(std::string(p, 5).c_str());
        snprintf(chk, sizeof chk, " %04d", v);
        if (v >= 0 && v <= 9999 && memcmp(chk, p, 5) == 0) { L.d[d].fmt = 1; L.d[d].v = v; }
        else { L.d[d].fmt = 2; L.d[d].v = 0; memcpy(L.d[d].raw, p, 5); }
      }
      L.d[d].f[0] = p[5]; L.d[d].f[1] = p[6]; L.d[d].f[2] = p[7];
    }
  }
  return true;
}

static void render(const std::vector<Line>& lines, std::vector<uint8_t>& out) {
  char buf[LINE + 2];
  for (const Line& L : lines) {
    memcpy(buf, L.id, 11);
    snprintf(buf + 11, 7, "%04d%02d", L.year, L.month);
    memcpy(buf + 17, L.elem, 4);
    for (int d = 0; d < 31; d++) {
      char* p = buf + 21 + d * 8;
      char tmp[16];
      if (L.d[d].fmt == 2) memcpy(tmp, L.d[d].raw, 5);
      else snprintf(tmp, sizeof tmp, L.d[d].fmt == 1 ? " %04d" : "%5d", L.d[d].v);
      memcpy(p, tmp, 5);
      p[5] = L.d[d].f[0]; p[6] = L.d[d].f[1]; p[7] = L.d[d].f[2];
    }
    buf[LINE] = '\n';
    out.insert(out.end(), buf, buf + LINE + 1);
  }
}

static int days_in(int y, int m) {
  static const int dm[12] = {31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31};
  if (m == 2 && ((y % 4 == 0 && y % 100 != 0) || y % 400 == 0)) return 29;
  return dm[m - 1];
}

struct ElemState {
  int last_val = 0;       // last non-missing value
  bool have = false;
  int last_nonzero = 0;
  int prev_month_v[31];
  bool prev_month_ok = false;
  int last_res = 0;
  int last_fmt = 0;
  ElemState() { for (int i = 0; i < 31; i++) prev_month_v[i] = -9999; }
};

static U32 bucket(int x) {
  int a = x < 0 ? -x : x;
  int b = 0;
  while (a) { b++; a >>= 1; }
  return (U32)((x < 0) ? 32 + b : b);
}

// Shared encode/decode routine; `lines` is input when encoding and output
// when decoding (n lines).
static void code_structured(Model& m, std::vector<Line>& lines, size_t n) {
  std::vector<std::string> elems;              // dictionary, first-seen order
  std::map<std::string, int> elem_idx;
  std::vector<int> succ(1024, -1);             // successor of element (for prediction)
  std::vector<ElemState> est;
  Line prev;
  memset(&prev, 0, sizeof prev);
  int prev_e = 1023;                           // sentinel
  int prev_ymcat = 0;
  int pl_v[31], pl_miss[31];
  uint8_t pl_f[31][3];
  for (int d = 0; d < 31; d++) { pl_v[d] = -9999; pl_miss[d] = 1; pl_f[d][0] = pl_f[d][1] = pl_f[d][2] = ' '; }
  for (size_t li = 0; li < n; li++) {
    Line& L = lines[li];
    // ---- station id
    {
      int same = li > 0 && memcmp(L.id, prev.id, 11) == 0;
      U32 c[2] = {hash3(1, 0, 0), hash3(1, li > 0, 0)};
      same = m.bit(same, c, 2, 1);
      if (same) memcpy(L.id, prev.id, 11);
      else
        for (int k = 0; k < 11; k++) L.id[k] = (char)code_bits(m, (uint8_t)L.id[k], 8, hash3(2, k, 0), hash3(2, k, li > 0 ? (uint8_t)prev.id[k] : 0), 2);
    }
    // ---- year/month
    {
      int cat;
      int nm = prev.month % 12 + 1, ny = prev.year + (prev.month == 12);
      if (li > 0 && L.year == prev.year && L.month == prev.month) cat = 0;
      else if (li > 0 && L.year == ny && L.month == nm) cat = 1;
      else cat = 2;
      int b0 = cat != 0;
      U32 c[2] = {hash3(3, prev_ymcat, prev_e), hash3(3, prev_ymcat, 999)};
      b0 = m.bit(b0, c, 2, 3);
      if (b0) {
        int b1 = cat != 1;
        U32 c2[2] = {hash3(4, prev_ymcat, prev_e), hash3(4, 0, 0)};
        b1 = m.bit(b1, c2, 2, 3);
        cat = b1 ? 2 : 1;
      } else cat = 0;
      if (cat == 0) { L.year = prev.year; L.month = prev.month; }
      else if (cat == 1) { L.year = ny; L.month = nm; }
      else {
        L.year = (int)code_bits(m, (U32)L.year, 16, hash3(5, 0, 0), hash3(5, 1, 0), 4);
        L.month = (int)code_bits(m, (U32)L.month, 4, hash3(6, 0, 0), hash3(6, 1, 0), 4);
      }
      prev_ymcat = cat;
    }
    // ---- element
    int e;
    {
      std::string es(L.elem, 4);
      int pred = (prev_e < (int)succ.size()) ? succ[prev_e] : -1;
      int known_e = -1;
      if (m.coder->enc) {
        auto it = elem_idx.find(es);
        known_e = it == elem_idx.end() ? -1 : it->second;
      }
      int hit = (pred >= 0 && known_e == pred);
      U32 c[2] = {hash3(7, prev_e, pred >= 0), hash3(7, prev_ymcat, 0)};
      hit = m.bit(hit, c, 2, 5);
      if (hit) e = pred;
      else {
        // index into dictionary; elems.size() means "new"
        U32 v = known_e >= 0 ? (U32)known_e : (U32)elems.size();
        v = code_uint(m, v, hash3(8, prev_e, 0), hash3(8, 0, 0), hash3(8, elems.size(), 0), 6);
        if (v < elems.size()) e = (int)v;
        else {
          for (int k = 0; k < 4; k++) L.elem[k] = (char)code_bits(m, (uint8_t)L.elem[k], 8, hash3(9, k, 0), hash3(9, k, k ? (uint8_t)L.elem[k - 1] : 0), 7);
          e = (int)elems.size();
          std::string ns(L.elem, 4);
          elems.push_back(ns);
          elem_idx[ns] = e;
          est.emplace_back();
          if ((int)succ.size() <= e) succ.resize(e + 1024, -1);
        }
      }
      memcpy(L.elem, elems[e].data(), 4);
      if (prev_e < (int)succ.size()) succ[prev_e] = e;
    }
    ElemState& S = est[e];
    U32 ek = hash3(0xE1, (uint8_t)L.elem[0] | ((uint8_t)L.elem[1] << 8), (uint8_t)L.elem[2] | ((uint8_t)L.elem[3] << 8));
    int ndays = days_in(L.year, L.month);
    int prev_missing = 1;
    int prev_v = S.have ? S.last_val : 0;
    uint8_t prevf[3] = {' ', ' ', ' '};
    int cur_v[31];
    int last_res = S.last_res;
    for (int d = 0; d < 31; d++) {
      Day& D = L.d[d];
      int inmonth = d < ndays;
      int pm_v = S.prev_month_ok ? S.prev_month_v[d] : -9999;
      // format class
      {
        int nz = D.fmt != 0;
        U32 c[2] = {hash3(70, ek, S.last_fmt), hash3(71, ek, d)};
        nz = m.bit(nz, c, 2, 21);
        if (!nz) D.fmt = 0;
        else {
          int raw = D.fmt == 2;
          U32 c2[2] = {hash3(72, ek, S.last_fmt), hash3(73, 0, 0)};
          raw = m.bit(raw, c2, 2, 22);
          D.fmt = raw ? 2 : 1;
        }
        S.last_fmt = D.fmt;
      }
      if (D.fmt == 2) {
        for (int k = 0; k < 5; k++)
          D.raw[k] = (char)code_bits(m, (uint8_t)D.raw[k], 8, hash3(74, k, 0), hash3(75, k, k ? (uint8_t)D.raw[k - 1] : 0), 23);
        D.v = 0;
      }
      // missing
      int miss = D.fmt == 2 ? 0 : D.v == -9999;
      {
        U32 c[5] = {hash3(10, ek, prev_missing * 2 + inmonth), hash3(11, ek, (pm_v == -9999) * 2 + prev_missing),
                    hash3(12, prev_missing, inmonth * 2 + pl_miss[d]), hash3(13, ek, d),
                    hash3(14, ek, pl_miss[d] * 4 + prev_missing * 2 + inmonth)};
        if (D.fmt != 2) miss = m.bit(miss, c, 5, 8 + inmonth + 2 * pl_miss[d]);
      }
      if (D.fmt == 2) {
      } else if (miss) D.v = -9999;
      else {
        int pz = prev_v == 0;
        int zero = D.v == 0;
        U32 c[5] = {hash3(20, ek, pz * 2 + prev_missing), hash3(21, ek, pm_v == 0), hash3(22, pz, prev_missing),
                    hash3(23, ek, bucket(prev_v)), hash3(24, ek, bucket(pl_v[d]) * 4 + pz * 2 + pl_miss[d])};
        zero = m.bit(zero, c, 5, 40 + pz);
        if (zero) D.v = 0;
        else {
          int pred = prev_v != 0 ? prev_v : S.last_nonzero;
          int r = D.v - pred;
          int rz = r == 0;
          U32 cb = bucket(last_res);
          U32 cx = pl_miss[d] ? 99 : bucket(pl_v[d] - (d > 0 && !pl_miss[d - 1] ? pl_v[d - 1] : pl_v[d]));
          U32 c2[5] = {hash3(30, ek, cb), hash3(31, ek, bucket(pred)), hash3(32, cb, prev_missing), hash3(33, ek, pz),
                       hash3(34 + 100, ek, cx)};
          rz = m.bit(rz, c2, 5, 11);
          if (rz) r = 0;
          else {
            int neg = r < 0;
            U32 c3[4] = {hash3(34, ek, cb), hash3(35, ek, last_res < 0), hash3(36, ek, bucket(pred)), hash3(136, ek, cx)};
            neg = m.bit(neg, c3, 4, 12);
            U32 mag = (U32)(r < 0 ? -r : r) - 1;
            mag = code_uint(m, mag, hash3(37, ek, cb * 2 + neg), hash3(38, ek, bucket(pred) * 2 + neg), hash3(39, ek, cx * 2 + neg), 13 + neg);
            r = neg ? -(int)(mag + 1) : (int)(mag + 1);
          }
          D.v = pred + r;
          last_res = r;
        }
        prev_v = D.v;
        if (D.v != 0) S.last_nonzero = D.v;
      }
      // flags
      for (int k = 0; k < 3; k++) {
        int same = D.f[k] == prevf[k];
        U32 c[4] = {hash3(40 + k, ek, miss * 256 + prevf[k]), hash3(43 + k, miss, prevf[k]), hash3(46 + k, ek, k ? D.f[k - 1] : 0),
                    hash3(140 + k, prevf[k] * 256 + pl_f[d][k], miss)};
        same = m.bit(same, c, 4, 15 + k);
        if (same) D.f[k] = prevf[k];
        else D.f[k] = (uint8_t)code_bits(m, D.f[k], 8, hash3(50 + k, ek, miss * 256 + pl_f[d][k]), hash3(53 + k, prevf[k], k ? D.f[k - 1] : 0), 18 + k);
        prevf[k] = D.f[k];
      }
      prev_missing = miss;
      cur_v[d] = D.v;
    }
    for (int d = 0; d < 31; d++) {
      pl_v[d] = L.d[d].v; pl_miss[d] = L.d[d].v == -9999 && L.d[d].fmt != 2;
      pl_f[d][0] = L.d[d].f[0]; pl_f[d][1] = L.d[d].f[1]; pl_f[d][2] = L.d[d].f[2];
    }
    S.last_res = last_res;
    for (int d = 0; d < 31; d++) S.prev_month_v[d] = cur_v[d];
    S.prev_month_ok = true;
    if (prev_v != 0 || S.have) { S.last_val = prev_v; S.have = true; }
    prev = L;
    prev_e = e;
  }
}

// ------------------------------------------------------------ generic mode

static void code_generic(Model& m, std::vector<uint8_t>& data, size_t n) {
  U32 h1 = 0, h2 = 0, h3 = 0, h4 = 0;
  for (size_t i = 0; i < n; i++) {
    U32 acc = 1;
    int c = m.coder->enc ? data[i] : 0;
    for (int b = 7; b >= 0; b--) {
      int y = (c >> b) & 1;
      U32 ctx[5] = {hash3(60, acc, 0), hash3(61, acc, h1), hash3(62, acc, h2), hash3(63, acc, h3), hash3(64, acc, h4)};
      y = m.bit(y, ctx, 5, 30 + (int)(h1 & 3));
      acc = (acc << 1) | y;
    }
    uint8_t byte = (uint8_t)(acc & 255);
    if (!m.coder->enc) data[i] = byte;
    h4 = (h3 * 773) ^ byte; h3 = (h2 * 431) ^ byte; h2 = (h1 * 257) ^ byte; h1 = byte;
    h4 &= 0xffffff; h3 &= 0xffffff;
  }
}

// ------------------------------------------------------------ container

static std::vector<uint8_t> read_all(const char* path) {
  std::vector<uint8_t> v;
  FILE* f = fopen(path, "rb");
  if (!f) { perror(path); exit(1); }
  uint8_t buf[1 << 16];
  size_t k;
  while ((k = fread(buf, 1, sizeof buf, f)) > 0) v.insert(v.end(), buf, buf + k);
  fclose(f);
  return v;
}

static void write_all(const char* path, const std::vector<uint8_t>& v) {
  FILE* f = fopen(path, "wb");
  if (!f) { perror(path); exit(1); }
  if (!v.empty() && fwrite(v.data(), 1, v.size(), f) != v.size()) { perror(path); exit(1); }
  if (fclose(f) != 0) { perror(path); exit(1); }
}

static void put_u32(std::vector<uint8_t>& o, U32 v) { for (int i = 0; i < 4; i++) o.push_back((uint8_t)(v >> (8 * i))); }
static U32 get_u32(const uint8_t* p) { return p[0] | (p[1] << 8) | (p[2] << 16) | ((U32)p[3] << 24); }

int main(int argc, char** argv) {
  if (argc != 4 || (strcmp(argv[1], "compress") && strcmp(argv[1], "decompress"))) {
    fprintf(stderr, "usage: codec compress|decompress IN OUT\n");
    return 2;
  }
  init_tables();
  std::vector<uint8_t> in = read_all(argv[2]);
  std::vector<uint8_t> out;
  if (!strcmp(argv[1], "compress")) {
    std::vector<Line> lines;
    bool structured = parse_file(in, lines);
    if (structured) {
      std::vector<uint8_t> chk;
      render(lines, chk);
      structured = chk == in;
    }
    out.push_back('G');
    out.push_back(structured ? 'S' : 'B');
    put_u32(out, structured ? (U32)lines.size() : (U32)in.size());
    Coder c;
    c.init_enc(&out);
    Model m(&c, 64);
    if (structured) code_structured(m, lines, lines.size());
    else code_generic(m, in, in.size());
    c.flush();
  } else {
    if (in.size() < 6 || in[0] != 'G') { fprintf(stderr, "codec: bad input\n"); return 1; }
    bool structured = in[1] == 'S';
    U32 n = get_u32(&in[2]);
    Coder c;
    c.init_dec(in.data() + 6, in.size() - 6);
    Model m(&c, 64);
    if (structured) {
      std::vector<Line> lines(n);
      code_structured(m, lines, n);
      render(lines, out);
    } else {
      out.resize(n);
      code_generic(m, out, n);
    }
  }
  write_all(argv[3], out);
  return 0;
}
