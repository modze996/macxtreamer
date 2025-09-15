# VLC Parameter Optimization for IPTV/Xtream Codes

## Overview
MacXtreamer now includes optimized VLC parameters specifically tuned for IPTV and Xtream Codes streaming, with automatic detection of stream types and appropriate parameter selection.

## Key Improvements

### 🎯 **IPTV/Xtream Codes Specific Optimizations**

#### Network & Buffering
- `--network-caching=X`: Network buffer size (varies by stream type)
- `--live-caching=X`: Additional buffer for live streams
- `--prefetch-buffer-size=X`: Pre-buffer data amount for smoother playback

#### Streaming Protocol Enhancements
- `--rtsp-tcp`: Forces TCP for RTSP streams (more reliable than UDP)
- `--http-reconnect`: Automatic reconnection on HTTP stream drops
- `--adaptive-logic=rate`: Adaptive bitrate based on connection quality

#### Synchronization & Performance
- `--clock-jitter=0`: Disables jitter correction (better for IPTV)
- `--clock-synchro=0`: Disables clock synchronization (reduces stuttering)
- `--network-synchronisation`: Enables network stream synchronization
- `--drop-late-frames`: Drops late frames instead of causing stutters
- `--skip-frames`: Allows frame skipping under network stress

#### HLS (M3U8) Optimizations
- `--hls-segment-threads=X`: Parallel segment loading for HLS streams
- `--demux-filter=record`: Better handling of live streams

#### User Experience
- `--intf=dummy`: Minimal interface for better performance
- `--no-video-title`: Disables video title display
- `--no-snapshot-preview`: Disables preview generation
- `--no-stats --no-osd`: Minimal overlay for cleaner viewing

## Stream Type Auto-Detection

The application automatically detects stream types and applies appropriate parameters:

### 📺 **Live TV/Channels** (Minimal Buffering)
**URL Patterns:** `/live/`, `.m3u8`, `playlist.m3u8`
```bash
vlc --fullscreen --network-caching=3000 --live-caching=1500 \
    --clock-jitter=0 --clock-synchro=0 --network-synchronisation \
    --drop-late-frames --skip-frames --rtsp-tcp --http-reconnect \
    --adaptive-logic=rate --hls-segment-threads=6 \
    --prefetch-buffer-size=2097152 --demux-filter=record
```

### 🎬 **VOD/Movies** (Quality Focused)
**URL Patterns:** `/movie/`, `.mp4`, `.mkv`, `.avi`
```bash
vlc --fullscreen --network-caching=8000 --file-caching=5000 \
    --sout-mux-caching=3000 --cr-average=2000 --rtsp-tcp \
    --http-reconnect --adaptive-logic=rate --hls-segment-threads=4 \
    --prefetch-buffer-size=8388608
```

### 📺 **Series Episodes** (Balanced)
**URL Patterns:** `/series/`
```bash
vlc --fullscreen --network-caching=6000 --file-caching=4000 \
    --audio-resampler=soxr --aout=pulse,alsa,oss --clock-master=audio \
    --sout-mux-caching=2500 --cr-average=1500 --rtsp-tcp \
    --http-reconnect --adaptive-logic=rate --pts-offset=0
```

### 🔧 **Error Fix** (Maximum Compatibility)
**For problematic streams with audio/timing errors**
```bash
vlc --fullscreen --network-caching=10000 --live-caching=5000 \
    --file-caching=8000 --audio-resampler=soxr --aout=pulse,alsa,oss,dummy \
    --clock-master=input --avcodec-error-resilience=1 --avcodec-workaround-bugs=1 \
    --prefetch-buffer-size=16777216 --input-repeat=999 --pts-offset=0
```

## Using the Optimization Presets

### In Settings Dialog
1. Open Settings → Player Command
2. Choose from preset buttons:
   - **"IPTV Optimized"**: General purpose optimized settings
   - **"Live TV"**: Minimal buffering for live channels
   - **"VOD/Movies"**: Larger buffer for better movie quality
   - **"Error Fix"**: Maximum compatibility for problematic streams

### Manual Configuration
You can still manually edit the player command or use your own VLC parameters. The presets are just convenient starting points.

## Buffer Size Recommendations

| Stream Type | Network Cache | Live Cache | Use Case |
|------------|---------------|------------|----------|
| Live TV    | 3000ms        | 1500ms     | Minimal delay, real-time |
| VOD/Movies | 8000ms        | N/A        | Quality over speed |
| Series     | 6000ms        | N/A        | Balanced approach |
| Default    | 5000ms        | 3000ms     | General IPTV streaming |

## Troubleshooting Common IPTV Issues

### 🔧 **"too low audio sample frequency (0)" / "failed to create audio output"**
**FIXED** with new parameters:
- `--audio-resampler=soxr`: High-quality audio resampling
- `--aout=pulse,alsa,oss,dummy`: Multiple audio output fallbacks
- `--audio-time-stretch`: Adaptive audio timing
- `--force-dolby-surround=0`: Disable problematic surround processing

### 🔧 **"ES_OUT_SET_(GROUP_)PCR is called too late" / PCR Timing Errors**
**FIXED** with new parameters:
- `--pts-offset=0`: Reset presentation timestamp offset
- `--clock-master=audio`: Use audio as timing reference
- `--ts-es-id-pid`: Better MPEG-TS stream handling
- `--avcodec-error-resilience=1`: Handle corrupted streams gracefully

### 🔧 **"Timestamp conversion failed" / "no reference clock"**
**FIXED** with new parameters:
- `--clock-master=input`: Use input stream as clock reference
- `--audio-desync=0`: Disable problematic desync compensation
- `--avcodec-workaround-bugs=1`: Enable FFmpeg bug workarounds
- `--input-repeat=999`: Better stream recovery

### 🔧 **Frequent Buffering**
- Use "Error Fix" preset for maximum buffering
- Increase `--network-caching` value (up to 15000ms)
- Enable larger `--prefetch-buffer-size`

### 🔧 **Stream Keeps Disconnecting**
- `--http-reconnect` handles reconnection automatically
- `--input-repeat=999` (in Error Fix preset) improves recovery
- Verify your Xtream Codes credentials

### 🔧 **Poor Video Quality**
- Use VOD preset for movies
- Increase `--prefetch-buffer-size`
- Check if provider offers multiple quality streams

## Advanced Customization

### Custom Parameters
You can add additional VLC parameters to the presets:
```bash
# Example: Add audio boost
vlc --fullscreen --network-caching=5000 --audio-gain=2.0 {URL}

# Example: Force specific audio/video codecs
vlc --fullscreen --network-caching=5000 --codec=h264,mp3 {URL}
```

### Provider-Specific Optimizations
Some Xtream Codes providers work better with specific settings:
- **High-end providers**: Use VOD settings even for live TV
- **Budget providers**: Use Live TV settings with increased caching
- **International providers**: May need higher `--network-caching` values

## Technical Details

### Why These Parameters Work for IPTV

1. **TCP over UDP**: IPTV streams often have packet loss; TCP ensures delivery
2. **Adaptive Bitrate**: Automatically adjusts quality based on connection
3. **Minimal Buffering for Live**: Reduces delay for real-time content
4. **Frame Dropping**: Prevents accumulating delays under network stress
5. **HLS Optimization**: Better handling of segmented streams (M3U8)

### Performance Impact
- **CPU Usage**: Optimized parameters reduce CPU load
- **Memory Usage**: Controlled buffering prevents excessive RAM usage
- **Network Usage**: Efficient streaming reduces bandwidth waste
