use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::Html,
    routing::get,
    Router,
};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::Arc;
use tokio::sync::broadcast;

const HTML_CONTENT: &str = r#"
<!DOCTYPE html>
<html>
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>电脑音频同步 (支持熄屏)</title>
  <style>
    body {
      display: flex;
      flex-direction: column;
      align-items: center;
      justify-content: center;
      height: 85vh;
      font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
      margin: 0;
      background: #f7f9fa;
    }
    button {
      padding: 16px 36px;
      font-size: 18px;
      font-weight: bold;
      color: #fff;
      background: #007aff;
      border: none;
      border-radius: 28px;
      box-shadow: 0 4px 12px rgba(0,122,255,0.3);
      cursor: pointer;
    }
    button:disabled { background: #bbb; box-shadow: none; }
    p { margin-top: 20px; color: #666; font-size: 15px; }
  </style>
</head>
<body>
  <h2>电脑系统声音实时播放</h2>
  <button id="btn">点击连接并开始播放</button>
  <p id="status">点击上方按钮启动</p>

  <!-- 用于欺骗系统后台保活的静音占位音频 -->
  <audio id="silentAudio" loop playsinline></audio>

  <script>
    const btn = document.getElementById('btn');
    const status = document.getElementById('status');
    const silentAudio = document.getElementById('silentAudio');

    // 生成极短的 1 秒无声 WAV 音频 Data URI
    function createSilentWav() {
      return "data:audio/wav;base64,UklGRigAAABXQVZFZm10IBAAAAABAAEARKwAAIhYAQACABAAZGF0YQQAAAAAAA==";
    }

    btn.onclick = async () => {
      btn.disabled = true;
      status.innerText = '正在初始化音频与后台服务...';

      // 1. 激活并播放无声 HTML5 媒体元素，骗取系统后台常驻权限
      silentAudio.src = createSilentWav();
      try {
        await silentAudio.play();
      } catch (e) {
        console.warn('Silent audio play failed:', e);
      }

      // 2. 注册系统控制中心 (锁屏界面展示)
      if ('mediaSession' in navigator) {
        navigator.mediaSession.metadata = new MediaMetadata({
          title: "电脑系统音频串流",
          artist: "局域网同步",
          album: "实时音频"
        });
        navigator.mediaSession.setActionHandler('play', () => { silentAudio.play(); });
        navigator.mediaSession.setActionHandler('pause', () => { silentAudio.pause(); });
      }

      // 3. 初始化 Web Audio API
      const audioCtx = new (window.AudioContext || window.webkitAudioContext)({ sampleRate: 48000 });
      await audioCtx.resume();

      const protocol = location.protocol === 'https:' ? 'wss:' : 'ws:';
      const ws = new WebSocket(`${protocol}//${location.host}/ws`);
      ws.binaryType = 'arraybuffer';

      let nextStartTime = audioCtx.currentTime;

      ws.onopen = () => {
        status.innerText = '正在实时播放电脑声音（支持熄屏）';
      };

      ws.onmessage = (event) => {
        const floatData = new Float32Array(event.data);
        const audioBuffer = audioCtx.createBuffer(2, floatData.length / 2, 48000);
        
        const leftChannel = audioBuffer.getChannelData(0);
        const rightChannel = audioBuffer.getChannelData(1);
        for (let i = 0, j = 0; i < floatData.length; i += 2, j++) {
          leftChannel[j] = floatData[i];
          rightChannel[j] = floatData[i + 1];
        }

        const source = audioCtx.createBufferSource();
        source.buffer = audioBuffer;
        source.connect(audioCtx.destination);

        const currentTime = audioCtx.currentTime;
        if (nextStartTime < currentTime) {
          nextStartTime = currentTime + 0.02;
        }
        source.start(nextStartTime);
        nextStartTime += audioBuffer.duration;
      };

      ws.onclose = () => {
        status.innerText = '连接已断开';
        btn.disabled = false;
      };

      ws.onerror = () => {
        status.innerText = '连接出错，请检查网络';
        btn.disabled = false;
      };
    };
  </script>
</body>
</html><!DOCTYPE html>
<html>
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>电脑音频同步 (支持熄屏)</title>
  <style>
    body {
      display: flex;
      flex-direction: column;
      align-items: center;
      justify-content: center;
      height: 85vh;
      font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
      margin: 0;
      background: #f7f9fa;
    }
    button {
      padding: 16px 36px;
      font-size: 18px;
      font-weight: bold;
      color: #fff;
      background: #007aff;
      border: none;
      border-radius: 28px;
      box-shadow: 0 4px 12px rgba(0,122,255,0.3);
      cursor: pointer;
    }
    button:disabled { background: #bbb; box-shadow: none; }
    p { margin-top: 20px; color: #666; font-size: 15px; }
  </style>
</head>
<body>
  <h2>电脑系统声音实时播放</h2>
  <button id="btn">点击连接并开始播放</button>
  <p id="status">点击上方按钮启动</p>

  <!-- 用于欺骗系统后台保活的静音占位音频 -->
  <audio id="silentAudio" loop playsinline></audio>

  <script>
    const btn = document.getElementById('btn');
    const status = document.getElementById('status');
    const silentAudio = document.getElementById('silentAudio');

    // 生成极短的 1 秒无声 WAV 音频 Data URI
    function createSilentWav() {
      return "data:audio/wav;base64,UklGRigAAABXQVZFZm10IBAAAAABAAEARKwAAIhYAQACABAAZGF0YQQAAAAAAA==";
    }

    btn.onclick = async () => {
      btn.disabled = true;
      status.innerText = '正在初始化音频与后台服务...';

      // 1. 激活并播放无声 HTML5 媒体元素，骗取系统后台常驻权限
      silentAudio.src = createSilentWav();
      try {
        await silentAudio.play();
      } catch (e) {
        console.warn('Silent audio play failed:', e);
      }

      // 2. 注册系统控制中心 (锁屏界面展示)
      if ('mediaSession' in navigator) {
        navigator.mediaSession.metadata = new MediaMetadata({
          title: "电脑系统音频串流",
          artist: "局域网同步",
          album: "实时音频"
        });
        navigator.mediaSession.setActionHandler('play', () => { silentAudio.play(); });
        navigator.mediaSession.setActionHandler('pause', () => { silentAudio.pause(); });
      }

      // 3. 初始化 Web Audio API
      const audioCtx = new (window.AudioContext || window.webkitAudioContext)({ sampleRate: 48000 });
      await audioCtx.resume();

      const protocol = location.protocol === 'https:' ? 'wss:' : 'ws:';
      const ws = new WebSocket(`${protocol}//${location.host}/ws`);
      ws.binaryType = 'arraybuffer';

      let nextStartTime = audioCtx.currentTime;

      ws.onopen = () => {
        status.innerText = '正在实时播放电脑声音（支持熄屏）';
      };

      ws.onmessage = (event) => {
        const floatData = new Float32Array(event.data);
        const audioBuffer = audioCtx.createBuffer(2, floatData.length / 2, 48000);
        
        const leftChannel = audioBuffer.getChannelData(0);
        const rightChannel = audioBuffer.getChannelData(1);
        for (let i = 0, j = 0; i < floatData.length; i += 2, j++) {
          leftChannel[j] = floatData[i];
          rightChannel[j] = floatData[i + 1];
        }

        const source = audioCtx.createBufferSource();
        source.buffer = audioBuffer;
        source.connect(audioCtx.destination);

        const currentTime = audioCtx.currentTime;
        if (nextStartTime < currentTime) {
          nextStartTime = currentTime + 0.02;
        }
        source.start(nextStartTime);
        nextStartTime += audioBuffer.duration;
      };

      ws.onclose = () => {
        status.innerText = '连接已断开';
        btn.disabled = false;
      };

      ws.onerror = () => {
        status.innerText = '连接出错，请检查网络';
        btn.disabled = false;
      };
    };
  </script>
</body>
</html>
"#;

#[tokio::main]
async fn main() {
    // 广播通道：将捕获的 PCM 数据发送给所有连接的客户端
    let (tx, _rx) = broadcast::channel::<Vec<u8>>(100);
    let tx = Arc::new(tx);
    let tx_clone = Arc::clone(&tx);

    // 启动系统音频 Loopback 捕获线程
    std::thread::spawn(move || {
        let host = cpal::default_host();
        
        // 获取默认的输出设备（扬声器/耳机）
        let device = host.default_output_device().expect("未找到默认音频输出设备");
        println!("正在捕获系统输出设备声音: {}", device.name().unwrap_or_default());

        let config = device.default_output_config().expect("获取设备默认配置失败");
        let sample_rate = config.sample_rate();
        let channels = config.channels();

        println!("音频配置: 采样率 {}, 声道数 {}", sample_rate.0, channels);

        // 构建输入流捕获输出混音 (WASAPI Loopback)
        let stream = device.build_input_stream(
            &config.into(),
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                // 将 f32 PCM 转为 raw 字节流
                let bytes: Vec<u8> = data.iter().flat_map(|&sample| sample.to_le_bytes()).collect();
                let _ = tx_clone.send(bytes);
            },
            |err| eprintln!("音频捕获出错: {}", err),
            None,
        ).expect("无法建立音频 Loopback 流");

        stream.play().expect("启动捕获流失败");
        std::thread::park(); // 保持线程常驻
    });

    let app = Router::new()
        .route("/", get(|| async { Html(HTML_CONTENT) }))
        .route("/ws", get(move |ws: WebSocketUpgrade| {
            let tx = Arc::clone(&tx);
            async move { ws.on_upgrade(|socket| handle_socket(socket, tx)) }
        }));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:9911").await.unwrap();
    println!("服务已启动，请用手机在同一 Wi-Fi 下访问: http://<电脑局域网IP>:9911");
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