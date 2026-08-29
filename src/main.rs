use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::Html,
    routing::get,
    Router,
};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use mdns_sd::{ServiceDaemon, ServiceInfo};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;

// 内嵌针对 iOS / Android / 桌面端深度优化的 HTML+JS 前端
const HTML_CONTENT: &str = r#"<!DOCTYPE html>
<html lang="zh-CN">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0, maximum-scale=1.0, user-scalable=no">
  <title>电脑音频同步</title>
  <style>
    * { box-sizing: border-box; margin: 0; padding: 0; }
    body {
      display: flex;
      flex-direction: column;
      align-items: center;
      justify-content: center;
      min-height: 100vh;
      background: #0d1117;
      color: #c9d1d9;
      font-family: -apple-system, BlinkMacSystemFont, "PingFang SC", "Segoe UI", Roboto, sans-serif;
      text-align: center;
      padding: 20px;
    }
    .card {
      background: #161b22;
      border: 1px solid #30363d;
      border-radius: 20px;
      padding: 32px 24px;
      width: 100%;
      max-width: 380px;
      box-shadow: 0 12px 32px rgba(0,0,0,0.4);
    }
    h2 { font-size: 20px; margin-bottom: 8px; color: #f0f6fc; }
    .subtitle { font-size: 13px; color: #8b949e; margin-bottom: 24px; line-height: 1.5; }
    .btn {
      width: 100%;
      padding: 16px 0;
      font-size: 17px;
      font-weight: 600;
      color: #000;
      background: #2ea043;
      border: none;
      border-radius: 12px;
      cursor: pointer;
      transition: all 0.2s ease;
      -webkit-tap-highlight-color: transparent;
    }
    .btn:active { transform: scale(0.98); opacity: 0.9; }
    .btn.active { background: #da3633; color: #fff; }
    .status-box {
      margin-top: 20px;
      font-size: 14px;
      color: #58a6ff;
      display: flex;
      align-items: center;
      justify-content: center;
      gap: 8px;
    }
    .status-dot {
      width: 8px;
      height: 8px;
      border-radius: 50%;
      background: #8b949e;
    }
    .status-dot.online { background: #3fb950; box-shadow: 0 0 10px #3fb950; }
    .tips {
      margin-top: 24px;
      font-size: 12px;
      color: #8b949e;
      text-align: left;
      line-height: 1.6;
      background: #0d1117;
      padding: 14px;
      border-radius: 10px;
      border: 1px solid #21262d;
    }
  </style>
</head>
<body>
  <div class="card">
    <h2>电脑系统音频同步</h2>
    <p class="subtitle">低延迟 • 自动重连 • 支持 iOS 锁屏播放</p>
    
    <button id="toggleBtn" class="btn">开始连接并播放</button>

    <div class="status-box">
      <div id="statusDot" class="status-dot"></div>
      <span id="statusText">等待连接...</span>
    </div>

    <div class="tips">
      • <b>iPhone 用户</b>：请关闭手机侧边静音开关。<br>
      • 启动后直接按电源键锁屏，后台音频持续播放。<br>
      • 电脑切换耳机/扬声器时，程序会自动无缝重连。
    </div>
  </div>

  <audio id="iosAudio" autoplay playsinline></audio>

  <script>
    const btn = document.getElementById('toggleBtn');
    const statusText = document.getElementById('statusText');
    const statusDot = document.getElementById('statusDot');
    const iosAudio = document.getElementById('iosAudio');

    let isPlaying = false;
    let ws = null;
    let audioCtx = null;
    let streamDest = null;
    let nextStartTime = 0;
    const SAMPLE_RATE = 48000;

    btn.onclick = async () => {
      if (!isPlaying) {
        startStreaming();
      } else {
        stopStreaming();
      }
    };

    async function startStreaming() {
      try {
        statusText.innerText = "正在激活音频引擎...";
        
        audioCtx = new (window.AudioContext || window.webkitAudioContext)({
          sampleRate: SAMPLE_RATE,
          latencyHint: "interactive"
        });
        await audioCtx.resume();

        // 核心：iOS 锁屏保活必须走 MediaStream 桥接
        streamDest = audioCtx.createMediaStreamDestination();
        iosAudio.srcObject = streamDest.stream;
        await iosAudio.play().catch(() => {});

        if ('mediaSession' in navigator) {
          navigator.mediaSession.metadata = new MediaMetadata({
            title: "电脑系统音频",
            artist: "实时串流中",
            album: "局域网同步"
          });
          navigator.mediaSession.playbackState = "playing";
        }

        const protocol = location.protocol === 'https:' ? 'wss:' : 'ws:';
        ws = new WebSocket(`${protocol}//${location.host}/ws`);
        ws.binaryType = 'arraybuffer';

        nextStartTime = audioCtx.currentTime;

        ws.onopen = () => {
          isPlaying = true;
          btn.innerText = "断开播放";
          btn.classList.add('active');
          statusText.innerText = "正在播放 (可直接熄屏)";
          statusDot.classList.add('online');
        };

        ws.onmessage = (event) => {
          const int16Array = new Int16Array(event.data);
          if (int16Array.length === 0) return;

          const numChannels = 2;
          const frameCount = int16Array.length / numChannels;
          
          const audioBuffer = audioCtx.createBuffer(numChannels, frameCount, SAMPLE_RATE);
          const leftChannel = audioBuffer.getChannelData(0);
          const rightChannel = audioBuffer.getChannelData(1);

          for (let i = 0, j = 0; i < int16Array.length; i += 2, j++) {
            leftChannel[j] = int16Array[i] / 32768.0;
            rightChannel[j] = int16Array[i + 1] / 32768.0;
          }

          const source = audioCtx.createBufferSource();
          source.buffer = audioBuffer;
          source.connect(streamDest);

          const now = audioCtx.currentTime;
          
          // 熄屏防爆音缓冲：预留 60ms 应对手机锁屏降频
          if (nextStartTime < now) {
            nextStartTime = now + 0.06;
          }
          
          source.start(nextStartTime);
          nextStartTime += audioBuffer.duration;

          // 自动追赶累积延迟（超过 250ms 时快进对齐）
          if (nextStartTime - now > 0.25) {
            nextStartTime = now + 0.06;
          }
        };

        ws.onclose = () => {
          stopStreaming();
          statusText.innerText = "连接已断开";
        };

        ws.onerror = () => {
          stopStreaming();
          statusText.innerText = "连接出错，请检查网络";
        };

      } catch (err) {
        console.error(err);
        stopStreaming();
        statusText.innerText = "启动失败: " + err.message;
      }
    }

    function stopStreaming() {
      isPlaying = false;
      btn.innerText = "开始连接并播放";
      btn.classList.remove('active');
      statusDot.classList.remove('online');
      if (ws) {
        ws.close();
        ws = null;
      }
      if (audioCtx) {
        audioCtx.close();
        audioCtx = null;
      }
      iosAudio.srcObject = null;
      if ('mediaSession' in navigator) {
        navigator.mediaSession.playbackState = "none";
      }
    }
  </script>
</body>
</html>
"#;

#[tokio::main]
async fn main() {
    println!("=========================================");
    println!("     系统音频实时串流服务 (全平台优化版)    ");
    println!("=========================================");

    // 1. 创建音频广播通道 (容量 256 帧，避免短暂卡顿丢包)
    let (tx, _rx) = broadcast::channel::<Vec<u8>>(256);
    let tx = Arc::new(tx);
    let tx_clone = Arc::clone(&tx);

    // 2. 独立音频采集与热切换监控线程
    std::thread::spawn(move || {
        let host = cpal::default_host();

        loop {
            // 获取当前系统的默认音频输出设备
            let device = match host.default_output_device() {
                Some(d) => d,
                None => {
                    eprintln!("【未检测到默认音频设备】1秒后重试...");
                    std::thread::sleep(Duration::from_secs(1));
                    continue;
                }
            };

            let current_device_name = device.name().unwrap_or_else(|_| "Default Audio Device".into());
            println!(">>> 正在捕获设备声音: {}", current_device_name);

            let default_config = match device.default_output_config() {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("【获取设备配置失败】: {}，1秒后重试...", e);
                    std::thread::sleep(Duration::from_secs(1));
                    continue;
                }
            };

            let config: cpal::StreamConfig = default_config.into();
            let channels = config.channels as usize;
            let tx_inner = Arc::clone(&tx_clone);
            let stream_active = Arc::new(AtomicBool::new(true));
            let stream_active_callback = Arc::clone(&stream_active);

            // 构建 WASAPI Loopback 输入流
            let stream_result = device.build_input_stream(
                &config,
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    if data.is_empty() { return; }

                    // 将任意声道与 Float32 转为固定的 2 声道 16-bit PCM (立体声)
                    let num_frames = data.len() / channels;
                    let mut pcm_bytes = Vec::with_capacity(num_frames * 4); // 2 channels * 2 bytes

                    for frame_idx in 0..num_frames {
                        let base = frame_idx * channels;
                        let left_sample = data[base];
                        let right_sample = if channels > 1 { data[base + 1] } else { left_sample };

                        let left_i16 = (left_sample.clamp(-1.0, 1.0) * 32767.0) as i16;
                        let right_i16 = (right_sample.clamp(-1.0, 1.0) * 32767.0) as i16;

                        pcm_bytes.extend_from_slice(&left_i16.to_le_bytes());
                        pcm_bytes.extend_from_slice(&right_i16.to_le_bytes());
                    }

                    let _ = tx_inner.send(pcm_bytes);
                },
                move |err| {
                    eprintln!("【音频流失效/设备被切换】: {}", err);
                    stream_active_callback.store(false, Ordering::SeqCst);
                },
                None,
            );

            match stream_result {
                Ok(stream) => {
                    if let Err(e) = stream.play() {
                        eprintln!("【启动音频流失败】: {}", e);
                        std::thread::sleep(Duration::from_secs(1));
                        continue;
                    }

                    println!(">>> 音频流已就绪并处于活跃状态");

                    // 持续监测设备切换或流失效
                    while stream_active.load(Ordering::SeqCst) {
                        std::thread::sleep(Duration::from_millis(500));

                        // 检测 Windows 是否切换了默认输出设备（如拔插耳机）
                        if let Some(new_dev) = host.default_output_device() {
                            if new_dev.name().unwrap_or_default() != current_device_name {
                                println!(">>> 检测到默认音频输出设备已变更，正在平滑无缝重连...");
                                break;
                            }
                        }
                    }
                }
                Err(e) => {
                    eprintln!("【构建音频流失败】: {}，1秒后重试...", e);
                    std::thread::sleep(Duration::from_secs(1));
                }
            }
        }
    });

    let port = 8000;
    let local_ip = local_ip_address::local_ip()
        .map(|ip| ip.to_string())
        .unwrap_or_else(|_| "127.0.0.1".into());

    // 3. 注册 mDNS 多播局域网域名 (audio.local)
    let mdns = ServiceDaemon::new().expect("初始化 mDNS 失败");
    let service_type = "_http._tcp.local.";
    let instance_name = "audio-stream";
    let host_name = "audio.local.";
    let properties: HashMap<String, String> = HashMap::new();

    let service_info = ServiceInfo::new(
        service_type,
        instance_name,
        host_name,
        &local_ip,
        port,
        properties,
    ).expect("配置 mDNS 失败");

    let _ = mdns.register(service_info);

    // 4. 构建 Web 路由服务
    let app = Router::new()
        .route("/", get(|| async { Html(HTML_CONTENT) }))
        .route("/ws", get(move |ws: WebSocketUpgrade| {
            let tx = Arc::clone(&tx);
            async move { ws.on_upgrade(|socket| handle_socket(socket, tx)) }
        }));

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port))
        .await
        .expect("端口 8000 绑定失败，可能被其他程序占用");

    println!("-----------------------------------------");
    println!(" 🚀 服务已启动！局域网已广播专属域名：");
    println!(" 📱 iPhone / Safari 专属直接访问：");
    println!("    👉 http://audio.local:{}", port);
    println!(" 🌐 原 IP 访问方式备选：");
    println!("    👉 http://{}:{}", local_ip, port);
    println!("-----------------------------------------");

    axum::serve(listener, app).await.unwrap();
}

async fn handle_socket(mut socket: WebSocket, tx: Arc<broadcast::Sender<Vec<u8>>>) {
    let mut rx = tx.subscribe();
    while let Ok(data) = rx.recv().await {
        if socket.send(Message::Binary(data)).await.is_err() {
            break;
        }
    }
}