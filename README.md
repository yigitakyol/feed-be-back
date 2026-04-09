# Feed Be-Back — Feedback Suppression Engine VST3

DeepFilterNet AI tabanlı gerçek zamanlı gürültü/feedback bastırma eklentisi.

## 🎯 Kurulum

### Windows
1. `Feed-Be-Back-Windows.zip` dosyasını indir
2. `feed_be_back.vst3` klasörünü `C:\Program Files\Common Files\VST3\` dizinine kopyala
3. DAW'ını yeniden başlat

### macOS (Intel / Apple Silicon)
1. İşlemcine uygun `.zip` dosyasını indir
2. `feed_be_back.vst3` klasörünü `~/Library/Audio/Plug-Ins/VST3/` dizinine kopyala
3. DAW'ını yeniden başlat

### Linux
1. `.tar.gz` dosyasını indir
2. `feed_be_back.vst3` klasörünü `~/.vst3/` dizinine kopyala
3. DAW'ını yeniden başlat

## 🔧 Geliştirme

### Gereksinimler
- [Rust](https://rustup.rs/) (stable)
- Windows: Visual Studio C++ Build Tools
- Linux: `libasound2-dev libgl-dev libx11-dev libxcursor-dev libxrandr-dev`

### Derleme
```bash
cargo xtask bundle feed_be_back --release
```

Çıktı: `target/bundled/feed_be_back.vst3`

## 📄 Lisans
Tüm hakları saklıdır — Yiğit Akyol (yigitakyol.com)
