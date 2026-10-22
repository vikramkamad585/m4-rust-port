// codec: compressor/decompressor for GHCN-Daily .dly station files.
//
//   codec compress   IN OUT
//   codec decompress IN OUT
//
// Starting implementation: the whole file through zlib at level 9, with an
// 8-byte header holding the original length.

#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <vector>
#include <zlib.h>

static std::vector<unsigned char> read_all(const char* path) {
  std::vector<unsigned char> v;
  FILE* f = fopen(path, "rb");
  if (!f) { perror(path); exit(1); }
  unsigned char buf[1 << 16];
  size_t k;
  while ((k = fread(buf, 1, sizeof buf, f)) > 0) v.insert(v.end(), buf, buf + k);
  fclose(f);
  return v;
}

static void write_all(const char* path, const unsigned char* p, size_t n) {
  FILE* f = fopen(path, "wb");
  if (!f) { perror(path); exit(1); }
  if (n && fwrite(p, 1, n, f) != n) { perror(path); exit(1); }
  if (fclose(f) != 0) { perror(path); exit(1); }
}

int main(int argc, char** argv) {
  if (argc != 4) {
    fprintf(stderr, "usage: codec compress|decompress IN OUT\n");
    return 2;
  }
  std::vector<unsigned char> in = read_all(argv[2]);
  if (!strcmp(argv[1], "compress")) {
    uLongf cap = compressBound(in.size());
    std::vector<unsigned char> out(8 + cap);
    uint64_t n = in.size();
    memcpy(out.data(), &n, 8);
    if (compress2(out.data() + 8, &cap, in.data(), in.size(), 9) != Z_OK) {
      fprintf(stderr, "codec: zlib error\n");
      return 1;
    }
    write_all(argv[3], out.data(), 8 + cap);
  } else if (!strcmp(argv[1], "decompress")) {
    if (in.size() < 8) { fprintf(stderr, "codec: truncated input\n"); return 1; }
    uint64_t n;
    memcpy(&n, in.data(), 8);
    std::vector<unsigned char> out(n ? n : 1);
    uLongf got = n;
    if (uncompress(out.data(), &got, in.data() + 8, in.size() - 8) != Z_OK || got != n) {
      fprintf(stderr, "codec: corrupt input\n");
      return 1;
    }
    write_all(argv[3], out.data(), n);
  } else {
    fprintf(stderr, "usage: codec compress|decompress IN OUT\n");
    return 2;
  }
  return 0;
}
