use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::Html,
    routing::get,
    Router,
};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::Arc;
use tokio::sync::broadcast;

// 内嵌优化后的前端页面 (针对 iOS 熄屏、Safari、Chrome 深度优化)
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
      background: #0f1117;
      color: #fff;
      font-family: -apple-system, BlinkMacSystemFont, "PingFang SC", "Segoe UI", Roboto, sans-serif;
      text-align: center;
      padding: 20px;
    }
    .card {
      background: #1a1d26;
      border: 1px solid #2d3139;
      border-radius: 20px;
      padding: 30px 24px;
      width: 100%;
      max-width: 380px;
      box-shadow: 0 10px 30px rgba(0,0,0,0.5);
    }
    h2 { font-size: 20px; margin-bottom: 8px; color: #fff; }
    .subtitle { font-size: 13px; color: #8b949e; margin-bottom: 25px; line-height: 1.5; }
    .btn {
      width: 100%;
      padding: 16px 0;
      font-size: 17px;
      font-weight: 600;
      color: #000;
      background: #00e676;
      border: none;
      border-radius: 14px;
      cursor: pointer;
      transition: all 0.2s ease;
      -webkit-tap-highlight-color: transparent;
    }
    .btn:active { transform: scale(0.98); opacity: 0.9; }
    .btn.active { background: #ff5252; color: #fff; }
    .status-box {
      margin-top: 20px;
      font-size: 14px;
      color: #00e676;
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
    .status-dot.online { background: #00e676; box-shadow: 0 0 10px #00e676; }
    .tips {
      margin-top: 24px;
      font-size: 12px;
      color: #6e7681;
      text-align: left;
      line-height: 1.6;
      background: #14161d;
      padding: 12px 14px;
      border-radius: 10px;
    }
  </style>
</head>
<body>
  <div class="card">
    <h2>电脑系统音频同步</h2>
    <p class="subtitle">低延迟实时串流 • 支持 iOS 锁屏播放</p>
    
    <button id="toggleBtn" class="btn">开始连接并播放</button>

    <div class="status-box">
      <div id="statusDot" class="status-dot"></div>
      <span id="statusText">等待连接...</span>
    </div>

    <div class="tips">
      • <b>iPhone 提示</b>：请确认手机侧边静音开关已关闭。<br>
      • 启动后直接按电源键锁屏，声音不会中断。
    </div>
  </div>

  <!-- 用于接管 iOS 媒体会话的流媒体 audio 标签 -->
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
        statusText.innerText = "正在初始化音频...";
        
        // 1. 初始化 AudioContext
        audioCtx = new (window.AudioContext || window.webkitAudioContext)({
          sampleRate: SAMPLE_RATE,
          latencyHint: "interactive"
        });
        await audioCtx.resume();

        // 2. iOS 专属后台保活：通过 MediaStreamDestination 桥接至 <audio>
        streamDest = audioCtx.createMediaStreamDestination();
        iosAudio.srcObject = streamDest.stream;
        await iosAudio.play().catch(() => {});

        // 3. 注册系统控制中心 (锁屏控件)
        if ('mediaSession' in navigator) {
          navigator.mediaSession.metadata = new MediaMetadata({
            title: "电脑扬声器音频",
            artist: "实时串流中 (支持熄屏)",
            album: "局域网同步"
          });
          navigator.mediaSession.playbackState = "playing";
        }

        // 4. 建立 WebSocket 连接
        const protocol = location.protocol === 'https:' ? 'wss:' : 'ws:';
        ws = new WebSocket(`${protocol}//${location.host}/ws`);
        ws.binaryType = 'arraybuffer';

        nextStartTime = audioCtx.currentTime;

        ws.onopen = () => {
          isPlaying = true;
          btn.innerText = "断开播放";
          btn.classList.add('active');
          statusText.innerText = "正在实时同步音频 (可锁屏)";
          statusDot.classList.add('online');
        };

        ws.onmessage = (event) => {
          // 接收 16-bit PCM (Int16) 数组并转为 WebAudio Float32
          const int16Array = new Int16Array(event.data);
          const numChannels = 2;
          const frameCount = int16Array.length / numChannels;
          
          const audioBuffer = audioCtx.createBuffer(numChannels, frameCount, SAMPLE_RATE);
          const leftChannel = audioBuffer.getChannelData(0);
          const rightChannel = audioBuffer.getChannelData(1);

          for (let i = 0, j = 0; i < int16Array.length; i += 2, j++) {
            // Int16 转 Float32 (-1.0 ~ 1.0)
            leftChannel[j] = int16Array[i] / 32768.0;
            rightChannel[j] = int16Array[i + 1] / 32768.0;
          }

          const source = audioCtx.createBufferSource();
          source.buffer = audioBuffer;
          source.connect(streamDest);

          // 动态防爆音与抗抖动机制
          const now = audioCtx.currentTime;
          if (nextStartTime < now) {
            nextStartTime = now + 0.015; // 15ms 极小缓冲
          }
          source.start(nextStartTime);
          nextStartTime += audioBuffer.duration;

          // 动态追赶延迟：防止网络卡顿后积攒过多延迟
          if (nextStartTime - now > 0.15) {
            nextStartTime = now + 0.03;
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
    println!("     系统音频实时串流服务器 (低延迟版)     ");
    println!("=========================================");

    // 创建广播队列 (可同时给多台手机广播音频)
    let (tx, _rx) = broadcast::channel::<Vec<u8>>(128);
    let tx = Arc::new(tx);
    let tx_clone = Arc::clone(&tx);

    // 在独立音频线程捕获系统声音 (WASAPI Loopback)
    std::thread::spawn(move || {
        let host = cpal::default_host();
        
        let device = host.default_output_device().expect("【错误】未找到系统默认扬声器/输出设备！");
        println!("正在捕获系统声音设备: {}", device.name().unwrap_or_else(|_| "Default".into()));

        let default_config = device.default_output_config().expect("【错误】无法获取设备音频配置");
        println!(
            "系统声卡参数: 采样率 {} Hz, 声道数 {}",
            default_config.sample_rate().0,
            default_config.channels()
        );

        let config: cpal::StreamConfig = default_config.into();

        // 捕获系统主混音输出
        let stream = device.build_input_stream(
            &config,
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                // 将 Float32 PCM 压缩编码为 16-bit PCM (Int16) 以节省 50% 传输带宽
                let mut pcm_bytes = Vec::with_capacity(data.len() * 2);
                for &sample in data {
                    // 削波保护 (-1.0 ~ 1.0)
                    let clamped = sample.clamp(-1.0, 1.0);
                    let sample_i16 = (clamped * 32767.0) as i16;
                    pcm_bytes.extend_from_slice(&sample_i16.to_le_bytes());
                }
                // 广播给所有已连接客户端
                let _ = tx_clone.send(pcm_bytes);
            },
            |err| eprintln!("【音频流错误】: {}", err),
            None,
        ).expect("【错误】无法初始化音频 Loopback 流，请确认系统声卡工作正常");

        stream.play().expect("【错误】启动音频流捕获失败");
        std::thread::park(); // 保持常驻
    });

    // 路由构建
    let app = Router::new()
        .route("/", get(|| async { Html(HTML_CONTENT) }))
        .route("/ws", get(move |ws: WebSocketUpgrade| {
            let tx = Arc::clone(&tx);
            async move { ws.on_upgrade(|socket| handle_socket(socket, tx)) }
        }));

    // 绑定端口
    let port = 8000;
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port))
        .await
        .expect("端口绑定失败，可能被占用");

    // 获取本机局域网 IP 并打印访问方式
    let local_ip = local_ip_address::local_ip().map(|ip| ip.to_string()).unwrap_or_else(|_| "127.0.0.1".into());

    println!("-----------------------------------------");
    println!(" 服务已就绪！请在手机 Safari 浏览器打开：");
    println!(" 👉 http://{}:{}", local_ip, port);
    println!("-----------------------------------------");

    axum::serve(listener, app).await.unwrap();
}

async fn handle_socket(mut socket: WebSocket, tx: Arc<broadcast::Sender<Vec<u8>>>) {
    let mut rx = tx.subscribe();
    // 异步循环下发音频帧
    while let Ok(data) = rx.recv().await {
        if socket.send(Message::Binary(data)).await.is_err() {
            // 客户端断开连接，退出处理
            break;
        }
    }
}